use anyhow::{Context, Result, ensure};
use psx_r3000a::{Instruction, Register, decode, encode};
use serde::Serialize;

use crate::pipeline::sha256_bytes;

use super::name_entry::OVERLAY_RUNTIME_BASE;

const PAGE_CYCLE_BRANCH_OFFSET: usize = 0x1ce8;
const PAGE_CYCLE_CONTINUE_OFFSET: usize = 0x1d00;
const NICKNAME_INITIAL_PAGE_STORE_OFFSET: usize = 0x1fa8;
const PAGE_INDEX_OBJECT_OFFSET: i16 = 9;
const PATCHED_INSTRUCTION_COUNT: usize = 2;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct NameEntryNicknameHangulPageReport {
    pub page_cycle_file_offset: String,
    pub page_cycle_runtime_address: String,
    pub nickname_initial_page_store_file_offset: String,
    pub nickname_initial_page_store_runtime_address: String,
    pub overwritten_byte_count: usize,
    pub source_instructions_verified: bool,
    pub typed_instruction_count: usize,
    pub nickname_initial_page_index: u8,
    pub nickname_page_cycle_includes_hangul: bool,
    pub replacement_sha256: String,
    pub installed: bool,
    pub runtime_execution_verified: bool,
}

pub(super) fn install_name_entry_nickname_hangul_page(
    source: &[u8],
    patched: &mut [u8],
) -> Result<NameEntryNicknameHangulPageReport> {
    let replacements = name_entry_nickname_hangul_page_replacements();
    let sites = [
        (
            PAGE_CYCLE_BRANCH_OFFSET,
            source_page_cycle_branch(),
            replacements[0].1.clone(),
        ),
        (
            NICKNAME_INITIAL_PAGE_STORE_OFFSET,
            source_nickname_initial_page_store(),
            replacements[1].1.clone(),
        ),
    ];

    let mut replacement_bytes = Vec::with_capacity(PATCHED_INSTRUCTION_COUNT * 4);
    for (offset, expected, replacement) in sites {
        ensure!(
            decode(read_word(source, offset)?, runtime_address(offset))? == expected,
            "name-entry nickname page source changed at {offset:#x}"
        );
        ensure!(
            read_word(patched, offset)? == read_word(source, offset)?,
            "another name-entry writer changed the nickname page site at {offset:#x}"
        );

        let bytes = encode(&replacement, runtime_address(offset))?.to_le_bytes();
        patched
            .get_mut(offset..offset + 4)
            .context("name-entry nickname page destination is truncated")?
            .copy_from_slice(&bytes);
        replacement_bytes.extend_from_slice(&bytes);
        ensure!(
            decode(read_word(patched, offset)?, runtime_address(offset))? == replacement,
            "name-entry nickname page readback changed at {offset:#x}"
        );
    }

    Ok(NameEntryNicknameHangulPageReport {
        page_cycle_file_offset: hex_offset(PAGE_CYCLE_BRANCH_OFFSET),
        page_cycle_runtime_address: hex_address(PAGE_CYCLE_BRANCH_OFFSET),
        nickname_initial_page_store_file_offset: hex_offset(NICKNAME_INITIAL_PAGE_STORE_OFFSET),
        nickname_initial_page_store_runtime_address: hex_address(
            NICKNAME_INITIAL_PAGE_STORE_OFFSET,
        ),
        overwritten_byte_count: PATCHED_INSTRUCTION_COUNT * 4,
        source_instructions_verified: true,
        typed_instruction_count: PATCHED_INSTRUCTION_COUNT,
        nickname_initial_page_index: 0,
        nickname_page_cycle_includes_hangul: true,
        replacement_sha256: sha256_bytes(&replacement_bytes),
        installed: true,
        runtime_execution_verified: false,
    })
}

pub(super) fn name_entry_nickname_hangul_page_replacements() -> [(usize, Instruction); 2] {
    [
        (
            PAGE_CYCLE_BRANCH_OFFSET,
            Instruction::J {
                target: runtime_address(PAGE_CYCLE_CONTINUE_OFFSET),
            },
        ),
        (
            NICKNAME_INITIAL_PAGE_STORE_OFFSET,
            Instruction::Sb {
                rt: Register::ZERO,
                base: Register::S0,
                offset: PAGE_INDEX_OBJECT_OFFSET,
            },
        ),
    ]
}

fn source_page_cycle_branch() -> Instruction {
    Instruction::Bne {
        rs: Register::V1,
        rt: Register::V0,
        target: runtime_address(PAGE_CYCLE_CONTINUE_OFFSET),
    }
}

fn source_nickname_initial_page_store() -> Instruction {
    Instruction::Sb {
        rt: Register::V0,
        base: Register::S0,
        offset: PAGE_INDEX_OBJECT_OFFSET,
    }
}

fn read_word(bytes: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        bytes
            .get(offset..offset + 4)
            .context("name-entry nickname page source is truncated")?
            .try_into()?,
    ))
}

fn hex_offset(offset: usize) -> String {
    format!("0x{offset:04x}")
}

fn hex_address(offset: usize) -> String {
    format!("0x{:08x}", runtime_address(offset))
}

const fn runtime_address(offset: usize) -> u32 {
    OVERLAY_RUNTIME_BASE + offset as u32
}
