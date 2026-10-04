use std::path::Path;

use crate::compression::decompress;
use crate::pipeline::sha256_bytes;
use crate::tim::{Cell, read_indexed_cell_in_prefix};

#[test]
#[ignore = "requires the user-supplied supported source disc"]
fn supported_source_matches_independent_records_pointers_callers_and_renderer_words() {
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

    const REPLACE: [u8; 80] = [
        0x01, 0x09, 0x04, 0x01, 0x0a, 0x04, 0x01, 0x03, 0x05, 0x00, 0x04, 0x06, 0x01, 0x05, 0x02,
        0x00, 0x04, 0x06, 0x01, 0x01, 0x07, 0x00, 0x00, 0x0b, 0x00, 0x02, 0x08, 0x00, 0x03, 0x08,
        0x00, 0x09, 0x07, 0x00, 0x07, 0x07, 0x00, 0x0a, 0x08, 0x80, 0x02, 0x07, 0x03, 0x00, 0x09,
        0x07, 0x01, 0x06, 0x07, 0x01, 0x03, 0x03, 0x01, 0x09, 0x05, 0x00, 0x00, 0x0b, 0x02, 0x04,
        0x0b, 0x00, 0x03, 0x08, 0x00, 0x0a, 0x08, 0x00, 0x0b, 0x07, 0x01, 0x00, 0x00, 0x00, 0x02,
        0x08, 0x00, 0x05, 0x07, 0x81,
    ];
    const RESTORE: [u8; 88] = [
        0x00, 0x02, 0x0a, 0x00, 0x0b, 0x08, 0x00, 0x04, 0x09, 0x01, 0x09, 0x04, 0x01, 0x0a, 0x04,
        0x01, 0x03, 0x05, 0x00, 0x04, 0x06, 0x01, 0x05, 0x02, 0x00, 0x04, 0x06, 0x01, 0x01, 0x07,
        0x00, 0x01, 0x09, 0x00, 0x02, 0x0a, 0x01, 0x04, 0x00, 0x00, 0x03, 0x08, 0x00, 0x0a, 0x08,
        0x80, 0x02, 0x07, 0x03, 0x00, 0x09, 0x07, 0x01, 0x06, 0x07, 0x01, 0x03, 0x03, 0x01, 0x09,
        0x05, 0x00, 0x00, 0x0b, 0x02, 0x04, 0x0b, 0x00, 0x03, 0x08, 0x00, 0x0a, 0x08, 0x00, 0x0b,
        0x07, 0x01, 0x00, 0x00, 0x00, 0x02, 0x08, 0x00, 0x05, 0x07, 0x81, 0x00, 0x00,
    ];
    for (offset, expected, expected_sha256, expected_counts) in [
        (
            0x1138,
            REPLACE.as_slice(),
            "1faa3655d9c3110ac23ac503f6b57ab6cf882a3c29896465538b759ff5efe167",
            vec![13, 13],
        ),
        (
            0x1188,
            RESTORE.as_slice(),
            "bf32a6461b275e17a02a8794bec172be142bfa47b33dd55db4a86937586687a0",
            vec![15, 13],
        ),
    ] {
        let actual = &overlay[offset..offset + expected.len()];
        assert_eq!(actual, expected);
        assert_eq!(sha256_bytes(actual), expected_sha256);
        assert_eq!(raw_line_command_counts(actual), expected_counts);
    }
    assert_eq!(REPLACE.iter().rposition(|byte| *byte == 0x81), Some(79));
    assert_eq!(RESTORE.iter().rposition(|byte| *byte == 0x81), Some(85));
    assert_eq!(&RESTORE[86..], &[0, 0]);

    for (offset, expected) in [
        (0x1410, 0x800a_3138),
        (0x1414, 0x800a_3188),
        (0x1418, 0x0201_0009),
    ] {
        assert_eq!(read_u32(&overlay, offset), expected);
    }
    for (offset, expected) in [
        (0x95bc, 0x00a0_9021),
        (0x95cc, 0x00e0_b821),
        (0x9618, 0x9246_0000),
        (0x961c, 0x0004_1880),
        (0x9620, 0x0064_1821),
        (0x9624, 0x0003_1880),
        (0x9628, 0x2402_0200),
        (0x962c, 0x0043_1023),
        (0x9630, 0x0002_9843),
        (0x9634, 0x2402_0081),
        (0x9638, 0x30c3_00ff),
        (0x963c, 0x1062_007e),
        (0x9640, 0x2416_0080),
        (0x9644, 0x30c3_00ff),
        (0x9648, 0x1476_0015),
        (0x964c, 0x2402_0063),
        (0x9650, 0x26f7_001a),
        (0x9654, 0x2652_0001),
        (0x96a0, 0x1462_0004),
        (0x96a4, 0x0000_2021),
        (0x96a8, 0x2673_0014),
        (0x96ac, 0x0802_ae09),
        (0x96b0, 0x2652_0003),
        (0x96b4, 0x2652_0001),
        (0xd968, 0x0220_2021),
        (0xd96c, 0x0000_3021),
        (0xd970, 0x3c05_800a),
        (0xd974, 0x8ca5_3410),
        (0xd978, 0x2407_0136),
        (0xd97c, 0x0c02_ad6b),
        (0xd980, 0xafa0_0010),
        (0xdd94, 0x0220_2021),
        (0xdd98, 0x2406_0002),
        (0xdd9c, 0x2407_015e),
        (0xdda0, 0x3c05_800a),
        (0xdda4, 0x8ca5_3414),
        (0xdda8, 0x2402_0006),
        (0xddac, 0x0c02_ad6b),
        (0xddb0, 0xafa2_0010),
    ] {
        assert_eq!(read_u32(&overlay, offset), expected);
    }

    let (_, stored) =
        crate::disc::rebuild::read_record(&cue.image_path, "DAT2/KOUBAI1.TIZ").unwrap();
    let decoded = decompress(&stored, false).unwrap();
    assert_eq!(
        sha256_bytes(&stored),
        "3f2799f520e73b672cb36eaca3dfabddaa2318790c04311782b87059911c9da4"
    );
    assert_eq!(
        sha256_bytes(&decoded),
        "d29a680ee12008bc88d7c690ab5db82d288a89273db1913867782c99818ce4fd"
    );
    let japanese_any = read_indexed_cell_in_prefix(
        &decoded,
        0x21000,
        Cell {
            x: 652,
            y: 60,
            width: 20,
            height: 20,
        },
    )
    .unwrap();
    assert_eq!(
        sha256_bytes(&japanese_any),
        "07137e99c824e8b3f6fb95f2f145a5f4b0704cebaa5d7e8d411963478bd16a08"
    );
}

fn raw_line_command_counts(bytes: &[u8]) -> Vec<usize> {
    let mut counts = vec![0usize];
    let mut cursor = 0;
    loop {
        match bytes[cursor] {
            0x81 => break,
            0x80 => {
                counts.push(0);
                cursor += 1;
            }
            0x63 => {
                assert_eq!(&bytes[cursor..cursor + 3], &[0x63; 3]);
                *counts.last_mut().unwrap() += 1;
                cursor += 3;
            }
            page => {
                assert!(page <= 3 && bytes[cursor + 1] <= 15 && bytes[cursor + 2] <= 15);
                *counts.last_mut().unwrap() += 1;
                cursor += 3;
            }
        }
    }
    counts
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}
