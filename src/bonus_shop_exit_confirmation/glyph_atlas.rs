use anyhow::{Result, ensure};

use crate::font::{HorizontalTextAlignment, IndexedTextRasterizer, RasterizedIndexedText};
use crate::pipeline::sha256_bytes;
use crate::tim::{read_indexed_cell_in_prefix, write_indexed_cell_in_prefix_with_report};

use super::glyph_slots::{
    QUESTION_MARK_CODE, QUESTION_MARK_SOURCE_INDEXED_SHA256, SOURCE_BLANK_GLYPH_INDEXED_SHA256,
};
use super::model::{
    BonusShopExitConfirmationFontSource, ShopExitGlyphAllocation, ShopExitGlyphPixels,
};
use super::source::{GLYPH_TIM_OFFSET, validate_tim_geometry};

pub(super) struct GlyphApplication {
    pub(super) allowed_ranges: Vec<[usize; 2]>,
    pub(super) changed_byte_count: usize,
}

pub(super) fn rasterize_exit_confirmation_glyphs(
    font: &BonusShopExitConfirmationFontSource,
    allocations: &[ShopExitGlyphAllocation],
) -> Result<(Vec<ShopExitGlyphPixels>, Vec<RasterizedIndexedText>)> {
    let rasterizer = IndexedTextRasterizer::load(&font.path)?;
    let mut rasters = Vec::with_capacity(allocations.len());
    let mut glyphs = Vec::with_capacity(allocations.len());
    for allocation in allocations {
        let raster = rasterize_glyph(&rasterizer, font, allocation.text)?;
        ensure!(
            raster.pixels.len() == allocation.cell.width * allocation.cell.height,
            "shop exit-confirmation glyph raster dimensions changed"
        );
        glyphs.push(ShopExitGlyphPixels {
            code: allocation.code,
            cell: allocation.cell,
            indexed_sha256: sha256_bytes(&raster.pixels),
            pixels: raster.pixels.clone(),
        });
        rasters.push(raster);
    }
    Ok((glyphs, rasters))
}

pub(super) fn apply_exit_confirmation_glyphs(
    shop_ui_decoded: &mut [u8],
    glyphs: &[ShopExitGlyphPixels],
) -> Result<Vec<GlyphApplication>> {
    validate_tim_geometry(shop_ui_decoded)?;
    let mut applications = Vec::with_capacity(glyphs.len());
    for glyph in glyphs {
        let source_pixels =
            read_indexed_cell_in_prefix(shop_ui_decoded, GLYPH_TIM_OFFSET, glyph.cell)?;
        ensure!(
            sha256_bytes(&source_pixels) == source_glyph_sha256(glyph.code),
            "shop exit-confirmation glyph cell has another writer or changed source pixels"
        );
        ensure!(
            sha256_bytes(&glyph.pixels) == glyph.indexed_sha256,
            "shop exit-confirmation glyph pixels differ from their build plan"
        );

        let write = write_indexed_cell_in_prefix_with_report(
            shop_ui_decoded,
            GLYPH_TIM_OFFSET,
            glyph.cell,
            &glyph.pixels,
        )?;
        ensure!(
            write.changed_byte_count > 0,
            "shop exit-confirmation glyph changed no bytes"
        );
        let output_pixels =
            read_indexed_cell_in_prefix(shop_ui_decoded, GLYPH_TIM_OFFSET, glyph.cell)?;
        ensure!(
            sha256_bytes(&output_pixels) == glyph.indexed_sha256,
            "shop exit-confirmation glyph readback differs from its build plan"
        );
        applications.push(GlyphApplication {
            allowed_ranges: write.allowed_ranges,
            changed_byte_count: write.changed_byte_count,
        });
    }
    Ok(applications)
}

fn rasterize_glyph(
    rasterizer: &IndexedTextRasterizer,
    font: &BonusShopExitConfirmationFontSource,
    text: char,
) -> Result<RasterizedIndexedText> {
    let raster = rasterizer.rasterize_shifted_with_coverage_ramp(
        &text.to_string(),
        20,
        20,
        font.font_px,
        font.tracking_px,
        0,
        0,
        1,
        15,
        HorizontalTextAlignment::Center,
    )?;
    shift_raster_vertically(raster, font.vertical_shift_px)
}

fn shift_raster_vertically(
    mut raster: RasterizedIndexedText,
    shift_px: i32,
) -> Result<RasterizedIndexedText> {
    if shift_px == 0 {
        return Ok(raster);
    }
    let mut pixels = vec![0; raster.pixels.len()];
    let mut ink_bounds = [20, 20, 0, 0];
    let mut has_ink = false;
    for source_y in 0..20 {
        for x in 0..20 {
            let pixel = raster.pixels[source_y * 20 + x];
            if pixel == 0 {
                continue;
            }
            let target_y = source_y as i32 + shift_px;
            ensure!(
                (0..20).contains(&target_y),
                "shop exit-confirmation glyph clips after vertical shift {shift_px}"
            );
            let target_y = target_y as usize;
            pixels[target_y * 20 + x] = pixel;
            has_ink = true;
            ink_bounds[0] = ink_bounds[0].min(x);
            ink_bounds[1] = ink_bounds[1].min(target_y);
            ink_bounds[2] = ink_bounds[2].max(x + 1);
            ink_bounds[3] = ink_bounds[3].max(target_y + 1);
        }
    }
    ensure!(has_ink, "shop exit-confirmation glyph has no shifted ink");
    raster.pixels = pixels;
    raster.ink_bounds = ink_bounds;
    Ok(raster)
}

pub(super) fn source_glyph_sha256(code: u16) -> &'static str {
    if code == QUESTION_MARK_CODE {
        QUESTION_MARK_SOURCE_INDEXED_SHA256
    } else {
        SOURCE_BLANK_GLYPH_INDEXED_SHA256
    }
}
