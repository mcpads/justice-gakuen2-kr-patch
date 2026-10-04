use super::atlas::{
    GLYPH_CELL_BYTE_COUNT, GLYPH_CELL_HEIGHT, GLYPH_CELL_WIDTH, parse_dialogue_atlas,
};
use super::parser::{DECODED_IMAGE_SIZE, SELECTOR_TABLE_OFFSET};

#[test]
fn separates_fixed_tim_cells_from_source_extension_slots() {
    let decoded = fixture(2);
    let atlas = parse_dialogue_atlas(&decoded).unwrap();

    assert_eq!(atlas.fixed_cell_count, 2);
    assert_eq!(atlas.tim_size, atlas.source_extension_start);
    assert_eq!(atlas.source_extension_end, SELECTOR_TABLE_OFFSET);
    assert_eq!(atlas.fixed_cell_sha256.len(), 2);
    assert_eq!(atlas.source_extension_nonzero_byte_count, 0);
    assert!(atlas.addressable_slot_count > atlas.fixed_cell_count);
}

#[test]
fn reports_source_bytes_in_the_runtime_extension_region() {
    let mut decoded = fixture(2);
    let atlas = parse_dialogue_atlas(&decoded).unwrap();
    decoded[atlas.source_extension_start + 7] = 1;

    let atlas = parse_dialogue_atlas(&decoded).unwrap();
    assert_eq!(atlas.source_extension_nonzero_byte_count, 1);
}

fn fixture(cell_count: usize) -> Vec<u8> {
    let mut decoded = vec![0u8; DECODED_IMAGE_SIZE];
    let clut_size = 12 + 16 * 2;
    let image_size = 12 + cell_count * GLYPH_CELL_BYTE_COUNT;
    decoded[0..4].copy_from_slice(&0x10u32.to_le_bytes());
    decoded[4..8].copy_from_slice(&0x08u32.to_le_bytes());
    decoded[8..12].copy_from_slice(&(clut_size as u32).to_le_bytes());
    decoded[16..18].copy_from_slice(&16u16.to_le_bytes());
    decoded[18..20].copy_from_slice(&1u16.to_le_bytes());

    let image_offset = 8 + clut_size;
    decoded[image_offset..image_offset + 4].copy_from_slice(&(image_size as u32).to_le_bytes());
    decoded[image_offset + 8..image_offset + 10]
        .copy_from_slice(&((GLYPH_CELL_WIDTH / 4) as u16).to_le_bytes());
    decoded[image_offset + 10..image_offset + 12]
        .copy_from_slice(&((cell_count * GLYPH_CELL_HEIGHT) as u16).to_le_bytes());
    decoded
}
