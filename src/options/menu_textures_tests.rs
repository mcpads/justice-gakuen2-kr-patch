use crate::tim::parse_4bpp_prefix;

use super::menu_textures::{describe_texture_for_test, detect_four_bit_tim_regions};

fn synthetic_tim() -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&0x10u32.to_le_bytes());
    data.extend_from_slice(&0x08u32.to_le_bytes());
    data.extend_from_slice(&44u32.to_le_bytes());
    data.extend_from_slice(&960u16.to_le_bytes());
    data.extend_from_slice(&480u16.to_le_bytes());
    data.extend_from_slice(&16u16.to_le_bytes());
    data.extend_from_slice(&1u16.to_le_bytes());
    data.extend_from_slice(&[0u8; 32]);
    data.extend_from_slice(&140u32.to_le_bytes());
    data.extend_from_slice(&768u16.to_le_bytes());
    data.extend_from_slice(&256u16.to_le_bytes());
    data.extend_from_slice(&4u16.to_le_bytes());
    data.extend_from_slice(&16u16.to_le_bytes());
    data.extend_from_slice(&[0u8; 128]);
    data
}

#[test]
fn reports_tim_vram_coordinates_without_converting_word_x_to_pixels() {
    let data = synthetic_tim();
    let tim = parse_4bpp_prefix(&data).unwrap();

    let region = describe_texture_for_test("synthetic", 0x20800, tim);

    assert_eq!(region.decoded_offset, "0x20800");
    assert_eq!(region.image_vram_word_x, 768);
    assert_eq!(region.image_vram_y, 256);
    assert_eq!(region.image_pixel_width, 16);
    assert_eq!(region.image_height, 16);
}

#[test]
fn detects_aligned_clut_and_clutless_tim_headers() {
    let first = synthetic_tim();
    let mut menu = first.clone();
    menu.extend_from_slice(&[0u8; 12]);
    let second_offset = menu.len();
    menu.extend_from_slice(&0x10u32.to_le_bytes());
    menu.extend_from_slice(&0u32.to_le_bytes());
    menu.extend_from_slice(&20u32.to_le_bytes());
    menu.extend_from_slice(&832u16.to_le_bytes());
    menu.extend_from_slice(&256u16.to_le_bytes());
    menu.extend_from_slice(&2u16.to_le_bytes());
    menu.extend_from_slice(&2u16.to_le_bytes());
    menu.extend_from_slice(&[0u8; 8]);

    let regions = detect_four_bit_tim_regions(&menu);
    assert_eq!(regions.len(), 2);
    assert_eq!(regions[0].decoded_offset, "0x00000");
    assert!(regions[0].has_clut);
    assert_eq!(regions[1].decoded_offset, format!("0x{second_offset:05x}"));
    assert!(!regions[1].has_clut);
    assert_eq!(regions[1].image_vram_word_x, 832);
    assert_eq!(regions[1].image_vram_y, 256);
}
