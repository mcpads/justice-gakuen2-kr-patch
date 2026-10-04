use std::path::Path;

use crate::pipeline::sha256_file;

use super::build::{
    BONUS_SHOP_EXIT_CONFIRMATION_OVERLAY_OUTPUT_FILE, build_bonus_shop_exit_confirmation,
};
use super::command_sequence::encode_exit_confirmation;
use super::model::{BonusShopExitConfirmationBuildConfig, BonusShopExitConfirmationFontSource};
use super::test_support::{authored_units, glyph_allocations};

#[test]
#[ignore = "requires the user-supplied supported source disc and font"]
fn source_gated_build_restyles_question_mark_without_changing_its_code() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let output = std::env::temp_dir().join(format!(
        "justice-shop-exit-confirmation-source-build-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&output);
    let build = build_bonus_shop_exit_confirmation(&BonusShopExitConfirmationBuildConfig {
        cue: root.join("roms/Shiritsu Justice Gakuen - Nekketsu Seishun Nikki 2 (Japan).cue"),
        assets: root.join("assets/menu/shop-ui/dynamic/exit-confirmation"),
        font: BonusShopExitConfirmationFontSource {
            path: root.join("../fonts/NEXON_Maplestory/TTF/Maplestory Bold.ttf"),
            font_px: 15.0,
            tracking_px: 0.0,
            vertical_shift_px: -1,
        },
        build_spec_sha256: zero_sha256(),
        output_dir: output.clone(),
        force: true,
    })
    .unwrap();

    let independently_reencoded =
        encode_exit_confirmation(&authored_units(), &glyph_allocations()).unwrap();
    assert_eq!(build.report.records.len(), 2);
    for (report, encoded) in build
        .report
        .records
        .iter()
        .zip(&independently_reencoded.records)
    {
        assert_eq!(report.variant_id, encoded.variant_id);
        assert_eq!(report.output_record_byte_length, encoded.bytes.len());
        assert_eq!(
            report.output_translation_command_counts,
            encoded.unit_command_counts
        );
        assert_eq!(report.output_line_count, typed_line_count(&encoded.bytes));
    }
    assert_eq!(build.report.reused_source_glyphs.len(), 1);
    assert!(build.report.reused_source_glyphs[0].source_indexed_pixels_match_declared_hash);
    let question = build
        .report
        .glyphs
        .iter()
        .find(|glyph| glyph.text == "?")
        .unwrap();
    assert_eq!(question.code, "0x0055");
    assert_ne!(
        question.source_indexed_pixel_sha256,
        question.output_indexed_pixel_sha256
    );
    assert!(question.changed_decoded_byte_count > 0);
    assert!(build.report.source_command_records_match);
    assert!(
        build
            .report
            .shop_ui_changes_confined_to_allocated_glyph_cells
    );
    assert!(build.report.overlay_changes_confined_to_fixed_record);
    assert!(
        build
            .report
            .glyph_ownership_evidence
            .physical_alias_source_cells_match_blank_hash
    );
    assert!(build.report.development_input_available);
    assert!(!build.report.release_candidate_input_eligible);
    assert_eq!(
        sha256_file(&output.join(BONUS_SHOP_EXIT_CONFIRMATION_OVERLAY_OUTPUT_FILE)).unwrap(),
        build.report.output_overlay_sha256
    );
    std::fs::remove_dir_all(output).unwrap();
}

fn typed_line_count(bytes: &[u8]) -> usize {
    let mut cursor = 0;
    let mut lines = 1;
    loop {
        match bytes[cursor] {
            0x81 => return lines,
            0x80 => {
                lines += 1;
                cursor += 1;
            }
            _ => cursor += 3,
        }
    }
}

fn zero_sha256() -> String {
    "0000000000000000000000000000000000000000000000000000000000000000".to_string()
}
