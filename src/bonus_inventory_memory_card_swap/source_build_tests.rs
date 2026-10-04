use std::path::Path;

use super::build::{
    BONUS_INVENTORY_MEMORY_CARD_SWAP_BUILD_MANIFEST_FILE,
    BONUS_INVENTORY_MEMORY_CARD_SWAP_OVERLAY_OUTPUT_FILE, build_bonus_inventory_memory_card_swap,
};
use super::model::{
    BonusInventoryMemoryCardSwapBuildConfig, BonusInventoryMemoryCardSwapFontSource,
};

#[test]
#[ignore = "requires the user-supplied supported source disc"]
fn supported_source_builds_two_memory_card_swap_units_with_independent_font_config() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let output_dir = std::env::temp_dir().join(format!(
        "justice-bonus-memory-card-swap-source-build-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&output_dir);
    let build = build_bonus_inventory_memory_card_swap(&BonusInventoryMemoryCardSwapBuildConfig {
        cue: root.join("roms/Shiritsu Justice Gakuen - Nekketsu Seishun Nikki 2 (Japan).cue"),
        assets: root.join("assets/menu/bonus-inventory/dynamic/memory-card-swap"),
        font: BonusInventoryMemoryCardSwapFontSource {
            path: root.join("../fonts/NEXON_Maplestory/TTF/Maplestory Bold.ttf"),
            font_px: 17.0,
            tracking_px: 0.0,
            vertical_shift_px: -1,
        },
        build_spec_sha256: "0000000000000000000000000000000000000000000000000000000000000000"
            .to_string(),
        output_dir: output_dir.clone(),
        force: true,
    })
    .unwrap();

    assert_eq!(build.report.glyphs.len(), 20);
    assert_eq!(build.report.reused_source_glyphs.len(), 0);
    assert_eq!(build.report.units.len(), 2);
    assert_eq!(build.report.font_role, "bonus_memory_card_swap");
    assert_eq!(build.report.font_px, 17.0);
    assert_eq!(build.report.tracking_px, 0.0);
    assert_eq!(build.report.vertical_shift_px, -1);
    assert_eq!(build.report.renderer_line_width_px, 512);
    assert_eq!(build.report.renderer_command_advance_px, 20);
    assert_eq!(build.report.renderer_line_command_limit, 25);
    for unit in &build.report.units {
        let expected_line_command_counts = unit
            .korean_text
            .split('\n')
            .map(|line| line.chars().count())
            .collect::<Vec<_>>();
        assert_eq!(expected_line_command_counts.len(), 2);
        assert_eq!(
            unit.output_line_command_counts,
            expected_line_command_counts
        );
        let expected_payload_byte_length = unit
            .korean_text
            .chars()
            .map(|character| if character == '\n' { 1 } else { 3 })
            .sum::<usize>()
            + 1;
        assert_eq!(
            unit.output_payload_byte_length,
            expected_payload_byte_length
        );
        assert!(unit.output_payload_byte_length <= unit.source_storage_length);
        assert!(
            unit.output_line_command_counts
                .iter()
                .all(|count| *count <= build.report.renderer_line_command_limit)
        );
    }
    assert!(build.report.source_command_records_match);
    assert!(
        build
            .report
            .glyph_ownership_evidence
            .declared_physical_alias_set_matches
    );
    assert!(
        build
            .report
            .glyph_ownership_evidence
            .existing_bonus_component_allocations_disjoint
    );
    assert_eq!(
        build
            .report
            .glyph_ownership_evidence
            .pointer_command_table_range,
        [0x11e0, 0x1418]
    );
    assert!(
        build
            .report
            .glyph_ownership_evidence
            .pointer_command_table_parsed_glyphs_disjoint
    );
    assert_eq!(
        build
            .report
            .glyph_ownership_evidence
            .declared_direct_selector_byte_region,
        [0x1418, 0x3afc]
    );
    assert!(
        build
            .report
            .glyph_ownership_evidence
            .declared_direct_selector_byte_region_scan_disjoint
    );
    assert!(
        build
            .report
            .inventory_changes_confined_to_allocated_glyph_cells
    );
    assert!(
        build
            .report
            .overlay_changes_confined_to_fixed_command_records
    );
    assert!(
        build
            .report
            .reused_source_glyphs
            .iter()
            .all(|glyph| glyph.source_indexed_pixels_match_declared_hash)
    );
    assert!(build.report.development_input_available);
    assert!(!build.report.release_candidate_input_eligible);
    assert!(build.report.runtime_verification_required);
    assert!(
        output_dir
            .join(BONUS_INVENTORY_MEMORY_CARD_SWAP_OVERLAY_OUTPUT_FILE)
            .is_file()
    );
    assert!(
        output_dir
            .join(BONUS_INVENTORY_MEMORY_CARD_SWAP_BUILD_MANIFEST_FILE)
            .is_file()
    );
    std::fs::remove_dir_all(output_dir).unwrap();
}
