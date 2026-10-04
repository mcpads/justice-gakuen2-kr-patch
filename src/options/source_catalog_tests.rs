use super::source::OVERLAY_RUNTIME_BASE;
use super::source_catalog::{catalog_large_placement_text_offsets, catalog_newopt_pointer_records};

const POINTER_TABLE_START: usize = 0x0b90;
const POINTER_TABLE_END: usize = 0x0de0;

#[test]
fn repeated_pointer_slots_become_one_source_record_with_all_references() {
    let mut overlay = vec![0u8; POINTER_TABLE_END];
    write_record(&mut overlay, 0x036c, &[0x0010, 0x0021, 0x0026]);
    for pointer_offset in (POINTER_TABLE_START..POINTER_TABLE_END).step_by(4) {
        write_pointer(&mut overlay, pointer_offset, 0x036c);
    }

    let catalog = catalog_newopt_pointer_records(&overlay).unwrap();

    assert_eq!(catalog.pointer_slot_count, 148);
    assert_eq!(catalog.records.len(), 1);
    assert_eq!(catalog.records[0].source_codes, [0x0010, 0x0021, 0x0026]);
    assert_eq!(catalog.records[0].pointer_offsets.len(), 148);
}

#[test]
fn pointer_outside_the_source_record_arena_is_rejected() {
    let mut overlay = vec![0u8; POINTER_TABLE_END];
    for pointer_offset in (POINTER_TABLE_START..POINTER_TABLE_END).step_by(4) {
        write_pointer(&mut overlay, pointer_offset, 0x036c);
    }
    write_pointer(&mut overlay, POINTER_TABLE_START, 0x0200);

    let error = catalog_newopt_pointer_records(&overlay).unwrap_err();

    assert!(error.to_string().contains("leaves the string arena"));
}

#[test]
fn placement_scale_marks_the_three_source_bound_large_titles() {
    let mut overlay = vec![0u8; POINTER_TABLE_END];
    write_record(&mut overlay, 0x036c, &[0x0300]);
    for pointer_offset in (POINTER_TABLE_START..POINTER_TABLE_END).step_by(4) {
        write_pointer(&mut overlay, pointer_offset, 0x036c);
    }
    for (record_offset, pointer_index) in [(0x0090, 0x3e), (0x0100, 0x5a), (0x0300, 0x3f)] {
        overlay[record_offset + 2] = 0x06;
        overlay[record_offset + 3] = pointer_index;
    }
    write_pointer(&mut overlay, POINTER_TABLE_START + 0x3e * 4, 0x0778);
    write_pointer(&mut overlay, POINTER_TABLE_START + 0x3f * 4, 0x078c);
    write_pointer(&mut overlay, POINTER_TABLE_START + 0x5a * 4, 0x08b8);

    assert_eq!(
        catalog_large_placement_text_offsets(&overlay).unwrap(),
        [0x0778, 0x078c, 0x08b8]
    );
}

fn write_record(overlay: &mut [u8], offset: usize, codes: &[u16]) {
    overlay[offset..offset + 2].copy_from_slice(&(codes.len() as u16).to_le_bytes());
    for (index, code) in codes.iter().enumerate() {
        let code_offset = offset + 2 + index * 2;
        overlay[code_offset..code_offset + 2].copy_from_slice(&code.to_le_bytes());
    }
}

fn write_pointer(overlay: &mut [u8], pointer_offset: usize, source_offset: usize) {
    let pointer = OVERLAY_RUNTIME_BASE + source_offset as u32;
    overlay[pointer_offset..pointer_offset + 4].copy_from_slice(&pointer.to_le_bytes());
}
