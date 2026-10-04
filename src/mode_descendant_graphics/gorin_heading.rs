use std::collections::VecDeque;

use anyhow::{Context, Result, ensure};

use crate::development_build_spec::SizedFontSource;
use crate::font::{HorizontalTextAlignment, IndexedTextRasterizer, centered_glyph_ink_spans};
use crate::tim::Cell;

const TRANSPARENT_MARKER: u8 = 0;
const OUTLINE_MARKER: u8 = 254;
const FILL_MARKER: u8 = 255;
const SOURCE_TITLE_MASK_DILATION_STEPS: usize = 2;
const BACKGROUND_DIFFUSION_STEPS: usize = 256;
const FIXED_POINT_SCALE: i32 = 256;
const SHADOW_INDEX: u8 = 1;
const SHADOW_WORD: u16 = 0x0921;
const OUTLINE_INDEX: u8 = 255;
const OUTLINE_WORD: u16 = 0x7fff;
const SHADOW_OFFSET: usize = 2;

#[derive(Clone, Copy)]
enum GradientAxis {
    Horizontal,
    Vertical,
}

#[derive(Clone, Copy)]
struct GlyphGradient {
    axis: GradientAxis,
    colors: [[u8; 3]; 3],
}

const GLYPH_GRADIENTS: [GlyphGradient; 3] = [
    GlyphGradient {
        axis: GradientAxis::Vertical,
        colors: [[12, 0, 28], [28, 0, 24], [28, 0, 4]],
    },
    GlyphGradient {
        axis: GradientAxis::Horizontal,
        colors: [[0, 2, 28], [0, 18, 28], [0, 28, 20]],
    },
    GlyphGradient {
        axis: GradientAxis::Horizontal,
        colors: [[0, 28, 0], [28, 28, 0], [28, 4, 0]],
    },
];

pub(super) struct GorinHeadingRaster {
    pub(super) font_name: String,
    pub(super) font_sha256: String,
    pub(super) pixels: Vec<u8>,
    pub(super) measured_advance_px: f32,
    pub(super) ink_bounds: [usize; 4],
    pub(super) glyph_ink_spans: Vec<[usize; 2]>,
}

pub(super) struct GorinHeadingComposition {
    pub(super) pixels: Vec<u8>,
    pub(super) source_exclusive_palette_index_count: usize,
    pub(super) reconstructed_background_pixel_count: usize,
    pub(super) korean_ink_pixel_count: usize,
    pub(super) source_pixels_preserved_outside_reconstruction_and_korean_ink: bool,
}

pub(super) fn rasterize_gorin_heading(
    rasterizer: &IndexedTextRasterizer,
    style: &SizedFontSource,
    cell: Cell,
    text: &str,
) -> Result<GorinHeadingRaster> {
    ensure!(
        text.chars().count() == GLYPH_GRADIENTS.len(),
        "Gorin heading must contain three Korean glyphs"
    );
    let raster = rasterizer.rasterize(
        text,
        cell.width,
        cell.height,
        style.font_px,
        0.0,
        TRANSPARENT_MARKER,
        Some(OUTLINE_MARKER),
        FILL_MARKER,
        HorizontalTextAlignment::Center,
    )?;
    let glyph_ink_spans = centered_glyph_ink_spans(
        rasterizer,
        text,
        cell.width,
        cell.height,
        style.font_px,
        0.0,
        raster.measured_advance_px,
    )?;
    Ok(GorinHeadingRaster {
        font_name: raster.font_name,
        font_sha256: raster.font_sha256,
        pixels: raster.pixels,
        measured_advance_px: raster.measured_advance_px,
        ink_bounds: raster.ink_bounds,
        glyph_ink_spans,
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn compose_gorin_heading(
    full_source_pixels: &[u8],
    full_width: usize,
    full_height: usize,
    cell: Cell,
    source_cell_pixels: &[u8],
    palette: &[u16; 256],
    raster: &GorinHeadingRaster,
) -> Result<GorinHeadingComposition> {
    ensure!(
        full_source_pixels.len() == full_width * full_height,
        "Gorin heading source texture geometry changed"
    );
    ensure!(
        cell.x + cell.width <= full_width && cell.y + cell.height <= full_height,
        "Gorin heading cell leaves its source texture"
    );
    ensure!(
        source_cell_pixels.len() == cell.width * cell.height
            && raster.pixels.len() == source_cell_pixels.len(),
        "Gorin heading cell geometry changed"
    );
    ensure!(
        palette[usize::from(SHADOW_INDEX)] == SHADOW_WORD
            && palette[usize::from(OUTLINE_INDEX)] == OUTLINE_WORD,
        "Gorin heading source palette roles changed"
    );
    ensure!(
        raster.glyph_ink_spans.len() == GLYPH_GRADIENTS.len(),
        "Gorin heading glyph spans changed"
    );

    let mut used_inside = [false; 256];
    let mut used_outside = [false; 256];
    for y in 0..full_height {
        for x in 0..full_width {
            let index = usize::from(full_source_pixels[y * full_width + x]);
            if contains(cell, x, y) {
                used_inside[index] = true;
            } else {
                used_outside[index] = true;
            }
        }
    }
    let source_exclusive =
        std::array::from_fn::<_, 256, _>(|index| used_inside[index] && !used_outside[index]);
    let source_exclusive_palette_index_count = source_exclusive
        .iter()
        .filter(|exclusive| **exclusive)
        .count();
    ensure!(
        source_exclusive_palette_index_count > 0,
        "Gorin heading has no source-exclusive palette indexes"
    );

    let mut reconstruction_mask = source_cell_pixels
        .iter()
        .map(|index| source_exclusive[usize::from(*index)])
        .collect::<Vec<_>>();
    for _ in 0..SOURCE_TITLE_MASK_DILATION_STEPS {
        reconstruction_mask = dilate_mask(&reconstruction_mask, cell.width, cell.height);
    }
    ensure!(
        mask_has_unowned_border(&reconstruction_mask, cell.width, cell.height),
        "Gorin heading reconstruction mask reached its owned-cell border"
    );
    let reconstructed_background_pixel_count =
        reconstruction_mask.iter().filter(|masked| **masked).count();
    ensure!(
        reconstructed_background_pixel_count > 0,
        "Gorin heading reconstruction mask is empty"
    );

    let background_palette_indexes = used_outside
        .iter()
        .enumerate()
        .filter_map(|(index, used)| used.then_some(index as u8))
        .collect::<Vec<_>>();
    ensure!(
        !background_palette_indexes.is_empty(),
        "Gorin heading has no background palette indexes"
    );
    let mut pixels = reconstruct_indexed_background(
        source_cell_pixels,
        &reconstruction_mask,
        cell.width,
        cell.height,
        palette,
        &background_palette_indexes,
    )?;
    ensure!(
        reconstruction_mask
            .iter()
            .enumerate()
            .all(|(offset, masked)| { !*masked || !source_exclusive[usize::from(pixels[offset])] }),
        "Gorin heading reconstruction retained source-title palette indexes"
    );

    let mut allowed_change = reconstruction_mask;
    overlay_shadow(
        &mut pixels,
        &raster.pixels,
        &mut allowed_change,
        cell.width,
        cell.height,
    )?;
    overlay_korean_glyphs(
        &mut pixels,
        raster,
        &mut allowed_change,
        palette,
        cell.width,
    )?;
    let korean_ink_pixel_count = raster
        .pixels
        .iter()
        .filter(|marker| **marker != TRANSPARENT_MARKER)
        .count();
    let source_pixels_preserved_outside_reconstruction_and_korean_ink = pixels
        .iter()
        .zip(source_cell_pixels)
        .zip(&allowed_change)
        .all(|((output, source), allowed)| *allowed || output == source);
    ensure!(
        source_pixels_preserved_outside_reconstruction_and_korean_ink,
        "Gorin heading changed source pixels outside reconstruction and Korean ink"
    );

    Ok(GorinHeadingComposition {
        pixels,
        source_exclusive_palette_index_count,
        reconstructed_background_pixel_count,
        korean_ink_pixel_count,
        source_pixels_preserved_outside_reconstruction_and_korean_ink,
    })
}

pub(super) fn reconstruct_indexed_background(
    source: &[u8],
    mask: &[bool],
    width: usize,
    height: usize,
    palette: &[u16; 256],
    background_palette_indexes: &[u8],
) -> Result<Vec<u8>> {
    let mut colors = source
        .iter()
        .map(|index| fixed_rgb5(palette[usize::from(*index)]))
        .collect::<Vec<_>>();
    let mut visited = mask.iter().map(|masked| !*masked).collect::<Vec<_>>();
    let mut queue = visited
        .iter()
        .enumerate()
        .filter_map(|(offset, known)| known.then_some(offset))
        .collect::<VecDeque<_>>();
    while let Some(offset) = queue.pop_front() {
        let (neighbors, neighbor_count) = orthogonal_neighbors(offset, width, height);
        for neighbor in &neighbors[..neighbor_count] {
            if visited[*neighbor] {
                continue;
            }
            visited[*neighbor] = true;
            colors[*neighbor] = colors[offset];
            queue.push_back(*neighbor);
        }
    }
    ensure!(
        visited.iter().all(|known| *known),
        "indexed background reconstruction left unknown pixels"
    );

    let mut next = colors.clone();
    for _ in 0..BACKGROUND_DIFFUSION_STEPS {
        for offset in 0..colors.len() {
            if !mask[offset] {
                continue;
            }
            let (neighbors, neighbor_count) = orthogonal_neighbors(offset, width, height);
            next[offset] = std::array::from_fn(|channel| {
                (neighbors[..neighbor_count]
                    .iter()
                    .map(|neighbor| colors[*neighbor][channel])
                    .sum::<i32>()
                    + neighbor_count as i32 / 2)
                    / neighbor_count as i32
            });
        }
        std::mem::swap(&mut colors, &mut next);
    }

    let mut output = source.to_vec();
    for (offset, masked) in mask.iter().enumerate() {
        if !*masked {
            continue;
        }
        let target = colors[offset].map(|component| {
            ((component + FIXED_POINT_SCALE / 2) / FIXED_POINT_SCALE).clamp(0, 31) as u8
        });
        output[offset] = nearest_palette_index(palette, background_palette_indexes, target)
            .context("indexed background palette disappeared")?;
    }
    Ok(output)
}

fn overlay_shadow(
    output: &mut [u8],
    raster: &[u8],
    allowed_change: &mut [bool],
    width: usize,
    height: usize,
) -> Result<()> {
    ensure!(
        output.len() == raster.len() && output.len() == allowed_change.len(),
        "Gorin heading shadow geometry changed"
    );
    for y in 0..height.saturating_sub(SHADOW_OFFSET) {
        for x in 0..width.saturating_sub(SHADOW_OFFSET) {
            if raster[y * width + x] == TRANSPARENT_MARKER {
                continue;
            }
            let target = (y + SHADOW_OFFSET) * width + x + SHADOW_OFFSET;
            output[target] = SHADOW_INDEX;
            allowed_change[target] = true;
        }
    }
    Ok(())
}

fn overlay_korean_glyphs(
    output: &mut [u8],
    raster: &GorinHeadingRaster,
    allowed_change: &mut [bool],
    palette: &[u16; 256],
    width: usize,
) -> Result<()> {
    let all_palette_indexes = (0u8..=255).collect::<Vec<_>>();
    for (offset, marker) in raster.pixels.iter().copied().enumerate() {
        if marker == TRANSPARENT_MARKER {
            continue;
        }
        ensure!(
            marker == OUTLINE_MARKER || marker == FILL_MARKER,
            "Gorin heading raster contains an unknown marker"
        );
        allowed_change[offset] = true;
        if marker == OUTLINE_MARKER {
            output[offset] = OUTLINE_INDEX;
            continue;
        }
        let x = offset % width;
        let y = offset / width;
        let role_index = raster
            .glyph_ink_spans
            .iter()
            .position(|[left, right]| *left <= x && x < *right)
            .context("Gorin heading ink is outside every glyph span")?;
        let role = GLYPH_GRADIENTS[role_index];
        let (position, start, end) = match role.axis {
            GradientAxis::Horizontal => {
                let [left, right] = raster.glyph_ink_spans[role_index];
                (x.saturating_sub(left), left, right)
            }
            GradientAxis::Vertical => (
                y.saturating_sub(raster.ink_bounds[1]),
                raster.ink_bounds[1],
                raster.ink_bounds[3],
            ),
        };
        let target = three_stop_gradient(role.colors, position, end.saturating_sub(start));
        output[offset] = nearest_palette_index(palette, &all_palette_indexes, target)
            .context("Gorin heading palette disappeared")?;
    }
    Ok(())
}

fn three_stop_gradient(colors: [[u8; 3]; 3], position: usize, span: usize) -> [u8; 3] {
    let denominator = span.saturating_sub(1).max(1);
    let doubled = position.min(denominator) * 2;
    if doubled <= denominator {
        interpolate_rgb5(colors[0], colors[1], doubled, denominator)
    } else {
        interpolate_rgb5(colors[1], colors[2], doubled - denominator, denominator)
    }
}

fn interpolate_rgb5(start: [u8; 3], end: [u8; 3], numerator: usize, denominator: usize) -> [u8; 3] {
    std::array::from_fn(|channel| {
        ((usize::from(start[channel]) * (denominator - numerator)
            + usize::from(end[channel]) * numerator
            + denominator / 2)
            / denominator) as u8
    })
}

fn nearest_palette_index(palette: &[u16; 256], candidates: &[u8], target: [u8; 3]) -> Option<u8> {
    candidates.iter().copied().min_by_key(|index| {
        rgb5(palette[usize::from(*index)])
            .iter()
            .zip(target)
            .map(|(actual, target)| {
                let delta = i32::from(*actual) - i32::from(target);
                delta * delta
            })
            .sum::<i32>()
    })
}

fn fixed_rgb5(word: u16) -> [i32; 3] {
    rgb5(word).map(|component| i32::from(component) * FIXED_POINT_SCALE)
}

fn rgb5(word: u16) -> [u8; 3] {
    [
        (word & 0x1f) as u8,
        ((word >> 5) & 0x1f) as u8,
        ((word >> 10) & 0x1f) as u8,
    ]
}

pub(super) fn dilate_mask(mask: &[bool], width: usize, height: usize) -> Vec<bool> {
    let mut dilated = mask.to_vec();
    for y in 0..height {
        for x in 0..width {
            if !mask[y * width + x] {
                continue;
            }
            for next_y in y.saturating_sub(1)..=(y + 1).min(height - 1) {
                for next_x in x.saturating_sub(1)..=(x + 1).min(width - 1) {
                    dilated[next_y * width + next_x] = true;
                }
            }
        }
    }
    dilated
}

pub(super) fn mask_has_unowned_border(mask: &[bool], width: usize, height: usize) -> bool {
    (0..width).all(|x| !mask[x] && !mask[(height - 1) * width + x])
        && (0..height).all(|y| !mask[y * width] && !mask[y * width + width - 1])
}

fn orthogonal_neighbors(offset: usize, width: usize, height: usize) -> ([usize; 4], usize) {
    let x = offset % width;
    let y = offset / width;
    let mut neighbors = [0usize; 4];
    let mut count = 0usize;
    if x > 0 {
        neighbors[count] = offset - 1;
        count += 1;
    }
    if x + 1 < width {
        neighbors[count] = offset + 1;
        count += 1;
    }
    if y > 0 {
        neighbors[count] = offset - width;
        count += 1;
    }
    if y + 1 < height {
        neighbors[count] = offset + width;
        count += 1;
    }
    (neighbors, count)
}

fn contains(cell: Cell, x: usize, y: usize) -> bool {
    x >= cell.x && x < cell.x + cell.width && y >= cell.y && y < cell.y + cell.height
}
