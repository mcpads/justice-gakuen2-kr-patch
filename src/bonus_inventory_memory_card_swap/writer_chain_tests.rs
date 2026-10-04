use std::path::Path;

use crate::bonus_inventory_card_acquisition::{
    BonusInventoryCardAcquisitionBuildConfig, BonusInventoryCardAcquisitionFontSource,
    build_bonus_inventory_card_acquisition_from_source,
};
use crate::source_disc::SupportedSourceDisc;

use super::build::build_bonus_inventory_memory_card_swap_from_source;
use super::model::{
    BonusInventoryMemoryCardSwapBuildConfig, BonusInventoryMemoryCardSwapFontSource,
};
use super::source::load_source;

#[test]
#[ignore = "requires the user-supplied supported source disc"]
fn memory_card_swap_writer_preserves_actual_prior_card_acquisition_writes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cue = root.join("roms/Shiritsu Justice Gakuen - Nekketsu Seishun Nikki 2 (Japan).cue");
    let font_path = root.join("../fonts/NEXON_Maplestory/TTF/Maplestory Bold.ttf");
    let output_root = std::env::temp_dir().join(format!(
        "justice-bonus-memory-card-swap-writer-chain-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&output_root);
    let source_disc = SupportedSourceDisc::open(&cue).unwrap();

    let prior = build_bonus_inventory_card_acquisition_from_source(
        &BonusInventoryCardAcquisitionBuildConfig {
            cue: cue.clone(),
            assets: root.join("assets/menu/bonus-inventory/dynamic/card-acquisition"),
            font: BonusInventoryCardAcquisitionFontSource {
                path: font_path.clone(),
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
    let current = build_bonus_inventory_memory_card_swap_from_source(
        &BonusInventoryMemoryCardSwapBuildConfig {
            cue: cue.clone(),
            assets: root.join("assets/menu/bonus-inventory/dynamic/memory-card-swap"),
            font: BonusInventoryMemoryCardSwapFontSource {
                path: font_path,
                font_px: 17.0,
                tracking_px: 0.0,
                vertical_shift_px: -1,
            },
            build_spec_sha256: zero_sha256(),
            output_dir: output_root.join("memory-card-swap"),
            force: true,
        },
        &source_disc,
    )
    .unwrap();

    let source = load_source(&source_disc).unwrap();
    let mut overlay = source.overlay;
    let prior_overlay_ranges = prior.apply_to_overlay(&mut overlay).unwrap();
    let prior_overlay_bytes = capture_ranges(&overlay, &prior_overlay_ranges);
    let current_overlay_ranges = current.apply_to_overlay(&mut overlay).unwrap();
    assert_disjoint(&prior_overlay_ranges, &current_overlay_ranges);
    assert_ranges_unchanged(&overlay, &prior_overlay_bytes);

    let mut inventory = source.inventory_decoded;
    let prior_glyph_ranges = prior.apply_to_inventory_decoded(&mut inventory).unwrap();
    let prior_glyph_bytes = capture_ranges(&inventory, &prior_glyph_ranges);
    let current_glyph_ranges = current.apply_to_inventory_decoded(&mut inventory).unwrap();
    assert_disjoint(&prior_glyph_ranges, &current_glyph_ranges);
    assert_ranges_unchanged(&inventory, &prior_glyph_bytes);
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
