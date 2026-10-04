use super::audit::compare_runtime_overlay;
use crate::embedded_tim::detect_embedded_tim_images;

fn synthetic_4bpp_tim() -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&0x10u32.to_le_bytes());
    data.extend_from_slice(&0x08u32.to_le_bytes());
    data.extend_from_slice(&44u32.to_le_bytes());
    data.extend_from_slice(&0u16.to_le_bytes());
    data.extend_from_slice(&0u16.to_le_bytes());
    data.extend_from_slice(&16u16.to_le_bytes());
    data.extend_from_slice(&1u16.to_le_bytes());
    data.extend_from_slice(&[0u8; 32]);
    data.extend_from_slice(&140u32.to_le_bytes());
    data.extend_from_slice(&0u16.to_le_bytes());
    data.extend_from_slice(&0u16.to_le_bytes());
    data.extend_from_slice(&4u16.to_le_bytes());
    data.extend_from_slice(&16u16.to_le_bytes());
    data.extend_from_slice(&[0u8; 128]);
    data
}

#[test]
fn tim_discovery_reports_exact_embedded_boundary_and_geometry() {
    let mut decoded = vec![0x55; 13];
    decoded.extend_from_slice(&synthetic_4bpp_tim());
    decoded.extend_from_slice(&[0xaa; 7]);

    let tims = detect_embedded_tim_images(&decoded);

    assert_eq!(tims.len(), 1);
    assert_eq!(tims[0].offset, 13);
    assert_eq!(tims[0].total_size, synthetic_4bpp_tim().len());
    assert_eq!(tims[0].bits_per_pixel, 4);
    assert_eq!((tims[0].pixel_width, tims[0].pixel_height), (16, 16));
}

#[test]
fn malformed_tim_marker_is_not_promoted_to_a_texture() {
    let mut decoded = 0x10u32.to_le_bytes().to_vec();
    decoded.extend_from_slice(&0x08u32.to_le_bytes());
    decoded.extend_from_slice(&[0; 12]);

    assert!(detect_embedded_tim_images(&decoded).is_empty());
}

#[test]
fn runtime_overlay_comparison_reports_exact_residency() {
    let source = [0x12, 0x34, 0x56, 0x78];
    let mut ram = vec![0; 2 * 1024 * 1024];
    ram[0x0a_2000..0x0a_2004].copy_from_slice(&source);

    let comparison = compare_runtime_overlay(&source, &ram).unwrap();

    assert!(comparison.source_matches_resident);
    assert_eq!(comparison.matching_byte_count, source.len());
    assert_eq!(comparison.matching_prefix_length, source.len());
}

#[test]
fn runtime_overlay_comparison_keeps_prefix_and_total_match_counts_distinct() {
    let source = [0x12, 0x34, 0x56, 0x78];
    let mut ram = vec![0; 2 * 1024 * 1024];
    ram[0x0a_2000..0x0a_2004].copy_from_slice(&[0x12, 0xff, 0x56, 0x78]);

    let comparison = compare_runtime_overlay(&source, &ram).unwrap();

    assert!(!comparison.source_matches_resident);
    assert_eq!(comparison.matching_byte_count, 3);
    assert_eq!(comparison.matching_prefix_length, 1);
}
