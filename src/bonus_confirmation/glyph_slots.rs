use crate::source_disc::profile::BONUS_INVENTORY_BLANK_GLYPH_INDEXED_SHA256;
use crate::tim::Cell;

pub(super) const SOURCE_BLANK_GLYPH_INDEXED_SHA256: &str =
    BONUS_INVENTORY_BLANK_GLYPH_INDEXED_SHA256;

pub(super) const CONFIRMATION_GLYPHS: [(&str, u16, Cell); 18] = [
    ("종", 0x0331, glyph_cell(0x0331)),
    ("료", 0x0332, glyph_cell(0x0332)),
    ("할", 0x0333, glyph_cell(0x0333)),
    ("까", 0x0334, glyph_cell(0x0334)),
    ("요", 0x0335, glyph_cell(0x0335)),
    ("예", 0x0336, glyph_cell(0x0336)),
    ("아", 0x0337, glyph_cell(0x0337)),
    ("니", 0x0338, glyph_cell(0x0338)),
    ("이", 0x0351, glyph_cell(0x0351)),
    ("카", 0x0352, glyph_cell(0x0352)),
    ("드", 0x0353, glyph_cell(0x0353)),
    ("로", 0x0354, glyph_cell(0x0354)),
    ("메", 0x0355, glyph_cell(0x0355)),
    ("모", 0x0356, glyph_cell(0x0356)),
    ("리", 0x0357, glyph_cell(0x0357)),
    ("에", 0x0358, glyph_cell(0x0358)),
    ("복", 0x0359, glyph_cell(0x0359)),
    ("사", 0x035a, glyph_cell(0x035a)),
];

pub(super) const FIXED_RECORD_SPACE_CODE: u16 = 0x035b;
pub(super) const FIXED_RECORD_SPACE_CELL: Cell = glyph_cell(FIXED_RECORD_SPACE_CODE);

pub(crate) fn reserved_glyph_codes() -> impl Iterator<Item = u16> {
    CONFIRMATION_GLYPHS
        .iter()
        .map(|(_, code, _)| *code)
        .chain([FIXED_RECORD_SPACE_CODE])
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
