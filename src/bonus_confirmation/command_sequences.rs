use std::collections::BTreeMap;

use anyhow::{Context, Result, ensure};

use super::model::ConfirmationTextUnit;
use super::records::glyph_command;
use super::text_units::authored_text;

pub(super) const MEMORY_CARD_DESTINATION_OFFSET: usize = 0x0fe4;
pub(super) const MEMORY_CARD_DESTINATION_LENGTH: usize = 32;
pub(super) const MEMORY_CARD_COPY_PROMPT_OFFSET: usize = 0x1004;
pub(super) const MEMORY_CARD_COPY_PROMPT_LENGTH: usize = 40;

pub(super) const SOURCE_MEMORY_CARD_DESTINATION: [u8; MEMORY_CARD_DESTINATION_LENGTH] = [
    0x00, 0x01, 0x08, 0x00, 0x04, 0x09, 0x01, 0x09, 0x04, 0x01, 0x0a, 0x04, 0x01, 0x03, 0x05, 0x00,
    0x04, 0x06, 0x01, 0x05, 0x02, 0x00, 0x04, 0x06, 0x01, 0x01, 0x07, 0x00, 0x01, 0x09, 0x81, 0x00,
];
pub(super) const SOURCE_MEMORY_CARD_COPY_PROMPT: [u8; MEMORY_CARD_COPY_PROMPT_LENGTH] = [
    0x01, 0x09, 0x02, 0x01, 0x08, 0x07, 0x00, 0x04, 0x06, 0x00, 0x03, 0x08, 0x00, 0x0a, 0x08, 0x00,
    0x05, 0x0a, 0x00, 0x0a, 0x0a, 0x00, 0x03, 0x08, 0x00, 0x05, 0x07, 0x01, 0x03, 0x00, 0x00, 0x04,
    0x08, 0x00, 0x09, 0x07, 0x00, 0x05, 0x05, 0x81,
];

pub(super) const SOURCE_MEMORY_CARD_DESTINATION_SHA256: &str =
    "58057bc1293f69ff5344316215f5f7860fe252a10529c6b78a44656a96cf82ee";
pub(super) const SOURCE_MEMORY_CARD_COPY_PROMPT_SHA256: &str =
    "ca161f7745553c8acbc49a8a893f8e3958279f5496db9ffa363fbf6d052f5da9";

pub(super) struct EncodedMemoryCardSequences {
    pub(super) destination: Vec<u8>,
    pub(super) copy_prompt: Vec<u8>,
}

pub(super) fn encode_memory_card_sequences(
    units: &[ConfirmationTextUnit],
    glyph_codes: &BTreeMap<&str, u16>,
) -> Result<EncodedMemoryCardSequences> {
    Ok(EncodedMemoryCardSequences {
        destination: encode_command_sequence(
            authored_text(units, "memory_card_destination")?,
            glyph_codes,
            MEMORY_CARD_DESTINATION_LENGTH,
        )?,
        copy_prompt: encode_command_sequence(
            authored_text(units, "memory_card_copy_prompt")?,
            glyph_codes,
            MEMORY_CARD_COPY_PROMPT_LENGTH,
        )?,
    })
}

fn encode_command_sequence(
    text: &str,
    glyph_codes: &BTreeMap<&str, u16>,
    storage_length: usize,
) -> Result<Vec<u8>> {
    let mut output = Vec::with_capacity(storage_length);
    for character in text.chars() {
        if character == ' ' {
            output.extend_from_slice(&[0x63; 3]);
            continue;
        }
        let code = if character == '?' {
            0x0055
        } else {
            let mut utf8 = [0; 4];
            let text = character.encode_utf8(&mut utf8);
            glyph_codes.get(text).copied().with_context(|| {
                format!("bonus confirmation command sequence lacks an allocated glyph for {text}")
            })?
        };
        output.extend_from_slice(&glyph_command(code));
    }
    output.push(0x81);
    ensure!(
        output.len() <= storage_length,
        "Korean memory-card command sequence exceeds its source storage"
    );
    output.resize(storage_length, 0);
    Ok(output)
}
