//! Loads and rebuilds a record whose sector-aligned members are compressed independently.

use std::ops::Range;

use anyhow::{Context, Result, ensure};

use crate::compression::decompress;
use crate::paged_compression::{
    PagedCompressionProfile, compress_page_safe_image_in_slot, source_paged_compression_profile,
};
use crate::pipeline::{difference_ranges, sha256_bytes};
use crate::tzz::{TzzMember, parse_tzz};

#[derive(Debug, Clone, Copy)]
pub(crate) struct IndexedMemberArchiveContract {
    pub(crate) record_size: usize,
    pub(crate) record_sha256: &'static str,
    pub(crate) header_size: usize,
    pub(crate) header_sha256: &'static str,
    pub(crate) members: &'static [IndexedMemberContract],
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct IndexedMemberContract {
    pub(crate) id: &'static str,
    pub(crate) index: usize,
    pub(crate) table_pair_offset: usize,
    pub(crate) stored_offset: usize,
    pub(crate) stored_size: usize,
    pub(crate) stored_sha256: &'static str,
    pub(crate) slot_size: usize,
    pub(crate) decoded_size: usize,
    pub(crate) decoded_sha256: &'static str,
}

#[derive(Debug)]
pub(crate) struct IndexedMemberArchiveBuild {
    pub(crate) physical: Vec<u8>,
    pub(crate) members: Vec<IndexedMemberBuild>,
}

#[derive(Debug)]
pub(crate) struct IndexedMemberBuild {
    pub(crate) id: &'static str,
    pub(crate) table_pair_offset: usize,
    pub(crate) stored_offset: usize,
    pub(crate) slot_size: usize,
    pub(crate) source_stored_size: usize,
    pub(crate) source_stored_sha256: &'static str,
    pub(crate) patched_stored_size: usize,
    pub(crate) patched_stored_sha256: String,
    pub(crate) decoded_size: usize,
    pub(crate) source_decoded_sha256: &'static str,
    pub(crate) patched_decoded_sha256: String,
    pub(crate) changed_stored_byte_count: usize,
}

struct LoadedMember {
    contract: &'static IndexedMemberContract,
    descriptor: TzzMember,
    decoded_range: Range<usize>,
    decoded: Vec<u8>,
    source_profile: PagedCompressionProfile,
}

struct LoadedArchive {
    decoded: Vec<u8>,
    members: Vec<LoadedMember>,
}

pub(crate) fn decode_indexed_member_archive(
    record: &[u8],
    contract: &'static IndexedMemberArchiveContract,
) -> Result<Vec<u8>> {
    Ok(load_archive(record, contract)?.decoded)
}

pub(crate) fn rebuild_indexed_member_archive(
    source_record: &[u8],
    contract: &'static IndexedMemberArchiveContract,
    patched_decoded: &[u8],
) -> Result<IndexedMemberArchiveBuild> {
    let source = load_archive(source_record, contract)?;
    ensure!(
        patched_decoded.len() == source.decoded.len(),
        "indexed-member archive changed its decoded extent"
    );

    let mut physical = source_record.to_vec();
    let mut reports = Vec::with_capacity(source.members.len());
    for member in &source.members {
        let patched_member = &patched_decoded[member.decoded_range.clone()];
        let source_stored = &source_record[member.descriptor.compressed_range()];
        let patched_stored = if patched_member == member.decoded {
            source_stored.to_vec()
        } else {
            compress_page_safe_image_in_slot(
                patched_member,
                member.source_profile,
                member.contract.slot_size,
            )
            .with_context(|| {
                format!(
                    "failed to compress indexed-member archive member {}",
                    member.contract.id
                )
            })?
        };
        ensure!(
            patched_stored.len() <= member.contract.slot_size,
            "indexed-member archive member {} exceeded its fixed physical slot",
            member.contract.id
        );
        ensure!(
            decompress(&patched_stored, false)? == patched_member,
            "indexed-member archive member {} failed exact stream readback",
            member.contract.id
        );
        let write_range =
            member.contract.stored_offset..member.contract.stored_offset + patched_stored.len();
        physical[write_range].copy_from_slice(&patched_stored);
        physical[member.contract.table_pair_offset + 4..member.contract.table_pair_offset + 8]
            .copy_from_slice(&(patched_stored.len() as u32).to_le_bytes());

        let compared_stream_size = member.contract.stored_size.max(patched_stored.len());
        let compared_range =
            member.contract.stored_offset..member.contract.stored_offset + compared_stream_size;
        reports.push(IndexedMemberBuild {
            id: member.contract.id,
            table_pair_offset: member.contract.table_pair_offset,
            stored_offset: member.contract.stored_offset,
            slot_size: member.contract.slot_size,
            source_stored_size: member.contract.stored_size,
            source_stored_sha256: member.contract.stored_sha256,
            patched_stored_size: patched_stored.len(),
            patched_stored_sha256: sha256_bytes(&patched_stored),
            decoded_size: member.contract.decoded_size,
            source_decoded_sha256: member.contract.decoded_sha256,
            patched_decoded_sha256: sha256_bytes(patched_member),
            changed_stored_byte_count: source_record[compared_range.clone()]
                .iter()
                .zip(&physical[compared_range])
                .filter(|(source, patched)| source != patched)
                .count(),
        });
    }

    let rebuilt_descriptors = parse_tzz(&physical)?;
    ensure!(
        rebuilt_descriptors.len() == source.members.len(),
        "indexed-member archive table changed its member denominator"
    );
    for ((member, rebuilt), report) in source
        .members
        .iter()
        .zip(&rebuilt_descriptors)
        .zip(&reports)
    {
        ensure!(
            rebuilt.index == member.descriptor.index
                && rebuilt.offset == member.descriptor.offset
                && rebuilt.slot_size == member.descriptor.slot_size
                && rebuilt.compressed_size == report.patched_stored_size,
            "indexed-member archive member {} rebuilt table geometry changed",
            member.contract.id
        );
        ensure!(
            decompress(&physical[rebuilt.compressed_range()], false)?
                == patched_decoded[member.decoded_range.clone()],
            "indexed-member archive member {} failed table-sized stream readback",
            member.contract.id
        );
        let slot = &physical[rebuilt.slot_range()];
        ensure!(
            decompress(slot, true)? == patched_decoded[member.decoded_range.clone()],
            "indexed-member archive member {} failed game-sized slot readback",
            member.contract.id
        );
    }
    let allowed_ranges = source
        .members
        .iter()
        .zip(&reports)
        .flat_map(|(member, report)| {
            [
                member.contract.table_pair_offset + 4..member.contract.table_pair_offset + 8,
                member.contract.stored_offset
                    ..member.contract.stored_offset
                        + member.contract.stored_size.max(report.patched_stored_size),
            ]
        })
        .collect::<Vec<_>>();
    ensure!(
        difference_ranges(source_record, &physical)
            .iter()
            .all(|[start, end]| allowed_ranges
                .iter()
                .any(|allowed| *start >= allowed.start && *end <= allowed.end)),
        "indexed-member archive changed bytes outside member streams"
    );
    Ok(IndexedMemberArchiveBuild {
        physical,
        members: reports,
    })
}

fn load_archive(
    record: &[u8],
    contract: &'static IndexedMemberArchiveContract,
) -> Result<LoadedArchive> {
    ensure!(
        record.len() == contract.record_size && sha256_bytes(record) == contract.record_sha256,
        "indexed-member archive source identity changed"
    );
    ensure!(
        contract.header_size <= record.len()
            && sha256_bytes(&record[..contract.header_size]) == contract.header_sha256,
        "indexed-member archive header identity changed"
    );
    let descriptors = parse_tzz(record)?;
    ensure!(
        descriptors.len() == contract.members.len(),
        "indexed-member archive member denominator changed"
    );

    let mut decoded = Vec::new();
    let mut members = Vec::with_capacity(descriptors.len());
    for (descriptor, member) in descriptors.into_iter().zip(contract.members) {
        ensure!(
            descriptor.index == member.index
                && descriptor.offset == member.stored_offset
                && descriptor.compressed_size == member.stored_size
                && descriptor.slot_size == member.slot_size,
            "indexed-member archive member {} table geometry changed",
            member.id
        );
        let expected_pair = [
            (member.stored_offset as u32).to_le_bytes(),
            (member.stored_size as u32).to_le_bytes(),
        ]
        .concat();
        ensure!(
            record.get(member.table_pair_offset..member.table_pair_offset + 8)
                == Some(expected_pair.as_slice()),
            "indexed-member archive member {} table pair changed",
            member.id
        );
        let source_stored = &record[descriptor.compressed_range()];
        ensure!(
            sha256_bytes(source_stored) == member.stored_sha256,
            "indexed-member archive member {} stored identity changed",
            member.id
        );
        let source_profile = source_paged_compression_profile(source_stored)?;
        ensure!(
            source_profile.stream_byte_count == member.stored_size,
            "indexed-member archive member {} stream boundary changed",
            member.id
        );
        let member_decoded = decompress(source_stored, false)?;
        ensure!(
            member_decoded.len() == member.decoded_size
                && sha256_bytes(&member_decoded) == member.decoded_sha256,
            "indexed-member archive member {} decoded identity changed",
            member.id
        );
        let decoded_start = decoded.len();
        decoded.extend_from_slice(&member_decoded);
        let decoded_end = decoded.len();
        members.push(LoadedMember {
            contract: member,
            descriptor,
            decoded_range: decoded_start..decoded_end,
            decoded: member_decoded,
            source_profile,
        });
    }
    Ok(LoadedArchive { decoded, members })
}

#[cfg(test)]
#[path = "indexed_member_archive_tests.rs"]
mod tests;
