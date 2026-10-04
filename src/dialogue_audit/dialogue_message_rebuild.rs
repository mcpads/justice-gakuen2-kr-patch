use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};

use super::corpus_model::{DialogueCorpusAsset, DialogueCorpusEntry};
use super::dialogue_message_encoding::encode_translated_message;
use super::parser::{
    DECODED_IMAGE_SIZE, DECODED_RUNTIME_BASE, SELECTOR_SLOT_COUNT, SELECTOR_TABLE_OFFSET,
    parse_dialogue_banks,
};
use super::runtime_insertion_messages::{
    encode_runtime_insertion_message, runtime_insertion_message_count_per_asset,
};
use super::script_topology::PRIMARY_SCRIPT_OFFSET;
use super::translation_model::DialogueDevelopmentInputPolicy;
use super::translation_model::DialogueTranslationControl;

const POINTER_TABLE_TERMINATOR: u32 = u32::MAX;

#[derive(Debug)]
pub(super) struct RebuiltDialogueMessageArena {
    pub(super) decoded: Vec<u8>,
    pub(super) bank_count: usize,
    pub(super) message_count: usize,
    pub(super) translated_message_count: usize,
    pub(super) runtime_insertion_message_count: usize,
    pub(super) preserved_untranslated_message_count: usize,
    pub(super) preserved_unreferenced_message_count: usize,
    pub(super) used_byte_count: usize,
    pub(super) spare_byte_count: usize,
}

struct MessageRebuildContext<'a> {
    source_path: &'a str,
    translated_segments: &'a BTreeMap<String, Vec<String>>,
    controls_by_semantic_hash: &'a BTreeMap<String, Vec<DialogueTranslationControl>>,
    referenced_coordinates: &'a BTreeSet<String>,
    character_codes: &'a BTreeMap<char, u16>,
    input_policy: DialogueDevelopmentInputPolicy,
}

pub(super) fn rebuild_dialogue_message_arena(
    decoded: &[u8],
    corpus_asset: &DialogueCorpusAsset,
    translated_segments: &BTreeMap<String, Vec<String>>,
    controls_by_semantic_hash: &BTreeMap<String, Vec<DialogueTranslationControl>>,
    referenced_coordinates: &BTreeSet<String>,
    character_codes: &BTreeMap<char, u16>,
    input_policy: DialogueDevelopmentInputPolicy,
) -> Result<RebuiltDialogueMessageArena> {
    ensure!(
        decoded.len() == DECODED_IMAGE_SIZE,
        "unexpected decoded dialogue image size"
    );
    let original_banks = parse_dialogue_banks(decoded)?;
    ensure!(
        original_banks.len() == corpus_asset.banks.len(),
        "dialogue corpus bank count changed"
    );
    let message_arena_start = SELECTOR_TABLE_OFFSET + SELECTOR_SLOT_COUNT * 4;
    ensure!(
        original_banks
            .first()
            .is_some_and(|bank| bank.message_data_start == message_arena_start),
        "dialogue message arena start changed"
    );

    let mut rebuilt = decoded.to_vec();
    rebuilt[SELECTOR_TABLE_OFFSET..PRIMARY_SCRIPT_OFFSET].fill(0);
    let mut cursor = message_arena_start;
    let mut expected_messages = Vec::with_capacity(original_banks.len());
    let mut translated_message_count = 0usize;
    let mut runtime_insertion_message_count = 0usize;
    let mut preserved_untranslated_message_count = 0usize;
    let mut preserved_unreferenced_message_count = 0usize;
    let context = MessageRebuildContext {
        source_path: &corpus_asset.source_path,
        translated_segments,
        controls_by_semantic_hash,
        referenced_coordinates,
        character_codes,
        input_policy,
    };

    for (original_bank, corpus_bank) in original_banks.iter().zip(&corpus_asset.banks) {
        ensure!(
            original_bank.selector_index == corpus_bank.selector_index
                && original_bank.messages.len() == corpus_bank.entries.len(),
            "dialogue corpus bank population changed"
        );
        let mut message_offsets = Vec::with_capacity(original_bank.messages.len());
        let mut bank_messages = Vec::with_capacity(original_bank.messages.len());
        for (original_message, corpus_entry) in
            original_bank.messages.iter().zip(&corpus_bank.entries)
        {
            let (bytes, kind) = rebuilt_message_bytes(
                original_bank.selector_index,
                original_message.data.as_slice(),
                corpus_entry,
                &context,
            )?;
            match kind {
                RebuiltMessageKind::Translated => translated_message_count += 1,
                RebuiltMessageKind::RuntimeInsertion => runtime_insertion_message_count += 1,
                RebuiltMessageKind::PreservedUntranslated => {
                    preserved_untranslated_message_count += 1
                }
                RebuiltMessageKind::PreservedUnreferenced => {
                    preserved_unreferenced_message_count += 1
                }
            }
            ensure!(
                cursor.is_multiple_of(4) && bytes.len().is_multiple_of(4),
                "rebuilt dialogue message lost word-pair alignment"
            );
            let end = cursor
                .checked_add(bytes.len())
                .context("rebuilt dialogue message offset overflow")?;
            ensure!(
                end <= PRIMARY_SCRIPT_OFFSET,
                "rebuilt dialogue messages overlap the primary script"
            );
            message_offsets.push(cursor);
            rebuilt[cursor..end].copy_from_slice(&bytes);
            cursor = end;
            bank_messages.push(bytes);
        }

        let pointer_table_offset = cursor;
        for message_offset in message_offsets {
            write_u32(&mut rebuilt, cursor, runtime_pointer(message_offset)?)?;
            cursor += 4;
        }
        write_u32(&mut rebuilt, cursor, POINTER_TABLE_TERMINATOR)?;
        cursor += 4;
        ensure!(
            cursor <= PRIMARY_SCRIPT_OFFSET,
            "rebuilt dialogue pointer table overlaps the primary script"
        );
        write_u32(
            &mut rebuilt,
            SELECTOR_TABLE_OFFSET + original_bank.selector_index * 4,
            runtime_pointer(pointer_table_offset)?,
        )?;
        expected_messages.push(bank_messages);
    }
    let expected_runtime_insertion_message_count = original_banks
        .iter()
        .any(|bank| bank.selector_index == 2)
        .then(runtime_insertion_message_count_per_asset)
        .unwrap_or_default();
    ensure!(
        runtime_insertion_message_count == expected_runtime_insertion_message_count,
        "renderer-owned runtime insertion message population changed"
    );

    ensure!(
        rebuilt[..SELECTOR_TABLE_OFFSET] == decoded[..SELECTOR_TABLE_OFFSET],
        "message rebuild changed bytes before the selector table"
    );
    ensure!(
        rebuilt[PRIMARY_SCRIPT_OFFSET..] == decoded[PRIMARY_SCRIPT_OFFSET..],
        "message rebuild changed the primary script or later data"
    );
    ensure!(
        rebuilt[cursor..PRIMARY_SCRIPT_OFFSET]
            .iter()
            .all(|byte| *byte == 0),
        "message rebuild left data in the unused arena suffix"
    );

    let parsed_rebuilt = parse_dialogue_banks(&rebuilt)?;
    ensure!(
        parsed_rebuilt.len() == expected_messages.len(),
        "rebuilt dialogue bank count changed during parse-back"
    );
    for (bank, expected) in parsed_rebuilt.iter().zip(&expected_messages) {
        ensure!(
            bank.messages.len() == expected.len()
                && bank
                    .messages
                    .iter()
                    .zip(expected)
                    .all(|(message, expected)| message.data == *expected),
            "rebuilt dialogue messages changed during parse-back"
        );
    }

    Ok(RebuiltDialogueMessageArena {
        decoded: rebuilt,
        bank_count: parsed_rebuilt.len(),
        message_count: parsed_rebuilt.iter().map(|bank| bank.messages.len()).sum(),
        translated_message_count,
        runtime_insertion_message_count,
        preserved_untranslated_message_count,
        preserved_unreferenced_message_count,
        used_byte_count: cursor - message_arena_start,
        spare_byte_count: PRIMARY_SCRIPT_OFFSET - cursor,
    })
}

fn rebuilt_message_bytes(
    bank_selector: usize,
    original: &[u8],
    corpus_entry: &DialogueCorpusEntry,
    context: &MessageRebuildContext<'_>,
) -> Result<(Vec<u8>, RebuiltMessageKind)> {
    if let Some(encoded) =
        encode_runtime_insertion_message(bank_selector, corpus_entry, context.character_codes)?
    {
        ensure!(
            !context
                .translated_segments
                .contains_key(&corpus_entry.semantic_source_sha256),
            "runtime insertion message also appears in the authored primary-dialogue input"
        );
        return Ok((encoded, RebuiltMessageKind::RuntimeInsertion));
    }
    let Some(segments) = context
        .translated_segments
        .get(&corpus_entry.semantic_source_sha256)
    else {
        let is_primary_reference = context
            .referenced_coordinates
            .contains(&corpus_entry.coordinate_id);
        ensure!(
            !is_primary_reference || !context.input_policy.requires_complete_scope(),
            "execution-referenced dialogue coordinate lacks authored Korean segments"
        );
        let kind = if is_primary_reference {
            RebuiltMessageKind::PreservedUntranslated
        } else {
            RebuiltMessageKind::PreservedUnreferenced
        };
        return Ok((original.to_vec(), kind));
    };
    let controls = context
        .controls_by_semantic_hash
        .get(&corpus_entry.semantic_source_sha256)
        .context("authored dialogue lacks protected source controls")?;
    super::quiz_answer_layout::validate_quiz_question_layout(
        context.source_path,
        bank_selector,
        corpus_entry.entry_index,
        segments,
        controls,
    )?;
    super::quiz_answer_layout::validate_quiz_answer_layout(
        context.source_path,
        bank_selector,
        corpus_entry.entry_index,
        segments,
        controls,
    )?;
    let (segments, controls) =
        super::choice_columns::prepare(&corpus_entry.semantic_source_sha256, segments, controls)?;
    Ok((
        encode_translated_message(&segments, &controls, context.character_codes)?,
        RebuiltMessageKind::Translated,
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RebuiltMessageKind {
    Translated,
    RuntimeInsertion,
    PreservedUntranslated,
    PreservedUnreferenced,
}

fn runtime_pointer(decoded_offset: usize) -> Result<u32> {
    DECODED_RUNTIME_BASE
        .checked_add(u32::try_from(decoded_offset)?)
        .context("decoded dialogue runtime pointer overflow")
}

fn write_u32(output: &mut [u8], offset: usize, value: u32) -> Result<()> {
    ensure!(
        offset + 4 <= output.len(),
        "dialogue u32 write is truncated"
    );
    output[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    Ok(())
}
