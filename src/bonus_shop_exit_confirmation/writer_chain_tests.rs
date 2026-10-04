use std::path::Path;

use crate::development_build_spec::SizedFontSource;
use crate::pipeline::difference_ranges;
use crate::shop_ui::{ShopUiBuildConfig, ShopUiFontSources, build_shop_ui};

use super::build::build_bonus_shop_exit_confirmation;
use super::model::{BonusShopExitConfirmationBuildConfig, BonusShopExitConfirmationFontSource};
use super::source::load_source;

#[test]
#[ignore = "requires the user-supplied supported source disc and font"]
fn exit_confirmation_writer_preserves_actual_prior_fixed_shop_ui_writes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cue = root.join("roms/Shiritsu Justice Gakuen - Nekketsu Seishun Nikki 2 (Japan).cue");
    let font_path = root.join("../fonts/NEXON_Maplestory/TTF/Maplestory Bold.ttf");
    let output_root = std::env::temp_dir().join(format!(
        "justice-shop-exit-confirmation-writer-chain-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&output_root);

    let prior = build_shop_ui(&ShopUiBuildConfig {
        cue: cue.clone(),
        assets: root.join("assets/menu/shop-ui"),
        fonts: ShopUiFontSources {
            heading: SizedFontSource {
                path: font_path.clone(),
                font_px: 22.0,
            },
            current_points: SizedFontSource {
                path: font_path.clone(),
                font_px: 17.0,
            },
            heading_tracking_px: 0.0,
            current_points_tracking_px: 0.0,
        },
        build_spec_sha256: zero_sha256(),
        output_dir: output_root.join("fixed-ui"),
        force: true,
    })
    .unwrap();
    let current = build_bonus_shop_exit_confirmation(&BonusShopExitConfirmationBuildConfig {
        cue: cue.clone(),
        assets: root.join("assets/menu/shop-ui/dynamic/exit-confirmation"),
        font: BonusShopExitConfirmationFontSource {
            path: font_path,
            font_px: 15.0,
            tracking_px: 0.0,
            vertical_shift_px: -1,
        },
        build_spec_sha256: zero_sha256(),
        output_dir: output_root.join("exit-confirmation"),
        force: true,
    })
    .unwrap();

    let source = load_source(&cue).unwrap();
    let prior_ranges = difference_ranges(&source.shop_ui_decoded, &prior.decoded);
    let prior_bytes = capture_ranges(&prior.decoded, &prior_ranges);
    let mut composed_shop_ui = prior.decoded;
    let current_ranges = current
        .apply_to_shop_ui_decoded(&mut composed_shop_ui)
        .unwrap();
    assert_disjoint(&prior_ranges, &current_ranges);
    assert_ranges_unchanged(&composed_shop_ui, &prior_bytes);

    let mut overlay = source.overlay;
    current.apply_to_overlay(&mut overlay).unwrap();
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
