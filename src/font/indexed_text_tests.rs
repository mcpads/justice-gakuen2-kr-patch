use super::IndexedTextRasterizer;
use super::indexed_text::horizontal_origin;
use super::model::HorizontalTextAlignment;
use super::rasterize_indexed_text;
use super::rasterize_indexed_text_with_coverage_ramp;
use super::rasterize_shifted_indexed_text_with_coverage_ramp;
use std::path::Path;

#[test]
#[ignore = "requires fonts in ../fonts/"]
fn left_aligned_location_preserves_a_negative_glyph_bearing() {
    let font = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../fonts/NEXON_Lv2_Gothic/TTF/NEXON Lv2 Gothic Bold.ttf");
    let raster = rasterize_indexed_text(
        &font,
        "체육관",
        118,
        24,
        18.0,
        0.0,
        0,
        Some(1),
        2,
        HorizontalTextAlignment::Left,
    )
    .unwrap();
    assert!(raster.ink_bounds[2] <= 118);
    assert!(raster.ink_bounds[3] <= 24);
    assert!(raster.pixels.contains(&2));
}

#[test]
fn horizontal_origin_matches_the_owned_text_rectangle() {
    assert_eq!(
        horizontal_origin(100, 40.0, HorizontalTextAlignment::Left),
        0.0
    );
    assert_eq!(
        horizontal_origin(100, 40.0, HorizontalTextAlignment::Center),
        30.0
    );
    assert_eq!(
        horizontal_origin(100, 40.0, HorizontalTextAlignment::Right),
        60.0
    );
}

#[test]
#[ignore = "requires fonts in ../fonts/"]
fn indexed_text_coverage_ramp_preserves_intermediate_palette_indices() {
    let font = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../fonts/NEXON_Maplestory/TTF/Maplestory Bold.ttf");
    let raster = rasterize_indexed_text_with_coverage_ramp(
        &font,
        "공격력",
        48,
        24,
        14.0,
        0.0,
        0,
        2,
        15,
        HorizontalTextAlignment::Center,
    )
    .unwrap();

    assert!(raster.pixels.contains(&0));
    assert!(raster.pixels.iter().any(|index| (2..15).contains(index)));
    assert!(raster.pixels.iter().all(|index| *index == 0 || *index >= 2));
}

#[test]
#[ignore = "requires fonts in ../fonts/"]
fn indexed_text_accepts_full_8bpp_palette_indices() {
    let font = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../fonts/NEXON_Maplestory/TTF/Maplestory Bold.ttf");
    let raster = rasterize_indexed_text(
        &font,
        "교문 앞",
        78,
        24,
        18.0,
        0.0,
        0,
        Some(1),
        255,
        HorizontalTextAlignment::Left,
    )
    .unwrap();

    assert!(raster.pixels.contains(&255));
    assert!(raster.pixels.contains(&1));
    assert!(raster.pixels.contains(&0));
}

#[test]
#[ignore = "requires fonts in ../fonts/"]
fn cached_rasterizer_matches_the_one_shot_outline_renderer() {
    let font = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../fonts/NEXON_Maplestory/TTF/Maplestory Bold.ttf");
    let one_shot = rasterize_indexed_text(
        &font,
        "협",
        40,
        40,
        30.0,
        0.0,
        0,
        Some(3),
        15,
        HorizontalTextAlignment::Center,
    )
    .unwrap();
    let cached = IndexedTextRasterizer::load(&font)
        .unwrap()
        .rasterize(
            "협",
            40,
            40,
            30.0,
            0.0,
            0,
            Some(3),
            15,
            HorizontalTextAlignment::Center,
        )
        .unwrap();

    assert_eq!(cached.pixels, one_shot.pixels);
    assert_eq!(cached.ink_bounds, one_shot.ink_bounds);
}

#[test]
#[ignore = "requires fonts in ../fonts/"]
fn vertical_shift_is_applied_before_the_owned_rectangle_is_checked() {
    let font = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../fonts/NEXON_Maplestory/TTF/Maplestory Bold.ttf");
    let unshifted = rasterize_indexed_text_with_coverage_ramp(
        &font,
        "동",
        20,
        20,
        17.0,
        0.0,
        0,
        1,
        15,
        HorizontalTextAlignment::Center,
    );
    assert!(unshifted.is_err());

    let shifted = rasterize_shifted_indexed_text_with_coverage_ramp(
        &font,
        "동",
        20,
        20,
        17.0,
        0.0,
        -1,
        0,
        1,
        15,
        HorizontalTextAlignment::Center,
    )
    .unwrap();

    assert_eq!(shifted.pixels.len(), 20 * 20);
    assert!(shifted.ink_bounds[3] <= 20);
}
