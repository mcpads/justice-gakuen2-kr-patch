use super::name_entry::analyze_name_entry_overlay;
use super::name_entry_renderer::REDISPLAY_ROUTINE_OFFSET;
use super::name_entry_test_fixture::name_entry_overlay;

#[test]
fn source_renderer_exposes_all_persistent_name_slots_and_font_upload_source() {
    let report = analyze_name_entry_overlay(
        &name_entry_overlay(),
        "source-bin".to_string(),
        "overlay".to_string(),
    )
    .unwrap();

    assert_eq!(report.renderer.redisplay_routine_file_offset, "0x7454");
    assert_eq!(
        report.renderer.code_lookup_table_runtime_address,
        "0x8017ad14"
    );
    assert_eq!(
        report.renderer.layout_triplet_table_runtime_address,
        "0x8017aee4"
    );
    assert_eq!(report.renderer.sprite_cell_width, 20);
    assert_eq!(report.renderer.sprite_cell_height, 20);
    assert_eq!(
        report
            .renderer
            .fields
            .iter()
            .map(|field| field.visible_slot_count)
            .collect::<Vec<_>>(),
        [6, 6, 4]
    );
    assert_eq!(
        report.renderer.name_font_source_buffer_runtime_address,
        "0x800e9000"
    );
    assert!(!report.renderer.tagged_hangul_consumer_installed);
}

#[test]
fn redisplay_consumer_change_fails_closed() {
    let mut overlay = name_entry_overlay();
    overlay[REDISPLAY_ROUTINE_OFFSET..REDISPLAY_ROUTINE_OFFSET + 4]
        .copy_from_slice(&0u32.to_le_bytes());

    let error =
        analyze_name_entry_overlay(&overlay, "source-bin".to_string(), "overlay".to_string())
            .unwrap_err();

    assert!(error.to_string().contains("redisplay consumer"));
}
