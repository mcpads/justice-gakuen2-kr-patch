use std::path::Path;

use super::build::{
    BONUS_INVENTORY_CARD_ACQUISITION_BUILD_MANIFEST_FILE,
    BONUS_INVENTORY_CARD_ACQUISITION_OVERLAY_OUTPUT_FILE, build_bonus_inventory_card_acquisition,
};
use super::model::{
    BonusInventoryCardAcquisitionBuildConfig, BonusInventoryCardAcquisitionFontSource,
};

#[test]
#[ignore = "requires the user-supplied supported source disc"]
fn supported_source_builds_three_card_acquisition_units_with_component_font_config() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let output_dir = std::env::temp_dir().join(format!(
        "justice-bonus-card-acquisition-source-build-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&output_dir);

    let build = build_bonus_inventory_card_acquisition(&BonusInventoryCardAcquisitionBuildConfig {
        cue: root.join("roms/Shiritsu Justice Gakuen - Nekketsu Seishun Nikki 2 (Japan).cue"),
        assets: root.join("assets/menu/bonus-inventory/dynamic/card-acquisition"),
        font: BonusInventoryCardAcquisitionFontSource {
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

    assert_eq!(build.report.glyphs.len(), 12);
    assert_eq!(build.report.reused_source_glyphs.len(), 3);
    assert_eq!(build.report.units.len(), 3);
    assert_eq!(build.report.font_role, "bonus_card_acquisition");
    assert_eq!(build.report.font_px, 17.0);
    assert_eq!(build.report.tracking_px, 0.0);
    assert_eq!(build.report.vertical_shift_px, -1);
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
            .pointer_command_table_range,
        [0x11e0, 0x1418]
    );
    assert!(
        build
            .report
            .glyph_ownership_evidence
            .declared_direct_selector_byte_region_scan_disjoint
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
    assert!(build.report.glyphs.iter().all(|glyph| {
        let [left, top, right, bottom] = glyph.ink_bounds;
        left < right && right <= 20 && top < bottom && bottom <= 20
    }));
    assert!(
        output_dir
            .join(BONUS_INVENTORY_CARD_ACQUISITION_OVERLAY_OUTPUT_FILE)
            .is_file()
    );
    assert!(
        output_dir
            .join(BONUS_INVENTORY_CARD_ACQUISITION_BUILD_MANIFEST_FILE)
            .is_file()
    );

    std::fs::remove_dir_all(output_dir).unwrap();
}
