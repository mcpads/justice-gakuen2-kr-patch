use std::collections::BTreeMap;

use super::command_sequences::{
    MEMORY_CARD_SWAP_SEQUENCES, encode_memory_card_swap_sequences, encode_memory_card_swap_text,
};
use super::consumer::RENDERER_LINE_COMMAND_LIMIT;
use super::glyph_slots::{MEMORY_CARD_SWAP_GLYPHS, REUSED_SOURCE_GLYPHS};
use super::test_support::authored_units;

#[test]
#[ignore = "requires assets/"]
fn two_independent_units_encode_spaces_newline_terminator_and_padding() {
    let units = authored_units();
    let encoded = encode_memory_card_swap_sequences(&units).unwrap();
    let glyph_codes = glyph_codes();
    assert_eq!(encoded.len(), 2);

    for ((unit, sequence), spec) in units.iter().zip(&encoded).zip(MEMORY_CARD_SWAP_SEQUENCES) {
        let text = unit.korean_text.as_deref().unwrap();
        let expected_line_command_counts = text
            .split('\n')
            .map(|line| line.chars().count())
            .collect::<Vec<_>>();
        assert_eq!(expected_line_command_counts.len(), 2);
        assert_eq!(sequence.line_command_counts, expected_line_command_counts);
        assert_eq!(sequence.bytes.len(), spec.storage_length);

        let mut cursor = 0;
        for character in text.chars() {
            match character {
                '\n' => {
                    assert_eq!(sequence.bytes[cursor], 0x80);
                    cursor += 1;
                }
                ' ' => {
                    assert_eq!(&sequence.bytes[cursor..cursor + 3], &[0x63; 3]);
                    cursor += 3;
                }
                _ => {
                    let mut utf8 = [0; 4];
                    let text = character.encode_utf8(&mut utf8);
                    assert_eq!(
                        &sequence.bytes[cursor..cursor + 3],
                        &glyph_command(glyph_codes[text])
                    );
                    cursor += 3;
                }
            }
        }
        assert_eq!(sequence.bytes[cursor], 0x81);
        cursor += 1;
        assert_eq!(sequence.payload_byte_length, cursor);
        assert!(
            sequence.bytes[cursor..].iter().all(|byte| *byte == 0),
            "fixed record padding must remain zero"
        );
        assert!(
            sequence
                .line_command_counts
                .iter()
                .all(|count| { *count <= RENDERER_LINE_COMMAND_LIMIT })
        );
    }
}

#[test]
fn encoder_rejects_extra_lines_line_overflow_and_record_overflow() {
    let codes = glyph_codes();
    let two_lines = "메\n모";
    assert!(encode_memory_card_swap_text(two_lines, 8, &codes).is_ok());
    assert!(encode_memory_card_swap_text("메\n모\n리", 16, &codes).is_err());

    let overwide = format!("{}\n모", "메".repeat(RENDERER_LINE_COMMAND_LIMIT + 1));
    let error = encode_memory_card_swap_text(&overwide, 128, &codes).unwrap_err();
    assert!(error.to_string().contains("renderer width"));

    let error = encode_memory_card_swap_text("메모리\n카드", 8, &codes).unwrap_err();
    assert!(error.to_string().contains("fixed source storage"));
}

fn glyph_codes() -> BTreeMap<&'static str, u16> {
    MEMORY_CARD_SWAP_GLYPHS
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

#[test]
#[ignore = "requires assets/"]
fn swap_prompts_request_a_button_without_the_japanese_any_character() {
    let units = authored_units();
    let encoded = encode_memory_card_swap_sequences(&units).unwrap();
    for sequence in encoded {
        let second_line = sequence.bytes.iter().position(|byte| *byte == 0x80).unwrap() + 1;
        assert_eq!(&sequence.bytes[second_line..second_line + 3], &[3, 9, 8]);
        assert_ne!(&sequence.bytes[second_line..second_line + 3], &[2, 7, 3]);
    }
}
