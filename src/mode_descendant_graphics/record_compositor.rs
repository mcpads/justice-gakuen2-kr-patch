//! Finalizes each physical mode-descendant record after all surface writers ran.

use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};

use crate::compression::decompress;
use crate::decoded_record_write_plan::{DecodedDataClaim, DecodedRecordWritePlan};
use crate::paged_compression::{
    compress_on_source_final_page_with_preserved_prefix, compress_page_safe_image,
    source_paged_compression_profile,
};
use crate::pipeline::{difference_ranges, sha256_bytes};

use super::catalog::{ModeDescendantRecordSpec, indexed_member_archive_contract};
use super::indexed_member_archive::rebuild_indexed_member_archive;
use super::model::{
    ModeDescendantIndexedMemberBuild, ModeDescendantRecordBuild, ModeDescendantStorageKind,
};
use super::source::ModeDescendantSourceRecord;

pub(super) struct ModeDescendantRecordDraft {
    pub(super) spec: &'static ModeDescendantRecordSpec,
    pub(super) decoded: Vec<u8>,
    pub(super) decoded_write_claims: Vec<DecodedDataClaim>,
}

pub(super) struct FinalizedModeDescendantRecord {
    pub(super) spec: &'static ModeDescendantRecordSpec,
    pub(super) physical: Vec<u8>,
    pub(super) preview_decoded: Option<Vec<u8>>,
    pub(super) decoded_write_claims: Vec<DecodedDataClaim>,
    pub(super) report: ModeDescendantRecordBuild,
}

pub(super) fn finalize_mode_descendant_records(
    drafts: Vec<ModeDescendantRecordDraft>,
    sources: &[ModeDescendantSourceRecord],
) -> Result<Vec<FinalizedModeDescendantRecord>> {
    let mut records = BTreeSet::new();
    let mut paths = BTreeSet::new();
    let mut finalized = Vec::with_capacity(drafts.len());
    for draft in drafts {
        ensure!(
            records.insert(draft.spec.record),
            "mode-descendant physical record {:?} was composed more than once",
            draft.spec.record
        );
        ensure!(
            paths.insert(draft.spec.source_path),
            "mode-descendant source {} has multiple physical record owners",
            draft.spec.source_path
        );
        let source = source_for_spec(sources, draft.spec)?;
        ensure!(
            draft.decoded.len() == source.decoded.len(),
            "mode-descendant source {} changed decoded extent",
            source.path
        );
        ensure!(
            !difference_ranges(&source.decoded, &draft.decoded).is_empty(),
            "mode-descendant source {} was finalized without an owned change",
            source.path
        );
        let source_decoded_sha256 = sha256_bytes(&source.decoded);
        let mut write_plan =
            DecodedRecordWritePlan::new(source.path, &source.decoded, &source_decoded_sha256)?;
        write_plan.register_data_candidate(
            source.role,
            &source_decoded_sha256,
            &draft.decoded,
            &draft.decoded_write_claims,
        )?;
        let planned_decoded = write_plan.apply(None)?;
        ensure!(
            planned_decoded == draft.decoded,
            "mode-descendant source {} decoded plan omitted a contribution",
            source.path
        );
        finalized.push(finalize_record(
            source,
            draft.spec,
            planned_decoded,
            draft.decoded_write_claims,
        )?);
    }
    ensure!(
        finalized.len() == records.len() && finalized.len() == paths.len(),
        "mode-descendant final record denominator changed"
    );
    Ok(finalized)
}

pub(super) fn source_for_spec<'a>(
    sources: &'a [ModeDescendantSourceRecord],
    spec: &ModeDescendantRecordSpec,
) -> Result<&'a ModeDescendantSourceRecord> {
    let source = sources
        .iter()
        .find(|source| source.path == spec.source_path)
        .with_context(|| format!("mode-descendant source {} disappeared", spec.source_path))?;
    ensure!(
        source.storage_kind == spec.storage_kind,
        "mode-descendant source {} storage kind changed",
        spec.source_path
    );
    Ok(source)
}

fn finalize_record(
    source: &ModeDescendantSourceRecord,
    spec: &'static ModeDescendantRecordSpec,
    decoded: Vec<u8>,
    decoded_write_claims: Vec<DecodedDataClaim>,
) -> Result<FinalizedModeDescendantRecord> {
    match spec.storage_kind {
        ModeDescendantStorageKind::IndexedCompressedMembers => {
            finalize_indexed_member_record(source, spec, decoded, decoded_write_claims)
        }
        ModeDescendantStorageKind::PagedCompressed => {
            finalize_paged_compressed_record(source, spec, decoded, decoded_write_claims)
        }
        ModeDescendantStorageKind::Raw => {
            finalize_raw_record(source, spec, decoded, decoded_write_claims)
        }
        ModeDescendantStorageKind::TzzCompressedMembers => {
            anyhow::bail!("TZZ evidence sources cannot be emitted as mode-descendant records")
        }
    }
}

fn finalize_indexed_member_record(
    source: &ModeDescendantSourceRecord,
    spec: &'static ModeDescendantRecordSpec,
    decoded: Vec<u8>,
    decoded_write_claims: Vec<DecodedDataClaim>,
) -> Result<FinalizedModeDescendantRecord> {
    ensure!(
        source.storage_kind == ModeDescendantStorageKind::IndexedCompressedMembers,
        "mode-descendant source {} is not an indexed-member archive",
        source.path
    );
    let contract = indexed_member_archive_contract(source.path).with_context(|| {
        format!(
            "indexed-member mode-descendant source {} has no archive contract",
            source.path
        )
    })?;
    let rebuilt = rebuild_indexed_member_archive(&source.stored, contract, &decoded)?;
    let indexed_members = rebuilt
        .members
        .into_iter()
        .map(|member| ModeDescendantIndexedMemberBuild {
            id: member.id.to_string(),
            table_pair_offset: member.table_pair_offset,
            stored_offset: member.stored_offset,
            slot_size: member.slot_size,
            source_stored_size: member.source_stored_size,
            source_stored_sha256: member.source_stored_sha256.to_string(),
            patched_stored_size: member.patched_stored_size,
            patched_stored_sha256: member.patched_stored_sha256,
            decoded_size: member.decoded_size,
            source_decoded_sha256: member.source_decoded_sha256.to_string(),
            patched_decoded_sha256: member.patched_decoded_sha256,
            changed_stored_byte_count: member.changed_stored_byte_count,
        })
        .collect();
    let report = record_build_report(
        source,
        spec,
        &rebuilt.physical,
        &decoded,
        None,
        Some(true),
        indexed_members,
    );
    Ok(FinalizedModeDescendantRecord {
        spec,
        physical: rebuilt.physical,
        preview_decoded: (!spec.texture_outputs.is_empty()).then_some(decoded),
        decoded_write_claims,
        report,
    })
}

fn finalize_paged_compressed_record(
    source: &ModeDescendantSourceRecord,
    spec: &'static ModeDescendantRecordSpec,
    decoded: Vec<u8>,
    decoded_write_claims: Vec<DecodedDataClaim>,
) -> Result<FinalizedModeDescendantRecord> {
    ensure!(
        source.storage_kind == ModeDescendantStorageKind::PagedCompressed,
        "mode-descendant source {} is not paged-compressed",
        source.path
    );
    let source_profile = source_paged_compression_profile(&source.stored)?;
    let ordinary = (!source.allows_trailing_bytes)
        .then(|| compress_page_safe_image(&decoded, source_profile))
        .transpose()?;
    // The catalog stores the first encoded word, not the first decoded word.
    // An unchanged decoded prefix can still receive different LZ tokens.
    let reencoded = if let Some(encoded) =
        ordinary.filter(|encoded| encoded.get(..4) == source.stored.get(..4))
    {
        encoded
    } else {
        let unchanged_decoded_prefix_byte_count = source
            .decoded
            .iter()
            .zip(&decoded)
            .take_while(|(source, patched)| source == patched)
            .count()
            & !1;
        let (reencoded, preserved_prefix) = compress_on_source_final_page_with_preserved_prefix(
            &decoded,
            &source.stored,
            source_profile,
            unchanged_decoded_prefix_byte_count,
        )?;
        ensure!(
            preserved_prefix.encoded_byte_count > 0 && preserved_prefix.decoded_byte_count > 0,
            "mode-descendant source {} did not preserve its catalog-bearing prefix",
            source.path
        );
        reencoded
    };
    ensure!(
        reencoded.len() <= source.stored.len(),
        "mode-descendant source {} exceeded its source record",
        source.path
    );
    ensure!(
        reencoded[..4] == source.stored[..4],
        "mode-descendant source {} changed its catalog prefix",
        source.path
    );
    ensure!(
        decompress(&reencoded, false)? == decoded,
        "mode-descendant source {} failed compression roundtrip",
        source.path
    );
    let unpadded_stored_size = reencoded.len();
    let mut physical = reencoded;
    physical.resize(source.stored.len(), 0);
    ensure!(
        decompress(&physical, true)? == decoded,
        "padded mode-descendant source {} changed decoded bytes",
        source.path
    );
    let report = record_build_report(
        source,
        spec,
        &physical,
        &decoded,
        Some(unpadded_stored_size),
        None,
        Vec::new(),
    );
    let preview_decoded = (!spec.texture_outputs.is_empty()).then_some(decoded);
    Ok(FinalizedModeDescendantRecord {
        spec,
        physical,
        preview_decoded,
        decoded_write_claims,
        report,
    })
}

fn finalize_raw_record(
    source: &ModeDescendantSourceRecord,
    spec: &'static ModeDescendantRecordSpec,
    decoded: Vec<u8>,
    decoded_write_claims: Vec<DecodedDataClaim>,
) -> Result<FinalizedModeDescendantRecord> {
    ensure!(
        source.storage_kind == ModeDescendantStorageKind::Raw
            && source.stored == source.decoded
            && decoded.len() == source.stored.len(),
        "raw mode-descendant source {} changed its transform boundary",
        source.path
    );
    let report = record_build_report(source, spec, &decoded, &decoded, None, None, Vec::new());
    Ok(FinalizedModeDescendantRecord {
        spec,
        physical: decoded,
        preview_decoded: None,
        decoded_write_claims,
        report,
    })
}

fn record_build_report(
    source: &ModeDescendantSourceRecord,
    spec: &ModeDescendantRecordSpec,
    physical: &[u8],
    decoded: &[u8],
    unpadded_stored_size: Option<usize>,
    indexed_member_offsets_preserved: Option<bool>,
    indexed_members: Vec<ModeDescendantIndexedMemberBuild>,
) -> ModeDescendantRecordBuild {
    let compressed = spec.storage_kind != ModeDescendantStorageKind::Raw;
    ModeDescendantRecordBuild {
        record: spec.record,
        surface: spec.surface,
        source_path: source.path.to_string(),
        storage_kind: spec.storage_kind,
        source_stored_sha256: sha256_bytes(&source.stored),
        source_decoded_sha256: sha256_bytes(&source.decoded),
        patched_stored_sha256: sha256_bytes(physical),
        patched_decoded_sha256: sha256_bytes(decoded),
        source_record_size: source.stored.len(),
        unpadded_stored_size,
        compression_roundtrip_verified: compressed.then_some(true),
        catalog_prefix_preserved: (spec.storage_kind == ModeDescendantStorageKind::PagedCompressed)
            .then_some(true),
        indexed_member_offsets_preserved,
        indexed_member_sizes_match_rebuilt_streams: indexed_member_offsets_preserved,
        indexed_members,
    }
}

#[cfg(test)]
mod catalog_prefix_tests {
    use super::*;

    fn literal_source() -> ModeDescendantSourceRecord {
        let mut stored = Vec::new();
        let mut decoded = Vec::new();
        // The retail-style first block contains a match; the ordinary image
        // encoder instead starts with sixteen literals. Later literal blocks
        // leave enough room to recompress the patched tail in the source slot.
        stored.extend_from_slice(&0x4000u16.to_le_bytes());
        stored.extend_from_slice(&1u16.to_le_bytes());
        stored.extend_from_slice(&0x1001u16.to_le_bytes());
        for _ in 0..3 {
            decoded.extend_from_slice(&1u16.to_le_bytes());
        }
        for n in 2..16u16 {
            stored.extend_from_slice(&n.to_le_bytes());
            decoded.extend_from_slice(&n.to_le_bytes());
        }
        for _ in 0..4 {
            stored.extend_from_slice(&0u16.to_le_bytes());
            for _ in 0..16 {
                stored.extend_from_slice(&0u16.to_le_bytes());
                decoded.extend_from_slice(&0u16.to_le_bytes());
            }
        }
        stored.extend_from_slice(&[0, 0x80, 0, 0, 0, 0]);
        assert_eq!(decompress(&stored, false).unwrap(), decoded);
        ModeDescendantSourceRecord {
            role: "catalog_prefix_fixture",
            path: "fixture",
            extent_lba: 0,
            stored,
            decoded,
            allows_trailing_bytes: false,
            storage_kind: ModeDescendantStorageKind::PagedCompressed,
        }
    }

    #[test]
    fn changed_lz_tokens_preserve_the_catalog_word_and_patched_tail() {
        let source = literal_source();
        let mut patched = source.decoded.clone();
        let last_word = patched.len() - 2;
        patched[last_word] = 99;
        let profile = source_paged_compression_profile(&source.stored).unwrap();
        let ordinary = compress_page_safe_image(&patched, profile).unwrap();
        assert_ne!(&ordinary[..4], &source.stored[..4]);
        let result = finalize_paged_compressed_record(
            &source,
            &super::super::main_title::SPEC,
            patched.clone(),
            Vec::new(),
        )
        .unwrap();
        assert_eq!(&result.physical[..4], &source.stored[..4]);
        let size = result.report.unpadded_stored_size.unwrap();
        assert_eq!(
            decompress(&result.physical[..size], false).unwrap(),
            patched
        );
    }

    #[test]
    fn changed_prefix_without_a_preservable_control_block_is_rejected() {
        let source = literal_source();
        let mut patched = source.decoded.clone();
        patched[0] = 99;
        assert!(
            finalize_paged_compressed_record(
                &source,
                &super::super::main_title::SPEC,
                patched,
                Vec::new(),
            )
            .is_err()
        );
    }
}
