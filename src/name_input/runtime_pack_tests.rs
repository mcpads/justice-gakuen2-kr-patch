use super::{NameGlyphPackReport, plan_name_input_runtime_pack};

fn report() -> NameGlyphPackReport {
    NameGlyphPackReport {
        kind: "test".to_string(),
        font_name: "test".to_string(),
        font_sha256: "00".repeat(32),
        font_px: 13.0,
        repertoire: "KS X 1001 Hangul".to_string(),
        supported_syllable_count: 2_350,
        unsupported_modern_syllable_count: 8_822,
        crop: [4, 6, 12, 13],
        occupied_coordinate_count: 140,
        bytes_per_component_mask: 18,
        no_final_base_component_count: 349,
        final_bearing_base_component_count: 328,
        final_component_count: 249,
        component_count: 926,
        no_final_rank_prefix_entry_bytes: 2,
        final_rank_prefix_entry_bytes: 1,
        no_final_rank_prefix_byte_range: [1612, 1712],
        final_bearing_rank_prefix_byte_range: [1712, 1812],
        final_rank_prefix_byte_range: [1812, 1883],
        runtime_coordinate_list_byte_count: 140,
        component_mask_byte_range: [1883, 18551],
        pack_bytes: 18551,
        storage_capacity_bytes: 19000,
        storage_bytes_remaining: 449,
        fits_storage: true,
        exact_reference_glyph_count: 484,
        missing_fill_pixel_count: 4710,
        unexpected_fill_pixel_count: 7290,
        synthesized_blank_count: 0,
        runtime_consumer_installed: false,
    }
}

#[test]
fn runtime_pack_layout_derives_every_decoder_section() {
    let layout = plan_name_input_runtime_pack(&report()).unwrap();

    assert_eq!(layout.coordinate_membership_byte_range, [24, 44]);
    assert_eq!(layout.repertoire_membership_byte_range, [44, 1441]);
    assert_eq!(layout.no_final_membership_byte_range, [1441, 1491]);
    assert_eq!(layout.final_bearing_membership_byte_range, [1491, 1541]);
    assert_eq!(layout.final_membership_byte_range, [1541, 1612]);
    assert_eq!(layout.runtime_coordinate_list_byte_count, 140);
    assert_eq!(layout.component_mask_byte_range, [1883, 18551]);
}

#[test]
fn runtime_pack_layout_rejects_a_shifted_section() {
    let mut report = report();
    report.component_mask_byte_range[0] += 1;

    let error = plan_name_input_runtime_pack(&report).unwrap_err();
    assert!(error.to_string().contains("not contiguous"));
}
