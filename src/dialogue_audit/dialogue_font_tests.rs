use super::atlas::parse_dialogue_atlas;
use super::atlas::{GLYPH_CELL_BYTE_COUNT, GLYPH_CELL_HEIGHT, GLYPH_CELL_WIDTH};
use super::dialogue_font::{
    install_dialogue_allocated_glyph, install_dialogue_cell_bytes, install_dialogue_extension_glyph,
};
use super::parser::{DECODED_IMAGE_SIZE, SELECTOR_TABLE_OFFSET};

#[test]
fn extension_install_packs_low_then_high_nibbles_and_changes_one_cell() {
    let mut decoded = fixture(2);
    let original = decoded.clone();
    let mut pixels = vec![0u8; GLYPH_CELL_WIDTH * GLYPH_CELL_HEIGHT];
    pixels[0] = 1;
    pixels[1] = 2;
    pixels[399] = 14;

    let install = install_dialogue_extension_glyph(&mut decoded, 2, &pixels).unwrap();

    assert_eq!(decoded[install.decoded_byte_range[0]], 0x21);
    assert_eq!(decoded[install.decoded_byte_range[1] - 1], 0xe0);
    assert_eq!(install.changed_decoded_byte_count, 2);
    assert!(original[..install.decoded_byte_range[0]] == decoded[..install.decoded_byte_range[0]]);
    assert!(original[install.decoded_byte_range[1]..] == decoded[install.decoded_byte_range[1]..]);
}

#[test]
fn extension_install_rejects_fixed_out_of_range_and_occupied_cells() {
    let mut decoded = fixture(2);
    let pixels = vec![1u8; GLYPH_CELL_WIDTH * GLYPH_CELL_HEIGHT];

    assert!(install_dialogue_extension_glyph(&mut decoded, 1, &pixels).is_err());
    let addressable_slots = (SELECTOR_TABLE_OFFSET - pixel_data_offset()) / GLYPH_CELL_BYTE_COUNT;
    assert!(
        install_dialogue_extension_glyph(&mut decoded, addressable_slots as u16, &pixels).is_err()
    );
    install_dialogue_extension_glyph(&mut decoded, 2, &pixels).unwrap();
    assert!(install_dialogue_extension_glyph(&mut decoded, 2, &pixels).is_err());
}

#[test]
fn allocated_install_rewrites_only_a_hash_bound_fixed_cell() {
    let mut decoded = fixture(2);
    decoded[pixel_data_offset()..pixel_data_offset() + GLYPH_CELL_BYTE_COUNT].fill(0x11);
    let original = decoded.clone();
    let source_hash = parse_dialogue_atlas(&decoded).unwrap().fixed_cell_sha256[0].clone();
    let pixels = vec![3u8; GLYPH_CELL_WIDTH * GLYPH_CELL_HEIGHT];

    let install =
        install_dialogue_allocated_glyph(&mut decoded, 0, &pixels, Some(&source_hash)).unwrap();

    assert_eq!(install.decoded_byte_range[0], pixel_data_offset());
    assert!(original[..install.decoded_byte_range[0]] == decoded[..install.decoded_byte_range[0]]);
    assert!(original[install.decoded_byte_range[1]..] == decoded[install.decoded_byte_range[1]..]);
    assert!(
        install_dialogue_allocated_glyph(&mut original.clone(), 0, &pixels, Some("wrong")).is_err()
    );
}

#[test]
fn shared_name_cells_admit_blank_cache_and_opaque_pack_payloads() {
    let mut decoded = fixture(1);
    let source_hash = parse_dialogue_atlas(&decoded).unwrap().fixed_cell_sha256[0].clone();
    let blank = [0_u8; GLYPH_CELL_BYTE_COUNT];
    let pack = [0xa5_u8; GLYPH_CELL_BYTE_COUNT];

    let cache = install_dialogue_cell_bytes(&mut decoded, 0, &blank, Some(&source_hash)).unwrap();
    assert!(
        decoded[cache.decoded_byte_range[0]..cache.decoded_byte_range[1]]
            .iter()
            .all(|byte| *byte == 0)
    );

    let installed_pack = install_dialogue_cell_bytes(&mut decoded, 1, &pack, None).unwrap();
    assert_eq!(
        &decoded[installed_pack.decoded_byte_range[0]..installed_pack.decoded_byte_range[1]],
        &pack
    );
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

fn pixel_data_offset() -> usize {
    let clut_size = 12 + 16 * 2;
    8 + clut_size + 12
}
