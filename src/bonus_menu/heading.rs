use anyhow::{Result, ensure};

use crate::tim::Cell;

use super::raster::{
    BonusMenuTextRaster, HEADING_FILL_MARKER, HEADING_OUTLINE_MARKER, TRANSPARENT_INDEX,
};

const HEADING_WIDTH: usize = 192;
const HEADING_HEIGHT: usize = 96;
const BACKGROUND_TOP_SAMPLE_Y: usize = 3;
const BACKGROUND_BOTTOM_SAMPLE_Y: usize = 92;
const BACKGROUND_LEFT_SAMPLE_X: usize = 15;
const BACKGROUND_RIGHT_SAMPLE_X: usize = 176;
const BACKGROUND_FIXED_POINT_SCALE: i32 = 256;
const BACKGROUND_DIFFUSION_STEPS: usize = 512;

pub(super) const HEADING_CLEANUP_CELL: Cell = Cell {
    x: 16,
    y: 4,
    width: 160,
    height: 88,
};

const GLYPH_PALETTE_ROLES: [HeadingGlyphPalette; 3] = [
    HeadingGlyphPalette {
        outline_index: 6,
        outline_word: 0x7c09,
        fill_index: 255,
        fill_word: 0x7fff,
    },
    HeadingGlyphPalette {
        outline_index: 87,
        outline_word: 0x7f40,
        fill_index: 83,
        fill_word: 0x0be0,
    },
    HeadingGlyphPalette {
        outline_index: 18,
        outline_word: 0x00be,
        fill_index: 216,
        fill_word: 0x03f9,
    },
];

#[derive(Clone, Copy)]
struct HeadingGlyphPalette {
    outline_index: u8,
    outline_word: u16,
    fill_index: u8,
    fill_word: u16,
}

#[derive(Debug)]
pub(super) struct HeadingComposition {
    pub(super) pixels: Vec<u8>,
    pub(super) source_background_preserved_outside_cleanup: bool,
}

pub(super) fn compose_heading_cell(
    source_pixels: &[u8],
    palette: &[u16; 256],
    raster: &BonusMenuTextRaster,
) -> Result<HeadingComposition> {
    ensure!(
        source_pixels.len() == HEADING_WIDTH * HEADING_HEIGHT,
        "bonus heading source cell geometry changed"
    );
    ensure!(
        raster.pixels.len() == source_pixels.len(),
        "bonus heading raster geometry changed"
    );
    ensure_heading_palette_roles(palette)?;
    ensure!(
        ink_is_inside(raster.ink_bounds, HEADING_CLEANUP_CELL),
        "bonus heading ink leaves its cleanup region"
    );

    let mut pixels = source_pixels.to_vec();
    reconstruct_background(&mut pixels, source_pixels, palette);
    overlay_styled_glyphs(&mut pixels, raster)?;
    let source_background_preserved_outside_cleanup = pixels
        .iter()
        .zip(source_pixels)
        .enumerate()
        .all(|(offset, (output, source))| {
            let x = offset % HEADING_WIDTH;
            let y = offset / HEADING_WIDTH;
            cell_contains(HEADING_CLEANUP_CELL, x, y) || output == source
        });
    ensure!(
        source_background_preserved_outside_cleanup,
        "bonus heading changed source background outside its cleanup region"
    );
    Ok(HeadingComposition {
        pixels,
        source_background_preserved_outside_cleanup,
    })
}

fn ensure_heading_palette_roles(palette: &[u16; 256]) -> Result<()> {
    for role in GLYPH_PALETTE_ROLES {
        ensure!(
            palette[usize::from(role.outline_index)] == role.outline_word
                && palette[usize::from(role.fill_index)] == role.fill_word,
            "bonus heading source palette roles changed"
        );
    }
    Ok(())
}

fn reconstruct_background(output: &mut [u8], source: &[u8], palette: &[u16; 256]) {
    let grid_width = HEADING_CLEANUP_CELL.width + 2;
    let grid_height = HEADING_CLEANUP_CELL.height + 2;
    let mut colors = vec![[0i32; 3]; grid_width * grid_height];
    for grid_y in 0..grid_height {
        let y = BACKGROUND_TOP_SAMPLE_Y + grid_y;
        for grid_x in 0..grid_width {
            let x = BACKGROUND_LEFT_SAMPLE_X + grid_x;
            let offset = grid_y * grid_width + grid_x;
            if grid_x == 0 || grid_x + 1 == grid_width || grid_y == 0 || grid_y + 1 == grid_height {
                colors[offset] = fixed_rgb5(palette[usize::from(source[y * HEADING_WIDTH + x])]);
                continue;
            }
            let top = fixed_rgb5(
                palette[usize::from(source[BACKGROUND_TOP_SAMPLE_Y * HEADING_WIDTH + x])],
            );
            let bottom = fixed_rgb5(
                palette[usize::from(source[BACKGROUND_BOTTOM_SAMPLE_Y * HEADING_WIDTH + x])],
            );
            let left = fixed_rgb5(
                palette[usize::from(source[y * HEADING_WIDTH + BACKGROUND_LEFT_SAMPLE_X])],
            );
            let right = fixed_rgb5(
                palette[usize::from(source[y * HEADING_WIDTH + BACKGROUND_RIGHT_SAMPLE_X])],
            );
            let vertical = interpolate_fixed(top, bottom, grid_y, grid_height - 1);
            let horizontal = interpolate_fixed(left, right, grid_x, grid_width - 1);
            colors[offset] =
                std::array::from_fn(|channel| (vertical[channel] + horizontal[channel] + 1) / 2);
        }
    }

    let mut next = colors.clone();
    for _ in 0..BACKGROUND_DIFFUSION_STEPS {
        for y in 1..grid_height - 1 {
            for x in 1..grid_width - 1 {
                let offset = y * grid_width + x;
                next[offset] = std::array::from_fn(|channel| {
                    (colors[offset - 1][channel]
                        + colors[offset + 1][channel]
                        + colors[offset - grid_width][channel]
                        + colors[offset + grid_width][channel]
                        + 2)
                        / 4
                });
            }
        }
        std::mem::swap(&mut colors, &mut next);
    }

    for grid_y in 1..grid_height - 1 {
        let y = BACKGROUND_TOP_SAMPLE_Y + grid_y;
        for grid_x in 1..grid_width - 1 {
            let x = BACKGROUND_LEFT_SAMPLE_X + grid_x;
            let fixed = colors[grid_y * grid_width + grid_x];
            let target = std::array::from_fn(|channel| {
                ((fixed[channel] + BACKGROUND_FIXED_POINT_SCALE / 2) / BACKGROUND_FIXED_POINT_SCALE)
                    .clamp(0, 31) as u8
            });
            output[y * HEADING_WIDTH + x] = nearest_palette_index(palette, target);
        }
    }
}

fn overlay_styled_glyphs(output: &mut [u8], raster: &BonusMenuTextRaster) -> Result<()> {
    ensure!(
        raster.glyph_ink_spans.len() == GLYPH_PALETTE_ROLES.len(),
        "bonus heading glyph count has no declared palette roles"
    );
    for (offset, marker) in raster.pixels.iter().copied().enumerate() {
        if marker == TRANSPARENT_INDEX {
            continue;
        }
        ensure!(
            marker == HEADING_OUTLINE_MARKER || marker == HEADING_FILL_MARKER,
            "bonus heading raster contains an unknown palette marker"
        );
        let x = offset % HEADING_WIDTH;
        let mut matching_roles = raster
            .glyph_ink_spans
            .iter()
            .enumerate()
            .filter(|(_, [left, right])| *left <= x && x < *right);
        let role_index = matching_roles
            .next()
            .map(|(index, _)| index)
            .ok_or_else(|| anyhow::anyhow!("bonus heading ink is outside every glyph span"))?;
        ensure!(
            matching_roles.next().is_none(),
            "bonus heading glyph spans overlap"
        );
        let role = GLYPH_PALETTE_ROLES[role_index];
        output[offset] = if marker == HEADING_FILL_MARKER {
            role.fill_index
        } else {
            role.outline_index
        };
    }
    Ok(())
}

fn interpolate_fixed(
    start: [i32; 3],
    end: [i32; 3],
    numerator: usize,
    denominator: usize,
) -> [i32; 3] {
    std::array::from_fn(|channel| {
        let top_weight = denominator - numerator;
        (start[channel] * top_weight as i32
            + end[channel] * numerator as i32
            + denominator as i32 / 2)
            / denominator as i32
    })
}

fn fixed_rgb5(word: u16) -> [i32; 3] {
    rgb5(word).map(|channel| i32::from(channel) * BACKGROUND_FIXED_POINT_SCALE)
}

fn nearest_palette_index(palette: &[u16; 256], target: [u8; 3]) -> u8 {
    palette
        .iter()
        .enumerate()
        .min_by_key(|(_, word)| {
            let color = rgb5(**word);
            color
                .iter()
                .zip(target)
                .map(|(actual, target)| {
                    let delta = i32::from(*actual) - i32::from(target);
                    delta * delta
                })
                .sum::<i32>()
        })
        .map(|(index, _)| index as u8)
        .expect("the fixed 8-bpp palette is nonempty")
}

fn rgb5(word: u16) -> [u8; 3] {
    [
        (word & 0x1f) as u8,
        ((word >> 5) & 0x1f) as u8,
        ((word >> 10) & 0x1f) as u8,
    ]
}

fn ink_is_inside(bounds: [usize; 4], cell: Cell) -> bool {
    let [left, top, right, bottom] = bounds;
    left >= cell.x
        && top >= cell.y
        && right <= cell.x + cell.width
        && bottom <= cell.y + cell.height
}

fn cell_contains(cell: Cell, x: usize, y: usize) -> bool {
    x >= cell.x && x < cell.x + cell.width && y >= cell.y && y < cell.y + cell.height
}
