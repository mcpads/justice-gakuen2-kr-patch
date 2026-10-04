use std::path::Path;

use super::build::{
    BONUS_J_BANK_RETURN_LABEL_BUILD_MANIFEST_FILE, BONUS_J_BANK_RETURN_LABEL_OVERLAY_OUTPUT_FILE,
    build_bonus_j_bank_return_label,
};
use super::model::{BonusJBankReturnLabelBuildConfig, BonusJBankReturnLabelFontSource};

#[test]
#[ignore = "requires the user-supplied supported source disc"]
fn supported_source_builds_the_authored_label_with_its_independent_font_role() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let output_dir = std::env::temp_dir().join(format!(
        "justice-bonus-j-bank-return-label-source-build-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&output_dir);
    let build = build_bonus_j_bank_return_label(&BonusJBankReturnLabelBuildConfig {
        cue: root.join("roms/Shiritsu Justice Gakuen - Nekketsu Seishun Nikki 2 (Japan).cue"),
        assets: root.join("assets/menu/bonus-inventory/dynamic/j-bank-return-label"),
        font: BonusJBankReturnLabelFontSource {
            path: root.join("../fonts/NEXON_Maplestory/TTF/Maplestory Bold.ttf"),
            font_px: 17.0,
            tracking_px: 0.0,
            vertical_shift_px: -1,
        },
        build_spec_sha256: zero_sha256(),
        output_dir: output_dir.clone(),
        force: true,
    })
    .unwrap();

    assert_eq!(
        build.report.glyphs.len(),
        build.report.unit.korean_text.chars().count()
    );
    assert_eq!(build.report.font_role, "bonus_j_bank_return_label");
    assert_eq!(build.report.font_px, 17.0);
    assert_eq!(build.report.tracking_px, 0.0);
    assert_eq!(build.report.vertical_shift_px, -1);
    assert_eq!(
        build.report.unit.output_payload_byte_length,
        build.report.unit.output_command_count * 3 + 1
    );
    assert!(
        build.report.unit.output_payload_byte_length <= build.report.unit.source_storage_length
    );
    assert!(build.report.source_command_record_matches);
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
            .physical_alias_source_cells_match_blank_hash
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
            .overlay_changes_confined_to_fixed_command_record
    );
    assert!(build.report.development_input_available);
    assert!(!build.report.release_candidate_input_eligible);
    assert!(build.report.runtime_verification_required);
    assert!(
        output_dir
            .join(BONUS_J_BANK_RETURN_LABEL_OVERLAY_OUTPUT_FILE)
            .is_file()
    );
    assert!(
        output_dir
            .join(BONUS_J_BANK_RETURN_LABEL_BUILD_MANIFEST_FILE)
            .is_file()
    );
    std::fs::remove_dir_all(output_dir).unwrap();
}

fn zero_sha256() -> String {
    "0000000000000000000000000000000000000000000000000000000000000000".to_string()
}
