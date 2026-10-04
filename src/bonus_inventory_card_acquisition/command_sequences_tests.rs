use std::collections::BTreeMap;

use super::command_sequences::{
    CARD_ACQUISITION_SEQUENCES, encode_card_acquisition_sequences, encode_card_acquisition_text,
};
use super::glyph_slots::{CARD_ACQUISITION_GLYPHS, REUSED_SOURCE_GLYPHS};
use super::test_support::authored_units;

#[test]
#[ignore = "requires assets/"]
fn three_independent_units_encode_to_fixed_source_records() {
    let units = authored_units();
    let encoded = encode_card_acquisition_sequences(&units).unwrap();
    let glyph_codes = glyph_codes();

    for ((unit, bytes), spec) in units.iter().zip(&encoded).zip(CARD_ACQUISITION_SEQUENCES) {
        let text = unit.korean_text.as_deref().unwrap();
        let command_count = text.chars().count();
        assert!(command_count <= (spec.storage_length - 1) / 3);
        assert_eq!(bytes.len(), spec.storage_length);
        for (index, character) in text.chars().enumerate() {
            let command = &bytes[index * 3..index * 3 + 3];
            if character == ' ' {
                assert_eq!(command, [0x63; 3]);
            } else {
                let mut utf8 = [0; 4];
                let text = character.encode_utf8(&mut utf8);
                assert_eq!(command, glyph_command(glyph_codes[text]));
            }
        }
        assert_eq!(bytes[command_count * 3], 0x81);
        assert!(bytes[command_count * 3 + 1..].iter().all(|byte| *byte == 0));
    }
}

#[test]
#[ignore = "requires assets/"]
fn fixed_record_capacity_rejects_an_extra_command() {
    let units = authored_units();
    let mut too_long = units[0].korean_text.clone().unwrap();
    let extra_character = units[1]
        .korean_text
        .as_deref()
        .unwrap()
        .chars()
        .next()
        .unwrap();
    too_long.push(extra_character);

    let error = encode_card_acquisition_text(&too_long, 8, &glyph_codes()).unwrap_err();
    assert!(error.to_string().contains("fixed source command capacity"));
}

fn glyph_codes() -> BTreeMap<&'static str, u16> {
    CARD_ACQUISITION_GLYPHS
        .iter()
        .map(|(text, code, _)| (*text, *code))
        .chain(
            REUSED_SOURCE_GLYPHS
                .iter()
                .map(|(text, code, _, _)| (*text, *code)),
        )
        .collect()
}

fn glyph_command(code: u16) -> [u8; 3] {
    [
        (code >> 8) as u8,
        (code & 0x000f) as u8,
        ((code >> 4) & 0x000f) as u8,
    ]
}
