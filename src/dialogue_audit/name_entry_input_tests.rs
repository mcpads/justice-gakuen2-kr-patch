use super::name_entry::analyze_name_entry_overlay;
use super::name_entry_input::SELECTED_CODE_STORE_OFFSET;
use super::name_entry_test_fixture::name_entry_overlay;

#[test]
fn source_input_path_exposes_three_pages_and_a_sixteen_bit_record_store() {
    let report = analyze_name_entry_overlay(
        &name_entry_overlay(),
        "source-bin".to_string(),
        "overlay".to_string(),
    )
    .unwrap();

    assert_eq!(report.input.page_cycle_input_mask, "0x0010");
    assert_eq!(report.input.physical_page_count, 3);
    assert!(report.input.nickname_skips_first_source_page);
    assert_eq!(report.input.page_index_object_offset, 9);
    assert_eq!(report.input.record_slot_index_object_offset, 10);
    assert_eq!(report.input.cursor_cell_object_offset, 12);
    assert_eq!(report.input.field_index_object_offset, 15);
    assert_eq!(report.input.selected_code_width_bytes, 2);
    assert_eq!(report.input.family_name_record_offset, 0x12);
    assert_eq!(report.input.given_name_record_offset, 0x22);
    assert_eq!(report.input.nickname_record_offset, 0x32);
    assert_eq!(report.input.navigation_position_count, 97);
    assert_eq!(report.input.navigation_candidate_position_count, 90);
    assert_eq!(report.input.navigation_action_position_count, 7);
    assert_eq!(report.input.navigation_handler_count, 11);
    assert!(report.input.navigation_uses_computed_handler_dispatch);
    assert_eq!(report.input.navigation_producer_file_offset, "0x8828");
    assert_eq!(report.input.navigation_source_map_count, 5);
    assert_eq!(report.input.navigation_source_map_sha256.len(), 5);
}

#[test]
fn selected_code_store_must_remain_a_halfword_write() {
    let mut overlay = name_entry_overlay();
    overlay[SELECTED_CODE_STORE_OFFSET..SELECTED_CODE_STORE_OFFSET + 4]
        .copy_from_slice(&0u32.to_le_bytes());

    let error =
        analyze_name_entry_overlay(&overlay, "source-bin".to_string(), "overlay".to_string())
            .unwrap_err();

    assert!(error.to_string().contains("selected-code store"));
}
