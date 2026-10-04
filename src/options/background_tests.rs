use std::path::{Path, PathBuf};

use crate::font::IndexedTextRasterizer;
use crate::tim::Cell;

use super::background_build::render_unit;
use super::background_model::OptionsBackgroundLayout;

fn maplestory_bold() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../fonts/NEXON_Maplestory/TTF/Maplestory Bold.ttf")
}

#[test]
#[ignore = "requires fonts in ../fonts/"]
fn vertical_school_name_renders_each_original_english_letter_in_its_slot() {
    let cell = Cell {
        x: 0,
        y: 0,
        width: 16,
        height: 96,
    };

    let font = maplestory_bold();
    let rasterizer = IndexedTextRasterizer::load(&font).unwrap();
    let (pixels, glyphs) = render_unit(
        &rasterizer,
        14.0,
        "MINAMI",
        cell,
        OptionsBackgroundLayout::VerticalGlyphs,
        13,
        1,
        5,
    )
    .unwrap();

    assert_eq!(glyphs.len(), 6);
    assert_eq!(pixels.len(), cell.width * cell.height);
    for slot in pixels.chunks_exact(cell.width * 16) {
        assert!(slot.contains(&5));
        assert!(slot.iter().all(|pixel| matches!(pixel, 1 | 5 | 13)));
    }
}

#[test]
#[ignore = "requires fonts in ../fonts/"]
fn crest_mark_uses_its_own_centered_surface_geometry() {
    let cell = Cell {
        x: 84,
        y: 28,
        width: 34,
        height: 42,
    };

    let font = maplestory_bold();
    let rasterizer = IndexedTextRasterizer::load(&font).unwrap();
    let (pixels, glyphs) = render_unit(
        &rasterizer,
        30.0,
        "남",
        cell,
        OptionsBackgroundLayout::Centered,
        15,
        5,
        8,
    )
    .unwrap();

    assert_eq!(glyphs.len(), 1);
    assert_eq!(glyphs[0].ink_bounds, [3, 6, 31, 35]);
    assert!(pixels.contains(&8));
    assert!(pixels.iter().all(|pixel| matches!(pixel, 5 | 8 | 15)));
}
