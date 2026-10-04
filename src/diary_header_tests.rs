// Exercise the validator with synthetic geometry. Canonical labels, cells, and
// source hashes are build inputs and must not be mirrored in unit tests.
use super::build::{rasterize_entry, validate_regions};
use super::model::{
    DiaryHeaderEntry, DiaryHeaderFontRole, DiaryHeaderProtectedRegion, DiaryHeaderTextLayout,
};
use crate::tim::Cell;

#[test]
#[ignore = "requires fonts in ../fonts/"]
fn diary_baseline_adjustment_precedes_clipping_for_both_layouts() {
    use super::model::{DiaryHeaderFontStyle, DiaryHeaderGlyphLayout, DiaryHeaderIndexedRendering};
    use crate::font::IndexedTextRasterizer;
    use std::path::Path;

    let font = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../fonts/NEXON_Maplestory/TTF/Maplestory Bold.ttf");
    let rasterizer = IndexedTextRasterizer::load(&font).unwrap();
    let style = DiaryHeaderFontStyle {
        path: font,
        font_px: 19.0,
        rendering: DiaryHeaderIndexedRendering::CoverageRamp {
            first_ink_index: 1,
            last_ink_index: 15,
        },
        glyph_layout: DiaryHeaderGlyphLayout {
            slot_width_px: 24,
            vertical_shift_px: -3,
        },
    };
    let mut entry = entry("descender", cell(0, 0, 24, 24));
    entry.korean_text = "종".to_string();
    for layout in [
        DiaryHeaderTextLayout::Continuous,
        DiaryHeaderTextLayout::GlyphSlots { slots: vec![0] },
    ] {
        assert!(rasterize_entry(&rasterizer, &entry, &style, &layout, 19.0, 0).is_err());
        let shifted = rasterize_entry(&rasterizer, &entry, &style, &layout, 19.0, -3).unwrap();
        assert!(shifted.pixels.iter().any(|pixel| *pixel != 0));
        assert!(shifted.ink_bounds[3] <= 22);
        assert!(rasterize_entry(&rasterizer, &entry, &style, &layout, 19.0, -24).is_err());
    }
}

#[test]
fn overlapping_diary_header_entries_are_rejected() {
    let entries = [
        entry("first", cell(0, 0, 32, 24)),
        entry("second", cell(16, 0, 32, 24)),
    ];
    let protected = [protected("meter", cell(64, 0, 32, 24))];

    assert!(validate_regions(&entries, &protected, 128, 64).is_err());
}

#[test]
fn diary_header_entry_cannot_overlap_protected_graphics() {
    let entries = [entry("label", cell(0, 0, 48, 24))];
    let protected = [protected("meter", cell(32, 0, 48, 24))];

    assert!(validate_regions(&entries, &protected, 128, 64).is_err());
}

#[test]
fn disjoint_diary_header_text_and_graphics_are_admitted() {
    let entries = [entry("label", cell(0, 0, 48, 24))];
    let protected = [protected("meter", cell(48, 0, 48, 24))];

    validate_regions(&entries, &protected, 128, 64).unwrap();
}

fn entry(id: &str, cell: Cell) -> DiaryHeaderEntry {
    DiaryHeaderEntry {
        id: id.to_string(),
        source_text: "source".to_string(),
        korean_text: "한국어".to_string(),
        font_role: DiaryHeaderFontRole::StatusLabel,
        cell,
        layout: DiaryHeaderTextLayout::Continuous,
        font_px: None,
        vertical_shift_px: None,
        indexed_art: None,
        source_region_sha256: "source-region".to_string(),
    }
}

fn protected(id: &str, cell: Cell) -> DiaryHeaderProtectedRegion {
    DiaryHeaderProtectedRegion {
        id: id.to_string(),
        cell,
        source_region_sha256: "source-region".to_string(),
    }
}

fn cell(x: usize, y: usize, width: usize, height: usize) -> Cell {
    Cell {
        x,
        y,
        width,
        height,
    }
}
