use crate::source_disc::profile::BONUS_INVENTORY_BLANK_GLYPH_INDEXED_SHA256;
use crate::tim::Cell;

pub(super) const SOURCE_BLANK_GLYPH_INDEXED_SHA256: &str =
    BONUS_INVENTORY_BLANK_GLYPH_INDEXED_SHA256;
pub(super) const GLYPH_CODES: [u16; 2] = [0x03b3, 0x03b5];

pub(crate) fn reserved_glyph_codes() -> impl Iterator<Item = u16> {
    GLYPH_CODES.into_iter()
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
