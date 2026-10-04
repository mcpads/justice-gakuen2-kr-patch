use anyhow::{Context, Result, ensure};
use psx_r3000a::{Instruction, decode, encode};
use serde::Serialize;

use crate::pipeline::sha256_bytes;

const MGAME_RUNTIME_BASE: u32 = 0x800a_2000;
const ORIGINAL_NAME_CONSUMER_ADDRESS: u32 = 0x800a_f9ac;
pub(super) const NAME_CONSUMER_HOOK_ADDRESSES: [u32; 2] = [0x800a_f750, 0x800a_f8dc];

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DialogueNameConsumerHookReport {
    pub byte_range: [usize; 2],
    pub address: String,
    pub source_sha256: String,
    pub source_instruction: String,
    pub replacement_sha256: String,
    pub replacement_instruction: String,
    pub source_verified: bool,
    pub installed: bool,
    pub runtime_execution_verified: bool,
}

pub(super) fn dialogue_name_consumer_replacements(
    consumer_address: u32,
) -> [(u32, Instruction); 2] {
    NAME_CONSUMER_HOOK_ADDRESSES.map(|address| {
        (
            address,
            Instruction::Jal {
                target: consumer_address,
            },
        )
    })
}

pub(super) fn install_dialogue_name_consumer_hooks(
    source: &[u8],
    patched: &mut [u8],
    consumer_address: u32,
) -> Result<[DialogueNameConsumerHookReport; 2]> {
    let source_instruction = Instruction::Jal {
        target: ORIGINAL_NAME_CONSUMER_ADDRESS,
    };
    let replacements = dialogue_name_consumer_replacements(consumer_address);

    let mut verified = Vec::with_capacity(replacements.len());
    for (address, replacement) in &replacements {
        let offset = runtime_offset(*address)?;
        let end = offset + 4;
        let source_bytes = source
            .get(offset..end)
            .context("dialogue name-consumer callsite is truncated")?;
        ensure!(
            patched.get(offset..end) == Some(source_bytes),
            "another MGAME writer changed a dialogue name-consumer callsite"
        );
        ensure!(
            decode(u32::from_le_bytes(source_bytes.try_into()?), *address)? == source_instruction,
            "dialogue name-consumer source call changed at 0x{address:08x}"
        );
        let replacement_bytes = encode(replacement, *address)?.to_le_bytes();
        verified.push((
            offset,
            source_bytes.to_vec(),
            replacement_bytes,
            replacement.clone(),
        ));
    }

    let mut reports = Vec::with_capacity(verified.len());
    for ((address, _), (offset, source_bytes, replacement_bytes, replacement)) in
        replacements.iter().zip(verified)
    {
        let end = offset + 4;
        patched
            .get_mut(offset..end)
            .context("dialogue name-consumer callsite destination is truncated")?
            .copy_from_slice(&replacement_bytes);
        ensure!(
            decode(
                u32::from_le_bytes(patched[offset..end].try_into()?),
                *address,
            )? == replacement,
            "dialogue name-consumer replacement changed at 0x{address:08x}"
        );
        reports.push(DialogueNameConsumerHookReport {
            byte_range: [offset, end],
            address: hex_address(*address),
            source_sha256: sha256_bytes(&source_bytes),
            source_instruction: format!("{source_instruction:?}"),
            replacement_sha256: sha256_bytes(&replacement_bytes),
            replacement_instruction: format!("{replacement:?}"),
            source_verified: true,
            installed: true,
            runtime_execution_verified: false,
        });
    }

    reports
        .try_into()
        .map_err(|_| anyhow::anyhow!("dialogue name-consumer hook report count changed"))
}

fn runtime_offset(address: u32) -> Result<usize> {
    usize::try_from(
        address
            .checked_sub(MGAME_RUNTIME_BASE)
            .context("dialogue name-consumer hook precedes MGAME runtime")?,
    )
    .map_err(Into::into)
}

fn hex_address(address: u32) -> String {
    format!("0x{address:08x}")
}
