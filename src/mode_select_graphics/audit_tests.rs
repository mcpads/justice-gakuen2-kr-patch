use super::inventory::{classify_tim, detect_embedded_tims};
use super::model::ModeSelectTimRole;

#[test]
fn detects_supported_tims_without_accepting_invalid_magic() {
    let mut decoded = vec![0xaa; 11];
    decoded.extend_from_slice(&tim_4bpp_without_clut(8, 4));
    decoded.extend_from_slice(&[0x10, 0, 0, 0, 0xff, 0xff, 0xff, 0xff]);
    decoded.extend_from_slice(&tim_16bpp(3, 2));

    let tims = detect_embedded_tims(&decoded);

    assert_eq!(tims.len(), 2);
    assert_eq!(tims[0].offset, 11);
    assert_eq!(tims[0].bits_per_pixel, 4);
    assert!(!tims[0].has_clut);
    assert_eq!(tims[0].pixel_width, 8);
    assert_eq!(tims[0].pixel_height, 4);
    assert_eq!(tims[0].role, ModeSelectTimRole::Unclassified);
    assert_eq!(tims[1].bits_per_pixel, 16);
    assert_eq!(tims[1].pixel_width, 3);
    assert_eq!(tims[1].pixel_height, 2);
}

#[test]
fn binds_artwork_and_preview_groups_through_the_panel_permutation() {
    assert_eq!(
        classify_tim(0x6e800),
        ModeSelectTimRole::ModeArtworkPanel {
            panel_index: 0,
            mode_index: 1,
        }
    );
    assert_eq!(
        classify_tim(0x83800),
        ModeSelectTimRole::ModePreviewPanel {
            panel_index: 0,
            mode_index: 1,
        }
    );
    assert_eq!(
        classify_tim(0x9d800),
        ModeSelectTimRole::ModePreviewPanel {
            panel_index: 13,
            mode_index: 7,
        }
    );
}

fn tim_4bpp_without_clut(width: u16, height: u16) -> Vec<u8> {
    assert_eq!(width % 4, 0);
    let word_width = width / 4;
    let image_size = 12 + usize::from(word_width) * usize::from(height) * 2;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&0x10u32.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&u32::try_from(image_size).unwrap().to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&word_width.to_le_bytes());
    bytes.extend_from_slice(&height.to_le_bytes());
    bytes.resize(8 + image_size, 0);
    bytes
}

fn tim_16bpp(width: u16, height: u16) -> Vec<u8> {
    let image_size = 12 + usize::from(width) * usize::from(height) * 2;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&0x10u32.to_le_bytes());
    bytes.extend_from_slice(&2u32.to_le_bytes());
    bytes.extend_from_slice(&u32::try_from(image_size).unwrap().to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&width.to_le_bytes());
    bytes.extend_from_slice(&height.to_le_bytes());
    bytes.resize(8 + image_size, 0);
    bytes
}
