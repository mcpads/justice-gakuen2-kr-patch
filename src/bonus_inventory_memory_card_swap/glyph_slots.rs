use crate::source_disc::profile::BONUS_INVENTORY_BLANK_GLYPH_INDEXED_SHA256;
use crate::tim::Cell;

pub(super) const SOURCE_BLANK_GLYPH_INDEXED_SHA256: &str =
    BONUS_INVENTORY_BLANK_GLYPH_INDEXED_SHA256;

pub(super) const MEMORY_CARD_SWAP_GLYPHS: [(&str, u16, Cell); 20] = [
    ("메", 0x0380, glyph_cell(0x0380)),
    ("모", 0x0381, glyph_cell(0x0381)),
    ("리", 0x0382, glyph_cell(0x0382)),
    ("카", 0x0383, glyph_cell(0x0383)),
    ("드", 0x0384, glyph_cell(0x0384)),
    ("를", 0x0385, glyph_cell(0x0385)),
    ("바", 0x0386, glyph_cell(0x0386)),
    ("꾼", 0x0387, glyph_cell(0x0387)),
    ("뒤", 0x0388, glyph_cell(0x0388)),
    ("버", 0x0389, glyph_cell(0x0389)),
    ("튼", 0x038a, glyph_cell(0x038a)),
    ("을", 0x038b, glyph_cell(0x038b)),
    ("눌", 0x0390, glyph_cell(0x0390)),
    ("러", 0x0391, glyph_cell(0x0391)),
    ("주", 0x0392, glyph_cell(0x0392)),
    ("세", 0x0393, glyph_cell(0x0393)),
    ("요", 0x0394, glyph_cell(0x0394)),
    ("원", 0x0395, glyph_cell(0x0395)),
    ("래", 0x0396, glyph_cell(0x0396)),
    ("로", 0x0397, glyph_cell(0x0397)),
];

// Both original prompts accept any button; no Circle glyph is required.
pub(super) const REUSED_SOURCE_GLYPHS: [(&str, u16, Cell, &str); 0] = [];

pub(crate) fn reserved_glyph_codes() -> impl Iterator<Item = u16> {
    MEMORY_CARD_SWAP_GLYPHS.iter().map(|(_, code, _)| *code)
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
