use std::collections::BTreeMap;

use super::command_sequence::RECORD_SPECS;
use super::model::{ShopExitGlyphAllocation, ShopExitManifest, ShopExitTextUnit};

pub(super) fn manifest() -> ShopExitManifest {
    serde_json::from_str(crate::test_input::read_str(
        "assets/menu/shop-ui/dynamic/exit-confirmation/manifest.json",
    ))
    .unwrap()
}

pub(super) fn authored_units() -> Vec<ShopExitTextUnit> {
    [
        crate::test_input::read_str(
            "assets/menu/shop-ui/dynamic/exit-confirmation/woman-prompt.json",
        ),
        crate::test_input::read_str("assets/menu/shop-ui/dynamic/exit-confirmation/woman-yes.json"),
        crate::test_input::read_str("assets/menu/shop-ui/dynamic/exit-confirmation/woman-no.json"),
        crate::test_input::read_str(
            "assets/menu/shop-ui/dynamic/exit-confirmation/elder-prompt.json",
        ),
        crate::test_input::read_str("assets/menu/shop-ui/dynamic/exit-confirmation/elder-yes.json"),
        crate::test_input::read_str("assets/menu/shop-ui/dynamic/exit-confirmation/elder-no.json"),
    ]
    .into_iter()
    .map(|json| serde_json::from_str(json).unwrap())
    .collect()
}

pub(super) fn glyph_allocations() -> Vec<ShopExitGlyphAllocation> {
    let units = authored_units();
    let unit_by_id = units
        .iter()
        .map(|unit| (unit.id.as_str(), unit))
        .collect::<BTreeMap<_, _>>();
    manifest()
        .glyphs
        .into_iter()
        .map(|asset| {
            let text = unit_by_id[asset.unit_id.as_str()]
                .korean_text
                .as_deref()
                .unwrap()
                .chars()
                .nth(asset.text_index)
                .unwrap();
            let code = u16::from_str_radix(asset.code.strip_prefix("0x").unwrap(), 16).unwrap();
            ShopExitGlyphAllocation {
                text,
                code,
                cell: asset.cell,
            }
        })
        .collect()
}

pub(super) fn source_overlay_fixture() -> Vec<u8> {
    let mut overlay = vec![0; 0x90c0];
    overlay[0x0100] = 0x81;
    overlay[0x25a4] = 0x81;
    overlay[0x2e08] = 0x81;
    for pointer_offset in (0x2458..0x25a4).step_by(4) {
        write_u32(&mut overlay, pointer_offset, 0x800a_2100);
    }
    for pointer_offset in (0x2cbc..0x2e08).step_by(4) {
        write_u32(&mut overlay, pointer_offset, 0x800a_45a4);
    }
    for pointer_offset in (0x371c..0x376c).step_by(4) {
        write_u32(&mut overlay, pointer_offset, 0x800a_4e08);
    }
    for spec in RECORD_SPECS {
        overlay[spec.record_offset..spec.record_offset + spec.source_record.len()]
            .copy_from_slice(spec.source_record);
        write_u32(
            &mut overlay,
            spec.pointer_storage_offset,
            0x800a_2000 + spec.record_offset as u32,
        );
    }
    for (offset, word) in raw_consumer_words() {
        write_u32(&mut overlay, offset, word);
    }
    overlay
}

pub(super) fn raw_consumer_words() -> Vec<(usize, u32)> {
    vec![
        (0x59e8, 0x0220_2021),
        (0x59ec, 0x9083_3b98),
        (0x59f0, 0x0000_3021),
        (0x59f4, 0xafa0_0010),
        (0x59f8, 0x0003_1080),
        (0x59fc, 0x0043_1021),
        (0x5a00, 0x0002_10c0),
        (0x5a04, 0x3c01_800a),
        (0x5a08, 0x0022_0821),
        (0x5a0c, 0x8c25_572c),
        (0x5a10, 0x0c02_a43f),
        (0x5a14, 0x0000_3821),
        (0x5a18, 0x0802_9e96),
        (0x70fc, 0x27bd_ffc8),
        (0x7110, 0x00a0_9021),
        (0x7140, 0x9246_0000),
        (0x7144, 0x2402_0081),
        (0x7148, 0x30c3_00ff),
        (0x714c, 0x1062_0062),
        (0x7150, 0x2416_0080),
        (0x7158, 0x30c3_00ff),
        (0x715c, 0x1476_0005),
        (0x7160, 0x2402_0063),
        (0x7164, 0x2694_001a),
        (0x7168, 0x2413_0050),
        (0x716c, 0x0802_a4b1),
        (0x7170, 0x2652_0001),
        (0x7174, 0x1462_0004),
        (0x717c, 0x2673_0014),
        (0x7180, 0x0802_a4b1),
        (0x7184, 0x2652_0003),
        (0x7188, 0x2652_0001),
        (0x7190, 0x2466_0009),
        (0x7194, 0x0006_3180),
        (0x723c, 0x9243_0000),
        (0x7240, 0x2652_0001),
        (0x724c, 0x0003_1080),
        (0x7250, 0x0043_1021),
        (0x7254, 0x0002_1080),
        (0x7258, 0xa202_0014),
        (0x725c, 0x9243_0000),
        (0x7274, 0x0003_1080),
        (0x7278, 0x0043_1021),
        (0x727c, 0x0002_1080),
        (0x7280, 0xa202_0015),
        (0x728c, 0x2652_0001),
        (0x729c, 0x2673_0014),
        (0x72c4, 0x9246_0000),
        (0x72c8, 0x2402_0081),
        (0x72cc, 0x30c3_00ff),
        (0x72d0, 0x1462_ffa1),
    ]
}

pub(super) fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

pub(super) fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
