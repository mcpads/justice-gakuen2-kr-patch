//! The EDIT title and gold oval share one texture and CLUT (112, 481).
//! Japanese shadows reuse the gold ramp, so palette-index erasure cannot
//! separate the lettering from its background.
use anyhow::{Result, ensure};

use crate::tim::Cell;

pub(super) fn compose_heading(cell: Cell, text: &mut [u8]) -> Result<()> {
    ensure!(
        (cell.x, cell.y, cell.width, cell.height) == (552, 136, 260, 32),
        "EDIT heading moved outside its source-bound banner"
    );
    ensure!(
        text.len() == 260 * 32,
        "EDIT heading raster dimensions changed"
    );
    for (y, row) in text.as_chunks_mut::<260>().0.iter_mut().enumerate() {
        for (x, pixel) in row.iter_mut().enumerate() {
            if *pixel == 0 {
                *pixel = oval_pixel(x + 40, y);
            }
        }
    }
    Ok(())
}

// The native 334x32 banner has a 24-row oval. These symmetric contour spans
// follow its exposed edges; the center hidden by the Japanese title is authored
// again using native gold indices. Never retain ambiguous source shadow pixels.
fn oval_pixel(x: usize, y: usize) -> u8 {
    const LEFT: [usize; 24] = [
        103, 76, 58, 42, 32, 23, 16, 10, 5, 2, 0, 0, 0, 0, 2, 5, 10, 16, 23, 32, 42, 58, 76, 103,
    ];
    let Some(&left) = y.checked_sub(6).and_then(|row| LEFT.get(row)) else {
        return 0;
    };
    if x < left || x >= 334 - left {
        return 0;
    }
    if x == left || x == 333 - left {
        return 7;
    }
    // Ordered quantization keeps the native indexed/pixel-art appearance.
    let gold = 8.0 + 7.0 * (1.0 - (x as f32 - 166.5).abs() / 166.5);
    let threshold = [[0.125, 0.625], [0.875, 0.375]][y % 2][x % 2];
    (gold.floor() as u8 + u8::from(gold.fract() > threshold)).min(15)
}

#[cfg(test)]
mod tests {
    use super::{compose_heading, oval_pixel};
    use crate::tim::Cell;

    #[test]
    fn reconstructs_gold_without_a_rectangle_or_source_letter_shadows() {
        for y in 0..32 {
            for x in 0..334 {
                let pixel = oval_pixel(x, y);
                assert!(pixel == 0 || (7..=15).contains(&pixel));
                if !(6..30).contains(&y) {
                    assert_eq!(pixel, 0);
                }
            }
        }
        assert_eq!(oval_pixel(166, 16), 15);
        assert_eq!(oval_pixel(0, 6), 0);
        assert_eq!(oval_pixel(0, 16), 7);
    }

    #[test]
    fn composites_only_behind_new_ink_in_the_owned_title_cell() {
        let mut pixels = vec![0; 260 * 32];
        pixels[260 * 16 + 130] = 5;
        pixels[260 * 16 + 129] = 1;
        compose_heading(
            Cell {
                x: 552,
                y: 136,
                width: 260,
                height: 32,
            },
            &mut pixels,
        )
        .unwrap();
        assert_eq!(pixels[260 * 16 + 130], 5);
        assert_eq!(pixels[260 * 16 + 129], 1);
        assert_eq!(pixels[0], 0);
        assert!((8..=15).contains(&pixels[260 * 16 + 131]));
    }
}
