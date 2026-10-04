use anyhow::{Context, Result, ensure};
use psx_r3000a::{Instruction, Register, decode, encode};
use serde::Serialize;

use crate::pipeline::sha256_bytes;

use super::name_entry::OVERLAY_RUNTIME_BASE;

pub(super) const HOOK_OFFSET: usize = 0x1dac;
const HOOK_INSTRUCTION_COUNT: usize = 4;
const CONFIRMATION_CONTINUE_OFFSET: usize = 0x1dbc;
const CONFIRMATION_EPILOGUE_OFFSET: usize = 0x1f54;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct NameEntryConfirmationHookReport {
    pub hook_file_offset: String,
    pub hook_runtime_address: String,
    pub overwritten_byte_count: usize,
    pub confirmation_handler_address: String,
    pub source_instructions_verified: bool,
    pub typed_hook_instruction_count: usize,
    pub replacement_sha256: String,
    pub installed: bool,
    pub runtime_execution_verified: bool,
}

pub(super) fn install_name_entry_confirmation_hook(
    source: &[u8],
    patched: &mut [u8],
    confirmation_handler_address: u32,
) -> Result<NameEntryConfirmationHookReport> {
    ensure!(
        confirmation_handler_address.is_multiple_of(4),
        "name confirmation handler address is unaligned"
    );
    let expected = source_instructions();
    let mut replacement_bytes = Vec::with_capacity(HOOK_INSTRUCTION_COUNT * 4);
    for (index, expected_instruction) in expected.iter().enumerate() {
        let offset = HOOK_OFFSET + index * 4;
        ensure!(
            decode(read_word(source, offset)?, runtime_address(offset))? == *expected_instruction,
            "name confirmation hook source changed at {offset:#x}"
        );
        ensure!(
            read_word(patched, offset)? == read_word(source, offset)?,
            "another name-entry writer changed the confirmation hook at {offset:#x}"
        );
    }

    let replacement = build_name_entry_confirmation_replacement(confirmation_handler_address);
    for (index, instruction) in replacement.iter().enumerate() {
        let offset = HOOK_OFFSET + index * 4;
        let bytes = encode(instruction, runtime_address(offset))?.to_le_bytes();
        patched
            .get_mut(offset..offset + 4)
            .context("name confirmation hook destination is truncated")?
            .copy_from_slice(&bytes);
        replacement_bytes.extend_from_slice(&bytes);
    }
    for (index, expected_instruction) in replacement.iter().enumerate() {
        let offset = HOOK_OFFSET + index * 4;
        ensure!(
            decode(read_word(patched, offset)?, runtime_address(offset))? == *expected_instruction,
            "name confirmation hook readback changed at {offset:#x}"
        );
    }

    Ok(NameEntryConfirmationHookReport {
        hook_file_offset: format!("0x{HOOK_OFFSET:04x}"),
        hook_runtime_address: format!("0x{:08x}", runtime_address(HOOK_OFFSET)),
        overwritten_byte_count: HOOK_INSTRUCTION_COUNT * 4,
        confirmation_handler_address: format!("0x{confirmation_handler_address:08x}"),
        source_instructions_verified: true,
        typed_hook_instruction_count: replacement.len(),
        replacement_sha256: sha256_bytes(&replacement_bytes),
        installed: true,
        runtime_execution_verified: false,
    })
}

pub(super) fn build_name_entry_confirmation_replacement(
    confirmation_handler_address: u32,
) -> [Instruction; HOOK_INSTRUCTION_COUNT] {
    let expected = source_instructions();
    [
        expected[0].clone(),
        expected[1].clone(),
        Instruction::J {
            target: confirmation_handler_address,
        },
        Instruction::Addu {
            rd: Register::A0,
            rs: Register::S0,
            rt: Register::ZERO,
        },
    ]
}

fn source_instructions() -> [Instruction; HOOK_INSTRUCTION_COUNT] {
    [
        Instruction::Beq {
            rs: Register::V0,
            rt: Register::ZERO,
            target: runtime_address(CONFIRMATION_CONTINUE_OFFSET),
        },
        Instruction::Andi {
            rt: Register::V0,
            rs: Register::V1,
            immediate: 0x00a0,
        },
        Instruction::J {
            target: runtime_address(CONFIRMATION_EPILOGUE_OFFSET),
        },
        Instruction::Addiu {
            rt: Register::V0,
            rs: Register::ZERO,
            immediate: 1,
        },
    ]
}

fn read_word(bytes: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        bytes
            .get(offset..offset + 4)
            .context("name confirmation hook source is truncated")?
            .try_into()?,
    ))
}

const fn runtime_address(offset: usize) -> u32 {
    OVERLAY_RUNTIME_BASE + offset as u32
}
