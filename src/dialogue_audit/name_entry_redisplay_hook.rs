use anyhow::{Context, Result, ensure};
use psx_r3000a::{Instruction, Register, decode, encode};
use serde::Serialize;

use crate::name_input::{
    NAME_INPUT_REDISPLAY_RUNTIME_BYTE_CAPACITY, NAME_INPUT_REDISPLAY_RUNTIME_ORIGIN,
};
use crate::pipeline::sha256_bytes;

use super::name_entry::OVERLAY_RUNTIME_BASE;
use super::name_entry_renderer::REDISPLAY_ROUTINE_OFFSET;

pub(super) const HOOK_START_OFFSET: usize = 0x7514;
const HOOK_INSTRUCTION_COUNT: usize = 7;
const HOOK_BYTE_COUNT: usize = HOOK_INSTRUCTION_COUNT * 4;
const FIELD_COMPLETE_OFFSET: usize = 0x76c0;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct NameEntryRedisplayHookReport {
    pub redisplay_routine_file_offset: String,
    pub hook_file_offset: String,
    pub hook_runtime_address: String,
    pub overwritten_byte_count: usize,
    pub tagged_code_resolver_address: String,
    pub source_instructions_verified: bool,
    pub typed_hook_instruction_count: usize,
    pub replacement_sha256: String,
    pub installed: bool,
    pub runtime_execution_verified: bool,
}

pub(super) fn install_name_entry_redisplay_hook(
    overlay: &mut [u8],
    tagged_code_resolver_address: u32,
) -> Result<NameEntryRedisplayHookReport> {
    let runtime_end = NAME_INPUT_REDISPLAY_RUNTIME_ORIGIN
        .checked_add(u32::try_from(NAME_INPUT_REDISPLAY_RUNTIME_BYTE_CAPACITY)?)
        .context("name-input redisplay runtime range overflow")?;
    ensure!(
        tagged_code_resolver_address >= NAME_INPUT_REDISPLAY_RUNTIME_ORIGIN
            && tagged_code_resolver_address < runtime_end
            && tagged_code_resolver_address.is_multiple_of(4),
        "tagged-name resolver lies outside its typed runtime"
    );

    let expected = source_instructions();
    for (index, expected_instruction) in expected.iter().enumerate() {
        let offset = HOOK_START_OFFSET + index * 4;
        let instruction = decode(read_word(overlay, offset)?, runtime_address(offset))?;
        ensure!(
            instruction == *expected_instruction,
            "name-entry redisplay hook source changed at {offset:#x}"
        );
    }

    let replacement = build_name_entry_redisplay_replacement(tagged_code_resolver_address);
    let mut replacement_bytes = Vec::with_capacity(HOOK_BYTE_COUNT);
    for (index, instruction) in replacement.iter().enumerate() {
        let offset = HOOK_START_OFFSET + index * 4;
        let bytes = encode(instruction, runtime_address(offset))?.to_le_bytes();
        overlay
            .get_mut(offset..offset + 4)
            .context("name-entry redisplay hook destination is truncated")?
            .copy_from_slice(&bytes);
        replacement_bytes.extend_from_slice(&bytes);
    }
    for (index, expected_instruction) in replacement.iter().enumerate() {
        let offset = HOOK_START_OFFSET + index * 4;
        ensure!(
            decode(read_word(overlay, offset)?, runtime_address(offset))? == *expected_instruction,
            "name-entry redisplay hook readback changed at {offset:#x}"
        );
    }

    Ok(NameEntryRedisplayHookReport {
        redisplay_routine_file_offset: format!("0x{REDISPLAY_ROUTINE_OFFSET:04x}"),
        hook_file_offset: format!("0x{HOOK_START_OFFSET:04x}"),
        hook_runtime_address: format!("0x{:08x}", runtime_address(HOOK_START_OFFSET)),
        overwritten_byte_count: HOOK_BYTE_COUNT,
        tagged_code_resolver_address: format!("0x{tagged_code_resolver_address:08x}"),
        source_instructions_verified: true,
        typed_hook_instruction_count: replacement.len(),
        replacement_sha256: sha256_bytes(&replacement_bytes),
        installed: true,
        runtime_execution_verified: false,
    })
}

fn source_instructions() -> [Instruction; HOOK_INSTRUCTION_COUNT] {
    [
        Instruction::Lhu {
            rt: Register::A3,
            base: Register::S4,
            offset: 0,
        },
        Instruction::Addiu {
            rt: Register::V0,
            rs: Register::ZERO,
            immediate: 0x3001,
        },
        Instruction::Beq {
            rs: Register::A3,
            rt: Register::V0,
            target: runtime_address(FIELD_COMPLETE_OFFSET),
        },
        Instruction::Addu {
            rd: Register::A2,
            rs: Register::ZERO,
            rt: Register::ZERO,
        },
        Instruction::Lui {
            rt: Register::A1,
            immediate: 0x8018,
        },
        Instruction::Addiu {
            rt: Register::A1,
            rs: Register::A1,
            immediate: -21228,
        },
        Instruction::Lui {
            rt: Register::V1,
            immediate: 0x801f,
        },
    ]
}

pub(super) fn build_name_entry_redisplay_replacement(
    tagged_code_resolver_address: u32,
) -> [Instruction; HOOK_INSTRUCTION_COUNT] {
    [
        Instruction::Lhu {
            rt: Register::A0,
            base: Register::S4,
            offset: 0,
        },
        Instruction::Addu {
            rd: Register::A1,
            rs: Register::S5,
            rt: Register::ZERO,
        },
        Instruction::Jal {
            target: tagged_code_resolver_address,
        },
        Instruction::Addu {
            rd: Register::A2,
            rs: Register::S3,
            rt: Register::ZERO,
        },
        Instruction::Addu {
            rd: Register::A3,
            rs: Register::V0,
            rt: Register::ZERO,
        },
        Instruction::Addiu {
            rt: Register::V0,
            rs: Register::ZERO,
            immediate: 0x3001,
        },
        Instruction::Beq {
            rs: Register::A3,
            rt: Register::V0,
            target: runtime_address(FIELD_COMPLETE_OFFSET),
        },
    ]
}

#[cfg(test)]
pub(super) fn install_name_entry_redisplay_hook_fixture(overlay: &mut [u8]) {
    for (index, instruction) in source_instructions().iter().enumerate() {
        let offset = HOOK_START_OFFSET + index * 4;
        overlay[offset..offset + 4].copy_from_slice(
            &encode(instruction, runtime_address(offset))
                .unwrap()
                .to_le_bytes(),
        );
    }
}

fn read_word(bytes: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        bytes
            .get(offset..offset + 4)
            .context("name-entry redisplay hook source is truncated")?
            .try_into()?,
    ))
}

fn runtime_address(offset: usize) -> u32 {
    OVERLAY_RUNTIME_BASE + u32::try_from(offset).expect("MGENT offsets fit u32")
}
