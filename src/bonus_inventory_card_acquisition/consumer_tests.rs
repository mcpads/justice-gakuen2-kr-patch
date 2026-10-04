use std::collections::BTreeSet;

use psx_r3000a::Instruction;

use super::command_sequences::CARD_ACQUISITION_SEQUENCES;
use super::consumer::{
    CALLER_RUNTIME_ADDRESSES, CARD_NUMBER_RENDERER_RUNTIME_ADDRESS,
    FOLLOWING_POINTER_STORAGE_OFFSET, source_consumer_instructions,
    validate_card_acquisition_consumer,
};
use super::glyph_ownership::allocated_physical_alias_codes;
use super::test_support::{overlay_fixture, write_instruction, write_u32};

#[test]
fn typed_consumer_guard_covers_every_parser_caller_renderer_and_spacing_instruction() {
    let source = overlay_fixture();
    validate_card_acquisition_consumer(&source).unwrap();

    for (offset, _) in source_consumer_instructions() {
        let mut drifted = source.clone();
        write_u32(&mut drifted, offset, 0);
        assert!(
            validate_card_acquisition_consumer(&drifted).is_err(),
            "instruction drift at +0x{offset:04x} escaped"
        );
    }
    assert_eq!(
        CALLER_RUNTIME_ADDRESSES,
        [
            0x800a_f344,
            0x800a_f370,
            0x800a_f410,
            0x800a_f42c,
            0x800a_f478,
        ]
    );
}

#[test]
fn every_owned_pointer_and_the_following_boundary_are_fail_closed() {
    let source = overlay_fixture();
    for offset in CARD_ACQUISITION_SEQUENCES
        .map(|spec| spec.pointer_storage_offset)
        .into_iter()
        .chain([FOLLOWING_POINTER_STORAGE_OFFSET])
    {
        let mut drifted = source.clone();
        write_u32(&mut drifted, offset, 0);
        assert!(
            validate_card_acquisition_consumer(&drifted).is_err(),
            "pointer drift at +0x{offset:04x} escaped"
        );
    }
}

#[test]
fn card_number_renderer_caller_set_rejects_an_extra_call() {
    let mut source = overlay_fixture();
    write_instruction(
        &mut source,
        0xd000,
        Instruction::Jal {
            target: CARD_NUMBER_RENDERER_RUNTIME_ADDRESS,
        },
    );

    assert!(validate_card_acquisition_consumer(&source).is_err());
}

#[test]
fn allocation_declares_wrapped_aliases_and_rejects_source_consumers() {
    assert_eq!(
        allocated_physical_alias_codes(),
        (0x0370..=0x037b)
            .chain(0x037d..=0x037f)
            .collect::<BTreeSet<_>>()
    );

    let mut pointer_collision = overlay_fixture();
    pointer_collision[0x0100..0x0104].copy_from_slice(&[0x03, 0x00, 0x07, 0x81]);
    assert!(validate_card_acquisition_consumer(&pointer_collision).is_err());

    let mut direct_collision = overlay_fixture();
    direct_collision[0x1418..0x141b].copy_from_slice(&[0x0b, 0x00, 0x07]);
    assert!(validate_card_acquisition_consumer(&direct_collision).is_err());
}
