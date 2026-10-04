use std::path::Path;

use crate::development_build_spec::SizedFontSource;
use crate::font::IndexedTextRasterizer;
use crate::tim::Cell;

use super::model::BonusMenuFontRole;
use super::raster::{TRANSPARENT_INDEX, rasterize_unit_text};

fn maplestory_bold(font_px: f32) -> SizedFontSource {
    SizedFontSource {
        path: Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../fonts/NEXON_Maplestory/TTF/Maplestory Bold.ttf"),
        font_px,
    }
}

#[test]
#[ignore = "requires fonts in ../fonts/"]
fn heading_is_centered_while_entries_remain_left_aligned() {
    let font = maplestory_bold(44.0);
    let rasterizer = IndexedTextRasterizer::load(&font.path).unwrap();
    let heading = rasterize_unit_text(
        &rasterizer,
        &font,
        BonusMenuFontRole::Heading,
        Cell {
            x: 160,
            y: 48,
            width: 192,
            height: 96,
        },
        "보너스",
    )
    .unwrap();
    let font = maplestory_bold(20.0);
    let entry = rasterize_unit_text(
        &rasterizer,
        &font,
        BonusMenuFontRole::Entry,
        Cell {
            x: 0,
            y: 0,
            width: 256,
            height: 32,
        },
        "구매부로 가기",
    )
    .unwrap();

    let heading_left_padding = heading.ink_bounds[0];
    let heading_right_padding = 192 - heading.ink_bounds[2];
    assert!(heading_left_padding.abs_diff(heading_right_padding) <= 1);
    assert_eq!(entry.ink_bounds[0], 0);
}

#[test]
#[ignore = "requires fonts in ../fonts/"]
fn each_heading_pixel_belongs_to_exactly_one_glyph_span() {
    let font = maplestory_bold(44.0);
    let rasterizer = IndexedTextRasterizer::load(&font.path).unwrap();
    let heading = rasterize_unit_text(
        &rasterizer,
        &font,
        BonusMenuFontRole::Heading,
        Cell {
            x: 160,
            y: 48,
            width: 192,
            height: 96,
        },
        "보너스",
    )
    .unwrap();

    assert_eq!(heading.glyph_ink_spans.len(), "보너스".chars().count());
    for (offset, pixel) in heading.pixels.iter().enumerate() {
        if *pixel == TRANSPARENT_INDEX {
            continue;
        }
        let x = offset % 192;
        let matches = heading
            .glyph_ink_spans
            .iter()
            .filter(|[left, right]| *left <= x && x < *right)
            .count();
        assert_eq!(
            matches, 1,
            "x={x} spans={:?} bounds={:?}",
            heading.glyph_ink_spans, heading.ink_bounds
        );
    }
}
