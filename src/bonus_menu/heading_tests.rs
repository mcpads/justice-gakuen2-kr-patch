use super::heading::{HEADING_CLEANUP_CELL, compose_heading_cell};
use super::raster::{BonusMenuTextRaster, HEADING_FILL_MARKER, HEADING_OUTLINE_MARKER};

const WIDTH: usize = 192;
const HEIGHT: usize = 96;

fn source_palette() -> [u16; 256] {
    let mut palette = std::array::from_fn(|index| {
        let component = (index % 32) as u16;
        component | (component << 5) | (component << 10)
    });
    for (index, word) in [
        (6, 0x7c09),
        (255, 0x7fff),
        (87, 0x7f40),
        (83, 0x0be0),
        (18, 0x00be),
        (216, 0x03f9),
    ] {
        palette[index] = word;
    }
    palette
}

fn raster(spans: Vec<[usize; 2]>) -> BonusMenuTextRaster {
    let mut pixels = vec![0; WIDTH * HEIGHT];
    for (x, marker) in [
        (60, HEADING_FILL_MARKER),
        (61, HEADING_FILL_MARKER),
        (62, HEADING_FILL_MARKER),
    ] {
        pixels[40 * WIDTH + x] = marker;
    }
    for (x, marker) in [
        (60, HEADING_OUTLINE_MARKER),
        (61, HEADING_OUTLINE_MARKER),
        (62, HEADING_OUTLINE_MARKER),
    ] {
        pixels[41 * WIDTH + x] = marker;
    }
    BonusMenuTextRaster {
        font_name: "fixture".to_string(),
        font_sha256: "fixture".to_string(),
        pixels,
        measured_advance_px: 3.0,
        ink_bounds: [60, 40, 63, 42],
        glyph_ink_spans: spans,
    }
}

#[test]
fn heading_composition_preserves_background_outside_cleanup_and_styles_each_glyph() {
    let mut source = vec![200; WIDTH * HEIGHT];
    for y in HEADING_CLEANUP_CELL.y..HEADING_CLEANUP_CELL.y + HEADING_CLEANUP_CELL.height {
        for x in HEADING_CLEANUP_CELL.x..HEADING_CLEANUP_CELL.x + HEADING_CLEANUP_CELL.width {
            source[y * WIDTH + x] = 42;
        }
    }
    let composition = compose_heading_cell(
        &source,
        &source_palette(),
        &raster(vec![[60, 61], [61, 62], [62, 63]]),
    )
    .unwrap();

    assert!(composition.source_background_preserved_outside_cleanup);
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            if !(HEADING_CLEANUP_CELL.x..HEADING_CLEANUP_CELL.x + HEADING_CLEANUP_CELL.width)
                .contains(&x)
                || !(HEADING_CLEANUP_CELL.y..HEADING_CLEANUP_CELL.y + HEADING_CLEANUP_CELL.height)
                    .contains(&y)
            {
                assert_eq!(composition.pixels[y * WIDTH + x], source[y * WIDTH + x]);
            }
        }
    }
    assert_eq!(
        &composition.pixels[40 * WIDTH + 60..40 * WIDTH + 63],
        &[255, 83, 216]
    );
    assert_eq!(
        &composition.pixels[41 * WIDTH + 60..41 * WIDTH + 63],
        &[6, 87, 18]
    );
}

#[test]
fn heading_composition_rejects_overlapping_glyph_spans() {
    let error = compose_heading_cell(
        &vec![200; WIDTH * HEIGHT],
        &source_palette(),
        &raster(vec![[60, 62], [61, 63], [63, 64]]),
    )
    .unwrap_err();

    assert!(error.to_string().contains("glyph spans overlap"));
}

#[test]
fn heading_composition_requires_the_source_palette_roles() {
    let mut palette = source_palette();
    palette[255] = 0;

    let error = compose_heading_cell(
        &vec![200; WIDTH * HEIGHT],
        &palette,
        &raster(vec![[60, 61], [61, 62], [62, 63]]),
    )
    .unwrap_err();

    assert!(error.to_string().contains("source palette roles changed"));
}
