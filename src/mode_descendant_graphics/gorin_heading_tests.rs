use super::gorin_heading::{GorinHeadingRaster, compose_gorin_heading};
use crate::tim::Cell;

#[test]
fn heading_replaces_source_exclusive_pixels_and_preserves_unowned_background() {
    let width = 20;
    let height = 14;
    let cell = Cell {
        x: 3,
        y: 3,
        width: 14,
        height: 8,
    };
    let mut full_source = vec![10u8; width * height];
    for y in 6..8 {
        for x in 8..12 {
            full_source[y * width + x] = 200;
        }
    }
    let source_cell = (cell.y..cell.y + cell.height)
        .flat_map(|y| {
            full_source[y * width + cell.x..y * width + cell.x + cell.width]
                .iter()
                .copied()
        })
        .collect::<Vec<_>>();
    let mut palette = [0u16; 256];
    palette[1] = 0x0921;
    palette[10] = 0x3eed;
    palette[200] = 0x701c;
    palette[255] = 0x7fff;
    let mut raster_pixels = vec![0; cell.width * cell.height];
    for (x, marker) in [(4, 255), (7, 255), (10, 255)] {
        raster_pixels[3 * cell.width + x] = marker;
        raster_pixels[4 * cell.width + x] = 254;
    }
    let raster = GorinHeadingRaster {
        font_name: "fixture".to_string(),
        font_sha256: "fixture".to_string(),
        pixels: raster_pixels,
        measured_advance_px: 7.0,
        ink_bounds: [4, 3, 11, 5],
        glyph_ink_spans: vec![[4, 6], [6, 9], [9, 11]],
    };

    let composition = compose_gorin_heading(
        &full_source,
        width,
        height,
        cell,
        &source_cell,
        &palette,
        &raster,
    )
    .unwrap();

    assert_eq!(composition.source_exclusive_palette_index_count, 1);
    assert!(composition.reconstructed_background_pixel_count >= 8);
    assert_eq!(composition.korean_ink_pixel_count, 6);
    assert!(composition.source_pixels_preserved_outside_reconstruction_and_korean_ink);
    assert_ne!(composition.pixels[3 * cell.width + 5], 200);
    assert_eq!(composition.pixels[4 * cell.width + 4], 255);
}
