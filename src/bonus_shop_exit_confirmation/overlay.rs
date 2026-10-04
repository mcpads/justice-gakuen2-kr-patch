use anyhow::{Context, Result, ensure};

use crate::pipeline::{difference_ranges, sha256_bytes};

use super::command_sequence::{
    EncodedShopExitConfirmation, RECORD_SPECS, encode_exit_confirmation,
};
use super::consumer::{
    validate_exit_confirmation_composed_consumer, validate_exit_confirmation_consumer,
};
use super::glyph_ownership::GlyphOwnershipValidation;
use super::model::{ShopExitGlyphAllocation, ShopExitTextUnit};

#[derive(Debug)]
pub(super) struct PatchedShopExitOverlay {
    pub(super) bytes: Vec<u8>,
    pub(super) expected_write_ranges: Vec<[usize; 2]>,
    pub(super) changed_byte_ranges: Vec<[usize; 2]>,
    pub(super) records: Vec<PatchedShopExitRecord>,
    pub(super) unit_command_counts: std::collections::BTreeMap<String, usize>,
    pub(super) source_command_records_match: bool,
    pub(super) glyph_ownership: GlyphOwnershipValidation,
    pub(super) changes_confined_to_fixed_record: bool,
}

#[derive(Debug)]
pub(super) struct PatchedShopExitRecord {
    pub(super) variant_id: &'static str,
    pub(super) output_record_sha256: String,
    pub(super) output_record_byte_length: usize,
    pub(super) output_line_count: usize,
    pub(super) output_translation_command_counts: [usize; 3],
}

pub(super) fn patch_exit_confirmation_overlay(
    source: &[u8],
    units: &[ShopExitTextUnit],
    allocations: &[ShopExitGlyphAllocation],
) -> Result<PatchedShopExitOverlay> {
    patch_guarded_exit_confirmation_overlay(source, units, allocations)
}

pub(super) fn patch_composed_exit_confirmation_overlay(
    source: &[u8],
    units: &[ShopExitTextUnit],
    allocations: &[ShopExitGlyphAllocation],
) -> Result<PatchedShopExitOverlay> {
    patch_guarded_exit_confirmation_overlay(source, units, allocations)
}

fn patch_guarded_exit_confirmation_overlay(
    source: &[u8],
    units: &[ShopExitTextUnit],
    allocations: &[ShopExitGlyphAllocation],
) -> Result<PatchedShopExitOverlay> {
    let source_command_records_match = validate_source_owned_records(source)?;
    let glyph_ownership = validate_exit_confirmation_consumer(source)?;
    let encoded = encode_exit_confirmation(units, allocations)?;
    let expected_write_ranges = RECORD_SPECS
        .iter()
        .map(|spec| {
            [
                spec.record_offset,
                spec.record_offset + spec.source_record.len(),
            ]
        })
        .collect::<Vec<_>>();
    let mut patched = source.to_vec();
    for (spec, record) in RECORD_SPECS.iter().zip(&encoded.records) {
        ensure!(
            record.variant_id == spec.variant_id,
            "shop exit-confirmation encoded clerk order changed"
        );
        patched[spec.record_offset..spec.record_offset + spec.source_record.len()]
            .copy_from_slice(&record.bytes);
    }

    let changed_byte_ranges = difference_ranges(source, &patched);
    let changes_confined_to_fixed_record = !changed_byte_ranges.is_empty()
        && changed_byte_ranges
            .iter()
            .all(|range| range_is_covered(*range, &expected_write_ranges));
    ensure!(
        changes_confined_to_fixed_record,
        "shop exit-confirmation overlay changed bytes outside its fixed record"
    );
    validate_output(&patched, &encoded)?;
    validate_exit_confirmation_composed_consumer(&patched)?;

    let records = RECORD_SPECS
        .iter()
        .zip(&encoded.records)
        .map(|(spec, record)| PatchedShopExitRecord {
            variant_id: spec.variant_id,
            output_record_sha256: sha256_bytes(&record.bytes),
            output_record_byte_length: record.bytes.len(),
            output_line_count: record.line_count,
            output_translation_command_counts: record.unit_command_counts,
        })
        .collect();

    Ok(PatchedShopExitOverlay {
        bytes: patched,
        expected_write_ranges,
        changed_byte_ranges,
        records,
        unit_command_counts: encoded.unit_command_counts,
        source_command_records_match,
        glyph_ownership,
        changes_confined_to_fixed_record,
    })
}

fn validate_source_owned_records(source: &[u8]) -> Result<bool> {
    for spec in RECORD_SPECS {
        let actual = source
            .get(spec.record_offset..spec.record_offset + spec.source_record.len())
            .with_context(|| {
                format!(
                    "KOUBAI shop exit-confirmation {} source record is truncated",
                    spec.variant_id
                )
            })?;
        ensure!(
            actual == spec.source_record && sha256_bytes(actual) == spec.source_record_sha256,
            "KOUBAI shop exit-confirmation {} source record changed",
            spec.variant_id
        );
        ensure!(
            spec.terminator_offset + 1 == spec.record_offset + spec.source_record.len()
                && actual.last() == Some(&0x81),
            "KOUBAI shop exit-confirmation {} terminator or no-padding boundary changed",
            spec.variant_id
        );
    }
    Ok(true)
}

fn validate_output(output: &[u8], encoded: &EncodedShopExitConfirmation) -> Result<()> {
    for (spec, encoded_record) in RECORD_SPECS.iter().zip(&encoded.records) {
        let record = output
            .get(spec.record_offset..spec.record_offset + spec.source_record.len())
            .with_context(|| {
                format!(
                    "Korean shop exit-confirmation {} output record is truncated",
                    spec.variant_id
                )
            })?;
        ensure!(
            encoded_record.variant_id == spec.variant_id
                && record == encoded_record.bytes
                && encoded_record.bytes.len() == spec.source_record.len()
                && encoded_record.bytes.last() == Some(&0x81),
            "Korean shop exit-confirmation {} output differs from its exact record geometry",
            spec.variant_id
        );
    }
    Ok(())
}

fn range_is_covered(range: [usize; 2], allowed: &[[usize; 2]]) -> bool {
    (range[0]..range[1]).all(|offset| {
        allowed
            .iter()
            .any(|[start, end]| *start <= offset && offset < *end)
    })
}
