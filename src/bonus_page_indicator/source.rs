use anyhow::{Result, ensure};

pub(super) use crate::bonus_inventory_source::{
    BonusInventorySource as BonusPageIndicatorSource, load_bonus_inventory_source as load_source,
};
use crate::source_disc::profile::{
    BONUS_INVENTORY_GLYPH_TIM, BONUS_INVENTORY_OVERLAY_RECORD, BONUS_INVENTORY_RECORD,
};
use crate::tim::Cell;

pub const BONUS_PAGE_INDICATOR_INVENTORY_PATH: &str = BONUS_INVENTORY_RECORD.path;
pub const BONUS_PAGE_INDICATOR_OVERLAY_PATH: &str = BONUS_INVENTORY_OVERLAY_RECORD.path;
pub(super) const INVENTORY_SOURCE_STORED_SHA256: &str = BONUS_INVENTORY_RECORD.stored_sha256;
pub(super) const INVENTORY_SOURCE_DECODED_SHA256: &str = BONUS_INVENTORY_RECORD.decoded_sha256;
pub(super) const OVERLAY_SOURCE_SHA256: &str = BONUS_INVENTORY_OVERLAY_RECORD.sha256;
pub(super) const GLYPH_TIM_OFFSET: usize = BONUS_INVENTORY_GLYPH_TIM.offset;
pub(super) const GLYPH_TIM_SIZE: usize = BONUS_INVENTORY_GLYPH_TIM.size;
pub(super) const SOURCE_SUFFIX_GLYPH_CODE: u16 =
    crate::bonus_inventory_source::PAGE_SUFFIX_GLYPH_CODE;
pub(super) const SOURCE_SUFFIX_GLYPH_PACKED_SHA256: &str =
    crate::bonus_inventory_source::PAGE_SUFFIX_GLYPH_PACKED_SHA256;
pub(super) const SOURCE_SUFFIX_GLYPH_INDEXED_SHA256: &str =
    crate::bonus_inventory_source::PAGE_SUFFIX_GLYPH_INDEXED_SHA256;
pub(super) const SUFFIX_GLYPH_CELL: Cell = crate::bonus_inventory_source::PAGE_SUFFIX_GLYPH_CELL;

pub(crate) fn reserved_glyph_codes() -> impl Iterator<Item = u16> {
    std::iter::once(SOURCE_SUFFIX_GLYPH_CODE)
}

pub(super) fn pack_indexed_pixels(pixels: &[u8]) -> Result<Vec<u8>> {
    ensure!(
        pixels.len().is_multiple_of(2),
        "4-bpp indexed pixel count is not even"
    );
    ensure!(
        pixels.iter().all(|pixel| *pixel < 16),
        "indexed page-indicator glyph exceeds 4 bpp"
    );
    Ok(pixels
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| pair[0] | (pair[1] << 4))
        .collect())
}
