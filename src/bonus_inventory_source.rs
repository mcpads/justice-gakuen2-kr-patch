use crate::font::{HorizontalTextAlignment, IndexedTextRasterizer, RasterizedIndexedText};
use anyhow::{Result, ensure};

use crate::compression::decompress;
use crate::pipeline::sha256_bytes;
use crate::source_disc::{
    SupportedSourceDisc,
    profile::{BONUS_INVENTORY_GLYPH_TIM, BONUS_INVENTORY_OVERLAY_RECORD, BONUS_INVENTORY_RECORD},
};
use crate::tim::{Cell, parse_4bpp_prefix, read_indexed_cell_in_prefix};

pub(crate) const FIXED_UI_TIM_OFFSET: usize = 0;
const FIXED_UI_TIM_SIZE: usize = 0x20200;
pub(crate) const PAGE_SUFFIX_GLYPH_CODE: u16 = 0x017a;
pub(crate) const PAGE_SUFFIX_GLYPH_PACKED_SHA256: &str =
    "5817da73d1329dab7bd0a08c02f17aba3314bfe252e4f0b47240a13b79655b72";
pub(crate) const PAGE_SUFFIX_GLYPH_INDEXED_SHA256: &str =
    "1903c9362cd739198e3a06f02ad98a90c288b0e74cec3e86652d2a053f45ec45";
pub(crate) const PAGE_SUFFIX_GLYPH_CELL: Cell = Cell {
    x: 456,
    y: 140,
    width: 20,
    height: 20,
};

// Fixed/derived consumers own cells even when the original pointer tables do
// not reference them. In particular, a reserved blank is a live text supplier.
pub(crate) fn reserved_dynamic_glyph_codes() -> impl Iterator<Item = u16> {
    crate::bonus_page_indicator::bonus_page_indicator_reserved_glyph_codes()
        .chain(crate::bonus_inventory_action_labels::bonus_action_label_reserved_glyph_codes())
        .chain(crate::bonus_confirmation::bonus_confirmation_reserved_glyph_codes())
        .chain(crate::bonus_inventory_stock_label::bonus_stock_label_reserved_glyph_codes())
        .chain(
            crate::bonus_inventory_card_acquisition::bonus_card_acquisition_reserved_glyph_codes(),
        )
        .chain(
            crate::bonus_inventory_memory_card_swap::bonus_memory_card_swap_reserved_glyph_codes(),
        )
        .chain(crate::bonus_j_bank_return_label::bonus_j_bank_return_label_reserved_glyph_codes())
}

pub(crate) struct BonusInventorySource {
    pub(crate) source_bin_sha256: String,
    pub(crate) inventory_stored: Vec<u8>,
    pub(crate) inventory_decoded: Vec<u8>,
    pub(crate) overlay: Vec<u8>,
}

pub(crate) fn load_bonus_inventory_source(
    source: &SupportedSourceDisc,
) -> Result<BonusInventorySource> {
    let (_, inventory_stored) = source.read_record(BONUS_INVENTORY_RECORD.path)?;
    ensure!(
        sha256_bytes(&inventory_stored) == BONUS_INVENTORY_RECORD.stored_sha256,
        "KOUBAI1.TIZ stored source identity changed"
    );
    let inventory_decoded = decompress(&inventory_stored, false)?;
    ensure!(
        inventory_decoded.len() == BONUS_INVENTORY_RECORD.decoded_size
            && sha256_bytes(&inventory_decoded) == BONUS_INVENTORY_RECORD.decoded_sha256,
        "KOUBAI1.TIZ decoded source identity changed"
    );
    validate_inventory_textures(&inventory_decoded)?;

    let (_, overlay) = source.read_record(BONUS_INVENTORY_OVERLAY_RECORD.path)?;
    ensure!(
        overlay.len() == BONUS_INVENTORY_OVERLAY_RECORD.size
            && sha256_bytes(&overlay) == BONUS_INVENTORY_OVERLAY_RECORD.sha256,
        "KOUBAI2.BIN source identity changed"
    );

    Ok(BonusInventorySource {
        source_bin_sha256: source.source_bin_sha256().to_string(),
        inventory_stored,
        inventory_decoded,
        overlay,
    })
}

fn validate_inventory_textures(decoded: &[u8]) -> Result<()> {
    let fixed_ui = parse_4bpp_prefix(decoded)?;
    ensure!(
        fixed_ui.total_size == FIXED_UI_TIM_SIZE
            && fixed_ui.pixel_width() == 1024
            && fixed_ui.image_height == 256,
        "KOUBAI1 fixed-UI TIM geometry changed"
    );
    let glyphs = parse_4bpp_prefix(
        decoded
            .get(BONUS_INVENTORY_GLYPH_TIM.offset..)
            .ok_or_else(|| anyhow::anyhow!("KOUBAI1 glyph TIM disappeared"))?,
    )?;
    ensure!(
        glyphs.total_size == BONUS_INVENTORY_GLYPH_TIM.size
            && glyphs.pixel_width() == BONUS_INVENTORY_GLYPH_TIM.pixel_width
            && glyphs.image_height == BONUS_INVENTORY_GLYPH_TIM.height
            && glyphs.image_x == BONUS_INVENTORY_GLYPH_TIM.image_x
            && glyphs.image_y == BONUS_INVENTORY_GLYPH_TIM.image_y,
        "KOUBAI1 glyph TIM geometry changed"
    );

    let suffix_pixels = read_indexed_cell_in_prefix(
        decoded,
        BONUS_INVENTORY_GLYPH_TIM.offset,
        PAGE_SUFFIX_GLYPH_CELL,
    )?;
    ensure!(
        sha256_bytes(&suffix_pixels) == PAGE_SUFFIX_GLYPH_INDEXED_SHA256
            && sha256_bytes(&pack_indexed_pixels(&suffix_pixels)?)
                == PAGE_SUFFIX_GLYPH_PACKED_SHA256,
        "KOUBAI1 page-indicator suffix glyph changed"
    );
    Ok(())
}

fn pack_indexed_pixels(pixels: &[u8]) -> Result<Vec<u8>> {
    ensure!(
        pixels.len().is_multiple_of(2) && pixels.iter().all(|pixel| *pixel < 16),
        "KOUBAI1 indexed glyph is not valid 4-bpp data"
    );
    Ok(pixels
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| pair[0] | (pair[1] << 4))
        .collect())
}

pub(crate) fn rasterize_message_glyph(
    rasterizer: &IndexedTextRasterizer,
    font: &crate::bonus_inventory::BonusInventoryTextStyleSource,
    text: &str,
) -> Result<RasterizedIndexedText> {
    let raster = rasterizer.rasterize_shifted_with_coverage_ramp(
        text,
        20,
        20,
        font.font_px,
        font.tracking_px,
        0,
        0,
        // Native message CLUTs use 2..=14 for coverage. Slots 1 and 15
        // are accent colors, not endpoints of that ramp (CLUT 6: yellow
        // at 14, blue at 15). White CLUT 0 hides an incorrect endpoint.
        2,
        14,
        HorizontalTextAlignment::Center,
    )?;
    shift_raster_vertically(raster, font.vertical_shift_px)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires the local Galmuri font"]
    fn message_ink_preserves_yellow_continuation_palette() {
        let font = crate::bonus_inventory::BonusInventoryTextStyleSource {
            path: std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../fonts/galmuri/Galmuri14.ttf"),
            font_px: 15.0,
            tracking_px: 0.0,
            vertical_shift_px: 0,
        };
        let rasterizer = IndexedTextRasterizer::load(&font.path).unwrap();
        for text in ["아", "무", "버", "튼", "눌", "요"] {
            let raster = rasterize_message_glyph(&rasterizer, &font, text).unwrap();
            assert!(raster.pixels.contains(&14), "{text}: no full yellow ink");
            assert!(
                raster
                    .pixels
                    .iter()
                    .all(|&index| index == 0 || (2..=14).contains(&index)),
                "{text}: message coverage consumes a palette accent"
            );
        }
    }
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
                "bonus message glyph clips after vertical shift {shift_px}"
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
    ensure!(has_ink, "bonus message glyph has no shifted ink");
    raster.pixels = pixels;
    raster.ink_bounds = ink_bounds;
    Ok(raster)
}
