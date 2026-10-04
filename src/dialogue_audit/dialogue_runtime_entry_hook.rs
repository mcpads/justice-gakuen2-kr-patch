use std::ops::Range;

use anyhow::{Context, Result, ensure};
use psx_r3000a::{Instruction, Register, decode, encode};
use serde::Serialize;

use crate::name_input::NAME_DIALOGUE_RUNTIME_BOOTSTRAP_ORIGIN;
use crate::pipeline::sha256_bytes;
use crate::source_disc::{MAIN_TEXT_RUNTIME_BASE, PSX_EXE_HEADER_SIZE};

const ENTRY_HOOK_TEXT_OFFSET: usize = 0x0000_0008;
const ENTRY_HOOK_FILE_OFFSET: usize = PSX_EXE_HEADER_SIZE + ENTRY_HOOK_TEXT_OFFSET;
const ENTRY_HOOK_BYTE_COUNT: usize = 8;
const ENTRY_HOOK_SOURCE_SHA256: &str =
    "98c8cb2497072bcef1e0cbc50d48e7d46e8e350093b91ddda6176d8384c2673d";
const ENTRY_DELAY_SLOT_IMMEDIATE: i16 = -26_680;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DialogueRuntimeEntryHookReport {
    pub file_byte_range: [usize; 2],
    pub address_range: [String; 2],
    pub source_sha256: String,
    pub source_instructions: Vec<String>,
    pub replacement_sha256: String,
    pub replacement_instructions: Vec<String>,
    pub source_verified: bool,
    pub installed: bool,
    pub runtime_execution_verified: bool,
}

pub(super) struct DialogueRuntimeEntryHookInstall {
    pub(super) report: DialogueRuntimeEntryHookReport,
    pub(super) file_range: Range<usize>,
    pub(super) runtime_address: u32,
    pub(super) replacement_instructions: Vec<Instruction>,
}

pub(super) fn install_dialogue_runtime_entry_hook(
    source: &[u8],
    patched: &mut [u8],
) -> Result<DialogueRuntimeEntryHookInstall> {
    let entry_address = MAIN_TEXT_RUNTIME_BASE + ENTRY_HOOK_TEXT_OFFSET as u32;
    let source_bytes = source
        .get(ENTRY_HOOK_FILE_OFFSET..ENTRY_HOOK_FILE_OFFSET + ENTRY_HOOK_BYTE_COUNT)
        .context("main executable entry hook is truncated")?;
    ensure!(
        sha256_bytes(source_bytes) == ENTRY_HOOK_SOURCE_SHA256,
        "main executable entry hook bytes changed"
    );
    let source_instructions = [
        Instruction::Lui {
            rt: Register::V0,
            immediate: 0x800a,
        },
        Instruction::Addiu {
            rt: Register::V0,
            rs: Register::V0,
            immediate: ENTRY_DELAY_SLOT_IMMEDIATE,
        },
    ];
    let replacement = [
        Instruction::Jal {
            target: NAME_DIALOGUE_RUNTIME_BOOTSTRAP_ORIGIN,
        },
        source_instructions[0].clone(),
    ];
    for (index, expected) in source_instructions.iter().enumerate() {
        let offset = index * 4;
        ensure!(
            decode(
                u32::from_le_bytes(source_bytes[offset..offset + 4].try_into()?),
                entry_address + offset as u32,
            )? == *expected,
            "main executable entry hook instruction changed"
        );
    }

    let mut replacement_bytes = Vec::with_capacity(ENTRY_HOOK_BYTE_COUNT);
    for (index, instruction) in replacement.iter().enumerate() {
        replacement_bytes.extend_from_slice(
            &encode(instruction, entry_address + (index * 4) as u32)?.to_le_bytes(),
        );
    }
    patched[ENTRY_HOOK_FILE_OFFSET..ENTRY_HOOK_FILE_OFFSET + ENTRY_HOOK_BYTE_COUNT]
        .copy_from_slice(&replacement_bytes);
    for (index, expected) in replacement.iter().enumerate() {
        let offset = ENTRY_HOOK_FILE_OFFSET + index * 4;
        ensure!(
            decode(
                u32::from_le_bytes(patched[offset..offset + 4].try_into()?),
                entry_address + (index * 4) as u32,
            )? == *expected,
            "main executable entry hook readback changed"
        );
    }

    Ok(DialogueRuntimeEntryHookInstall {
        report: DialogueRuntimeEntryHookReport {
            file_byte_range: [
                ENTRY_HOOK_FILE_OFFSET,
                ENTRY_HOOK_FILE_OFFSET + ENTRY_HOOK_BYTE_COUNT,
            ],
            address_range: [
                hex_address(entry_address),
                hex_address(entry_address + ENTRY_HOOK_BYTE_COUNT as u32),
            ],
            source_sha256: ENTRY_HOOK_SOURCE_SHA256.to_string(),
            source_instructions: source_instructions
                .iter()
                .map(|instruction| format!("{instruction:?}"))
                .collect(),
            replacement_sha256: sha256_bytes(&replacement_bytes),
            replacement_instructions: replacement
                .iter()
                .map(|instruction| format!("{instruction:?}"))
                .collect(),
            source_verified: true,
            installed: true,
            runtime_execution_verified: false,
        },
        file_range: ENTRY_HOOK_FILE_OFFSET..ENTRY_HOOK_FILE_OFFSET + ENTRY_HOOK_BYTE_COUNT,
        runtime_address: entry_address,
        replacement_instructions: replacement.to_vec(),
    })
}

fn hex_address(address: u32) -> String {
    format!("0x{address:08x}")
}
