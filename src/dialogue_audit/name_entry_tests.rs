use super::name_entry::{PAGE_POINTER_TABLE_OFFSET, analyze_name_entry_overlay};
use super::name_entry_test_fixture::name_entry_overlay;

#[test]
fn visible_name_pages_define_development_candidate_capacity() {
    let overlay = name_entry_overlay();

    let report =
        analyze_name_entry_overlay(&overlay, "source-bin".to_string(), "overlay".to_string())
            .unwrap();

    assert_eq!(report.source_selectable_cell_count, 228);
    assert_eq!(report.source_unique_selectable_code_count, 228);
    assert_eq!(report.pages[0].selectable_cell_count, 84);
    assert_eq!(report.pages[1].selectable_cell_count, 82);
    assert_eq!(report.pages[2].selectable_cell_count, 62);
    assert_eq!(
        report
            .fields
            .iter()
            .map(|field| field.visible_glyph_capacity)
            .collect::<Vec<_>>(),
        [6, 6, 4]
    );
}

#[test]
fn page_pointer_must_address_its_visible_source_cells() {
    let mut overlay = name_entry_overlay();
    overlay[PAGE_POINTER_TABLE_OFFSET..PAGE_POINTER_TABLE_OFFSET + 4]
        .copy_from_slice(&0x8017_b000u32.to_le_bytes());

    let error =
        analyze_name_entry_overlay(&overlay, "source-bin".to_string(), "overlay".to_string())
            .unwrap_err();

    assert!(error.to_string().contains("hiragana page pointer"));
}
