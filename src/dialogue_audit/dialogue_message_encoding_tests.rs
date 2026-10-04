use std::collections::BTreeMap;

use super::dialogue_message_encoding::{encode_translated_message, encoded_message_byte_count};
use super::translation_model::DialogueTranslationControl;

#[test]
fn translated_message_preserves_control_arguments_and_alignment() {
    let segments = vec!["한글".to_string(), "!".to_string(), "".to_string()];
    let controls = vec![
        control("palette_style", &["0x0001"]),
        control("message_end", &[]),
    ];
    let codes = BTreeMap::from([('한', 0x0010), ('글', 0x0011), ('!', 0x0012)]);

    let encoded = encode_translated_message(&segments, &controls, &codes).unwrap();

    assert_eq!(
        encoded,
        [
            0x10, 0x00, 0x11, 0x00, 0x03, 0x30, 0x01, 0x00, 0x12, 0x00, 0x01, 0x30,
        ]
    );
    assert_eq!(
        encoded_message_byte_count(&segments, &controls).unwrap(),
        12
    );
}

#[test]
fn translated_message_rejects_missing_character_codes() {
    let error = encode_translated_message(
        &["한".to_string(), "".to_string()],
        &[control("message_end", &[])],
        &BTreeMap::new(),
    )
    .unwrap_err();

    assert!(error.to_string().contains("lacks a code"));
}

#[test]
fn translated_message_rejects_text_after_terminator() {
    let error = encode_translated_message(
        &["".to_string(), "뒤".to_string()],
        &[control("message_end", &[])],
        &BTreeMap::from([('뒤', 1)]),
    )
    .unwrap_err();

    assert!(error.to_string().contains("after message_end"));
}

#[test]
fn translated_message_rejects_an_early_terminator() {
    let error = encode_translated_message(
        &["".to_string(), "".to_string(), "".to_string()],
        &[control("message_end", &[]), control("message_end", &[])],
        &BTreeMap::new(),
    )
    .unwrap_err();

    assert!(error.to_string().contains("more than one message_end"));
}

#[test]
fn translated_message_shape_preserves_every_control_boundary() {
    let error = encoded_message_byte_count(&["한글".to_string()], &[control("message_end", &[])])
        .unwrap_err();

    assert!(error.to_string().contains("segment/control shape"));
}

fn control(name: &str, arguments: &[&str]) -> DialogueTranslationControl {
    DialogueTranslationControl {
        semantic_name: name.to_string(),
        arguments: arguments
            .iter()
            .map(|argument| (*argument).to_string())
            .collect(),
    }
}
