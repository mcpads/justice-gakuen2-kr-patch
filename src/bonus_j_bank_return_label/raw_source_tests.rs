use std::path::Path;

use crate::compression::decompress;
use crate::pipeline::sha256_bytes;
use crate::tim::{Cell, read_indexed_cell_in_prefix};

use super::consumer::validate_return_label_consumer;
use super::test_support::{raw_consumer_words, read_u32};

#[test]
#[ignore = "requires the user-supplied supported source disc"]
fn supported_source_matches_independent_record_pointer_caller_renderer_and_glyph_oracles() {
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

    let source_record = [
        0x00, 0x02, 0x0a, 0x01, 0x04, 0x00, 0x00, 0x08, 0x0a, 0x81, 0x00, 0x00,
    ];
    assert_eq!(&overlay[0x0d78..0x0d84], &source_record);
    assert_eq!(
        sha256_bytes(&source_record),
        "b27fab01f6fe2bbaa50059405077a6b58203c72e9bf9a6b2e216ab0b4ac3f9af"
    );
    assert_eq!(source_record.iter().position(|byte| *byte == 0x81), Some(9));
    assert_eq!(&source_record[10..], &[0, 0]);
    assert_eq!(read_u32(&overlay, 0x13b4), 0x800a_2d78);
    assert_eq!(read_u32(&overlay, 0x13b8), 0x800a_2d84);
    for (offset, word) in raw_consumer_words() {
        assert_eq!(
            read_u32(&overlay, offset),
            word,
            "raw word at +0x{offset:04x}"
        );
    }
    validate_return_label_consumer(&overlay).unwrap();

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
    for cell in [
        Cell {
            x: 828,
            y: 220,
            width: 20,
            height: 20,
        },
        Cell {
            x: 812,
            y: 220,
            width: 20,
            height: 20,
        },
        Cell {
            x: 868,
            y: 220,
            width: 20,
            height: 20,
        },
    ] {
        let pixels = read_indexed_cell_in_prefix(&decoded, 0x21000, cell).unwrap();
        assert_eq!(
            sha256_bytes(&pixels),
            "7a12e561363385e9dfeeab326368731c030ed4b374e7f5897ac819159d2884c5"
        );
    }
}
