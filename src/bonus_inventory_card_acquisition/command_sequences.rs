use std::collections::BTreeMap;

use anyhow::{Context, Result, ensure};

use super::glyph_slots::{CARD_ACQUISITION_GLYPHS, REUSED_SOURCE_GLYPHS};
use super::model::CardAcquisitionUnit;

const RARE_LABEL_SOURCE: [u8; 8] = [0x01, 0x05, 0x05, 0x01, 0x00, 0x02, 0x81, 0x00];
const CARD_NUMBER_PREFIX_SOURCE: [u8; 28] = [
    0x01, 0x00, 0x0a, 0x01, 0x01, 0x0a, 0x01, 0x05, 0x02, 0x00, 0x04, 0x06, 0x01, 0x01, 0x07, 0x00,
    0x0b, 0x01, 0x00, 0x02, 0x04, 0x01, 0x09, 0x09, 0x81, 0x00, 0x00, 0x00,
];
const ACQUIRED_MESSAGE_SOURCE: [u8; 28] = [
    0x00, 0x00, 0x0b, 0x02, 0x01, 0x00, 0x00, 0x01, 0x09, 0x02, 0x02, 0x00, 0x00, 0x09, 0x0a, 0x00,
    0x0a, 0x09, 0x00, 0x03, 0x08, 0x00, 0x07, 0x08, 0x81, 0x00, 0x00, 0x00,
];

#[derive(Clone, Copy)]
pub(super) struct CardAcquisitionSequenceSpec {
    pub(super) id: &'static str,
    pub(super) source_text: &'static str,
    pub(super) sequence_offset: usize,
    pub(super) storage_length: usize,
    pub(super) terminator_offset: usize,
    pub(super) pointer_storage_offset: usize,
    pub(super) source_sha256: &'static str,
    pub(super) source_bytes: &'static [u8],
    pub(super) caller_runtime_addresses: &'static [u32],
}

pub(super) const CARD_ACQUISITION_SEQUENCES: [CardAcquisitionSequenceSpec; 3] = [
    CardAcquisitionSequenceSpec {
        id: "rare_label",
        source_text: "レア",
        sequence_offset: 0x102c,
        storage_length: 8,
        terminator_offset: 0x1032,
        pointer_storage_offset: 0x13f4,
        source_sha256: "f059432d64fa6739cf2bb5ed3072ddc6c7863b706604c94ef6ba620e38dda324",
        source_bytes: &RARE_LABEL_SOURCE,
        caller_runtime_addresses: &[0x800a_f344, 0x800a_f410],
    },
    CardAcquisitionSequenceSpec {
        id: "card_number_prefix",
        source_text: "熱血カードNo.",
        sequence_offset: 0x1034,
        storage_length: 28,
        terminator_offset: 0x104c,
        pointer_storage_offset: 0x13f8,
        source_sha256: "4167bf8d5c656997ff9dc86d2aef52b1d946f26afff7675fba019b68974d60aa",
        source_bytes: &CARD_NUMBER_PREFIX_SOURCE,
        caller_runtime_addresses: &[0x800a_f370, 0x800a_f42c],
    },
    CardAcquisitionSequenceSpec {
        id: "acquired_message",
        source_text: "を手に入れました",
        sequence_offset: 0x1050,
        storage_length: 28,
        terminator_offset: 0x1068,
        pointer_storage_offset: 0x13fc,
        source_sha256: "cf7f1b758d870f9e468f61ce78faf4bf954aff41114844abd76a24996dbbc7df",
        source_bytes: &ACQUIRED_MESSAGE_SOURCE,
        caller_runtime_addresses: &[0x800a_f478],
    },
];

pub(super) fn encode_card_acquisition_sequences(
    units: &[CardAcquisitionUnit],
) -> Result<Vec<Vec<u8>>> {
    ensure!(
        units.len() == CARD_ACQUISITION_SEQUENCES.len(),
        "bonus card-acquisition unit count changed"
    );
    let glyph_codes = CARD_ACQUISITION_GLYPHS
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
        .zip(CARD_ACQUISITION_SEQUENCES)
        .map(|(unit, spec)| {
            ensure!(
                unit.id == spec.id,
                "bonus card-acquisition unit order changed"
            );
            encode_card_acquisition_text(
                unit.korean_text
                    .as_deref()
                    .context("authored bonus card-acquisition text disappeared")?,
                spec.storage_length,
                &glyph_codes,
            )
        })
        .collect()
}

pub(super) fn encode_card_acquisition_text(
    text: &str,
    storage_length: usize,
    glyph_codes: &BTreeMap<&str, u16>,
) -> Result<Vec<u8>> {
    ensure!(storage_length > 0, "command-sequence storage is empty");
    let command_capacity = (storage_length - 1) / 3;
    let mut encoded = Vec::with_capacity(storage_length);
    let mut command_count = 0;
    for character in text.chars() {
        if character == ' ' {
            encoded.extend_from_slice(&[0x63; 3]);
        } else {
            let mut utf8 = [0; 4];
            let text = character.encode_utf8(&mut utf8);
            let code = glyph_codes.get(text).with_context(|| {
                format!("bonus card-acquisition text lacks an allocated glyph for {text}")
            })?;
            encoded.extend_from_slice(&glyph_command(*code));
        }
        command_count += 1;
    }
    ensure!(
        command_count <= command_capacity,
        "bonus card-acquisition text exceeds its fixed source command capacity"
    );
    encoded.push(0x81);
    encoded.resize(storage_length, 0);
    ensure!(
        encoded.len() == storage_length,
        "bonus card-acquisition fixed record length changed"
    );
    Ok(encoded)
}

fn glyph_command(code: u16) -> [u8; 3] {
    [
        (code >> 8) as u8,
        (code & 0x000f) as u8,
        ((code >> 4) & 0x000f) as u8,
    ]
}
