use std::collections::BTreeMap;

use super::corpus_model::{DialogueCorpusEntry, DialogueCorpusToken};
use super::runtime_insertion_messages::{
    RUNTIME_INSERTION_BANK_SELECTOR, encode_runtime_insertion_message,
    runtime_insertion_message_count_per_asset,
};

#[test]
fn rewrites_renderer_owned_honorific_messages_with_korean_codes() {
    let codes = BTreeMap::from([('군', 0x0123), ('씨', 0x0234), ('짱', 0x0345)]);
    let cases = [
        (5, "君{#message_end}", 0x0123),
        (6, "クン{#message_end}", 0x0123),
        (7, "さん{#message_end}", 0x0234),
        (8, "ちゃん{#message_end}", 0x0345),
    ];

    assert_eq!(runtime_insertion_message_count_per_asset(), cases.len());
    for (entry_index, markup, expected_code) in cases {
        let encoded = encode_runtime_insertion_message(
            RUNTIME_INSERTION_BANK_SELECTOR,
            &entry(entry_index, markup),
            &codes,
        )
        .unwrap()
        .unwrap();

        assert_eq!(
            encoded,
            [expected_code as u8, (expected_code >> 8) as u8, 0x01, 0x30,]
        );
    }
}

#[test]
fn ignores_messages_not_owned_by_the_runtime_insertion_dispatcher() {
    let message = entry(5, "君{#message_end}");

    assert!(
        encode_runtime_insertion_message(1, &message, &BTreeMap::new())
            .unwrap()
            .is_none()
    );
    assert!(
        encode_runtime_insertion_message(
            RUNTIME_INSERTION_BANK_SELECTOR,
            &entry(4, "保健室{#message_end}"),
            &BTreeMap::new()
        )
        .unwrap()
        .is_none()
    );
}

#[test]
fn rejects_a_shifted_runtime_insertion_table() {
    let error = encode_runtime_insertion_message(
        RUNTIME_INSERTION_BANK_SELECTOR,
        &entry(5, "クン{#message_end}"),
        &BTreeMap::from([('군', 1)]),
    )
    .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("runtime insertion source changed")
    );
}

fn entry(entry_index: usize, source_markup: &str) -> DialogueCorpusEntry {
    DialogueCorpusEntry {
        coordinate_id: format!("DAT2/MGK01.BIZ#bank-2-entry-{entry_index:04}"),
        entry_index,
        decoded_offset: String::new(),
        runtime_address: String::new(),
        raw_entry_sha256: String::new(),
        semantic_source_sha256: String::new(),
        semantic_shared_coordinate_count: 10,
        raw_words: Vec::new(),
        alignment_padding_word_count: 0,
        source_markup: source_markup.to_string(),
        tokens: Vec::<DialogueCorpusToken>::new(),
    }
}
