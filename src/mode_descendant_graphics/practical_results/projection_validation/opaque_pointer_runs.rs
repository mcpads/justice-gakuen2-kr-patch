//! Exact source validation for pointer runs with no claimed consumer meaning.

use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};

use crate::mode_descendant_graphics::source::ModeDescendantSourceRecord;
use crate::pipeline::{BASELINE_BIN_SHA256, sha256_bytes};

use super::super::model::PracticalResultAssets;
use super::super::opaque_pointer_run_model::{
    PracticalResultOpaquePointerRunAssociationStatus, PracticalResultOpaquePointerRunCatalogKind,
    PracticalResultOpaquePointerRunConsumerExclusionConclusion,
    PracticalResultOpaquePointerRunConsumerExclusionEvidence,
};
use super::source_evidence::{
    ValidationCounts, address_and_span, checked_end, parse_hex, parse_hex_u32, source_for_path,
    validate_declared_overlay_base,
};

pub(super) struct ValidatedOpaquePointerRuns {
    pub(super) declared_count: usize,
    pub(super) excluded_count: usize,
    pub(super) unresolved_count: usize,
}

pub(super) fn validate_opaque_pointer_runs(
    assets: &PracticalResultAssets,
    sources: &[ModeDescendantSourceRecord],
    counts: &mut ValidationCounts,
) -> Result<ValidatedOpaquePointerRuns> {
    let catalog = &assets.opaque_pointer_run_catalog;
    ensure!(
        matches!(
            catalog.kind,
            PracticalResultOpaquePointerRunCatalogKind::JusticeGakuen2PracticalResultOpaquePointerRunCatalog
        ) && !catalog.records.is_empty(),
        "unsupported or empty practical-result opaque-pointer-run catalog"
    );

    let mut ids = BTreeSet::new();
    let mut physical_runs = BTreeSet::new();
    let mut excluded_count = 0usize;
    let mut unresolved_count = 0usize;
    for record in &catalog.records {
        ensure!(
            !record.id.as_str().trim().is_empty()
                && ids.insert(record.id.as_str())
                && physical_runs.insert((
                    record.overlay_path.as_str(),
                    record.pointer_table_offset.as_str(),
                    record.target_arena_offset.as_str(),
                )),
            "duplicate or empty practical-result opaque pointer run {}",
            record.id
        );
        let overlay = source_for_path(sources, &record.overlay_path)?;
        ensure!(
            overlay.decoded.len() == record.overlay_size
                && sha256_bytes(&overlay.decoded) == record.overlay_sha256,
            "opaque pointer run {} overlay identity changed",
            record.id
        );
        let base = validate_declared_overlay_base(
            &record.overlay_path,
            &record.runtime_base,
            "opaque pointer run",
        )?;
        let pointer_table_offset = address_and_span(
            &overlay.decoded,
            base,
            &record.pointer_table_offset,
            &record.pointer_table_runtime_address,
            record.pointer_table_size,
            &record.pointer_table_sha256,
            "opaque pointer table",
            counts,
        )?;
        let target_arena_offset = address_and_span(
            &overlay.decoded,
            base,
            &record.target_arena_offset,
            &record.target_arena_runtime_address,
            record.target_arena_size,
            &record.target_arena_sha256,
            "opaque pointer target arena",
            counts,
        )?;
        ensure!(
            record.pointer_count > 0
                && record.pointer_table_size == record.pointer_count * 4
                && !record.association_reason.trim().is_empty(),
            "opaque pointer run {} denominator or association reason changed",
            record.id
        );
        let target_arena_end = checked_end(
            target_arena_offset,
            record.target_arena_size,
            "opaque pointer target arena",
        )?;
        for index in 0..record.pointer_count {
            let pointer_offset = pointer_table_offset
                .checked_add(
                    index
                        .checked_mul(4)
                        .context("opaque pointer index overflow")?,
                )
                .context("opaque pointer offset overflow")?;
            let pointer = u32::from_le_bytes(
                overlay.decoded[pointer_offset..pointer_offset + 4]
                    .try_into()
                    .expect("validated four-byte pointer"),
            );
            let target_offset = usize::try_from(
                pointer
                    .checked_sub(base)
                    .with_context(|| format!("opaque pointer {index} precedes its overlay"))?,
            )?;
            ensure!(
                target_offset >= target_arena_offset && target_offset < target_arena_end,
                "opaque pointer run {} pointer {index} escapes its target arena",
                record.id
            );
        }
        match record.association_status {
            PracticalResultOpaquePointerRunAssociationStatus::Unresolved => {
                ensure!(
                    record.adopted_consumer_exclusion_evidence.is_none(),
                    "unresolved opaque pointer run {} claims exclusion evidence",
                    record.id
                );
                unresolved_count += 1;
            }
            PracticalResultOpaquePointerRunAssociationStatus::ExcludedFromSourceAtlasConsumerDenominator => {
                let evidence = record
                    .adopted_consumer_exclusion_evidence
                    .as_ref()
                    .with_context(|| {
                        format!(
                            "source-atlas-excluded opaque pointer run {} lacks adopted evidence",
                            record.id
                        )
                    })?;
                validate_adopted_consumer_exclusion(
                    record.id.as_str(),
                    record.pointer_count,
                    &record.overlay_sha256,
                    evidence,
                    &overlay.decoded,
                )?;
                excluded_count += 1;
            }
        }
    }

    Ok(ValidatedOpaquePointerRuns {
        declared_count: catalog.records.len(),
        excluded_count,
        unresolved_count,
    })
}

fn validate_adopted_consumer_exclusion(
    pointer_run_id: &str,
    pointer_count: usize,
    overlay_sha256: &str,
    evidence: &PracticalResultOpaquePointerRunConsumerExclusionEvidence,
    overlay: &[u8],
) -> Result<()> {
    ensure!(
        !evidence.analysis_report_path.is_empty()
            && evidence.analysis_report_path.ends_with(".json")
            && valid_sha256(&evidence.analysis_report_sha256)
            && evidence.source_bin_sha256 == BASELINE_BIN_SHA256
            && evidence.loaded_image_sha256 == overlay_sha256
            && evidence.value_flow_state_budget == 262_144
            && evidence.decoded_pointer_count == pointer_count
            && evidence.raw_address_reference_count == pointer_count
            && evidence.external_raw_address_reference_count == 0
            && evidence.reachable_derived_reference_count == 0
            && evidence.reachable_pointer_table_reference_count == 0
            && evidence.reachable_target_arena_reference_count == 0
            && evidence.unprofiled_exhausted_seed_count == 0
            && evidence.unresolved_indirect_jump_count == 0
            && !evidence.analysis_product_build_input
            && matches!(
                evidence.conclusion,
                PracticalResultOpaquePointerRunConsumerExclusionConclusion::NoReachablePointerRunReferenceInReviewedDeclaredEntrypointFlow
            ),
        "opaque pointer run {pointer_run_id} adopted exclusion evidence is incomplete"
    );
    ensure!(
        !evidence.profiled_non_address_exhausted_seed_ids.is_empty()
            && evidence
                .profiled_non_address_exhausted_seed_ids
                .iter()
                .all(|id| !id.trim().is_empty())
            && evidence
                .profiled_non_address_exhausted_seed_ids
                .iter()
                .collect::<BTreeSet<_>>()
                .len()
                == evidence.profiled_non_address_exhausted_seed_ids.len(),
        "opaque pointer run {pointer_run_id} non-address seed evidence is empty or duplicated"
    );
    let mut entrypoint_offsets = BTreeSet::new();
    ensure!(
        !evidence.declared_entrypoints.is_empty(),
        "opaque pointer run {pointer_run_id} has no adopted entrypoints"
    );
    for entrypoint in &evidence.declared_entrypoints {
        let offset = parse_hex(
            &entrypoint.source_reference_offset,
            "adopted entrypoint source offset",
        )?;
        let end = checked_end(offset, 4, "adopted entrypoint source word")?;
        let bytes: [u8; 4] = overlay
            .get(offset..end)
            .with_context(|| {
                format!("opaque pointer run {pointer_run_id} adopted entrypoint is truncated")
            })?
            .try_into()
            .expect("four-byte entrypoint source");
        ensure!(
            offset % 4 == 0
                && entrypoint_offsets.insert(offset)
                && u32::from_le_bytes(bytes)
                    == parse_hex_u32(&entrypoint.runtime_address, "adopted entrypoint address")?,
            "opaque pointer run {pointer_run_id} adopted entrypoint changed"
        );
    }
    Ok(())
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
#[path = "opaque_pointer_runs_tests.rs"]
mod tests;
