use std::collections::BTreeMap;

use anyhow::{Context, Result, ensure};
use psx_r3000a::{Instruction, Register, encode};

use crate::pipeline::{difference_ranges, sha256_bytes};

use super::command_sequences::{
    MEMORY_CARD_COPY_PROMPT_LENGTH, MEMORY_CARD_COPY_PROMPT_OFFSET, MEMORY_CARD_DESTINATION_LENGTH,
    MEMORY_CARD_DESTINATION_OFFSET, SOURCE_MEMORY_CARD_COPY_PROMPT,
    SOURCE_MEMORY_CARD_COPY_PROMPT_SHA256, SOURCE_MEMORY_CARD_DESTINATION,
    SOURCE_MEMORY_CARD_DESTINATION_SHA256, encode_memory_card_sequences,
};
use super::consumer::{
    CHOICE_LOOP_BOUND_INSTRUCTION_OFFSET, CHOICE_SEPARATOR_INDEX_INSTRUCTION_OFFSET,
    CHOICE_X_ORIGIN_INSTRUCTION_OFFSET, OVERLAY_RUNTIME_BASE, decode_instruction,
    validate_confirmation_composed_consumer, validate_confirmation_source_consumer,
};
use super::glyph_slots::CONFIRMATION_GLYPHS;
use super::model::ConfirmationTextUnit;
use super::records::{
    ALTERNATE_PROMPT_RECORD_LENGTH, ALTERNATE_PROMPT_RECORD_OFFSET, EXIT_PROMPT_RECORD_LENGTH,
    EXIT_PROMPT_RECORD_OFFSET, SHARED_CHOICE_RECORD_LENGTH, SHARED_CHOICE_RECORD_OFFSET,
    SOURCE_ALTERNATE_PROMPT_RECORD, SOURCE_ALTERNATE_PROMPT_SHA256, SOURCE_EXIT_PROMPT_RECORD,
    SOURCE_EXIT_PROMPT_SHA256, SOURCE_SHARED_CHOICE_RECORD, SOURCE_SHARED_CHOICE_SHA256,
    encode_confirmation_records,
};

const OUTPUT_CHOICE_X_ORIGIN: i16 = 196;
const OUTPUT_CHOICE_SEPARATOR_INDEX: i16 = 0;
const OUTPUT_CHOICE_GLYPH_COUNT: i16 = 4;

#[derive(Debug)]
pub(super) struct PatchedConfirmationOverlay {
    pub(super) bytes: Vec<u8>,
    pub(super) expected_write_ranges: Vec<[usize; 2]>,
    pub(super) changed_byte_ranges: Vec<[usize; 2]>,
    pub(super) output_exit_prompt_sha256: String,
    pub(super) output_selected_card_prompt_sha256: String,
    pub(super) output_memory_card_destination_sha256: String,
    pub(super) output_memory_card_copy_prompt_sha256: String,
    pub(super) output_shared_choice_sha256: String,
    pub(super) output_unit_sha256s: Vec<String>,
}

pub(super) fn patch_confirmation_overlay(
    source: &[u8],
    units: &[ConfirmationTextUnit],
) -> Result<PatchedConfirmationOverlay> {
    validate_confirmation_source_consumer(source)?;
    patch_owned_regions(source, units)
}

pub(super) fn patch_composed_confirmation_overlay(
    source: &[u8],
    units: &[ConfirmationTextUnit],
) -> Result<PatchedConfirmationOverlay> {
    validate_confirmation_composed_consumer(source)?;
    patch_owned_regions(source, units)
}

fn patch_owned_regions(
    source: &[u8],
    units: &[ConfirmationTextUnit],
) -> Result<PatchedConfirmationOverlay> {
    validate_source_owned_regions(source)?;
    let glyph_codes = CONFIRMATION_GLYPHS
        .iter()
        .map(|(text, code, _)| (*text, *code))
        .collect::<BTreeMap<_, _>>();
    let records = encode_confirmation_records(units, &glyph_codes)?;
    let commands = encode_memory_card_sequences(units, &glyph_codes)?;

    let mut patched = source.to_vec();
    patched[MEMORY_CARD_DESTINATION_OFFSET
        ..MEMORY_CARD_DESTINATION_OFFSET + MEMORY_CARD_DESTINATION_LENGTH]
        .copy_from_slice(&commands.destination);
    patched[MEMORY_CARD_COPY_PROMPT_OFFSET
        ..MEMORY_CARD_COPY_PROMPT_OFFSET + MEMORY_CARD_COPY_PROMPT_LENGTH]
        .copy_from_slice(&commands.copy_prompt);
    patched[EXIT_PROMPT_RECORD_OFFSET..EXIT_PROMPT_RECORD_OFFSET + EXIT_PROMPT_RECORD_LENGTH]
        .copy_from_slice(&records.exit_prompt);
    patched[ALTERNATE_PROMPT_RECORD_OFFSET
        ..ALTERNATE_PROMPT_RECORD_OFFSET + ALTERNATE_PROMPT_RECORD_LENGTH]
        .copy_from_slice(&records.selected_card_prompt);
    patched[SHARED_CHOICE_RECORD_OFFSET..SHARED_CHOICE_RECORD_OFFSET + SHARED_CHOICE_RECORD_LENGTH]
        .copy_from_slice(&records.choices);
    write_choice_geometry(&mut patched)?;

    let expected_write_ranges = expected_write_ranges();
    let changed_byte_ranges = difference_ranges(source, &patched);
    ensure!(
        !changed_byte_ranges.is_empty()
            && changed_byte_ranges.iter().all(|[start, end]| {
                expected_write_ranges
                    .iter()
                    .any(|[allowed_start, allowed_end]| {
                        allowed_start <= start && end <= allowed_end
                    })
            }),
        "bonus confirmation overlay changed bytes outside owned records and instructions"
    );
    validate_output(&patched, &records, &commands)?;

    let output_shared_choice_sha256 = sha256_bytes(&records.choices);
    Ok(PatchedConfirmationOverlay {
        bytes: patched,
        expected_write_ranges,
        changed_byte_ranges,
        output_exit_prompt_sha256: sha256_bytes(&records.exit_prompt),
        output_selected_card_prompt_sha256: sha256_bytes(&records.selected_card_prompt),
        output_memory_card_destination_sha256: sha256_bytes(&commands.destination),
        output_memory_card_copy_prompt_sha256: sha256_bytes(&commands.copy_prompt),
        output_shared_choice_sha256,
        output_unit_sha256s: vec![
            sha256_bytes(&records.exit_prompt),
            sha256_bytes(&records.selected_card_prompt),
            sha256_bytes(&commands.destination),
            sha256_bytes(&commands.copy_prompt),
            sha256_bytes(&records.choices[..3]),
            sha256_bytes(&records.choices[3..12]),
        ],
    })
}

fn validate_source_owned_regions(source: &[u8]) -> Result<()> {
    for (offset, expected, expected_sha256, label) in [
        (
            MEMORY_CARD_DESTINATION_OFFSET,
            SOURCE_MEMORY_CARD_DESTINATION.as_slice(),
            SOURCE_MEMORY_CARD_DESTINATION_SHA256,
            "memory-card destination command sequence",
        ),
        (
            MEMORY_CARD_COPY_PROMPT_OFFSET,
            SOURCE_MEMORY_CARD_COPY_PROMPT.as_slice(),
            SOURCE_MEMORY_CARD_COPY_PROMPT_SHA256,
            "memory-card copy prompt command sequence",
        ),
        (
            EXIT_PROMPT_RECORD_OFFSET,
            SOURCE_EXIT_PROMPT_RECORD.as_slice(),
            SOURCE_EXIT_PROMPT_SHA256,
            "exit prompt record",
        ),
        (
            ALTERNATE_PROMPT_RECORD_OFFSET,
            SOURCE_ALTERNATE_PROMPT_RECORD.as_slice(),
            SOURCE_ALTERNATE_PROMPT_SHA256,
            "selected-card prompt record",
        ),
        (
            SHARED_CHOICE_RECORD_OFFSET,
            SOURCE_SHARED_CHOICE_RECORD.as_slice(),
            SOURCE_SHARED_CHOICE_SHA256,
            "shared confirmation choice record",
        ),
    ] {
        let actual = source
            .get(offset..offset + expected.len())
            .with_context(|| format!("KOUBAI2 {label} is truncated"))?;
        ensure!(
            actual == expected && sha256_bytes(actual) == expected_sha256,
            "KOUBAI2 {label} changed"
        );
    }
    Ok(())
}

fn write_choice_geometry(output: &mut [u8]) -> Result<()> {
    for (offset, instruction) in [
        (
            CHOICE_X_ORIGIN_INSTRUCTION_OFFSET,
            Instruction::Addiu {
                rt: Register::S3,
                rs: Register::ZERO,
                immediate: OUTPUT_CHOICE_X_ORIGIN,
            },
        ),
        (
            CHOICE_SEPARATOR_INDEX_INSTRUCTION_OFFSET,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::ZERO,
                immediate: OUTPUT_CHOICE_SEPARATOR_INDEX,
            },
        ),
        (
            CHOICE_LOOP_BOUND_INSTRUCTION_OFFSET,
            Instruction::Slti {
                rt: Register::V0,
                rs: Register::S4,
                immediate: OUTPUT_CHOICE_GLYPH_COUNT,
            },
        ),
    ] {
        let word = encode(&instruction, OVERLAY_RUNTIME_BASE + offset as u32)?;
        output
            .get_mut(offset..offset + 4)
            .context("KOUBAI2 confirmation instruction write is truncated")?
            .copy_from_slice(&word.to_le_bytes());
    }
    Ok(())
}

fn validate_output(
    output: &[u8],
    records: &super::records::EncodedConfirmationRecords,
    commands: &super::command_sequences::EncodedMemoryCardSequences,
) -> Result<()> {
    for (offset, expected, label) in [
        (
            MEMORY_CARD_DESTINATION_OFFSET,
            commands.destination.as_slice(),
            "destination",
        ),
        (
            MEMORY_CARD_COPY_PROMPT_OFFSET,
            commands.copy_prompt.as_slice(),
            "copy prompt",
        ),
        (
            EXIT_PROMPT_RECORD_OFFSET,
            records.exit_prompt.as_slice(),
            "exit prompt",
        ),
        (
            ALTERNATE_PROMPT_RECORD_OFFSET,
            records.selected_card_prompt.as_slice(),
            "selected-card prompt",
        ),
        (
            SHARED_CHOICE_RECORD_OFFSET,
            records.choices.as_slice(),
            "shared choices",
        ),
    ] {
        ensure!(
            output.get(offset..offset + expected.len()) == Some(expected),
            "Korean confirmation {label} differs from its plan"
        );
    }
    ensure!(
        decode_instruction(output, CHOICE_X_ORIGIN_INSTRUCTION_OFFSET)?
            == Instruction::Addiu {
                rt: Register::S3,
                rs: Register::ZERO,
                immediate: OUTPUT_CHOICE_X_ORIGIN,
            }
            && decode_instruction(output, CHOICE_SEPARATOR_INDEX_INSTRUCTION_OFFSET)?
                == Instruction::Addiu {
                    rt: Register::V0,
                    rs: Register::ZERO,
                    immediate: OUTPUT_CHOICE_SEPARATOR_INDEX,
                }
            && decode_instruction(output, CHOICE_LOOP_BOUND_INSTRUCTION_OFFSET)?
                == Instruction::Slti {
                    rt: Register::V0,
                    rs: Register::S4,
                    immediate: OUTPUT_CHOICE_GLYPH_COUNT,
                },
        "Korean shared confirmation choice geometry differs from its plan"
    );
    Ok(())
}

fn expected_write_ranges() -> Vec<[usize; 2]> {
    vec![
        [
            MEMORY_CARD_DESTINATION_OFFSET,
            MEMORY_CARD_DESTINATION_OFFSET + MEMORY_CARD_DESTINATION_LENGTH,
        ],
        [
            MEMORY_CARD_COPY_PROMPT_OFFSET,
            MEMORY_CARD_COPY_PROMPT_OFFSET + MEMORY_CARD_COPY_PROMPT_LENGTH,
        ],
        [
            EXIT_PROMPT_RECORD_OFFSET,
            EXIT_PROMPT_RECORD_OFFSET + EXIT_PROMPT_RECORD_LENGTH,
        ],
        [
            ALTERNATE_PROMPT_RECORD_OFFSET,
            ALTERNATE_PROMPT_RECORD_OFFSET + ALTERNATE_PROMPT_RECORD_LENGTH,
        ],
        [
            SHARED_CHOICE_RECORD_OFFSET,
            SHARED_CHOICE_RECORD_OFFSET + SHARED_CHOICE_RECORD_LENGTH,
        ],
        [
            CHOICE_X_ORIGIN_INSTRUCTION_OFFSET,
            CHOICE_X_ORIGIN_INSTRUCTION_OFFSET + 4,
        ],
        [
            CHOICE_SEPARATOR_INDEX_INSTRUCTION_OFFSET,
            CHOICE_SEPARATOR_INDEX_INSTRUCTION_OFFSET + 4,
        ],
        [
            CHOICE_LOOP_BOUND_INSTRUCTION_OFFSET,
            CHOICE_LOOP_BOUND_INSTRUCTION_OFFSET + 4,
        ],
    ]
}
