use anyhow::{Context, Result, ensure};

use crate::pipeline::{difference_ranges, sha256_bytes};

use super::command_sequence::{
    EncodedJBankReturnLabel, RETURN_LABEL_SEQUENCE, encode_return_label,
};
use super::consumer::{validate_return_label_composed_consumer, validate_return_label_consumer};
use super::glyph_ownership::GlyphOwnershipValidation;
use super::model::{JBankReturnLabelGlyphAllocation, JBankReturnLabelUnit};

#[derive(Debug)]
pub(super) struct PatchedJBankReturnLabelOverlay {
    pub(super) bytes: Vec<u8>,
    pub(super) expected_write_ranges: Vec<[usize; 2]>,
    pub(super) changed_byte_ranges: Vec<[usize; 2]>,
    pub(super) output_sequence_sha256: String,
    pub(super) output_payload_byte_length: usize,
    pub(super) output_command_count: usize,
    pub(super) source_command_record_matches: bool,
    pub(super) glyph_ownership: GlyphOwnershipValidation,
    pub(super) changes_confined_to_fixed_command_record: bool,
}

pub(super) fn patch_return_label_overlay(
    source: &[u8],
    unit: &JBankReturnLabelUnit,
    allocations: &[JBankReturnLabelGlyphAllocation],
) -> Result<PatchedJBankReturnLabelOverlay> {
    patch_guarded_return_label_overlay(source, unit, allocations)
}

pub(super) fn patch_composed_return_label_overlay(
    source: &[u8],
    unit: &JBankReturnLabelUnit,
    allocations: &[JBankReturnLabelGlyphAllocation],
) -> Result<PatchedJBankReturnLabelOverlay> {
    patch_guarded_return_label_overlay(source, unit, allocations)
}

fn patch_guarded_return_label_overlay(
    source: &[u8],
    unit: &JBankReturnLabelUnit,
    allocations: &[JBankReturnLabelGlyphAllocation],
) -> Result<PatchedJBankReturnLabelOverlay> {
    let source_command_record_matches = validate_source_owned_record(source)?;
    let glyph_ownership = validate_return_label_consumer(source)?;
    let encoded = encode_return_label(unit, allocations)?;
    let expected_write_ranges = vec![[
        RETURN_LABEL_SEQUENCE.sequence_offset,
        RETURN_LABEL_SEQUENCE.sequence_offset + RETURN_LABEL_SEQUENCE.storage_length,
    ]];
    let mut patched = source.to_vec();
    patched[RETURN_LABEL_SEQUENCE.sequence_offset
        ..RETURN_LABEL_SEQUENCE.sequence_offset + RETURN_LABEL_SEQUENCE.storage_length]
        .copy_from_slice(&encoded.bytes);

    let changed_byte_ranges = difference_ranges(source, &patched);
    let changes_confined_to_fixed_command_record = !changed_byte_ranges.is_empty()
        && changed_byte_ranges
            .iter()
            .all(|range| range_is_covered(*range, &expected_write_ranges));
    ensure!(
        changes_confined_to_fixed_command_record,
        "J-BANK return-label overlay changed bytes outside its fixed command record"
    );
    validate_output(&patched, &encoded)?;
    validate_return_label_composed_consumer(&patched)?;

    Ok(PatchedJBankReturnLabelOverlay {
        bytes: patched,
        expected_write_ranges,
        changed_byte_ranges,
        output_sequence_sha256: sha256_bytes(&encoded.bytes),
        output_payload_byte_length: encoded.payload_byte_length,
        output_command_count: encoded.command_count,
        source_command_record_matches,
        glyph_ownership,
        changes_confined_to_fixed_command_record,
    })
}

fn validate_source_owned_record(source: &[u8]) -> Result<bool> {
    let actual = source
        .get(
            RETURN_LABEL_SEQUENCE.sequence_offset
                ..RETURN_LABEL_SEQUENCE.sequence_offset + RETURN_LABEL_SEQUENCE.storage_length,
        )
        .context("KOUBAI2 J-BANK return-label source record is truncated")?;
    let source_command_record_matches = actual == RETURN_LABEL_SEQUENCE.source_bytes
        && sha256_bytes(actual) == RETURN_LABEL_SEQUENCE.source_sha256;
    ensure!(
        source_command_record_matches,
        "KOUBAI2 J-BANK return-label source record changed"
    );
    ensure!(
        actual.get(RETURN_LABEL_SEQUENCE.terminator_offset - RETURN_LABEL_SEQUENCE.sequence_offset)
            == Some(&0x81)
            && actual
                .get(
                    RETURN_LABEL_SEQUENCE.terminator_offset + 1
                        - RETURN_LABEL_SEQUENCE.sequence_offset..,
                )
                .is_some_and(|padding| padding.iter().all(|byte| *byte == 0)),
        "KOUBAI2 J-BANK return-label source terminator or padding changed"
    );
    Ok(source_command_record_matches)
}

fn validate_output(output: &[u8], encoded: &EncodedJBankReturnLabel) -> Result<()> {
    let record = output
        .get(
            RETURN_LABEL_SEQUENCE.sequence_offset
                ..RETURN_LABEL_SEQUENCE.sequence_offset + RETURN_LABEL_SEQUENCE.storage_length,
        )
        .context("Korean J-BANK return-label output record is truncated")?;
    ensure!(
        record == encoded.bytes
            && encoded.payload_byte_length <= RETURN_LABEL_SEQUENCE.storage_length
            && encoded.command_count <= (RETURN_LABEL_SEQUENCE.storage_length - 1) / 3,
        "Korean J-BANK return-label output exceeds or differs from its fixed record"
    );
    ensure!(
        record.get(encoded.payload_byte_length - 1) == Some(&0x81)
            && record[encoded.payload_byte_length..]
                .iter()
                .all(|byte| *byte == 0),
        "Korean J-BANK return-label output terminator or padding changed"
    );
    Ok(())
}

fn range_is_covered(range: [usize; 2], allowed: &[[usize; 2]]) -> bool {
    (range[0]..range[1]).all(|offset| {
        allowed
            .iter()
            .any(|[start, end]| *start <= offset && offset < *end)
    })
}
