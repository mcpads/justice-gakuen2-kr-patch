use crate::bonus_shop_source;
use crate::tim::Cell;

pub(super) const SOURCE_BLANK_GLYPH_INDEXED_SHA256: &str =
    "7a12e561363385e9dfeeab326368731c030ed4b374e7f5897ac819159d2884c5";
pub(super) const QUESTION_MARK_CODE: u16 = 0x0055;
pub(super) const QUESTION_MARK_SOURCE_INDEXED_SHA256: &str =
    "59beca3f4eab78f669570e5f99e1e77c35ff027041c00e02261d056434336f9b";
pub(super) const GLYPH_CODES: [u16; 12] = [
    0x0390, 0x0391, 0x0392, 0x0393, 0x0394, 0x0395, 0x0396, 0x0397, 0x0398, 0x0399, 0x039a, 0x039b,
];
pub(super) const EXPECTED_PHYSICAL_ALIAS_CODES: [u16; 15] = [
    0x0390, 0x0391, 0x0392, 0x0393, 0x0394, 0x0395, 0x0396, 0x0397, 0x0398, 0x0399, 0x039a, 0x039b,
    0x039d, 0x039e, 0x039f,
];

pub(crate) fn reserved_glyph_codes() -> impl Iterator<Item = u16> {
    GLYPH_CODES.into_iter()
}

pub(super) const fn glyph_cell(code: u16) -> Cell {
    bonus_shop_source::glyph_cell(code)
}
