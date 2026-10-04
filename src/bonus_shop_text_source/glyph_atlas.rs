use anyhow::{Context, Result, ensure};

use super::glyph_allocation::BonusShopGlyphAllocation;
use super::model::BonusShopTextFontSources;
use crate::bonus_shop_source::{GLYPH_TIM_OFFSET, validate_tim_geometry};
use crate::font::{
    HorizontalTextAlignment, IndexedTextRasterizer, IndexedTextRasterizers, RasterizedIndexedText,
};
use crate::pipeline::sha256_bytes;
use crate::tim::{
    Cell, read_4bpp_palette_words_in_prefix, read_indexed_cell_in_prefix,
    write_indexed_cell_in_prefix_with_report,
};

// The same glyphs use white and gold CLUTs. Index 15 in the gold CLUT is
// a blue marker, not the bright endpoint of its text ramp.
const OUTLINE_INDEX: u8 = 2;
const FILL_INDEX: u8 = 14;

pub(super) struct BonusShopGlyphPixels {
    pub(super) code: u16,
    pub(super) cell: Cell,
    pub(super) pixels: Vec<u8>,
    pub(super) indexed_sha256: String,
    pub(super) source_indexed_sha256: String,
}

pub(super) struct BonusShopGlyphApplication {
    pub(super) allowed_ranges: Vec<[usize; 2]>,
    pub(super) changed_byte_count: usize,
}

pub(super) fn rasterize_bonus_shop_glyphs(
    fonts: &BonusShopTextFontSources,
    allocations: &[BonusShopGlyphAllocation],
) -> Result<(Vec<BonusShopGlyphPixels>, Vec<RasterizedIndexedText>)> {
    let mut glyphs = Vec::with_capacity(allocations.len());
    let mut rasters = Vec::with_capacity(allocations.len());
    let mut rasterizers = IndexedTextRasterizers::default();
    for allocation in allocations {
        let style = fonts.for_role(allocation.font_role);
        let raster = rasterize_glyph(
            rasterizers.for_font(&style.path)?,
            style,
            allocation.character,
        )
        .with_context(|| {
            format!(
                "failed to rasterize bonus-shop {:?} glyph {:?}",
                allocation.font_role, allocation.character
            )
        })?;
        ensure!(
            raster.pixels.len() == allocation.cell.width * allocation.cell.height,
            "bonus-shop glyph raster dimensions changed"
        );
        glyphs.push(BonusShopGlyphPixels {
            code: allocation.code,
            cell: allocation.cell,
            indexed_sha256: sha256_bytes(&raster.pixels),
            source_indexed_sha256: allocation.source_indexed_sha256.clone(),
            pixels: raster.pixels.clone(),
        });
        rasters.push(raster);
    }
    Ok((glyphs, rasters))
}

pub(super) fn apply_bonus_shop_glyphs(
    shop_ui_decoded: &mut [u8],
    glyphs: &[BonusShopGlyphPixels],
) -> Result<Vec<BonusShopGlyphApplication>> {
    validate_tim_geometry(shop_ui_decoded)?;
    for (palette_index, outline, fill) in [(0, 0x9084, 0xffff), (1, 0x8422, 0x86ff)] {
        let palette =
            read_4bpp_palette_words_in_prefix(shop_ui_decoded, GLYPH_TIM_OFFSET, palette_index)?;
        ensure!(
            palette[0] == 0
                && palette[usize::from(OUTLINE_INDEX)] == outline
                && palette[usize::from(FILL_INDEX)] == fill,
            "bonus-shop white/gold text palette roles changed"
        );
    }
    let mut applications = Vec::with_capacity(glyphs.len());
    for glyph in glyphs {
        let source_pixels =
            read_indexed_cell_in_prefix(shop_ui_decoded, GLYPH_TIM_OFFSET, glyph.cell)?;
        ensure!(
            sha256_bytes(&source_pixels) == glyph.source_indexed_sha256,
            "bonus-shop glyph cell 0x{:04x} has another writer or changed source pixels",
            glyph.code
        );
        ensure!(
            sha256_bytes(&glyph.pixels) == glyph.indexed_sha256,
            "bonus-shop glyph pixels differ from their build plan"
        );

        let write = write_indexed_cell_in_prefix_with_report(
            shop_ui_decoded,
            GLYPH_TIM_OFFSET,
            glyph.cell,
            &glyph.pixels,
        )?;
        ensure!(
            write.changed_byte_count > 0,
            "bonus-shop glyph changed no bytes"
        );
        let output_pixels =
            read_indexed_cell_in_prefix(shop_ui_decoded, GLYPH_TIM_OFFSET, glyph.cell)?;
        ensure!(
            sha256_bytes(&output_pixels) == glyph.indexed_sha256,
            "bonus-shop glyph readback differs from its build plan"
        );
        applications.push(BonusShopGlyphApplication {
            allowed_ranges: write.allowed_ranges,
            changed_byte_count: write.changed_byte_count,
        });
    }
    Ok(applications)
}

fn rasterize_glyph(
    rasterizer: &IndexedTextRasterizer,
    font: &super::model::BonusShopTextFontSource,
    text: char,
) -> Result<RasterizedIndexedText> {
    ensure!(
        font.font_px.is_finite()
            && font.font_px > 0.0
            && font.tracking_px.is_finite()
            && (-19..=19).contains(&font.vertical_shift_px),
        "bonus-shop font settings are invalid"
    );
    rasterizer.rasterize_shifted(
        &text.to_string(),
        20,
        20,
        font.font_px,
        font.tracking_px,
        font.vertical_shift_px,
        0,
        Some(OUTLINE_INDEX),
        FILL_INDEX,
        HorizontalTextAlignment::Center,
    )
}
