use std::collections::BTreeMap;

use anyhow::{Context, Result, ensure};

use super::consumer::RENDERER_LINE_COMMAND_LIMIT;
use super::glyph_slots::{MEMORY_CARD_SWAP_GLYPHS, REUSED_SOURCE_GLYPHS};
use super::model::MemoryCardSwapUnit;

const REPLACE_MEMORY_CARD_SOURCE: [u8; 80] = [
    0x01, 0x09, 0x04, 0x01, 0x0a, 0x04, 0x01, 0x03, 0x05, 0x00, 0x04, 0x06, 0x01, 0x05, 0x02, 0x00,
    0x04, 0x06, 0x01, 0x01, 0x07, 0x00, 0x00, 0x0b, 0x00, 0x02, 0x08, 0x00, 0x03, 0x08, 0x00, 0x09,
    0x07, 0x00, 0x07, 0x07, 0x00, 0x0a, 0x08, 0x80, 0x02, 0x07, 0x03, 0x00, 0x09, 0x07, 0x01, 0x06,
    0x07, 0x01, 0x03, 0x03, 0x01, 0x09, 0x05, 0x00, 0x00, 0x0b, 0x02, 0x04, 0x0b, 0x00, 0x03, 0x08,
    0x00, 0x0a, 0x08, 0x00, 0x0b, 0x07, 0x01, 0x00, 0x00, 0x00, 0x02, 0x08, 0x00, 0x05, 0x07, 0x81,
];
const RESTORE_ORIGINAL_MEMORY_CARD_SOURCE: [u8; 88] = [
    0x00, 0x02, 0x0a, 0x00, 0x0b, 0x08, 0x00, 0x04, 0x09, 0x01, 0x09, 0x04, 0x01, 0x0a, 0x04, 0x01,
    0x03, 0x05, 0x00, 0x04, 0x06, 0x01, 0x05, 0x02, 0x00, 0x04, 0x06, 0x01, 0x01, 0x07, 0x00, 0x01,
    0x09, 0x00, 0x02, 0x0a, 0x01, 0x04, 0x00, 0x00, 0x03, 0x08, 0x00, 0x0a, 0x08, 0x80, 0x02, 0x07,
    0x03, 0x00, 0x09, 0x07, 0x01, 0x06, 0x07, 0x01, 0x03, 0x03, 0x01, 0x09, 0x05, 0x00, 0x00, 0x0b,
    0x02, 0x04, 0x0b, 0x00, 0x03, 0x08, 0x00, 0x0a, 0x08, 0x00, 0x0b, 0x07, 0x01, 0x00, 0x00, 0x00,
    0x02, 0x08, 0x00, 0x05, 0x07, 0x81, 0x00, 0x00,
];

#[derive(Clone, Copy)]
pub(super) struct MemoryCardSwapSequenceSpec {
    pub(super) id: &'static str,
    pub(super) source_text: &'static str,
    pub(super) sequence_offset: usize,
    pub(super) storage_length: usize,
    pub(super) terminator_offset: usize,
    pub(super) pointer_storage_offset: usize,
    pub(super) source_sha256: &'static str,
    pub(super) source_bytes: &'static [u8],
    pub(super) source_line_command_counts: &'static [usize],
    pub(super) caller_runtime_addresses: &'static [u32],
}

pub(super) const MEMORY_CARD_SWAP_SEQUENCES: [MemoryCardSwapSequenceSpec; 2] = [
    MemoryCardSwapSequenceSpec {
        id: "replace_memory_card",
        source_text: "メモリーカードをさしかえて\n何かボタンを押してください",
        sequence_offset: 0x1138,
        storage_length: 80,
        terminator_offset: 0x1187,
        pointer_storage_offset: 0x1410,
        source_sha256: "1faa3655d9c3110ac23ac503f6b57ab6cf882a3c29896465538b759ff5efe167",
        source_bytes: &REPLACE_MEMORY_CARD_SOURCE,
        source_line_command_counts: &[13, 13],
        caller_runtime_addresses: &[0x800a_f97c],
    },
    MemoryCardSwapSequenceSpec {
        id: "restore_original_memory_card",
        source_text: "もとのメモリーカードにもどして\n何かボタンを押してください",
        sequence_offset: 0x1188,
        storage_length: 88,
        terminator_offset: 0x11dd,
        pointer_storage_offset: 0x1414,
        source_sha256: "bf32a6461b275e17a02a8794bec172be142bfa47b33dd55db4a86937586687a0",
        source_bytes: &RESTORE_ORIGINAL_MEMORY_CARD_SOURCE,
        source_line_command_counts: &[15, 13],
        caller_runtime_addresses: &[0x800a_fdac],
    },
];

#[derive(Debug)]
pub(super) struct EncodedMemoryCardSwapSequence {
    pub(super) bytes: Vec<u8>,
    pub(super) payload_byte_length: usize,
    pub(super) line_command_counts: Vec<usize>,
}

pub(super) fn encode_memory_card_swap_sequences(
    units: &[MemoryCardSwapUnit],
) -> Result<Vec<EncodedMemoryCardSwapSequence>> {
    ensure!(
        units.len() == MEMORY_CARD_SWAP_SEQUENCES.len(),
        "bonus memory-card-swap unit count changed"
    );
    let glyph_codes = MEMORY_CARD_SWAP_GLYPHS
        .iter()
        .map(|(text, code, _)| (*text, *code))
        .chain(
            REUSED_SOURCE_GLYPHS
                .iter()
                .map(|(text, code, _, _)| (*text, *code)),
        )
        .collect::<BTreeMap<_, _>>();
    units
        .iter()
        .zip(MEMORY_CARD_SWAP_SEQUENCES)
        .map(|(unit, spec)| {
            ensure!(
                unit.id == spec.id,
                "bonus memory-card-swap unit order changed"
            );
            encode_memory_card_swap_text(
                unit.korean_text
                    .as_deref()
                    .context("authored bonus memory-card-swap text disappeared")?,
                spec.storage_length,
                &glyph_codes,
            )
        })
        .collect()
}

pub(super) fn encode_memory_card_swap_text(
    text: &str,
    storage_length: usize,
    glyph_codes: &BTreeMap<&str, u16>,
) -> Result<EncodedMemoryCardSwapSequence> {
    ensure!(storage_length > 0, "command-sequence storage is empty");
    let mut bytes = Vec::with_capacity(storage_length);
    let mut line_command_counts = vec![0usize];
    for character in text.chars() {
        match character {
            '\n' => {
                ensure!(
                    line_command_counts.last().copied() != Some(0),
                    "bonus memory-card-swap text has an empty line"
                );
                bytes.push(0x80);
                line_command_counts.push(0);
            }
            ' ' => {
                bytes.extend_from_slice(&[0x63; 3]);
                *line_command_counts.last_mut().expect("one line") += 1;
            }
            _ => {
                let mut utf8 = [0; 4];
                let text = character.encode_utf8(&mut utf8);
                let code = glyph_codes.get(text).with_context(|| {
                    format!("bonus memory-card-swap text lacks an allocated glyph for {text}")
                })?;
                bytes.extend_from_slice(&glyph_command(*code));
                *line_command_counts.last_mut().expect("one line") += 1;
            }
        }
    }
    ensure!(
        line_command_counts.len() == 2 && line_command_counts.iter().all(|count| *count > 0),
        "bonus memory-card-swap text must contain exactly two non-empty lines"
    );
    ensure!(
        line_command_counts
            .iter()
            .all(|count| *count <= RENDERER_LINE_COMMAND_LIMIT),
        "bonus memory-card-swap line exceeds the renderer width"
    );
    bytes.push(0x81);
    let payload_byte_length = bytes.len();
    ensure!(
        payload_byte_length <= storage_length,
        "bonus memory-card-swap text exceeds its fixed source storage"
    );
    bytes.resize(storage_length, 0);
    Ok(EncodedMemoryCardSwapSequence {
        bytes,
        payload_byte_length,
        line_command_counts,
    })
}

fn glyph_command(code: u16) -> [u8; 3] {
    [
        (code >> 8) as u8,
        (code & 0x000f) as u8,
        ((code >> 4) & 0x000f) as u8,
    ]
}
