//! Final record selection after surface-local and EDIT runtime-text builds.

use anyhow::{Context, Result, ensure};

use crate::compression::decompress;
use crate::decoded_record_write_plan::{DecodedDataClaim, DecodedRecordWritePlan};
use crate::edit_runtime_text::EditRuntimeTextBuild;
use crate::mode_descendant_graphics::{
    ModeDescendantGraphicsBuild, ModeDescendantRecord, ModeDescendantRecordBuild,
    ModeDescendantStorageKind,
};
use crate::pipeline::{difference_ranges, sha256_bytes};
use crate::practical_instruction_graphics::PracticalInstructionGraphicsBuild;

pub(super) struct FinalModeDescendantRecord<'a> {
    pub(super) report: &'a ModeDescendantRecordBuild,
    pub(super) stored: Vec<u8>,
    pub(super) stored_sha256: String,
    pub(super) decoded_sha256: String,
}

pub(super) fn final_mode_descendant_records<'a>(
    build: &'a ModeDescendantGraphicsBuild,
    edit_runtime_text: &'a EditRuntimeTextBuild,
    practical_instructions: &'a PracticalInstructionGraphicsBuild,
) -> Result<Vec<FinalModeDescendantRecord<'a>>> {
    ensure!(
        build.reported_records_match_stored_records(),
        "mode-descendant build record outputs do not match its report"
    );
    build
        .report
        .records
        .iter()
        .map(|report| {
            let (stored, stored_sha256, decoded_sha256) = match report.record {
                ModeDescendantRecord::EditSharedUi => (
                    edit_runtime_text.edit_shared_ui_stored.clone(),
                    edit_runtime_text
                        .report
                        .output_edit_shared_ui_stored_sha256
                        .clone(),
                    edit_runtime_text
                        .report
                        .output_edit_shared_ui_decoded_sha256
                        .clone(),
                ),
                record => {
                    let stored = build
                        .stored_record(record)
                        .with_context(|| format!("mode-descendant build lost {record:?}"))?;
                    if report.source_path
                        == practical_instructions.battle_subject_title_consumer.path
                    {
                        let stored =
                            compose_battle_subject_title_consumer(stored, practical_instructions)?;
                        let decoded = decode_mode_descendant_bytes(report.storage_kind, &stored)?;
                        let stored_sha256 = sha256_bytes(&stored);
                        let decoded_sha256 = sha256_bytes(&decoded);
                        (stored, stored_sha256, decoded_sha256)
                    } else {
                        (
                            stored.to_vec(),
                            report.patched_stored_sha256.clone(),
                            report.patched_decoded_sha256.clone(),
                        )
                    }
                }
            };
            ensure!(
                sha256_bytes(&stored) == stored_sha256,
                "{} final mode-descendant physical bytes changed",
                report.source_path
            );
            let decoded = decode_mode_descendant_bytes(report.storage_kind, &stored)?;
            ensure!(
                sha256_bytes(&decoded) == decoded_sha256,
                "{} final mode-descendant decoded bytes changed",
                report.source_path
            );
            Ok(FinalModeDescendantRecord {
                report,
                stored,
                stored_sha256,
                decoded_sha256,
            })
        })
        .collect()
}

fn compose_battle_subject_title_consumer(
    mode_descendant: &[u8],
    practical_instructions: &PracticalInstructionGraphicsBuild,
) -> Result<Vec<u8>> {
    let consumer = &practical_instructions.battle_subject_title_consumer;
    ensure!(
        consumer.path == "DAT1/SIKEN.BIN"
            && consumer.source_record.len() == consumer.source_record_size
            && mode_descendant.len() == consumer.source_record_size
            && consumer.record.len() == consumer.source_record_size
            && sha256_bytes(&consumer.source_record) == consumer.source_record_sha256
            && sha256_bytes(&consumer.record) == consumer.patched_record_sha256,
        "battle subject-title consumer composition inputs changed"
    );
    let mode_claims = DecodedDataClaim::from_effective_ranges(
        "mode-descendant-siken",
        "retain the independently owned practical selection and result descriptor writes",
        &consumer.source_record,
        mode_descendant,
        difference_ranges(&consumer.source_record, mode_descendant),
    )?;
    let title_claims = DecodedDataClaim::from_effective_ranges(
        "battle-subject-title-consumer",
        "move the old glyph prefix offscreen, reposition the member strip, and bind its widths",
        &consumer.source_record,
        &consumer.record,
        difference_ranges(&consumer.source_record, &consumer.record),
    )?;
    ensure!(
        !mode_claims.is_empty() && !title_claims.is_empty(),
        "battle subject-title consumer composition has an empty contributor"
    );
    let mut plan = DecodedRecordWritePlan::new(
        &consumer.path,
        &consumer.source_record,
        &consumer.source_record_sha256,
    )?;
    plan.register_data_candidate(
        "mode-descendant-siken",
        &consumer.source_record_sha256,
        mode_descendant,
        &mode_claims,
    )?;
    plan.register_data_candidate(
        "battle-subject-title-consumer",
        &consumer.source_record_sha256,
        &consumer.record,
        &title_claims,
    )?;
    plan.apply(None)
}

pub(super) fn decode_mode_descendant_bytes(
    storage_kind: ModeDescendantStorageKind,
    physical: &[u8],
) -> Result<Vec<u8>> {
    match storage_kind {
        ModeDescendantStorageKind::IndexedCompressedMembers
        | ModeDescendantStorageKind::TzzCompressedMembers => {
            let mut decoded = Vec::new();
            for member in crate::tzz::parse_tzz(physical)? {
                decoded.extend_from_slice(&decompress(&physical[member.compressed_range()], true)?);
            }
            Ok(decoded)
        }
        ModeDescendantStorageKind::PagedCompressed => decompress(physical, true),
        ModeDescendantStorageKind::Raw => Ok(physical.to_vec()),
    }
}
