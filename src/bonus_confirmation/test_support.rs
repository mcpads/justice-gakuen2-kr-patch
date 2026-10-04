use psx_r3000a::{Instruction, encode};

use super::command_sequences::{
    MEMORY_CARD_COPY_PROMPT_OFFSET, MEMORY_CARD_DESTINATION_OFFSET, SOURCE_MEMORY_CARD_COPY_PROMPT,
    SOURCE_MEMORY_CARD_DESTINATION,
};
use super::consumer::{
    MEMORY_CARD_COPY_PROMPT_POINTER_OFFSET, MEMORY_CARD_DESTINATION_POINTER_OFFSET,
    OVERLAY_RUNTIME_BASE, source_consumer_instructions, source_stock_renderer_evidence,
};
use super::model::ConfirmationTextUnit;
use super::records::{
    ALTERNATE_PROMPT_RECORD_LENGTH, ALTERNATE_PROMPT_RECORD_OFFSET, EXIT_PROMPT_RECORD_LENGTH,
    EXIT_PROMPT_RECORD_OFFSET, SHARED_CHOICE_RECORD_LENGTH, SHARED_CHOICE_RECORD_OFFSET,
    SOURCE_ALTERNATE_PROMPT_RECORD, SOURCE_EXIT_PROMPT_RECORD, SOURCE_SHARED_CHOICE_RECORD,
};

pub(super) fn authored_units() -> Vec<ConfirmationTextUnit> {
    [
        crate::test_input::read_str("assets/menu/bonus-inventory/dynamic/confirmation/prompt.json"),
        crate::test_input::read_str(
            "assets/menu/bonus-inventory/dynamic/confirmation/selected-card-prompt.json",
        ),
        crate::test_input::read_str(
            "assets/menu/bonus-inventory/dynamic/confirmation/memory-card-destination.json",
        ),
        crate::test_input::read_str(
            "assets/menu/bonus-inventory/dynamic/confirmation/memory-card-copy-prompt.json",
        ),
        crate::test_input::read_str("assets/menu/bonus-inventory/dynamic/confirmation/yes.json"),
        crate::test_input::read_str("assets/menu/bonus-inventory/dynamic/confirmation/no.json"),
    ]
    .into_iter()
    .map(|document| serde_json::from_str(document).unwrap())
    .collect()
}

pub(super) fn overlay_fixture() -> Vec<u8> {
    let mut overlay = vec![0; 0xdb00];
    overlay[0x0100] = 0x81;
    for pointer_offset in (0x11e0..0x1418).step_by(4) {
        write_u32(&mut overlay, pointer_offset, OVERLAY_RUNTIME_BASE + 0x0100);
    }
    overlay[MEMORY_CARD_DESTINATION_OFFSET
        ..MEMORY_CARD_DESTINATION_OFFSET + SOURCE_MEMORY_CARD_DESTINATION.len()]
        .copy_from_slice(&SOURCE_MEMORY_CARD_DESTINATION);
    overlay[MEMORY_CARD_COPY_PROMPT_OFFSET
        ..MEMORY_CARD_COPY_PROMPT_OFFSET + SOURCE_MEMORY_CARD_COPY_PROMPT.len()]
        .copy_from_slice(&SOURCE_MEMORY_CARD_COPY_PROMPT);
    write_u32(
        &mut overlay,
        MEMORY_CARD_DESTINATION_POINTER_OFFSET,
        OVERLAY_RUNTIME_BASE + MEMORY_CARD_DESTINATION_OFFSET as u32,
    );
    write_u32(
        &mut overlay,
        MEMORY_CARD_COPY_PROMPT_POINTER_OFFSET,
        OVERLAY_RUNTIME_BASE + MEMORY_CARD_COPY_PROMPT_OFFSET as u32,
    );
    overlay[EXIT_PROMPT_RECORD_OFFSET..EXIT_PROMPT_RECORD_OFFSET + EXIT_PROMPT_RECORD_LENGTH]
        .copy_from_slice(&SOURCE_EXIT_PROMPT_RECORD);
    overlay[ALTERNATE_PROMPT_RECORD_OFFSET
        ..ALTERNATE_PROMPT_RECORD_OFFSET + ALTERNATE_PROMPT_RECORD_LENGTH]
        .copy_from_slice(&SOURCE_ALTERNATE_PROMPT_RECORD);
    overlay[SHARED_CHOICE_RECORD_OFFSET..SHARED_CHOICE_RECORD_OFFSET + SHARED_CHOICE_RECORD_LENGTH]
        .copy_from_slice(&SOURCE_SHARED_CHOICE_RECORD);
    for (offset, instruction) in source_consumer_instructions() {
        write_instruction(&mut overlay, offset, instruction);
    }
    for (offset, instruction) in source_stock_renderer_evidence() {
        write_instruction(&mut overlay, offset, instruction);
    }
    overlay
}

pub(super) fn write_instruction(overlay: &mut [u8], offset: usize, instruction: Instruction) {
    let word = encode(&instruction, OVERLAY_RUNTIME_BASE + offset as u32).unwrap();
    overlay[offset..offset + 4].copy_from_slice(&word.to_le_bytes());
}

pub(super) fn write_u32(overlay: &mut [u8], offset: usize, value: u32) {
    overlay[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
