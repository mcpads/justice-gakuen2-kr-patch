use std::path::Path;

use crate::bonus_confirmation::{
    BonusConfirmationBuildConfig, BonusConfirmationFontSource, build_bonus_confirmation_from_source,
};
use crate::source_disc::SupportedSourceDisc;

use super::build::build_bonus_inventory_card_acquisition_from_source;
use super::model::{
    BonusInventoryCardAcquisitionBuildConfig, BonusInventoryCardAcquisitionFontSource,
};
use super::source::load_source;

#[test]
#[ignore = "requires the user-supplied supported source disc"]
fn card_writer_chain_preserves_applied_confirmation_overlay_and_glyph_ranges() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cue = root.join("roms/Shiritsu Justice Gakuen - Nekketsu Seishun Nikki 2 (Japan).cue");
    let output_root = std::env::temp_dir().join(format!(
        "justice-bonus-card-acquisition-writer-chain-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&output_root);
    let source_disc = SupportedSourceDisc::open(&cue).unwrap();

    let confirmation = build_bonus_confirmation_from_source(
        &BonusConfirmationBuildConfig {
            cue: cue.clone(),
            assets: root.join("assets/menu/bonus-inventory/dynamic/confirmation"),
            font: BonusConfirmationFontSource {
                path: root.join("../fonts/NEXON_Maplestory/TTF/Maplestory Bold.ttf"),
                font_px: 15.0,
                tracking_px: 0.0,
                vertical_shift_px: -1,
            },
            build_spec_sha256: zero_sha256(),
            output_dir: output_root.join("confirmation"),
            force: true,
        },
        &source_disc,
    )
    .unwrap();
    let card = build_bonus_inventory_card_acquisition_from_source(
        &BonusInventoryCardAcquisitionBuildConfig {
            cue: cue.clone(),
            assets: root.join("assets/menu/bonus-inventory/dynamic/card-acquisition"),
            font: BonusInventoryCardAcquisitionFontSource {
                path: root.join("../fonts/NEXON_Maplestory/TTF/Maplestory Bold.ttf"),
                font_px: 17.0,
                tracking_px: 0.0,
                vertical_shift_px: -1,
            },
            build_spec_sha256: zero_sha256(),
            output_dir: output_root.join("card-acquisition"),
            force: true,
        },
        &source_disc,
    )
    .unwrap();

    let source = load_source(&source_disc).unwrap();
    let mut overlay = source.overlay;
    let confirmation_overlay_ranges = confirmation.apply_to_overlay(&mut overlay).unwrap();
    let confirmation_overlay_bytes = capture_ranges(&overlay, &confirmation_overlay_ranges);
    let card_overlay_ranges = card.apply_to_overlay(&mut overlay).unwrap();
    assert_disjoint(&confirmation_overlay_ranges, &card_overlay_ranges);
    assert_ranges_unchanged(&overlay, &confirmation_overlay_bytes);

    let mut inventory = source.inventory_decoded;
    let confirmation_glyph_ranges = confirmation
        .apply_to_inventory_decoded(&mut inventory)
        .unwrap();
    let confirmation_glyph_bytes = capture_ranges(&inventory, &confirmation_glyph_ranges);
    let card_glyph_ranges = card.apply_to_inventory_decoded(&mut inventory).unwrap();
    assert_disjoint(&confirmation_glyph_ranges, &card_glyph_ranges);
    assert_ranges_unchanged(&inventory, &confirmation_glyph_bytes);

    std::fs::remove_dir_all(output_root).unwrap();
}

fn zero_sha256() -> String {
    "0000000000000000000000000000000000000000000000000000000000000000".to_string()
}

fn capture_ranges(bytes: &[u8], ranges: &[[usize; 2]]) -> Vec<([usize; 2], Vec<u8>)> {
    ranges
        .iter()
        .map(|range| (*range, bytes[range[0]..range[1]].to_vec()))
        .collect()
}

fn assert_ranges_unchanged(bytes: &[u8], captured: &[([usize; 2], Vec<u8>)]) {
    for (range, expected) in captured {
        assert_eq!(&bytes[range[0]..range[1]], expected);
    }
}

fn assert_disjoint(left: &[[usize; 2]], right: &[[usize; 2]]) {
    assert!(left.iter().all(|left| {
        right
            .iter()
            .all(|right| left[1] <= right[0] || right[1] <= left[0])
    }));
}
