use std::collections::BTreeMap;

use anyhow::{Context, Result, ensure};

use super::model::{JBankReturnLabelGlyphAllocation, JBankReturnLabelUnit};

const SOURCE_BYTES: [u8; 12] = [
    0x00, 0x02, 0x0a, 0x01, 0x04, 0x00, 0x00, 0x08, 0x0a, 0x81, 0x00, 0x00,
];

#[derive(Clone, Copy)]
pub(super) struct JBankReturnLabelSequenceSpec {
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

pub(super) const RETURN_LABEL_SEQUENCE: JBankReturnLabelSequenceSpec =
    JBankReturnLabelSequenceSpec {
        id: "return_label",
        source_text: "もどる",
        sequence_offset: 0x0d78,
        storage_length: SOURCE_BYTES.len(),
        terminator_offset: 0x0d81,
        pointer_storage_offset: 0x13b4,
        source_sha256: "b27fab01f6fe2bbaa50059405077a6b58203c72e9bf9a6b2e216ab0b4ac3f9af",
        source_bytes: &SOURCE_BYTES,
        caller_runtime_addresses: &[0x800a_ea98, 0x800b_04b8],
    };

#[derive(Debug)]
pub(super) struct EncodedJBankReturnLabel {
    pub(super) bytes: Vec<u8>,
    pub(super) payload_byte_length: usize,
    pub(super) command_count: usize,
}

pub(super) fn encode_return_label(
    unit: &JBankReturnLabelUnit,
    allocations: &[JBankReturnLabelGlyphAllocation],
) -> Result<EncodedJBankReturnLabel> {
    ensure!(
        unit.id == RETURN_LABEL_SEQUENCE.id,
        "J-BANK return-label unit identity changed"
    );
    let glyph_codes = allocations
        .iter()
        .map(|allocation| (allocation.text, allocation.code))
        .collect::<BTreeMap<_, _>>();
    let text = unit
        .korean_text
        .as_deref()
        .context("authored J-BANK return-label text disappeared")?;
    let command_capacity = (RETURN_LABEL_SEQUENCE.storage_length - 1) / 3;
    let mut bytes = Vec::with_capacity(RETURN_LABEL_SEQUENCE.storage_length);
    let mut command_count = 0;
    for character in text.chars() {
        ensure!(
            !character.is_control() && !character.is_whitespace(),
            "J-BANK return label does not support control or whitespace commands"
        );
        let code = glyph_codes
            .get(&character)
            .with_context(|| format!("no J-BANK return-label glyph for {character:?}"))?;
        bytes.extend_from_slice(&glyph_command(*code));
        command_count += 1;
    }
    ensure!(
        command_count > 0 && command_count <= command_capacity,
        "J-BANK return label exceeds its source command capacity"
    );
    bytes.push(0x81);
    let payload_byte_length = bytes.len();
    bytes.resize(RETURN_LABEL_SEQUENCE.storage_length, 0);
    Ok(EncodedJBankReturnLabel {
        bytes,
        payload_byte_length,
        command_count,
    })
}

fn glyph_command(code: u16) -> [u8; 3] {
    [
        (code >> 8) as u8,
        (code & 0x000f) as u8,
        ((code >> 4) & 0x000f) as u8,
    ]
}
