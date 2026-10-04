use anyhow::{Context, Result, ensure};

use crate::pipeline::{difference_ranges, sha256_bytes};

use super::command_sequences::{CARD_ACQUISITION_SEQUENCES, encode_card_acquisition_sequences};
use super::consumer::{
    validate_card_acquisition_composed_consumer, validate_card_acquisition_consumer,
};
use super::glyph_ownership::GlyphOwnershipValidation;
use super::model::CardAcquisitionUnit;

#[derive(Debug)]
pub(super) struct PatchedCardAcquisitionOverlay {
    pub(super) bytes: Vec<u8>,
    pub(super) expected_write_ranges: Vec<[usize; 2]>,
    pub(super) changed_byte_ranges: Vec<[usize; 2]>,
    pub(super) output_sequence_sha256s: Vec<String>,
    pub(super) source_command_records_match: bool,
    pub(super) glyph_ownership: GlyphOwnershipValidation,
    pub(super) changes_confined_to_fixed_command_records: bool,
}

pub(super) fn patch_card_acquisition_overlay(
    source: &[u8],
    units: &[CardAcquisitionUnit],
) -> Result<PatchedCardAcquisitionOverlay> {
    patch_guarded_card_acquisition_overlay(source, units)
}

pub(super) fn patch_composed_card_acquisition_overlay(
    source: &[u8],
    units: &[CardAcquisitionUnit],
) -> Result<PatchedCardAcquisitionOverlay> {
    patch_guarded_card_acquisition_overlay(source, units)
}

fn patch_guarded_card_acquisition_overlay(
    source: &[u8],
    units: &[CardAcquisitionUnit],
) -> Result<PatchedCardAcquisitionOverlay> {
    let source_command_records_match = validate_source_owned_regions(source)?;
    let glyph_ownership = validate_card_acquisition_consumer(source)?;
    let encoded = encode_card_acquisition_sequences(units)?;
    let expected_write_ranges = CARD_ACQUISITION_SEQUENCES
        .map(|spec| {
            [
                spec.sequence_offset,
                spec.sequence_offset + spec.storage_length,
            ]
        })
        .to_vec();
    let mut patched = source.to_vec();
    for (bytes, spec) in encoded.iter().zip(CARD_ACQUISITION_SEQUENCES) {
        patched[spec.sequence_offset..spec.sequence_offset + spec.storage_length]
            .copy_from_slice(bytes);
    }

    let changed_byte_ranges = difference_ranges(source, &patched);
    let changes_confined_to_fixed_command_records = !changed_byte_ranges.is_empty()
        && changed_byte_ranges
            .iter()
            .all(|range| range_is_covered(*range, &expected_write_ranges));
    ensure!(
        changes_confined_to_fixed_command_records,
        "bonus card-acquisition overlay changed bytes outside its fixed command records"
    );
    validate_output(&patched, &encoded)?;
    // Pointer words, callers, renderer and parser must remain identical after the data-only write.
    validate_card_acquisition_composed_consumer(&patched)?;

    Ok(PatchedCardAcquisitionOverlay {
        bytes: patched,
        expected_write_ranges,
        changed_byte_ranges,
        output_sequence_sha256s: encoded.iter().map(|bytes| sha256_bytes(bytes)).collect(),
        source_command_records_match,
        glyph_ownership,
        changes_confined_to_fixed_command_records,
    })
}

fn validate_source_owned_regions(source: &[u8]) -> Result<bool> {
    let mut source_command_records_match = true;
    for spec in CARD_ACQUISITION_SEQUENCES {
        let actual = source
            .get(spec.sequence_offset..spec.sequence_offset + spec.storage_length)
            .with_context(|| {
                format!(
                    "KOUBAI2 card-acquisition source record {} is truncated",
                    spec.id
                )
            })?;
        let record_matches =
            actual == spec.source_bytes && sha256_bytes(actual) == spec.source_sha256;
        source_command_records_match &= record_matches;
        ensure!(
            record_matches,
            "KOUBAI2 card-acquisition source record changed for {}",
            spec.id
        );
        ensure!(
            actual.get(spec.terminator_offset - spec.sequence_offset) == Some(&0x81),
            "KOUBAI2 card-acquisition source terminator changed for {}",
            spec.id
        );
    }
    Ok(source_command_records_match)
}

fn validate_output(output: &[u8], encoded: &[Vec<u8>]) -> Result<()> {
    ensure!(
        encoded.len() == CARD_ACQUISITION_SEQUENCES.len(),
        "card-acquisition output sequence count changed"
    );
    for (bytes, spec) in encoded.iter().zip(CARD_ACQUISITION_SEQUENCES) {
        ensure!(
            bytes.len() == spec.storage_length
                && output.get(spec.sequence_offset..spec.sequence_offset + spec.storage_length)
                    == Some(bytes.as_slice()),
            "Korean card-acquisition output differs from its fixed record for {}",
            spec.id
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
