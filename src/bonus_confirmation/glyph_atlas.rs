use anyhow::{Result, ensure};

use crate::font::{IndexedTextRasterizer, RasterizedIndexedText};
use crate::pipeline::sha256_bytes;
use crate::tim::{
    parse_4bpp_prefix, read_indexed_cell_in_prefix, write_indexed_cell_in_prefix_with_report,
};

use super::glyph_slots::{
    CONFIRMATION_GLYPHS, FIXED_RECORD_SPACE_CELL, SOURCE_BLANK_GLYPH_INDEXED_SHA256,
};
use super::model::{BonusConfirmationFontSource, ConfirmationGlyphPixels};
use super::source::{GLYPH_TIM_OFFSET, GLYPH_TIM_SIZE};

pub(super) struct GlyphApplication {
    pub(super) allowed_ranges: Vec<[usize; 2]>,
    pub(super) changed_byte_count: usize,
}

pub(super) fn rasterize_confirmation_glyphs(
    font: &BonusConfirmationFontSource,
) -> Result<(Vec<ConfirmationGlyphPixels>, Vec<RasterizedIndexedText>)> {
    let rasterizer = IndexedTextRasterizer::load(&font.path)?;
    let mut rasters = Vec::with_capacity(CONFIRMATION_GLYPHS.len());
    let mut glyphs = Vec::with_capacity(CONFIRMATION_GLYPHS.len());
    for (text, code, cell) in CONFIRMATION_GLYPHS {
        let raster = rasterize_confirmation_glyph(&rasterizer, font, text)?;
        ensure!(
            raster.pixels.len() == cell.width * cell.height,
            "bonus confirmation glyph raster dimensions changed"
        );
        glyphs.push(ConfirmationGlyphPixels {
            code,
            cell,
            indexed_sha256: sha256_bytes(&raster.pixels),
            pixels: raster.pixels.clone(),
        });
        rasters.push(raster);
    }
    Ok((glyphs, rasters))
}

pub(super) fn apply_confirmation_glyphs(
    inventory_decoded: &mut [u8],
    glyphs: &[ConfirmationGlyphPixels],
) -> Result<Vec<GlyphApplication>> {
    let glyph_tim = parse_4bpp_prefix(
        inventory_decoded
            .get(GLYPH_TIM_OFFSET..)
            .ok_or_else(|| anyhow::anyhow!("KOUBAI1 confirmation glyph TIM disappeared"))?,
    )?;
    ensure!(
        glyph_tim.total_size == GLYPH_TIM_SIZE
            && glyph_tim.pixel_width() == 1024
            && glyph_tim.image_height == 256
            && glyph_tim.image_x == 512
            && glyph_tim.image_y == 0,
        "KOUBAI1 confirmation glyph TIM geometry changed during composition"
    );
    validate_fixed_record_space_is_blank(inventory_decoded)?;
    ensure!(
        glyphs.len() == CONFIRMATION_GLYPHS.len(),
        "bonus confirmation glyph plan changed"
    );
    let mut applications = Vec::with_capacity(glyphs.len());
    for (glyph, (_, expected_code, expected_cell)) in glyphs.iter().zip(CONFIRMATION_GLYPHS) {
        ensure!(
            glyph.code == expected_code && glyph.cell == expected_cell,
            "bonus confirmation glyph identity changed"
        );
        let source_pixels =
            read_indexed_cell_in_prefix(inventory_decoded, GLYPH_TIM_OFFSET, glyph.cell)?;
        ensure!(
            sha256_bytes(&source_pixels) == SOURCE_BLANK_GLYPH_INDEXED_SHA256,
            "bonus confirmation glyph cell has another writer or changed source pixels"
        );
        ensure!(
            sha256_bytes(&glyph.pixels) == glyph.indexed_sha256,
            "bonus confirmation glyph pixels differ from their build plan"
        );
        let write = write_indexed_cell_in_prefix_with_report(
            inventory_decoded,
            GLYPH_TIM_OFFSET,
            glyph.cell,
            &glyph.pixels,
        )?;
        ensure!(
            write.changed_byte_count > 0,
            "bonus confirmation glyph changed no bytes"
        );
        let output_pixels =
            read_indexed_cell_in_prefix(inventory_decoded, GLYPH_TIM_OFFSET, glyph.cell)?;
        ensure!(
            sha256_bytes(&output_pixels) == glyph.indexed_sha256,
            "bonus confirmation glyph readback differs from its build plan"
        );
        applications.push(GlyphApplication {
            allowed_ranges: write.allowed_ranges,
            changed_byte_count: write.changed_byte_count,
        });
    }
    validate_fixed_record_space_is_blank(inventory_decoded)?;
    Ok(applications)
}

pub(crate) fn validate_fixed_record_space_is_blank(inventory_decoded: &[u8]) -> Result<String> {
    let pixels =
        read_indexed_cell_in_prefix(inventory_decoded, GLYPH_TIM_OFFSET, FIXED_RECORD_SPACE_CELL)?;
    let indexed_sha256 = sha256_bytes(&pixels);
    ensure!(
        indexed_sha256 == SOURCE_BLANK_GLYPH_INDEXED_SHA256,
        "fixed confirmation record space cell is no longer blank"
    );
    Ok(indexed_sha256)
}

fn rasterize_confirmation_glyph(
    rasterizer: &IndexedTextRasterizer,
    font: &BonusConfirmationFontSource,
    text: &str,
) -> Result<RasterizedIndexedText> {
    crate::bonus_inventory_source::rasterize_message_glyph(rasterizer, font, text)
}
