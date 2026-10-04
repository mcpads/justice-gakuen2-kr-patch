use anyhow::{Result, ensure};

use crate::font::{IndexedTextRasterizer, RasterizedIndexedText};
use crate::pipeline::sha256_bytes;
use crate::tim::{
    parse_4bpp_prefix, read_indexed_cell_in_prefix, write_indexed_cell_in_prefix_with_report,
};

use super::glyph_slots::SOURCE_BLANK_GLYPH_INDEXED_SHA256;
use super::model::{
    BonusJBankReturnLabelFontSource, JBankReturnLabelGlyphAllocation, JBankReturnLabelGlyphPixels,
};
use super::source::{GLYPH_TIM_OFFSET, GLYPH_TIM_SIZE};

pub(super) struct GlyphApplication {
    pub(super) allowed_ranges: Vec<[usize; 2]>,
    pub(super) changed_byte_count: usize,
}

pub(super) fn rasterize_return_label_glyphs(
    font: &BonusJBankReturnLabelFontSource,
    allocations: &[JBankReturnLabelGlyphAllocation],
) -> Result<(Vec<JBankReturnLabelGlyphPixels>, Vec<RasterizedIndexedText>)> {
    let rasterizer = IndexedTextRasterizer::load(&font.path)?;
    let mut rasters = Vec::with_capacity(allocations.len());
    let mut glyphs = Vec::with_capacity(allocations.len());
    for allocation in allocations {
        let raster = rasterize_return_label_glyph(&rasterizer, font, allocation.text)?;
        ensure!(
            raster.pixels.len() == allocation.cell.width * allocation.cell.height,
            "J-BANK return-label glyph raster dimensions changed"
        );
        glyphs.push(JBankReturnLabelGlyphPixels {
            code: allocation.code,
            cell: allocation.cell,
            indexed_sha256: sha256_bytes(&raster.pixels),
            pixels: raster.pixels.clone(),
        });
        rasters.push(raster);
    }
    Ok((glyphs, rasters))
}

pub(super) fn apply_return_label_glyphs(
    inventory_decoded: &mut [u8],
    glyphs: &[JBankReturnLabelGlyphPixels],
) -> Result<Vec<GlyphApplication>> {
    let glyph_tim =
        parse_4bpp_prefix(inventory_decoded.get(GLYPH_TIM_OFFSET..).ok_or_else(|| {
            anyhow::anyhow!("KOUBAI1 J-BANK return-label glyph TIM disappeared")
        })?)?;
    ensure!(
        glyph_tim.total_size == GLYPH_TIM_SIZE
            && glyph_tim.pixel_width() == 1024
            && glyph_tim.image_height == 256
            && glyph_tim.image_x == 512
            && glyph_tim.image_y == 0,
        "KOUBAI1 J-BANK return-label glyph TIM geometry changed during composition"
    );

    let mut applications = Vec::with_capacity(glyphs.len());
    for glyph in glyphs {
        let source_pixels =
            read_indexed_cell_in_prefix(inventory_decoded, GLYPH_TIM_OFFSET, glyph.cell)?;
        ensure!(
            sha256_bytes(&source_pixels) == SOURCE_BLANK_GLYPH_INDEXED_SHA256,
            "J-BANK return-label glyph cell has another writer or changed source pixels"
        );
        ensure!(
            sha256_bytes(&glyph.pixels) == glyph.indexed_sha256,
            "J-BANK return-label glyph pixels differ from their build plan"
        );

        let write = write_indexed_cell_in_prefix_with_report(
            inventory_decoded,
            GLYPH_TIM_OFFSET,
            glyph.cell,
            &glyph.pixels,
        )?;
        ensure!(
            write.changed_byte_count > 0,
            "J-BANK return-label glyph changed no bytes"
        );
        let output_pixels =
            read_indexed_cell_in_prefix(inventory_decoded, GLYPH_TIM_OFFSET, glyph.cell)?;
        ensure!(
            sha256_bytes(&output_pixels) == glyph.indexed_sha256,
            "J-BANK return-label glyph readback differs from its build plan"
        );
        applications.push(GlyphApplication {
            allowed_ranges: write.allowed_ranges,
            changed_byte_count: write.changed_byte_count,
        });
    }
    Ok(applications)
}

fn rasterize_return_label_glyph(
    rasterizer: &IndexedTextRasterizer,
    font: &BonusJBankReturnLabelFontSource,
    text: char,
) -> Result<RasterizedIndexedText> {
    crate::bonus_inventory_source::rasterize_message_glyph(rasterizer, font, &text.to_string())
}
