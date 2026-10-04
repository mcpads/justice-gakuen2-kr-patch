use std::collections::BTreeMap;

use anyhow::{Context, Result, ensure};

use super::corpus_codec::parse_hex_word;
use super::runtime_insertions::dialogue_control_spec_by_name;
use super::translation_model::DialogueTranslationControl;

pub(super) fn encode_translated_message(
    segments: &[String],
    controls: &[DialogueTranslationControl],
    character_codes: &BTreeMap<char, u16>,
) -> Result<Vec<u8>> {
    ensure!(
        segments.len() == controls.len() + 1,
        "translated segment/control shape changed"
    );
    ensure!(
        controls
            .last()
            .is_some_and(|control| control.semantic_name == "message_end"),
        "translated message does not end with message_end"
    );
    ensure!(
        controls
            .iter()
            .filter(|control| control.semantic_name == "message_end")
            .count()
            == 1,
        "translated message contains more than one message_end"
    );
    ensure!(
        segments.last().is_some_and(String::is_empty),
        "translated message contains text after message_end"
    );

    let mut words = Vec::new();
    for (segment, control) in segments.iter().zip(controls) {
        for character in segment.chars() {
            let code = character_codes
                .get(&character)
                .with_context(|| format!("translated character {character:?} lacks a code"))?;
            words.push(*code);
        }
        let spec = dialogue_control_spec_by_name(&control.semantic_name).with_context(|| {
            format!(
                "translated control {} has no runtime specification",
                control.semantic_name
            )
        })?;
        ensure!(
            control.arguments.len() == spec.argument_word_count,
            "translated control {} argument width changed",
            control.semantic_name
        );
        words.push(spec.code);
        for argument in &control.arguments {
            words.push(parse_hex_word(argument)?);
        }
    }

    if !words.len().is_multiple_of(2) {
        words.push(0);
    }
    Ok(words
        .into_iter()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>())
}

pub(super) fn encoded_message_byte_count(
    segments: &[String],
    controls: &[DialogueTranslationControl],
) -> Result<usize> {
    ensure!(
        segments.len() == controls.len() + 1,
        "translated segment/control shape changed"
    );
    let glyph_word_count = segments
        .iter()
        .map(|segment| segment.chars().count())
        .sum::<usize>();
    let control_word_count = controls.iter().try_fold(0usize, |count, control| {
        count
            .checked_add(1 + control.arguments.len())
            .context("translated control word count overflow")
    })?;
    let word_count = glyph_word_count
        .checked_add(control_word_count)
        .context("translated message word count overflow")?;
    let aligned_word_count = word_count
        .checked_add(word_count % 2)
        .context("translated message alignment overflow")?;
    aligned_word_count
        .checked_mul(2)
        .context("translated message byte count overflow")
}
