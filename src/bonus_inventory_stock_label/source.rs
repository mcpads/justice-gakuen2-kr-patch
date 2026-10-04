pub(super) use crate::bonus_inventory_source::{
    BonusInventorySource as BonusInventoryStockLabelSource,
    load_bonus_inventory_source as load_source,
};
use crate::source_disc::profile::{
    BONUS_INVENTORY_BLANK_GLYPH_INDEXED_SHA256, BONUS_INVENTORY_GLYPH_TIM,
    BONUS_INVENTORY_OVERLAY_RECORD, BONUS_INVENTORY_RECORD,
};
use crate::tim::Cell;

pub const BONUS_INVENTORY_STOCK_LABEL_INVENTORY_PATH: &str = BONUS_INVENTORY_RECORD.path;
pub const BONUS_INVENTORY_STOCK_LABEL_OVERLAY_PATH: &str = BONUS_INVENTORY_OVERLAY_RECORD.path;
pub(super) const INVENTORY_SOURCE_STORED_SHA256: &str = BONUS_INVENTORY_RECORD.stored_sha256;
pub(super) const INVENTORY_SOURCE_DECODED_SHA256: &str = BONUS_INVENTORY_RECORD.decoded_sha256;
pub(super) const OVERLAY_SOURCE_SHA256: &str = BONUS_INVENTORY_OVERLAY_RECORD.sha256;
pub(super) const GLYPH_TIM_OFFSET: usize = BONUS_INVENTORY_GLYPH_TIM.offset;
pub(super) const GLYPH_TIM_SIZE: usize = BONUS_INVENTORY_GLYPH_TIM.size;
pub(super) const SOURCE_BLANK_GLYPH_INDEXED_SHA256: &str =
    BONUS_INVENTORY_BLANK_GLYPH_INDEXED_SHA256;

// The first source selector doubles as the loop discriminator, so its column must remain 1.
// Row 4 provides two source-blank, unreferenced cells without rewriting that control flow.
pub(super) const STOCK_LABEL_GLYPHS: [(&str, u16, Cell); 2] = [
    ("소", 0x0341, glyph_cell(0x0341)),
    ("지", 0x034a, glyph_cell(0x034a)),
];

pub(crate) fn reserved_glyph_codes() -> impl Iterator<Item = u16> {
    STOCK_LABEL_GLYPHS.iter().map(|(_, code, _)| *code)
}

pub(super) const fn glyph_cell(code: u16) -> Cell {
    let page = (code >> 8) as usize;
    let column = (code & 0x000f) as usize;
    let row = ((code >> 4) & 0x000f) as usize;
    Cell {
        x: page * 256 + ((column * 20) & 0xff),
        y: (row * 20) & 0xff,
        width: 20,
        height: 20,
    }
}
