use super::{detect_embedded_tim_images, parse_embedded_tim_at};

fn bounded_four_bit_tim() -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&0x10u32.to_le_bytes());
    bytes.extend_from_slice(&0x08u32.to_le_bytes());
    bytes.extend_from_slice(&44u32.to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&[0; 32]);
    bytes.extend_from_slice(&14u32.to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&[0; 2]);
    bytes
}

#[test]
fn exact_offset_parser_ignores_earlier_false_magic() {
    let nested_tim = bounded_four_bit_tim();
    let tim_offset = 24;
    let mut bytes = vec![0; tim_offset + nested_tim.len()];
    bytes[..4].copy_from_slice(&0x10u32.to_le_bytes());
    bytes[8..12].copy_from_slice(&82u32.to_le_bytes());
    bytes[16..18].copy_from_slice(&35u16.to_le_bytes());
    bytes[18..20].copy_from_slice(&1u16.to_le_bytes());
    bytes[tim_offset..].copy_from_slice(&nested_tim);

    assert!(
        detect_embedded_tim_images(&bytes)
            .iter()
            .all(|candidate| candidate.offset != tim_offset),
        "the heuristic scanner must reproduce the false candidate that skips the declared TIM"
    );

    let tim = parse_embedded_tim_at(&bytes, tim_offset).unwrap();

    assert_eq!(tim.offset, tim_offset);
    assert_eq!(
        (tim.bits_per_pixel, tim.pixel_width, tim.pixel_height),
        (4, 4, 1)
    );
}

#[test]
fn skips_false_magic_and_finds_a_bounded_four_bit_tim() {
    let mut bytes = vec![0x55; 9];
    bytes.extend_from_slice(&0x10u32.to_le_bytes());
    bytes.extend_from_slice(&0x08u32.to_le_bytes());
    bytes.extend_from_slice(&44u32.to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&[0; 32]);
    bytes.extend_from_slice(&14u32.to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&[0; 2]);

    let tims = detect_embedded_tim_images(&bytes);
    assert_eq!(tims.len(), 1);
    assert_eq!(tims[0].offset, 9);
    assert_eq!(tims[0].bits_per_pixel, 4);
    assert_eq!(tims[0].pixel_width, 4);
    assert_eq!(tims[0].pixel_height, 1);
}

#[test]
fn finds_a_clut_free_runtime_texture_page() {
    let mut bytes = vec![0x55; 7];
    bytes.extend_from_slice(&0x10u32.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&20u32.to_le_bytes());
    bytes.extend_from_slice(&704u16.to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&4u16.to_le_bytes());
    bytes.extend_from_slice(&[0x10; 8]);

    let tims = detect_embedded_tim_images(&bytes);
    assert_eq!(tims.len(), 1);
    assert_eq!(tims[0].offset, 7);
    assert_eq!(tims[0].image_vram_word_x, 704);
    assert_eq!(tims[0].image_vram_y, 0);
    assert_eq!(tims[0].palette_count, 0);
}
