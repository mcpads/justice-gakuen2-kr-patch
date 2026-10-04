use anyhow::{Context, Result};

use crate::development_build_spec::SizedFontSource;
use crate::font::{HorizontalTextAlignment, IndexedTextRasterizer, centered_glyph_ink_spans};
use crate::tim::Cell;

use super::model::{BonusMenuFontRole, BonusMenuFontSources};

pub(super) const TRANSPARENT_INDEX: u8 = 0;
const OUTLINE_INDEX: u8 = 4;
const FILL_INDEX: u8 = 12;
pub(super) const HEADING_OUTLINE_MARKER: u8 = 254;
pub(super) const HEADING_FILL_MARKER: u8 = 255;
const COMPACT_SOURCE_HEIGHT: usize = 32;

pub(super) struct BonusMenuTextRaster {
    pub(super) font_name: String,
    pub(super) font_sha256: String,
    pub(super) pixels: Vec<u8>,
    pub(super) measured_advance_px: f32,
    pub(super) ink_bounds: [usize; 4],
    pub(super) glyph_ink_spans: Vec<[usize; 2]>,
}

pub(super) fn font_for_role(
    fonts: &BonusMenuFontSources,
    role: BonusMenuFontRole,
) -> &SizedFontSource {
    match role {
        BonusMenuFontRole::Heading => &fonts.heading,
        BonusMenuFontRole::Entry => &fonts.entry,
        BonusMenuFontRole::CompactEntry => &fonts.compact_entry,
    }
}

pub(super) fn rasterize_unit_text(
    rasterizer: &IndexedTextRasterizer,
    style: &SizedFontSource,
    role: BonusMenuFontRole,
    cell: Cell,
    text: &str,
) -> Result<BonusMenuTextRaster> {
    let raster_height = match role {
        BonusMenuFontRole::CompactEntry => COMPACT_SOURCE_HEIGHT,
        BonusMenuFontRole::Heading | BonusMenuFontRole::Entry => cell.height,
    };
    let (outline_index, fill_index, alignment) = match role {
        BonusMenuFontRole::Heading => (
            HEADING_OUTLINE_MARKER,
            HEADING_FILL_MARKER,
            HorizontalTextAlignment::Center,
        ),
        BonusMenuFontRole::Entry | BonusMenuFontRole::CompactEntry => {
            (OUTLINE_INDEX, FILL_INDEX, HorizontalTextAlignment::Left)
        }
    };
    let mut raster = rasterizer.rasterize(
        text,
        cell.width,
        raster_height,
        style.font_px,
        0.0,
        TRANSPARENT_INDEX,
        Some(outline_index),
        fill_index,
        alignment,
    )?;
    let glyph_ink_spans = if role == BonusMenuFontRole::Heading {
        centered_glyph_ink_spans(
            rasterizer,
            text,
            cell.width,
            cell.height,
            style.font_px,
            0.0,
            raster.measured_advance_px,
        )?
    } else {
        Vec::new()
    };
    if raster_height != cell.height {
        raster.pixels = fit_indexed_ink_rows(
            &raster.pixels,
            cell.width,
            raster_height,
            cell.height,
            raster.ink_bounds,
        );
        raster.ink_bounds = indexed_ink_bounds(&raster.pixels, cell.width, cell.height)?;
    }
    Ok(BonusMenuTextRaster {
        font_name: raster.font_name,
        font_sha256: raster.font_sha256,
        pixels: raster.pixels,
        measured_advance_px: raster.measured_advance_px,
        ink_bounds: raster.ink_bounds,
        glyph_ink_spans,
    })
}

pub(super) fn fit_indexed_ink_rows(
    pixels: &[u8],
    width: usize,
    source_height: usize,
    target_height: usize,
    ink_bounds: [usize; 4],
) -> Vec<u8> {
    let source_ink_height = ink_bounds[3] - ink_bounds[1];
    let mut result = Vec::with_capacity(width * target_height);
    for target_y in 0..target_height {
        let source_y = ink_bounds[1] + target_y * source_ink_height / target_height;
        debug_assert!(source_y < source_height);
        result.extend_from_slice(&pixels[source_y * width..(source_y + 1) * width]);
    }
    result
}

fn indexed_ink_bounds(pixels: &[u8], width: usize, height: usize) -> Result<[usize; 4]> {
    let mut bounds: Option<[usize; 4]> = None;
    for y in 0..height {
        for x in 0..width {
            if pixels[y * width + x] == TRANSPARENT_INDEX {
                continue;
            }
            bounds = Some(match bounds {
                Some([min_x, min_y, max_x, max_y]) => [
                    min_x.min(x),
                    min_y.min(y),
                    max_x.max(x + 1),
                    max_y.max(y + 1),
                ],
                None => [x, y, x + 1, y + 1],
            });
        }
    }
    bounds.context("vertically resampled bonus text has no ink")
}
