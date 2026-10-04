use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use super::model::{BonusShopTextSourceTokenKind, ShopTextRole};
use super::parser::{TABLE_SPECS, parse_shop_text_tables};
use super::{BonusShopTextSourceConfig, initialize_bonus_shop_text_source};
use crate::bonus_shop_source::{
    GLYPH_TIM_OFFSET, OVERLAY_RUNTIME_BASE, POINTER_COMMAND_TABLE_RANGES, load_source,
};

#[test]
fn parses_every_pointer_entry_as_a_role_scoped_source_unit() {
    let overlay = source_table_fixture();
    let tables = parse_shop_text_tables(&overlay, &glyph_tim_fixture(), &BTreeMap::new()).unwrap();

    assert_eq!(tables.len(), 3);
    assert_eq!(
        tables
            .iter()
            .map(|table| (table.spec.role, table.records.len()))
            .collect::<Vec<_>>(),
        [
            (ShopTextRole::ProductDescription, 83),
            (ShopTextRole::ProductLabel, 83),
            (ShopTextRole::ClerkDialogue, 20),
        ]
    );
    assert_eq!(
        tables
            .iter()
            .flat_map(|table| table.records.iter().map(|record| record.source_offset))
            .collect::<BTreeSet<_>>()
            .len(),
        186
    );
    assert!(tables.iter().all(|table| {
        table
            .records
            .iter()
            .all(|record| record.unit.exact_source_text.is_none())
    }));
}

#[test]
fn preserves_glyph_blank_line_break_and_terminator_semantics() {
    let tables = parse_shop_text_tables(
        &source_table_fixture(),
        &glyph_tim_fixture(),
        &BTreeMap::new(),
    )
    .unwrap();
    let unit = &tables[0].records[0].unit;

    assert_eq!(unit.line_count, 2);
    assert_eq!(unit.glyph_count, 2);
    assert_eq!(unit.source_record_byte_count, 11);
    assert_eq!(unit.source_pointer_interval_byte_count, 12);
    assert_eq!(unit.trailing_interval_byte_count, 1);
    assert!(unit.trailing_interval_all_zero);
    assert_eq!(
        unit.tokens
            .iter()
            .map(|token| token.kind)
            .collect::<Vec<_>>(),
        [
            BonusShopTextSourceTokenKind::Glyph,
            BonusShopTextSourceTokenKind::Blank,
            BonusShopTextSourceTokenKind::LineBreak,
            BonusShopTextSourceTokenKind::Glyph,
            BonusShopTextSourceTokenKind::Terminator,
        ]
    );
}

#[test]
fn preserves_nonzero_trailing_pointer_interval_bytes_without_calling_them_free_space() {
    let mut overlay = source_table_fixture();
    let first_record_padding = TABLE_SPECS[0].record_region[0] + 11;
    overlay[first_record_padding] = 0x5a;

    let tables = parse_shop_text_tables(&overlay, &glyph_tim_fixture(), &BTreeMap::new()).unwrap();
    let unit = &tables[0].records[0].unit;

    assert_eq!(unit.source_record_byte_count, 11);
    assert_eq!(unit.trailing_interval_byte_count, 1);
    assert!(!unit.trailing_interval_all_zero);
    assert_ne!(
        unit.trailing_interval_sha256,
        crate::pipeline::sha256_bytes(&[0])
    );
}

#[test]
fn rejects_a_repeated_pointer_instead_of_silently_losing_a_translation_unit() {
    let mut overlay = source_table_fixture();
    let table = POINTER_COMMAND_TABLE_RANGES[1];
    let first = overlay[table[0]..table[0] + 4].to_vec();
    overlay[table[0] + 4..table[0] + 8].copy_from_slice(&first);

    let error = parse_shop_text_tables(&overlay, &glyph_tim_fixture(), &BTreeMap::new())
        .err()
        .expect("repeated pointer must fail");

    assert!(error.to_string().contains("strictly increasing and unique"));
}

#[test]
#[ignore = "requires the user-supplied supported source disc"]
fn supported_source_has_the_complete_unique_83_plus_83_plus_20_record_set() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cue = root.join("roms/Shiritsu Justice Gakuen - Nekketsu Seishun Nikki 2 (Japan).cue");
    let source = load_source(&cue).unwrap();
    let tables =
        parse_shop_text_tables(&source.overlay, &source.shop_ui_decoded, &BTreeMap::new()).unwrap();

    assert_eq!(
        tables
            .iter()
            .map(|table| table.records.len())
            .sum::<usize>(),
        186
    );
    assert_eq!(
        tables
            .iter()
            .flat_map(|table| table.records.iter().map(|record| record.source_offset))
            .collect::<BTreeSet<_>>()
            .len(),
        186
    );
    assert_eq!(tables[0].records[0].source_offset, 0x0010);
    assert_eq!(tables[1].records[0].source_offset, 0x25a4);
    assert_eq!(tables[2].records[0].source_offset, 0x2e08);
    assert_eq!(
        tables
            .iter()
            .flat_map(|table| &table.records)
            .map(|record| record.unit.source_pointer_interval_byte_count)
            .sum::<usize>(),
        13_428
    );
    assert_eq!(
        tables
            .iter()
            .flat_map(|table| &table.records)
            .map(|record| record.unit.trailing_interval_byte_count)
            .sum::<usize>(),
        273
    );
    assert!(
        tables
            .iter()
            .flat_map(|table| &table.records)
            .all(|record| {
                record.unit.trailing_interval_all_zero
                    && record.unit.source_record_byte_count
                        + record.unit.trailing_interval_byte_count
                        == record.unit.source_pointer_interval_byte_count
            })
    );
    assert_eq!(
        tables
            .iter()
            .flat_map(|table| &table.records)
            .flat_map(|record| &record.unit.tokens)
            .filter_map(|token| token.code.as_deref())
            .collect::<BTreeSet<_>>()
            .len(),
        217
    );
}

#[test]
#[ignore = "requires the user-supplied supported source disc and verified codebook"]
fn writes_sharded_units_and_contact_sheets_without_translation_text() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let output = std::env::temp_dir().join(format!(
        "justice-bonus-shop-text-source-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&output);
    let manifest = initialize_bonus_shop_text_source(&BonusShopTextSourceConfig {
        cue: root.join("roms/Shiritsu Justice Gakuen - Nekketsu Seishun Nikki 2 (Japan).cue"),
        dialogue_codebook: root.join("assets/dialogue/codebook.json"),
        output: output.clone(),
        force: true,
    })
    .unwrap();

    assert!(manifest.acquisition_complete);
    assert_eq!(manifest.total_record_count, 186);
    assert_eq!(manifest.unique_record_target_count, 186);
    assert_eq!(manifest.table_count, 3);
    for role in ShopTextRole::ALL {
        let unit_dir = output.join(role.slug()).join("units");
        assert_eq!(
            std::fs::read_dir(unit_dir).unwrap().count(),
            role.expected_record_count()
        );
        assert!(output.join(role.slug()).join("index.json").is_file());
        assert!(
            output
                .join(role.slug())
                .join("previews/sheet-000.png")
                .is_file()
        );
    }
    std::fs::remove_dir_all(output).unwrap();
}

fn source_table_fixture() -> Vec<u8> {
    const RECORD_STRIDE: usize = 12;
    let mut overlay = vec![0u8; 0x90c0];
    for spec in TABLE_SPECS {
        for record_index in 0..spec.role.expected_record_count() {
            let source_offset = spec.record_region[0] + record_index * RECORD_STRIDE;
            assert!(source_offset + RECORD_STRIDE <= spec.record_region[1]);
            overlay[source_offset..source_offset + 11].copy_from_slice(&[
                0x00, 0x00, 0x00, 0x63, 0x63, 0x63, 0x80, 0x00, 0x00, 0x00, 0x81,
            ]);
            let pointer_offset = spec.pointer_table_range[0] + record_index * 4;
            write_u32(
                &mut overlay,
                pointer_offset,
                OVERLAY_RUNTIME_BASE + source_offset as u32,
            );
        }
    }
    overlay
}

fn glyph_tim_fixture() -> Vec<u8> {
    const CLUT_SIZE: usize = 12 + 16 * 2;
    const IMAGE_WORD_WIDTH: usize = 256;
    const IMAGE_HEIGHT: usize = 256;
    const IMAGE_SIZE: usize = 12 + IMAGE_WORD_WIDTH * IMAGE_HEIGHT * 2;
    let mut decoded = vec![0u8; GLYPH_TIM_OFFSET + 8 + CLUT_SIZE + IMAGE_SIZE];
    let tim = &mut decoded[GLYPH_TIM_OFFSET..];
    write_u32(tim, 0, 0x10);
    write_u32(tim, 4, 0x08);
    write_u32(tim, 8, CLUT_SIZE as u32);
    write_u16(tim, 16, 16);
    write_u16(tim, 18, 1);
    let image_offset = 8 + CLUT_SIZE;
    write_u32(tim, image_offset, IMAGE_SIZE as u32);
    write_u16(tim, image_offset + 8, IMAGE_WORD_WIDTH as u16);
    write_u16(tim, image_offset + 10, IMAGE_HEIGHT as u16);
    decoded
}

fn write_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
