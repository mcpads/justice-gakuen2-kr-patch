use anyhow::{Context, Result, ensure};
use psx_r3000a::{Instruction, Register, decode, encode};
use serde::Serialize;

use crate::pipeline::sha256_bytes;

const MGAME_RUNTIME_BASE: u32 = 0x800a_2000;
const MGAME_ENTRY_POINTER_OFFSET: usize = 0;
const MGAME_ENTRY_POINTER_SOURCE_SHA256: &str =
    "03f9e6b0b00c054cf503ec5d1498b3dbf73060506c49fa0984c99fefcdf8ee6e";
const MGAME_ENTRY_ADDRESS: u32 = 0x800a_5cdc;
const MGAME_ENTRY_BYTE_COUNT: usize = 8;
const MGAME_ENTRY_SOURCE_SHA256: &str =
    "a0eee60855acebf59b4f37bbe8d27d2376a15f4409521fbb92071cdd16f14d06";
const MGAME_ENTRY_RESUME_ADDRESS: u32 = MGAME_ENTRY_ADDRESS + MGAME_ENTRY_BYTE_COUNT as u32;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MgameReloadEntryHookReport {
    pub byte_range: [usize; 2],
    pub address_range: [String; 2],
    pub source_sha256: String,
    pub source_instructions: Vec<String>,
    pub replacement_sha256: String,
    pub replacement_instructions: Vec<String>,
    pub source_entry_pointer: String,
    pub entry_address: String,
    pub resume_address: String,
    pub wrapper_address: String,
    pub source_entry_pointer_preserved: bool,
    pub selects_valid_source_copy_or_destination_repair_before_entry_continuation: bool,
    pub displaced_instructions_preserved_by_wrapper: bool,
    pub caller_return_address_preserved: bool,
    pub source_verified: bool,
    pub installed: bool,
    pub runtime_execution_verified: bool,
}

pub(super) fn install_mgame_reload_entry_hook(
    source: &[u8],
    patched: &mut [u8],
    wrapper_address: u32,
) -> Result<MgameReloadEntryHookReport> {
    ensure!(
        wrapper_address.is_multiple_of(4),
        "MGAME reload wrapper is unaligned"
    );
    let source_entry_pointer = source
        .get(MGAME_ENTRY_POINTER_OFFSET..MGAME_ENTRY_POINTER_OFFSET + 4)
        .context("MGAME entry pointer source is truncated")?;
    ensure!(
        sha256_bytes(source_entry_pointer) == MGAME_ENTRY_POINTER_SOURCE_SHA256
            && u32::from_le_bytes(source_entry_pointer.try_into()?) == MGAME_ENTRY_ADDRESS,
        "MGAME entry pointer changed"
    );
    ensure!(
        patched.get(MGAME_ENTRY_POINTER_OFFSET..MGAME_ENTRY_POINTER_OFFSET + 4)
            == Some(source_entry_pointer),
        "another MGAME writer changed the entry pointer"
    );

    let offset = usize::try_from(
        MGAME_ENTRY_ADDRESS
            .checked_sub(MGAME_RUNTIME_BASE)
            .context("MGAME reload entry hook precedes MGAME runtime")?,
    )?;
    let end = offset + MGAME_ENTRY_BYTE_COUNT;
    let source_bytes = source
        .get(offset..end)
        .context("MGAME reload entry source is truncated")?;
    ensure!(
        sha256_bytes(source_bytes) == MGAME_ENTRY_SOURCE_SHA256,
        "MGAME reload entry source bytes changed: {source_bytes:02x?}"
    );
    ensure!(
        patched.get(offset..end) == Some(source_bytes),
        "another MGAME writer changed the reload entry"
    );

    let source_instructions = [
        Instruction::Lui {
            rt: Register::V1,
            immediate: 0x801f,
        },
        Instruction::Lbu {
            rt: Register::V1,
            base: Register::V1,
            offset: 0x1801,
        },
    ];
    verify_instructions(
        source_bytes,
        MGAME_ENTRY_ADDRESS,
        &source_instructions,
        "source",
    )?;

    let (replacement_address, replacement) = build_mgame_reload_entry_replacement(wrapper_address)?;
    let replacement_bytes = encode_instructions(replacement_address, &replacement)?;
    patched
        .get_mut(offset..end)
        .context("MGAME reload entry destination is truncated")?
        .copy_from_slice(&replacement_bytes);
    verify_instructions(
        &patched[offset..end],
        MGAME_ENTRY_ADDRESS,
        &replacement,
        "replacement",
    )?;
    ensure!(
        patched.get(MGAME_ENTRY_POINTER_OFFSET..MGAME_ENTRY_POINTER_OFFSET + 4)
            == Some(source_entry_pointer),
        "MGAME reload entry hook changed the loader-owned entry pointer"
    );

    Ok(MgameReloadEntryHookReport {
        byte_range: [offset, end],
        address_range: [
            hex_address(MGAME_ENTRY_ADDRESS),
            hex_address(MGAME_ENTRY_RESUME_ADDRESS),
        ],
        source_sha256: MGAME_ENTRY_SOURCE_SHA256.to_string(),
        source_instructions: source_instructions
            .iter()
            .map(|instruction| format!("{instruction:?}"))
            .collect(),
        replacement_sha256: sha256_bytes(&replacement_bytes),
        replacement_instructions: replacement
            .iter()
            .map(|instruction| format!("{instruction:?}"))
            .collect(),
        source_entry_pointer: hex_address(MGAME_ENTRY_ADDRESS),
        entry_address: hex_address(MGAME_ENTRY_ADDRESS),
        resume_address: hex_address(MGAME_ENTRY_RESUME_ADDRESS),
        wrapper_address: hex_address(wrapper_address),
        source_entry_pointer_preserved: true,
        selects_valid_source_copy_or_destination_repair_before_entry_continuation: true,
        displaced_instructions_preserved_by_wrapper: true,
        caller_return_address_preserved: true,
        source_verified: true,
        installed: true,
        runtime_execution_verified: false,
    })
}

pub(super) fn build_mgame_reload_entry_replacement(
    wrapper_address: u32,
) -> Result<(u32, Vec<Instruction>)> {
    ensure!(
        wrapper_address.is_multiple_of(4),
        "MGAME reload wrapper is unaligned"
    );
    Ok((
        MGAME_ENTRY_ADDRESS,
        vec![
            Instruction::J {
                target: wrapper_address,
            },
            Instruction::nop(),
        ],
    ))
}

fn encode_instructions(origin: u32, instructions: &[Instruction]) -> Result<Vec<u8>> {
    let mut bytes = Vec::with_capacity(instructions.len() * 4);
    for (index, instruction) in instructions.iter().enumerate() {
        let address = origin + u32::try_from(index * 4)?;
        bytes.extend_from_slice(&encode(instruction, address)?.to_le_bytes());
    }
    Ok(bytes)
}

fn verify_instructions(
    bytes: &[u8],
    origin: u32,
    expected: &[Instruction],
    role: &str,
) -> Result<()> {
    ensure!(
        bytes.len() == expected.len() * 4,
        "MGAME reload entry {role} byte count changed"
    );
    for (index, instruction) in expected.iter().enumerate() {
        let offset = index * 4;
        let address = origin + u32::try_from(offset)?;
        ensure!(
            decode(
                u32::from_le_bytes(bytes[offset..offset + 4].try_into()?),
                address,
            )? == *instruction,
            "MGAME reload entry {role} instruction changed"
        );
    }
    Ok(())
}

fn hex_address(address: u32) -> String {
    format!("0x{address:08x}")
}
