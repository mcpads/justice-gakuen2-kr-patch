use super::atlas::{GLYPH_CELL_BYTE_COUNT, GLYPH_CELL_HEIGHT, GLYPH_CELL_WIDTH};
use super::preview::render_cell;

#[test]
fn contact_sheet_preserves_low_then_high_nibble_pixel_order() {
    let mut cell = vec![0u8; GLYPH_CELL_BYTE_COUNT];
    cell[0] = 0x21;
    let output_width = GLYPH_CELL_WIDTH * 3;
    let mut output = vec![0xff; output_width * GLYPH_CELL_HEIGHT * 3];

    render_cell(&cell, &mut output, output_width, 0, 0, 3);

    assert_eq!(output[0], 255 - 17);
    assert_eq!(output[3], 255 - 34);
    assert_eq!(output[6], 255);
}
