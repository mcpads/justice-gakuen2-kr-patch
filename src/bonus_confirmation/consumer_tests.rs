use std::collections::BTreeSet;

use psx_r3000a::{Instruction, Register};

use super::consumer::{
    MEMORY_CARD_DESTINATION_POINTER_OFFSET, OVERLAY_RUNTIME_BASE,
    validate_confirmation_source_consumer,
};
use super::glyph_ownership::{allocated_physical_alias_codes, validate_allocated_glyph_ownership};
use super::overlay::{patch_composed_confirmation_overlay, patch_confirmation_overlay};
use super::test_support::{authored_units, overlay_fixture, write_instruction, write_u32};

#[test]
#[ignore = "requires assets/"]
fn rejects_drift_in_the_selected_card_selector_caller() {
    let mut source = overlay_fixture();
    write_instruction(
        &mut source,
        0x9344,
        Instruction::Addiu {
            rt: Register::A2,
            rs: Register::ZERO,
            immediate: 2,
        },
    );

    let error = patch_confirmation_overlay(&source, &authored_units()).unwrap_err();

    assert!(error.to_string().contains("consumer changed at +0x9344"));
}

#[test]
#[ignore = "requires assets/"]
fn rejects_drift_in_the_memory_card_command_renderer_caller() {
    let mut source = overlay_fixture();
    write_instruction(
        &mut source,
        0xdaa0,
        Instruction::Addiu {
            rt: Register::A2,
            rs: Register::ZERO,
            immediate: 1,
        },
    );

    let error = patch_confirmation_overlay(&source, &authored_units()).unwrap_err();

    assert!(error.to_string().contains("consumer changed at +0xdaa0"));
}

#[test]
#[ignore = "requires assets/"]
fn rejects_a_memory_card_command_pointer_that_leaves_its_bound_sequence() {
    let mut source = overlay_fixture();
    write_u32(
        &mut source,
        MEMORY_CARD_DESTINATION_POINTER_OFFSET,
        OVERLAY_RUNTIME_BASE + 0x0100,
    );

    let error = patch_confirmation_overlay(&source, &authored_units()).unwrap_err();

    assert!(error.to_string().contains("command pointer changed"));
}

#[test]
fn rejects_source_command_and_direct_sprite_wrap_aliases() {
    let mut command_source = overlay_fixture();
    command_source[0x0100..0x0104].copy_from_slice(&[3, 14, 5, 0x81]);
    assert!(
        validate_allocated_glyph_ownership(&command_source)
            .unwrap_err()
            .to_string()
            .contains("source command glyph")
    );

    let mut sprite_source = overlay_fixture();
    sprite_source[0x2000..0x2003].copy_from_slice(&[11, 14, 5]);
    assert!(
        validate_allocated_glyph_ownership(&sprite_source)
            .unwrap_err()
            .to_string()
            .contains("direct-sprite glyph")
    );
}

#[test]
fn new_row_aliases_are_exact_and_exclude_neighbor_allocations() {
    let aliases = allocated_physical_alias_codes();
    let new_row_aliases = aliases
        .iter()
        .copied()
        .filter(|code| (code & 0x00f0) == 0x0050)
        .collect::<BTreeSet<_>>();

    assert_eq!(
        new_row_aliases,
        (0x0351..=0x035b).chain([0x035d, 0x035e, 0x035f]).collect()
    );
    for neighbor in [
        0x017a, 0x0320, 0x0321, 0x0322, 0x0323, 0x0324, 0x0325, 0x0341, 0x034a, 0x034d, 0x034e,
    ] {
        assert!(!aliases.contains(&neighbor));
    }
}

#[test]
#[ignore = "requires assets/"]
fn stock_selector_evidence_is_source_only_and_composed_bytes_are_preserved() {
    let mut composed = overlay_fixture();
    for (offset, instruction) in [
        (
            0x66e0,
            Instruction::Addiu {
                rt: Register::S4,
                rs: Register::ZERO,
                immediate: 4,
            },
        ),
        (
            0x66e8,
            Instruction::Addiu {
                rt: Register::S4,
                rs: Register::ZERO,
                immediate: 4,
            },
        ),
        (
            0x673c,
            Instruction::Addiu {
                rt: Register::A2,
                rs: Register::ZERO,
                immediate: 0x02c0,
            },
        ),
    ] {
        write_instruction(&mut composed, offset, instruction);
    }
    let stock_bytes = [0x66e0, 0x66e8, 0x673c].map(|offset| composed[offset..offset + 4].to_vec());

    assert!(validate_confirmation_source_consumer(&composed).is_err());
    let patched = patch_composed_confirmation_overlay(&composed, &authored_units()).unwrap();
    for (offset, expected) in [0x66e0, 0x66e8, 0x673c].into_iter().zip(stock_bytes) {
        assert_eq!(&patched.bytes[offset..offset + 4], expected);
    }
}
