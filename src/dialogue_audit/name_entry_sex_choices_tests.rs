use psx_r3000a::{Instruction, Register, encode};

use super::name_entry::OVERLAY_RUNTIME_BASE;
use super::name_entry_sex_choices::audit_sex_choice_surface;
use super::name_entry_test_fixture::name_entry_overlay;

#[test]
fn sex_choice_producer_binds_two_adjacent_twenty_pixel_cells() {
    let surface = audit_sex_choice_surface(&name_entry_overlay()).unwrap();

    assert_eq!(surface.logical_choice_count, 2);
    assert_eq!(surface.texture_page, 0x0c);
    assert_eq!(surface.source_cells[0].x, 180);
    assert_eq!(surface.source_cells[1].x, 200);
    assert!(
        surface
            .source_cells
            .iter()
            .all(|cell| { cell.y == 140 && cell.width == 20 && cell.height == 20 })
    );
}

#[test]
fn sex_choice_producer_rejects_a_changed_cell_stride() {
    let mut overlay = name_entry_overlay();
    let offset = 0x3040;
    let instruction = Instruction::Addiu {
        rt: Register::S3,
        rs: Register::S3,
        immediate: 19,
    };
    overlay[offset..offset + 4].copy_from_slice(
        &encode(&instruction, OVERLAY_RUNTIME_BASE + offset as u32)
            .unwrap()
            .to_le_bytes(),
    );

    let error = audit_sex_choice_surface(&overlay).unwrap_err();

    assert!(error.to_string().contains("sex-choice producer"));
}
