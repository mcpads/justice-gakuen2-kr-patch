use std::collections::BTreeMap;

use anyhow::{Result, ensure};

use super::corpus_model::DialogueCorpusEntry;
use super::dialogue_message_encoding::encode_translated_message;
use super::translation_model::DialogueTranslationControl;

pub(super) const RUNTIME_INSERTION_BANK_SELECTOR: usize = 2;

struct RuntimeInsertionMessageSpec {
    entry_index: usize,
    source_markup: &'static str,
    korean_text: &'static str,
}

const RUNTIME_INSERTION_MESSAGE_SPECS: &[RuntimeInsertionMessageSpec] = &[
    RuntimeInsertionMessageSpec {
        entry_index: 5,
        source_markup: "君{#message_end}",
        korean_text: "군",
    },
    RuntimeInsertionMessageSpec {
        entry_index: 6,
        source_markup: "クン{#message_end}",
        korean_text: "군",
    },
    RuntimeInsertionMessageSpec {
        entry_index: 7,
        source_markup: "さん{#message_end}",
        korean_text: "씨",
    },
    RuntimeInsertionMessageSpec {
        entry_index: 8,
        source_markup: "ちゃん{#message_end}",
        korean_text: "짱",
    },
];

pub(super) fn encode_runtime_insertion_message(
    bank_selector: usize,
    entry: &DialogueCorpusEntry,
    character_codes: &BTreeMap<char, u16>,
) -> Result<Option<Vec<u8>>> {
    let Some(korean_text) = runtime_insertion_korean_text(bank_selector, entry)? else {
        return Ok(None);
    };
    let segments = [korean_text.to_string(), String::new()];
    let controls = [DialogueTranslationControl {
        semantic_name: "message_end".to_string(),
        arguments: Vec::new(),
    }];
    Ok(Some(encode_translated_message(
        &segments,
        &controls,
        character_codes,
    )?))
}

pub(super) fn runtime_insertion_korean_text(
    bank_selector: usize,
    entry: &DialogueCorpusEntry,
) -> Result<Option<&'static str>> {
    if bank_selector != RUNTIME_INSERTION_BANK_SELECTOR {
        return Ok(None);
    }
    let Some(spec) = RUNTIME_INSERTION_MESSAGE_SPECS
        .iter()
        .find(|spec| spec.entry_index == entry.entry_index)
    else {
        return Ok(None);
    };
    ensure!(
        entry.source_markup == spec.source_markup,
        "runtime insertion source changed at {}",
        entry.coordinate_id
    );
    Ok(Some(spec.korean_text))
}

pub(super) fn runtime_insertion_message_count_per_asset() -> usize {
    RUNTIME_INSERTION_MESSAGE_SPECS.len()
}
