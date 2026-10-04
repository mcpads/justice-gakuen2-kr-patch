use crate::font::RasterizedIndexedText;

use super::render::crop_indexed_raster;

#[test]
fn segmented_line_preserves_each_row_across_head_and_tail_cells() {
    let width = 336;
    let height = 24;
    let pixels = (0..height)
        .flat_map(|y| (0..width).map(move |x| ((x + y * 3) % 15 + 1) as u8))
        .collect::<Vec<_>>();
    let logical = RasterizedIndexedText {
        font_name: "fixture".to_string(),
        font_sha256: "fixture".to_string(),
        pixels: pixels.clone(),
        measured_advance_px: 300.0,
        ink_bounds: [0, 0, width, height],
    };

    let head = crop_indexed_raster(&logical, width, height, [0, 0], 256, height, 1).unwrap();
    let tail = crop_indexed_raster(&logical, width, height, [256, 0], 80, height, 1).unwrap();

    for y in 0..height {
        let mut reconstructed = head.pixels[y * 256..(y + 1) * 256].to_vec();
        reconstructed.extend_from_slice(&tail.pixels[y * 80..(y + 1) * 80]);
        assert_eq!(reconstructed, pixels[y * width..(y + 1) * width]);
    }
}
