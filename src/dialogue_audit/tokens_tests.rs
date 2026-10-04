use super::tokens::{DialogueTokenKind, tokenize_dialogue_message};

#[test]
fn separates_glyphs_control_arguments_and_alignment_padding() {
    let message = tokenize_dialogue_message(
        &[
            0x0041, 0x0300, 0x2003, 0x0002, 0x3003, 0x00ff, 0x3000, 0x3001, 0x0000,
        ],
        0x02ea,
        0x03e9,
    )
    .unwrap();

    let kinds: Vec<_> = message.tokens.iter().map(|token| token.kind).collect();
    assert_eq!(
        kinds,
        [
            DialogueTokenKind::FixedGlyph,
            DialogueTokenKind::RuntimeExtensionGlyph,
            DialogueTokenKind::ParameterizedRuntimeInsertion,
            DialogueTokenKind::PaletteStyle,
            DialogueTokenKind::LineBreak,
            DialogueTokenKind::MessageEnd,
        ]
    );
    assert_eq!(message.tokens[2].arguments, [0x0002]);
    assert_eq!(message.tokens[3].arguments, [0x00ff]);
    assert_eq!(message.alignment_padding_word_count, 1);
}

#[test]
fn rejects_non_alignment_data_after_the_terminator() {
    let error = tokenize_dialogue_message(&[0x0041, 0x3001, 0x0002], 0x02ea, 0x03e9).unwrap_err();
    assert!(error.to_string().contains("after 0x3001"));
}

#[test]
fn rejects_codes_without_a_backing_slot_or_known_control_path() {
    let error = tokenize_dialogue_message(&[0x0400, 0x3001], 0x02ea, 0x03e9).unwrap_err();
    assert!(error.to_string().contains("unrecognized dialogue code"));
}

#[test]
fn rejects_a_parameterized_code_without_a_message_terminator() {
    let error = tokenize_dialogue_message(&[0x2003, 0x0002], 0x02ea, 0x03e9).unwrap_err();
    assert!(error.to_string().contains("lacks 0x3001"));
}
