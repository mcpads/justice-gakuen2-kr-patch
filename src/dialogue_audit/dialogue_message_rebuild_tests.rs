use std::collections::{BTreeMap, BTreeSet};

use super::corpus_model::{DialogueCorpusAsset, DialogueCorpusBank, DialogueCorpusEntry};
use super::dialogue_message_rebuild::rebuild_dialogue_message_arena;
use super::parser::{
    DECODED_IMAGE_SIZE, DECODED_RUNTIME_BASE, SELECTOR_TABLE_OFFSET, parse_dialogue_banks,
};
use super::script_topology::PRIMARY_SCRIPT_OFFSET;
use super::translation_model::DialogueDevelopmentInputPolicy;
use super::translation_model::DialogueTranslationControl;

const MESSAGE_START: usize = SELECTOR_TABLE_OFFSET + 32;

#[test]
#[ignore = "requires assets/"]
fn rebuilt_arena_relocates_pointer_table_and_parses_back_messages() {
    let mut decoded = source_image();
    decoded[PRIMARY_SCRIPT_OFFSET] = 0x5a;
    let corpus = corpus_asset();
    let segments = BTreeMap::from([(
        "semantic".to_string(),
        vec!["가나".to_string(), "".to_string()],
    )]);
    let controls = source_controls();
    let referenced = BTreeSet::from(["DAT2/MGK01.BIZ#bank-0-entry-0000".to_string()]);
    let codes = BTreeMap::from([('가', 2), ('나', 3)]);

    let rebuilt = rebuild_dialogue_message_arena(
        &decoded,
        &corpus,
        &segments,
        &controls,
        &referenced,
        &codes,
        DialogueDevelopmentInputPolicy::CompleteScope,
    )
    .unwrap();
    let parsed = parse_dialogue_banks(&rebuilt.decoded).unwrap();

    assert_eq!(parsed[0].messages[0].data, [2, 0, 3, 0, 1, 0x30, 0, 0]);
    assert_eq!(parsed[0].pointer_table_offset, MESSAGE_START + 8);
    assert_eq!(rebuilt.translated_message_count, 1);
    assert_eq!(rebuilt.runtime_insertion_message_count, 0);
    assert_eq!(rebuilt.preserved_unreferenced_message_count, 0);
    assert_eq!(rebuilt.decoded[PRIMARY_SCRIPT_OFFSET], 0x5a);
}

#[test]
fn rebuilt_arena_rejects_untranslated_referenced_coordinates() {
    let error = rebuild_dialogue_message_arena(
        &source_image(),
        &corpus_asset(),
        &BTreeMap::new(),
        &source_controls(),
        &BTreeSet::from(["DAT2/MGK01.BIZ#bank-0-entry-0000".to_string()]),
        &BTreeMap::new(),
        DialogueDevelopmentInputPolicy::CompleteScope,
    )
    .unwrap_err();

    assert!(error.to_string().contains("lacks authored Korean"));
}

#[test]
#[ignore = "requires assets/"]
fn authored_selector_hash_rebuilds_without_a_primary_reference() {
    let rebuilt = rebuild_dialogue_message_arena(
        &source_image(),
        &corpus_asset(),
        &BTreeMap::from([(
            "semantic".to_string(),
            vec!["가나".to_string(), String::new()],
        )]),
        &source_controls(),
        &BTreeSet::new(),
        &BTreeMap::from([('가', 2), ('나', 3)]),
        DialogueDevelopmentInputPolicy::CompleteScope,
    )
    .unwrap();

    assert_eq!(rebuilt.translated_message_count, 1);
    assert_eq!(rebuilt.preserved_unreferenced_message_count, 0);
}

#[test]
fn authored_subset_preserves_an_untranslated_primary_reference_explicitly() {
    let rebuilt = rebuild_dialogue_message_arena(
        &source_image(),
        &corpus_asset(),
        &BTreeMap::new(),
        &source_controls(),
        &BTreeSet::from(["DAT2/MGK01.BIZ#bank-0-entry-0000".to_string()]),
        &BTreeMap::new(),
        DialogueDevelopmentInputPolicy::AuthoredSubset,
    )
    .unwrap();

    assert_eq!(rebuilt.translated_message_count, 0);
    assert_eq!(rebuilt.preserved_untranslated_message_count, 1);
    assert_eq!(rebuilt.preserved_unreferenced_message_count, 0);
}

fn source_image() -> Vec<u8> {
    let mut decoded = vec![0u8; DECODED_IMAGE_SIZE];
    let pointer_table = MESSAGE_START + 4;
    write_u32(
        &mut decoded,
        SELECTOR_TABLE_OFFSET,
        DECODED_RUNTIME_BASE + pointer_table as u32,
    );
    decoded[MESSAGE_START..MESSAGE_START + 4].copy_from_slice(&[1, 0, 1, 0x30]);
    write_u32(
        &mut decoded,
        pointer_table,
        DECODED_RUNTIME_BASE + MESSAGE_START as u32,
    );
    write_u32(&mut decoded, pointer_table + 4, u32::MAX);
    decoded
}

fn corpus_asset() -> DialogueCorpusAsset {
    DialogueCorpusAsset {
        source_path: "DAT2/MGK01.BIZ".to_string(),
        stored_sha256: String::new(),
        decoded_sha256: String::new(),
        fixed_cell_count: 10,
        banks: vec![DialogueCorpusBank {
            selector_index: 0,
            entries: vec![DialogueCorpusEntry {
                coordinate_id: "DAT2/MGK01.BIZ#bank-0-entry-0000".to_string(),
                entry_index: 0,
                decoded_offset: "0x31020".to_string(),
                runtime_address: "0x80101020".to_string(),
                raw_entry_sha256: String::new(),
                semantic_source_sha256: "semantic".to_string(),
                semantic_shared_coordinate_count: 1,
                raw_words: vec!["0x0001".to_string(), "0x3001".to_string()],
                alignment_padding_word_count: 0,
                source_markup: String::new(),
                tokens: Vec::new(),
            }],
        }],
    }
}

fn source_controls() -> BTreeMap<String, Vec<DialogueTranslationControl>> {
    BTreeMap::from([(
        "semantic".to_string(),
        vec![DialogueTranslationControl {
            semantic_name: "message_end".to_string(),
            arguments: Vec::new(),
        }],
    )])
}

fn write_u32(output: &mut [u8], offset: usize, value: u32) {
    output[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
