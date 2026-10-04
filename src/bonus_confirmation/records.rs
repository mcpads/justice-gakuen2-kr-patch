use std::collections::BTreeMap;

use anyhow::{Context, Result, ensure};

use super::glyph_slots::FIXED_RECORD_SPACE_CODE;
use super::model::ConfirmationTextUnit;
use super::text_units::authored_text;

pub(super) const EXIT_PROMPT_RECORD_OFFSET: usize = 0x167c;
pub(super) const EXIT_PROMPT_RECORD_LENGTH: usize = 24;
pub(super) const ALTERNATE_PROMPT_RECORD_OFFSET: usize = 0x1694;
pub(super) const ALTERNATE_PROMPT_RECORD_LENGTH: usize = 44;
pub(super) const SHARED_CHOICE_RECORD_OFFSET: usize = 0x16c0;
pub(super) const SHARED_CHOICE_RECORD_LENGTH: usize = 16;

pub(super) const SOURCE_EXIT_PROMPT_RECORD: [u8; EXIT_PROMPT_RECORD_LENGTH] = [
    7, 2, 10, 11, 2, 11, 11, 0, 3, 8, 0, 10, 9, 0, 4, 8, 0, 9, 7, 0, 5, 5, 0, 0,
];
pub(super) const SOURCE_ALTERNATE_PROMPT_RECORD: [u8; ALTERNATE_PROMPT_RECORD_LENGTH] = [
    14, 0, 1, 8, 0, 4, 9, 1, 5, 2, 0, 4, 6, 1, 1, 7, 1, 3, 0, 0, 5, 10, 0, 10, 10, 0, 3, 8, 0, 5,
    7, 1, 3, 0, 0, 4, 8, 0, 9, 7, 0, 5, 5, 0,
];
pub(super) const SOURCE_SHARED_CHOICE_RECORD: [u8; SHARED_CHOICE_RECORD_LENGTH] =
    [0, 5, 9, 0, 5, 7, 0, 5, 7, 0, 5, 7, 0, 7, 7, 0];

pub(super) const SOURCE_EXIT_PROMPT_SHA256: &str =
    "f21ccab237f104267fa5c7b5d7ecd32a799558815ebbe341d00a006a2e382721";
pub(super) const SOURCE_ALTERNATE_PROMPT_SHA256: &str =
    "d7bc880847bea6d085173f30390e2eda6349fcfe0be75ffb7462c1ffcfe51592";
pub(super) const SOURCE_SHARED_CHOICE_SHA256: &str =
    "f12ba518783954566252387b1788bca43e04047fe807f9176b4183ef7649da1d";

pub(super) struct EncodedConfirmationRecords {
    pub(super) exit_prompt: Vec<u8>,
    pub(super) selected_card_prompt: Vec<u8>,
    pub(super) choices: Vec<u8>,
}

pub(super) fn encode_confirmation_records(
    units: &[ConfirmationTextUnit],
    glyph_codes: &BTreeMap<&str, u16>,
) -> Result<EncodedConfirmationRecords> {
    Ok(EncodedConfirmationRecords {
        exit_prompt: encode_counted_prompt(
            authored_text(units, "exit_prompt")?,
            glyph_codes,
            EXIT_PROMPT_RECORD_LENGTH,
            7,
        )?,
        selected_card_prompt: encode_counted_prompt(
            authored_text(units, "selected_card_prompt")?,
            glyph_codes,
            ALTERNATE_PROMPT_RECORD_LENGTH,
            14,
        )?,
        choices: encode_choices(
            authored_text(units, "shared_yes")?,
            authored_text(units, "shared_no")?,
            glyph_codes,
        )?,
    })
}

fn encode_counted_prompt(
    text: &str,
    glyph_codes: &BTreeMap<&str, u16>,
    record_length: usize,
    source_capacity: usize,
) -> Result<Vec<u8>> {
    let codes = encode_text_codes(text, glyph_codes, true)?;
    ensure!(
        codes.len() <= source_capacity,
        "Korean confirmation prompt exceeds source sprite capacity"
    );
    let mut output = Vec::with_capacity(record_length);
    output.push(u8::try_from(codes.len())?);
    for code in codes {
        output.extend_from_slice(&glyph_command(code));
    }
    output.resize(record_length, 0);
    Ok(output)
}

fn encode_choices(yes: &str, no: &str, glyph_codes: &BTreeMap<&str, u16>) -> Result<Vec<u8>> {
    let yes_codes = encode_text_codes(yes, glyph_codes, false)?;
    let no_codes = encode_text_codes(no, glyph_codes, false)?;
    ensure!(
        yes_codes.len() == 1 && no_codes.len() == 3,
        "shared confirmation choices must remain one and three glyphs"
    );
    let mut output = Vec::with_capacity(SHARED_CHOICE_RECORD_LENGTH);
    for code in yes_codes.into_iter().chain(no_codes) {
        output.extend_from_slice(&glyph_command(code));
    }
    output.resize(SHARED_CHOICE_RECORD_LENGTH, 0);
    Ok(output)
}

fn encode_text_codes(
    text: &str,
    glyph_codes: &BTreeMap<&str, u16>,
    allow_space: bool,
) -> Result<Vec<u16>> {
    text.chars()
        .map(|character| {
            if character == '?' {
                return Ok(0x0055);
            }
            if character == ' ' && allow_space {
                return Ok(FIXED_RECORD_SPACE_CODE);
            }
            let mut utf8 = [0; 4];
            let text = character.encode_utf8(&mut utf8);
            glyph_codes
                .get(text)
                .copied()
                .with_context(|| format!("bonus confirmation lacks an allocated glyph for {text}"))
        })
        .collect()
}

pub(super) fn glyph_command(code: u16) -> [u8; 3] {
    [
        (code >> 8) as u8,
        (code & 0x000f) as u8,
        ((code >> 4) & 0x000f) as u8,
    ]
}
