use std::path::Path;

use crate::pipeline::{BASELINE_BIN_SHA256, sha256_bytes};
use crate::tim::read_indexed_cell_in_prefix;

use super::assets::load_assets;
use super::command_sequence::RECORD_SPECS;
use super::consumer::{OVERLAY_RUNTIME_BASE, validate_exit_confirmation_consumer};
use super::glyph_ownership::allocated_physical_alias_codes;
use super::glyph_slots::{
    QUESTION_MARK_CODE, QUESTION_MARK_SOURCE_INDEXED_SHA256, SOURCE_BLANK_GLYPH_INDEXED_SHA256,
    glyph_cell,
};
use super::source::{GLYPH_TIM_OFFSET, load_source};

#[test]
#[ignore = "requires the user-supplied supported source disc"]
fn supported_source_matches_record_pointer_caller_renderer_and_atlas_oracles() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cue = root.join("roms/Shiritsu Justice Gakuen - Nekketsu Seishun Nikki 2 (Japan).cue");
    let source = load_source(&cue).unwrap();
    assert_eq!(source.source_bin_sha256, BASELINE_BIN_SHA256);

    for spec in RECORD_SPECS {
        let record =
            &source.overlay[spec.record_offset..spec.record_offset + spec.source_record.len()];
        assert_eq!(record, spec.source_record);
        assert_eq!(sha256_bytes(record), spec.source_record_sha256);
        assert_eq!(
            spec.terminator_offset + 1,
            spec.record_offset + spec.source_record.len()
        );
        assert_eq!(record.last(), Some(&0x81));
        assert_eq!(
            u32::from_le_bytes(
                source.overlay[spec.pointer_storage_offset..spec.pointer_storage_offset + 4]
                    .try_into()
                    .unwrap()
            ),
            OVERLAY_RUNTIME_BASE + spec.record_offset as u32
        );
    }

    let consumer = validate_exit_confirmation_consumer(&source.overlay).unwrap();
    assert!(consumer.declared_physical_alias_set_matches);
    assert_eq!(consumer.pointer_command_record_count, 186);
    assert_eq!(consumer.pointer_command_unique_glyph_count, 217);
    assert!(consumer.pointer_command_table_parsed_glyphs_disjoint);
    assert!(consumer.declared_direct_selector_byte_region_scan_disjoint);

    let question = read_indexed_cell_in_prefix(
        &source.shop_ui_decoded,
        GLYPH_TIM_OFFSET,
        glyph_cell(QUESTION_MARK_CODE),
    )
    .unwrap();
    assert_eq!(sha256_bytes(&question), QUESTION_MARK_SOURCE_INDEXED_SHA256);
    for alias in allocated_physical_alias_codes() {
        let pixels = read_indexed_cell_in_prefix(
            &source.shop_ui_decoded,
            GLYPH_TIM_OFFSET,
            glyph_cell(alias),
        )
        .unwrap();
        assert_eq!(sha256_bytes(&pixels), SOURCE_BLANK_GLYPH_INDEXED_SHA256);
    }

    let assets = load_assets(
        &root.join("assets/menu/shop-ui/dynamic/exit-confirmation"),
        &source,
    )
    .unwrap();
    assert!(assets.physical_alias_source_cells_match_blank_hash);
}
