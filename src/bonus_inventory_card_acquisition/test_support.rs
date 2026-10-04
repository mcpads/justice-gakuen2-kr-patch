use psx_r3000a::{Instruction, encode};

use super::command_sequences::CARD_ACQUISITION_SEQUENCES;
use super::consumer::{
    FOLLOWING_POINTER_STORAGE_OFFSET, FOLLOWING_SEQUENCE_OFFSET, OVERLAY_RUNTIME_BASE,
    POINTER_TABLE_END, POINTER_TABLE_OFFSET, source_consumer_instructions,
};
use super::model::CardAcquisitionUnit;

pub(super) fn authored_units() -> Vec<CardAcquisitionUnit> {
    [
        crate::test_input::read_str(
            "assets/menu/bonus-inventory/dynamic/card-acquisition/rare-label.json",
        ),
        crate::test_input::read_str(
            "assets/menu/bonus-inventory/dynamic/card-acquisition/card-number-prefix.json",
        ),
        crate::test_input::read_str(
            "assets/menu/bonus-inventory/dynamic/card-acquisition/acquired-message.json",
        ),
    ]
    .into_iter()
    .map(|document| serde_json::from_str(document).unwrap())
    .collect()
}

pub(super) fn overlay_fixture() -> Vec<u8> {
    let mut overlay = vec![0; 94_704];
    overlay[0x0100] = 0x81;
    for pointer_offset in (POINTER_TABLE_OFFSET..POINTER_TABLE_END).step_by(4) {
        write_u32(&mut overlay, pointer_offset, OVERLAY_RUNTIME_BASE + 0x0100);
    }
    for spec in CARD_ACQUISITION_SEQUENCES {
        overlay[spec.sequence_offset..spec.sequence_offset + spec.storage_length]
            .copy_from_slice(spec.source_bytes);
        write_u32(
            &mut overlay,
            spec.pointer_storage_offset,
            OVERLAY_RUNTIME_BASE + spec.sequence_offset as u32,
        );
    }
    overlay[FOLLOWING_SEQUENCE_OFFSET] = 0x81;
    write_u32(
        &mut overlay,
        FOLLOWING_POINTER_STORAGE_OFFSET,
        OVERLAY_RUNTIME_BASE + FOLLOWING_SEQUENCE_OFFSET as u32,
    );
    for (offset, instruction) in source_consumer_instructions() {
        write_instruction(&mut overlay, offset, instruction);
    }
    overlay
}

pub(super) fn write_instruction(overlay: &mut [u8], offset: usize, instruction: Instruction) {
    let word = encode(&instruction, OVERLAY_RUNTIME_BASE + offset as u32).unwrap();
    write_u32(overlay, offset, word);
}

pub(super) fn write_u32(overlay: &mut [u8], offset: usize, value: u32) {
    overlay[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
