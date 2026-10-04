use anyhow::{Result, ensure};

use crate::bonus_inventory_source::rasterize_message_glyph;
use crate::font::{IndexedTextRasterizer, RasterizedIndexedText};
use crate::pipeline::sha256_bytes;
use crate::tim::{
    parse_4bpp_prefix, read_indexed_cell_in_prefix, write_indexed_cell_in_prefix_with_report,
};

use super::glyph_slots::{MEMORY_CARD_SWAP_GLYPHS, SOURCE_BLANK_GLYPH_INDEXED_SHA256};
use super::model::{BonusInventoryMemoryCardSwapFontSource, MemoryCardSwapGlyphPixels};
use super::source::{GLYPH_TIM_OFFSET, GLYPH_TIM_SIZE};

pub(super) struct GlyphApplication {
    pub(super) allowed_ranges: Vec<[usize; 2]>,
    pub(super) changed_byte_count: usize,
}

pub(super) fn rasterize_memory_card_swap_glyphs(
    font: &BonusInventoryMemoryCardSwapFontSource,
) -> Result<(Vec<MemoryCardSwapGlyphPixels>, Vec<RasterizedIndexedText>)> {
    let rasterizer = IndexedTextRasterizer::load(&font.path)?;
    let mut rasters = Vec::with_capacity(MEMORY_CARD_SWAP_GLYPHS.len());
    let mut glyphs = Vec::with_capacity(MEMORY_CARD_SWAP_GLYPHS.len());
    for (text, code, cell) in MEMORY_CARD_SWAP_GLYPHS {
        let raster = rasterize_message_glyph(&rasterizer, font, text)?;
        ensure!(
            raster.pixels.len() == cell.width * cell.height,
            "bonus memory-card-swap glyph raster dimensions changed"
        );
        glyphs.push(MemoryCardSwapGlyphPixels {
            code,
            cell,
            indexed_sha256: sha256_bytes(&raster.pixels),
            pixels: raster.pixels.clone(),
        });
        rasters.push(raster);
    }
    Ok((glyphs, rasters))
}

pub(super) fn apply_memory_card_swap_glyphs(
    inventory_decoded: &mut [u8],
    glyphs: &[MemoryCardSwapGlyphPixels],
) -> Result<Vec<GlyphApplication>> {
    let glyph_tim = parse_4bpp_prefix(
        inventory_decoded
            .get(GLYPH_TIM_OFFSET..)
            .ok_or_else(|| anyhow::anyhow!("KOUBAI1 memory-card-swap glyph TIM disappeared"))?,
    )?;
    ensure!(
        glyph_tim.total_size == GLYPH_TIM_SIZE
            && glyph_tim.pixel_width() == 1024
            && glyph_tim.image_height == 256
            && glyph_tim.image_x == 512
            && glyph_tim.image_y == 0,
        "KOUBAI1 memory-card-swap glyph TIM geometry changed during composition"
    );
    ensure!(
        glyphs.len() == MEMORY_CARD_SWAP_GLYPHS.len(),
        "bonus memory-card-swap glyph plan changed"
    );

    let mut applications = Vec::with_capacity(glyphs.len());
    for (glyph, (_, expected_code, expected_cell)) in glyphs.iter().zip(MEMORY_CARD_SWAP_GLYPHS) {
        ensure!(
            glyph.code == expected_code && glyph.cell == expected_cell,
            "bonus memory-card-swap glyph identity changed"
        );
        let source_pixels =
            read_indexed_cell_in_prefix(inventory_decoded, GLYPH_TIM_OFFSET, glyph.cell)?;
        ensure!(
            sha256_bytes(&source_pixels) == SOURCE_BLANK_GLYPH_INDEXED_SHA256,
            "bonus memory-card-swap glyph cell has another writer or changed source pixels"
        );
        ensure!(
            sha256_bytes(&glyph.pixels) == glyph.indexed_sha256,
            "bonus memory-card-swap glyph pixels differ from their build plan"
        );

        let write = write_indexed_cell_in_prefix_with_report(
            inventory_decoded,
            GLYPH_TIM_OFFSET,
            glyph.cell,
            &glyph.pixels,
        )?;
        ensure!(
            write.changed_byte_count > 0,
            "bonus memory-card-swap glyph changed no bytes"
        );
        let output_pixels =
            read_indexed_cell_in_prefix(inventory_decoded, GLYPH_TIM_OFFSET, glyph.cell)?;
        ensure!(
            sha256_bytes(&output_pixels) == glyph.indexed_sha256,
            "bonus memory-card-swap glyph readback differs from its build plan"
        );
        applications.push(GlyphApplication {
            allowed_ranges: write.allowed_ranges,
            changed_byte_count: write.changed_byte_count,
        });
    }
    Ok(applications)
}
