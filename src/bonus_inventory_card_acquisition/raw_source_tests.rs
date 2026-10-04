use std::path::Path;

use crate::pipeline::sha256_bytes;

#[test]
#[ignore = "requires the user-supplied supported source disc"]
fn supported_source_matches_independent_raw_records_pointers_and_caller_words() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cue = crate::cue::CueSheet::parse(
        &root.join("roms/Shiritsu Justice Gakuen - Nekketsu Seishun Nikki 2 (Japan).cue"),
    )
    .unwrap();
    let (_, overlay) =
        crate::disc::rebuild::read_record(&cue.image_path, "DAT1/KOUBAI2.BIN").unwrap();

    assert_eq!(overlay.len(), 94_704);
    assert_eq!(
        sha256_bytes(&overlay),
        "9c1e0b2f226e3c16dc4622068477ff77ab97ecc738db1273680d391f3fa4e393"
    );

    const RARE_LABEL: [u8; 8] = [0x01, 0x05, 0x05, 0x01, 0x00, 0x02, 0x81, 0x00];
    const CARD_NUMBER_PREFIX: [u8; 28] = [
        0x01, 0x00, 0x0a, 0x01, 0x01, 0x0a, 0x01, 0x05, 0x02, 0x00, 0x04, 0x06, 0x01, 0x01, 0x07,
        0x00, 0x0b, 0x01, 0x00, 0x02, 0x04, 0x01, 0x09, 0x09, 0x81, 0x00, 0x00, 0x00,
    ];
    const ACQUIRED_MESSAGE: [u8; 28] = [
        0x00, 0x00, 0x0b, 0x02, 0x01, 0x00, 0x00, 0x01, 0x09, 0x02, 0x02, 0x00, 0x00, 0x09, 0x0a,
        0x00, 0x0a, 0x09, 0x00, 0x03, 0x08, 0x00, 0x07, 0x08, 0x81, 0x00, 0x00, 0x00,
    ];
    for (offset, expected, expected_sha256) in [
        (
            0x102c,
            RARE_LABEL.as_slice(),
            "f059432d64fa6739cf2bb5ed3072ddc6c7863b706604c94ef6ba620e38dda324",
        ),
        (
            0x1034,
            CARD_NUMBER_PREFIX.as_slice(),
            "4167bf8d5c656997ff9dc86d2aef52b1d946f26afff7675fba019b68974d60aa",
        ),
        (
            0x1050,
            ACQUIRED_MESSAGE.as_slice(),
            "cf7f1b758d870f9e468f61ce78faf4bf954aff41114844abd76a24996dbbc7df",
        ),
    ] {
        let actual = &overlay[offset..offset + expected.len()];
        assert_eq!(
            actual, expected,
            "raw source record changed at +0x{offset:04x}"
        );
        assert_eq!(sha256_bytes(actual), expected_sha256);
    }

    for (offset, expected) in [
        (0x13f4, 0x800a_302c),
        (0x13f8, 0x800a_3034),
        (0x13fc, 0x800a_3050),
        (0x1400, 0x800a_306c),
    ] {
        assert_eq!(read_u32(&overlay, offset), expected);
    }

    for (offset, expected) in [
        (0xd328, 0x0000_3021),
        (0xd32c, 0x2407_0030),
        (0xd334, 0x3c05_800a),
        (0xd338, 0x8ca5_33f4),
        (0xd344, 0x0c02_acdd),
        (0xd348, 0xafb3_0010),
        (0xd350, 0x2406_0001),
        (0xd354, 0x0280_3821),
        (0xd35c, 0x3c02_800a),
        (0xd360, 0x2442_33f8),
        (0xd368, 0x8c45_0000),
        (0xd370, 0x0c02_acdd),
        (0xd374, 0xafb3_0010),
        (0xd3a4, 0x0c02_c148),
        (0xd3f0, 0x2406_0002),
        (0xd3f4, 0x2407_0030),
        (0xd400, 0x3c05_800a),
        (0xd404, 0x8ca5_33f4),
        (0xd410, 0x0c02_acdd),
        (0xd414, 0xafb3_0010),
        (0xd41c, 0x2406_0003),
        (0xd420, 0x3c05_800a),
        (0xd424, 0x8ca5_33f8),
        (0xd428, 0x2407_0058),
        (0xd42c, 0x0c02_acdd),
        (0xd430, 0xafb3_0010),
        (0xd464, 0x2673_0014),
        (0xd468, 0x2406_0004),
        (0xd46c, 0x3c05_800a),
        (0xd470, 0x8ca5_33fc),
        (0xd474, 0x2407_0030),
        (0xd478, 0x0c02_acdd),
        (0xd47c, 0xafb3_0010),
    ] {
        assert_eq!(
            read_u32(&overlay, offset),
            expected,
            "raw caller instruction changed at +0x{offset:04x}"
        );
    }
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}
