use std::path::Path;

use crate::development_build_spec::load_development_build_spec;
use crate::source_disc::SupportedSourceDisc;
use crate::tim::write_indexed_cell_in_prefix;

use super::build::build_bonus_confirmation_from_source;
use super::glyph_slots::{FIXED_RECORD_SPACE_CELL, SOURCE_BLANK_GLYPH_INDEXED_SHA256};
use super::model::BonusConfirmationBuildConfig;
use super::source::{GLYPH_TIM_OFFSET, load_source};

#[test]
#[ignore = "requires the user-supplied supported source disc"]
fn supported_source_builds_all_confirmation_surfaces_and_protects_the_blank_cell() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cue = root.join("roms/Shiritsu Justice Gakuen - Nekketsu Seishun Nikki 2 (Japan).cue");
    let spec = load_development_build_spec(&root.join("assets/build/development.json")).unwrap();
    let output_dir = std::env::temp_dir().join(format!(
        "justice-bonus-confirmation-source-build-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&output_dir);
    let source_disc = SupportedSourceDisc::open(&cue).unwrap();

    let build = build_bonus_confirmation_from_source(
        &BonusConfirmationBuildConfig {
            cue: cue.clone(),
            assets: spec.assets.bonus_confirmation,
            font: spec.fonts.bonus_confirmation,
            build_spec_sha256: spec.sha256,
            output_dir: output_dir.clone(),
            force: true,
        },
        &source_disc,
    )
    .unwrap();

    assert_eq!(build.report.glyphs.len(), 18);
    assert_eq!(build.report.units.len(), 6);
    assert!(build.report.selected_card_prompt_translated);
    assert!(build.report.memory_card_destination_translated);
    assert!(build.report.memory_card_copy_prompt_translated);
    assert!(build.report.fixed_record_space.remains_blank);
    assert!(
        !build
            .report
            .fixed_record_space
            .included_in_expected_write_ranges
    );
    assert_eq!(
        build.report.fixed_record_space.output_indexed_pixel_sha256,
        SOURCE_BLANK_GLYPH_INDEXED_SHA256
    );
    assert!(build.report.glyphs.iter().all(|glyph| {
        let [left, top, right, bottom] = glyph.ink_bounds;
        left < right && right <= 20 && top < bottom && bottom <= 20
    }));

    let source = load_source(&source_disc).unwrap();
    let mut contaminated = source.inventory_decoded;
    write_indexed_cell_in_prefix(
        &mut contaminated,
        GLYPH_TIM_OFFSET,
        FIXED_RECORD_SPACE_CELL,
        &vec![1; FIXED_RECORD_SPACE_CELL.width * FIXED_RECORD_SPACE_CELL.height],
    )
    .unwrap();
    let error = build
        .apply_to_inventory_decoded(&mut contaminated)
        .unwrap_err();
    assert!(error.to_string().contains("space cell is no longer blank"));

    std::fs::remove_dir_all(output_dir).unwrap();
}
