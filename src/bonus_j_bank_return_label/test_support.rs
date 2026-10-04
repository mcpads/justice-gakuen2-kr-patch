use super::glyph_slots::{GLYPH_CODES, glyph_cell};
use super::model::{JBankReturnLabelGlyphAllocation, JBankReturnLabelUnit};

pub(super) fn authored_unit() -> JBankReturnLabelUnit {
    serde_json::from_str(crate::test_input::read_str(
        "assets/menu/bonus-inventory/dynamic/j-bank-return-label/return-label.json",
    ))
    .unwrap()
}

pub(super) fn glyph_allocations() -> Vec<JBankReturnLabelGlyphAllocation> {
    authored_unit()
        .korean_text
        .as_deref()
        .unwrap()
        .chars()
        .zip(GLYPH_CODES)
        .map(|(text, code)| JBankReturnLabelGlyphAllocation {
            text,
            code,
            cell: glyph_cell(code),
        })
        .collect()
}

pub(super) fn source_overlay_fixture() -> Vec<u8> {
    let mut overlay = vec![0; 94_704];
    overlay[0x0100] = 0x81;
    for pointer_offset in (0x11e0..0x1418).step_by(4) {
        write_u32(&mut overlay, pointer_offset, 0x800a_2100);
    }
    overlay[0x0d78..0x0d84].copy_from_slice(&[
        0x00, 0x02, 0x0a, 0x01, 0x04, 0x00, 0x00, 0x08, 0x0a, 0x81, 0x00, 0x00,
    ]);
    write_u32(&mut overlay, 0x13b4, 0x800a_2d78);
    for (offset, word) in raw_consumer_words() {
        write_u32(&mut overlay, offset, word);
    }
    overlay
}

pub(super) fn raw_consumer_words() -> Vec<(usize, u32)> {
    vec![
        (0x95bc, 0x00a0_9021),
        (0x95cc, 0x00e0_b821),
        (0x9618, 0x9246_0000),
        (0x9634, 0x2402_0081),
        (0x9638, 0x30c3_00ff),
        (0x963c, 0x1062_007e),
        (0x9640, 0x2416_0080),
        (0x9644, 0x30c3_00ff),
        (0x9648, 0x1476_0015),
        (0x964c, 0x2402_0063),
        (0x9654, 0x2652_0001),
        (0x96a0, 0x1462_0004),
        (0x96a4, 0x0000_2021),
        (0x96a8, 0x2673_0014),
        (0x96ac, 0x0802_ae09),
        (0x96b0, 0x2652_0003),
        (0x96b4, 0x2652_0001),
        (0x979c, 0x9243_0000),
        (0x97a0, 0x2652_0001),
        (0x97b8, 0xa202_0014),
        (0x97bc, 0x9243_0000),
        (0x97e0, 0xa202_0015),
        (0x97ec, 0x2652_0001),
        (0x97fc, 0x2673_0014),
        (0x9824, 0x9246_0000),
        (0x9828, 0x2402_0081),
        (0x982c, 0x30c3_00ff),
        (0x9830, 0x1462_ff84),
        (0xca8c, 0x3c05_800a),
        (0xca90, 0x8ca5_33b4),
        (0xca94, 0x2407_0110),
        (0xca98, 0x0c02_ad6b),
        (0xca9c, 0xafa0_0010),
        (0xe4ac, 0x3c05_800a),
        (0xe4b0, 0x8ca5_33b4),
        (0xe4b4, 0x2407_0110),
        (0xe4b8, 0x0c02_ad6b),
        (0xe4bc, 0xafa0_0010),
    ]
}

pub(super) fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

pub(super) fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
