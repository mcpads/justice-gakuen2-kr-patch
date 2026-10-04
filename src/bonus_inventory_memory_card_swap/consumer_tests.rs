use std::collections::BTreeSet;

use psx_r3000a::{Instruction, Register};

use super::command_sequences::MEMORY_CARD_SWAP_SEQUENCES;
use super::consumer::{
    CALLER_RUNTIME_ADDRESSES, COMMAND_RENDERER_RUNTIME_ADDRESS, RENDERER_COMMAND_ADVANCE_PX,
    RENDERER_LINE_COMMAND_LIMIT, RENDERER_LINE_WIDTH_PX, source_consumer_instructions,
    validate_memory_card_swap_consumer,
};
use super::glyph_ownership::allocated_physical_alias_codes;
use super::test_support::{overlay_fixture, write_instruction, write_u32};

#[test]
fn typed_guard_covers_renderer_grammar_geometry_and_both_pointer_consumers() {
    let source = overlay_fixture();
    validate_memory_card_swap_consumer(&source).unwrap();
    for (offset, _) in source_consumer_instructions() {
        let mut drifted = source.clone();
        write_u32(&mut drifted, offset, 0);
        assert!(
            validate_memory_card_swap_consumer(&drifted).is_err(),
            "instruction drift at +0x{offset:04x} escaped"
        );
    }
    assert_eq!(CALLER_RUNTIME_ADDRESSES, [0x800a_f97c, 0x800a_fdac]);
    assert_eq!(COMMAND_RENDERER_RUNTIME_ADDRESS, 0x800a_b5ac);
    assert_eq!(RENDERER_LINE_WIDTH_PX, 512);
    assert_eq!(RENDERER_COMMAND_ADVANCE_PX, 20);
    assert_eq!(RENDERER_LINE_COMMAND_LIMIT, 25);
}

#[test]
fn every_owned_pointer_and_pointer_load_site_is_fail_closed() {
    let source = overlay_fixture();
    for offset in MEMORY_CARD_SWAP_SEQUENCES.map(|spec| spec.pointer_storage_offset) {
        let mut drifted = source.clone();
        write_u32(&mut drifted, offset, 0);
        assert!(validate_memory_card_swap_consumer(&drifted).is_err());
    }

    let mut extra_load = source;
    write_instruction(
        &mut extra_load,
        0xe000,
        Instruction::Lw {
            rt: Register::A1,
            base: Register::A1,
            offset: 0x3410,
        },
    );
    assert!(validate_memory_card_swap_consumer(&extra_load).is_err());
}

#[test]
fn allocation_declares_every_wrapped_alias_and_rejects_source_consumers() {
    assert_eq!(
        allocated_physical_alias_codes(),
        (0x0380..=0x038b)
            .chain(0x038d..=0x038f)
            .chain(0x0390..=0x0397)
            .chain(0x039d..=0x039f)
            .collect::<BTreeSet<_>>()
    );

    let mut pointer_collision = overlay_fixture();
    pointer_collision[0x0100..0x0104].copy_from_slice(&[0x03, 0x00, 0x08, 0x81]);
    assert!(validate_memory_card_swap_consumer(&pointer_collision).is_err());

    let mut direct_collision = overlay_fixture();
    direct_collision[0x1418..0x141b].copy_from_slice(&[0x0b, 0x00, 0x08]);
    assert!(validate_memory_card_swap_consumer(&direct_collision).is_err());
}
