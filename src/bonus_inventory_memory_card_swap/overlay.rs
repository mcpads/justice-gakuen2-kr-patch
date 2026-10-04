use anyhow::{Context, Result, ensure};

use crate::pipeline::{difference_ranges, sha256_bytes};

use super::command_sequences::{MEMORY_CARD_SWAP_SEQUENCES, encode_memory_card_swap_sequences};
use super::consumer::{
    validate_memory_card_swap_composed_consumer, validate_memory_card_swap_consumer,
};
use super::glyph_ownership::GlyphOwnershipValidation;
use super::model::MemoryCardSwapUnit;

#[derive(Debug)]
pub(super) struct PatchedMemoryCardSwapOverlay {
    pub(super) bytes: Vec<u8>,
    pub(super) expected_write_ranges: Vec<[usize; 2]>,
    pub(super) changed_byte_ranges: Vec<[usize; 2]>,
    pub(super) output_sequence_sha256s: Vec<String>,
    pub(super) output_payload_byte_lengths: Vec<usize>,
    pub(super) output_line_command_counts: Vec<Vec<usize>>,
    pub(super) source_command_records_match: bool,
    pub(super) glyph_ownership: GlyphOwnershipValidation,
    pub(super) changes_confined_to_fixed_command_records: bool,
}

pub(super) fn patch_memory_card_swap_overlay(
    source: &[u8],
    units: &[MemoryCardSwapUnit],
) -> Result<PatchedMemoryCardSwapOverlay> {
    patch_guarded_memory_card_swap_overlay(source, units)
}

pub(super) fn patch_composed_memory_card_swap_overlay(
    source: &[u8],
    units: &[MemoryCardSwapUnit],
) -> Result<PatchedMemoryCardSwapOverlay> {
    patch_guarded_memory_card_swap_overlay(source, units)
}

fn patch_guarded_memory_card_swap_overlay(
    source: &[u8],
    units: &[MemoryCardSwapUnit],
) -> Result<PatchedMemoryCardSwapOverlay> {
    let source_command_records_match = validate_source_owned_regions(source)?;
    let glyph_ownership = validate_memory_card_swap_consumer(source)?;
    let encoded = encode_memory_card_swap_sequences(units)?;
    let expected_write_ranges = MEMORY_CARD_SWAP_SEQUENCES
        .map(|spec| {
            [
                spec.sequence_offset,
                spec.sequence_offset + spec.storage_length,
            ]
        })
        .to_vec();
    let mut patched = source.to_vec();
    for (sequence, spec) in encoded.iter().zip(MEMORY_CARD_SWAP_SEQUENCES) {
        patched[spec.sequence_offset..spec.sequence_offset + spec.storage_length]
            .copy_from_slice(&sequence.bytes);
    }

    let changed_byte_ranges = difference_ranges(source, &patched);
    let changes_confined_to_fixed_command_records = !changed_byte_ranges.is_empty()
        && changed_byte_ranges
            .iter()
            .all(|range| range_is_covered(*range, &expected_write_ranges));
    ensure!(
        changes_confined_to_fixed_command_records,
        "bonus memory-card-swap overlay changed bytes outside its fixed command records"
    );
    validate_output(&patched, &encoded)?;
    validate_memory_card_swap_composed_consumer(&patched)?;

    Ok(PatchedMemoryCardSwapOverlay {
        bytes: patched,
        expected_write_ranges,
        changed_byte_ranges,
        output_sequence_sha256s: encoded
            .iter()
            .map(|sequence| sha256_bytes(&sequence.bytes))
            .collect(),
        output_payload_byte_lengths: encoded
            .iter()
            .map(|sequence| sequence.payload_byte_length)
            .collect(),
        output_line_command_counts: encoded
            .iter()
            .map(|sequence| sequence.line_command_counts.clone())
            .collect(),
        source_command_records_match,
        glyph_ownership,
        changes_confined_to_fixed_command_records,
    })
}

fn validate_source_owned_regions(source: &[u8]) -> Result<bool> {
    let mut source_command_records_match = true;
    for spec in MEMORY_CARD_SWAP_SEQUENCES {
        let actual = source
            .get(spec.sequence_offset..spec.sequence_offset + spec.storage_length)
            .with_context(|| {
                format!(
                    "KOUBAI2 memory-card-swap source record {} is truncated",
                    spec.id
                )
            })?;
        let record_matches =
            actual == spec.source_bytes && sha256_bytes(actual) == spec.source_sha256;
        source_command_records_match &= record_matches;
        ensure!(
            record_matches,
            "KOUBAI2 memory-card-swap source record changed for {}",
            spec.id
        );
        ensure!(
            actual.get(spec.terminator_offset - spec.sequence_offset) == Some(&0x81),
            "KOUBAI2 memory-card-swap source terminator changed for {}",
            spec.id
        );
    }
    Ok(source_command_records_match)
}

fn validate_output(
    output: &[u8],
    encoded: &[super::command_sequences::EncodedMemoryCardSwapSequence],
) -> Result<()> {
    ensure!(
        encoded.len() == MEMORY_CARD_SWAP_SEQUENCES.len(),
        "memory-card-swap output sequence count changed"
    );
    for (sequence, spec) in encoded.iter().zip(MEMORY_CARD_SWAP_SEQUENCES) {
        ensure!(
            sequence.bytes.len() == spec.storage_length
                && output.get(spec.sequence_offset..spec.sequence_offset + spec.storage_length)
                    == Some(sequence.bytes.as_slice()),
            "Korean memory-card-swap output differs from its fixed record for {}",
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
