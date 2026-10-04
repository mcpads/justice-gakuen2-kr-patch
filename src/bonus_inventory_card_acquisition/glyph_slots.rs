use crate::source_disc::profile::BONUS_INVENTORY_BLANK_GLYPH_INDEXED_SHA256;
use crate::tim::Cell;

pub(super) const SOURCE_BLANK_GLYPH_INDEXED_SHA256: &str =
    BONUS_INVENTORY_BLANK_GLYPH_INDEXED_SHA256;

pub(super) const CARD_ACQUISITION_GLYPHS: [(&str, u16, Cell); 12] = [
    ("레", 0x0370, glyph_cell(0x0370)),
    ("어", 0x0371, glyph_cell(0x0371)),
    ("열", 0x0372, glyph_cell(0x0372)),
    ("혈", 0x0373, glyph_cell(0x0373)),
    ("카", 0x0374, glyph_cell(0x0374)),
    ("드", 0x0375, glyph_cell(0x0375)),
    ("획", 0x0376, glyph_cell(0x0376)),
    ("득", 0x0377, glyph_cell(0x0377)),
    ("했", 0x0378, glyph_cell(0x0378)),
    ("습", 0x0379, glyph_cell(0x0379)),
    ("니", 0x037a, glyph_cell(0x037a)),
    ("다", 0x037b, glyph_cell(0x037b)),
];

pub(super) const REUSED_SOURCE_GLYPHS: [(&str, u16, Cell, &str); 3] = [
    (
        "N",
        0x001b,
        glyph_cell(0x001b),
        "0420390c9560f3849aed3713afc67e6b5e3e7080146eae78516e96401fddd9fa",
    ),
    (
        "o",
        0x0042,
        glyph_cell(0x0042),
        "ab32a154c9359f38ddcd4c8e19dc75a83c8dee02378d9d39d1fe63ddfaf5b2fa",
    ),
    (
        ".",
        0x0199,
        glyph_cell(0x0199),
        "50ad69ef649151cd4d286db1dd6eca77cea07afb7183f6f1799e13dc5d5ac9bf",
    ),
];

pub(crate) fn reserved_glyph_codes() -> impl Iterator<Item = u16> {
    CARD_ACQUISITION_GLYPHS.iter().map(|(_, code, _)| *code)
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
