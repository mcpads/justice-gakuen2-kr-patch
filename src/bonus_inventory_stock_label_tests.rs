use psx_r3000a::{Instruction, Register, decode, encode};

use crate::bonus_confirmation::bonus_confirmation_reserved_glyph_codes;

use super::consumer::{
    CATEGORY_JUMP_TABLE_OFFSET, CATEGORY_STATE_TARGETS, CHANGED_INSTRUCTION_OFFSETS,
    OVERLAY_RUNTIME_BASE, RENDERER_CALL_OFFSETS, SELECTOR_INSTRUCTION_OFFSETS,
    TEXTURE_PAGE_INSTRUCTION_OFFSET, category_dispatch_instructions, renderer_source_instructions,
};
use super::overlay::patch_stock_label_overlay;
use super::ownership::{
    allocated_physical_alias_codes, cells_overlap, validate_allocated_glyph_ownership,
};
use super::source::{STOCK_LABEL_GLYPHS, glyph_cell};

#[test]
fn stock_label_patch_selects_page_three_glyphs_without_changing_the_loop_discriminator() {
    let source = overlay_fixture();

    let patched = patch_stock_label_overlay(&source).unwrap();

    assert_eq!(
        decode_instruction(&patched.bytes, 0x66d8),
        Instruction::Addiu {
            rt: Register::S1,
            rs: Register::ZERO,
            immediate: 1,
        }
    );
    assert_eq!(
        decode_instruction(&patched.bytes, 0x66e0),
        Instruction::Addiu {
            rt: Register::S4,
            rs: Register::ZERO,
            immediate: 4,
        }
    );
    assert_eq!(
        decode_instruction(&patched.bytes, 0x66e4),
        Instruction::Addiu {
            rt: Register::S1,
            rs: Register::ZERO,
            immediate: 10,
        }
    );
    assert_eq!(
        decode_instruction(&patched.bytes, 0x66e8),
        Instruction::Addiu {
            rt: Register::S4,
            rs: Register::ZERO,
            immediate: 4,
        }
    );
    assert_eq!(
        decode_instruction(&patched.bytes, TEXTURE_PAGE_INSTRUCTION_OFFSET),
        Instruction::Addiu {
            rt: Register::A2,
            rs: Register::ZERO,
            immediate: 0x02c0,
        }
    );
    assert_eq!(
        patched.expected_write_ranges,
        CHANGED_INSTRUCTION_OFFSETS.map(|offset| [offset, offset + 4])
    );
    assert!(patched.changed_byte_ranges.iter().all(|[start, end]| {
        patched
            .expected_write_ranges
            .iter()
            .any(|[allowed_start, allowed_end]| allowed_start <= start && end <= allowed_end)
    }));
}

#[test]
fn rejects_using_the_first_column_selector_as_a_new_glyph_code() {
    let mut source = overlay_fixture();
    write_instruction(
        &mut source,
        SELECTOR_INSTRUCTION_OFFSETS[0],
        Instruction::Addiu {
            rt: Register::S1,
            rs: Register::ZERO,
            immediate: 5,
        },
    );

    let error = patch_stock_label_overlay(&source).unwrap_err();

    assert!(error.to_string().contains("renderer changed at +0x66d8"));
}

#[test]
fn states_zero_through_five_share_five_calls_and_state_six_skips_the_renderer() {
    let source = overlay_fixture();

    patch_stock_label_overlay(&source).unwrap();

    assert_eq!(CATEGORY_STATE_TARGETS.len(), 7);
    assert_eq!(RENDERER_CALL_OFFSETS.len(), 5);
    assert_eq!(CATEGORY_STATE_TARGETS[6], OVERLAY_RUNTIME_BASE + 0x78bc);
}

#[test]
fn rejects_an_exit_state_redirected_to_a_rendering_state() {
    let mut source = overlay_fixture();
    write_u32(
        &mut source,
        CATEGORY_JUMP_TABLE_OFFSET + 6 * 4,
        CATEGORY_STATE_TARGETS[0],
    );

    let error = patch_stock_label_overlay(&source).unwrap_err();

    assert!(error.to_string().contains("state table changed"));
}

#[test]
fn rejects_a_missing_shared_renderer_call() {
    let mut source = overlay_fixture();
    write_instruction(&mut source, RENDERER_CALL_OFFSETS[2], Instruction::nop());

    let error = patch_stock_label_overlay(&source).unwrap_err();

    assert!(
        error
            .to_string()
            .contains("bonus-category dispatch changed")
    );
}

#[test]
fn stock_label_cells_have_only_the_audited_wrapped_aliases() {
    let aliases = allocated_physical_alias_codes();

    assert_eq!(
        aliases.into_iter().collect::<Vec<_>>(),
        [0x0341, 0x034a, 0x034d, 0x034e]
    );
    assert_eq!(STOCK_LABEL_GLYPHS[0].2.x, 788);
    assert_eq!(STOCK_LABEL_GLYPHS[1].2.x, 968);
    assert!(
        STOCK_LABEL_GLYPHS
            .iter()
            .all(|(_, _, cell)| { cell.y == 80 && cell.width == 20 && cell.height == 20 })
    );
}

#[test]
fn stock_label_cells_are_disjoint_from_every_confirmation_reserved_cell() {
    let stock_cells = STOCK_LABEL_GLYPHS.map(|(_, _, cell)| cell);
    let confirmation_cells = bonus_confirmation_reserved_glyph_codes()
        .map(glyph_cell)
        .collect::<Vec<_>>();

    assert!(confirmation_cells.iter().all(|confirmation_cell| {
        stock_cells
            .iter()
            .all(|stock_cell| !cells_overlap(*stock_cell, *confirmation_cell))
    }));
}

#[test]
fn rejects_a_source_command_that_uses_a_wrapped_stock_label_alias() {
    let mut source = overlay_fixture();
    source[0x0100..0x0104].copy_from_slice(&[3, 13, 4, 0x81]);

    let error = validate_allocated_glyph_ownership(&source).unwrap_err();

    assert!(error.to_string().contains("aliases a source command glyph"));
}

#[test]
fn stock_overlay_composition_preserves_action_page_and_confirmation_component_bytes() {
    let mut source = overlay_fixture();
    source[0x07e8..0x07ec].copy_from_slice(&[0x5a, 0xa5, 0x5a, 0xa5]);
    source[0x86dc..0x86e0].copy_from_slice(&[0xa5, 0x5a, 0xa5, 0x5a]);
    source[0x8d70..0x8d74].copy_from_slice(&[0x33, 0xcc, 0x33, 0xcc]);

    let patched = patch_stock_label_overlay(&source).unwrap();

    assert_eq!(&patched.bytes[0x07e8..0x07ec], &[0x5a, 0xa5, 0x5a, 0xa5]);
    assert_eq!(&patched.bytes[0x86dc..0x86e0], &[0xa5, 0x5a, 0xa5, 0x5a]);
    assert_eq!(&patched.bytes[0x8d70..0x8d74], &[0x33, 0xcc, 0x33, 0xcc]);
}

fn overlay_fixture() -> Vec<u8> {
    let mut overlay = vec![0; 0x17200];
    overlay[0x0100] = 0x81;
    for pointer_offset in (0x11e0..0x1418).step_by(4) {
        write_u32(&mut overlay, pointer_offset, OVERLAY_RUNTIME_BASE + 0x0100);
    }
    for (offset, instruction) in renderer_source_instructions()
        .into_iter()
        .chain(category_dispatch_instructions())
    {
        write_instruction(&mut overlay, offset, instruction);
    }
    for (index, target) in CATEGORY_STATE_TARGETS.into_iter().enumerate() {
        write_u32(&mut overlay, CATEGORY_JUMP_TABLE_OFFSET + index * 4, target);
    }
    overlay
}

fn write_instruction(overlay: &mut [u8], offset: usize, instruction: Instruction) {
    let word = encode(&instruction, OVERLAY_RUNTIME_BASE + offset as u32).unwrap();
    overlay[offset..offset + 4].copy_from_slice(&word.to_le_bytes());
}

fn decode_instruction(overlay: &[u8], offset: usize) -> Instruction {
    let word = u32::from_le_bytes(overlay[offset..offset + 4].try_into().unwrap());
    decode(word, OVERLAY_RUNTIME_BASE + offset as u32).unwrap()
}

fn write_u32(overlay: &mut [u8], offset: usize, value: u32) {
    overlay[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
