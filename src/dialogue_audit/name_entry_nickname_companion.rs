use anyhow::{Context, Result, ensure};
use psx_r3000a::{Assembler, Instruction, Register, verify_placed_program};
use serde::Serialize;

use crate::name_input::{NAME_GLYPH_CODE_COUNT, NameField, NameGlyphConsumerLayout};
use crate::pipeline::sha256_bytes;

use super::name_entry::OVERLAY_RUNTIME_BASE;

pub(super) const ROUTINE_OFFSET: usize = 0x6a10;
const ROUTINE_BYTE_COUNT: usize = 0x0088;
const SOURCE_ROUTINE_SHA256: &str =
    "e4dd467bcaf62da622799b5d33a0aecee360e1fd256ac33c392adf091de51309";

const NICKNAME_COMPANION_BUFFER_ADDRESS: u32 = 0x801f_1886;
const NAME_CODE_LOOKUP_ADDRESS: u32 = 0x8017_ad14;
const NICKNAME_COMPANION_LOOKUP_ADDRESS: u32 = 0x8017_b198;
const NAME_CODE_LOOKUP_PREFIX_ENTRY_COUNT: usize = 2;
const NICKNAME_COMPANION_LOOKUP_BYTE_COUNT: usize =
    (NAME_CODE_LOOKUP_PREFIX_ENTRY_COUNT + NAME_GLYPH_CODE_COUNT) * 2;
const NICKNAME_RECORD_OFFSET: i16 = 0x32;
const MESSAGE_END_CODE: u16 = 0x3001;
const TAG_MASK: u16 = 0xc000;
const INVALID_TAG: u16 = 0xc000;
const LOOKUP_END_CODE: u16 = u16::MAX;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct NicknameCompanionProgram {
    pub bytes: Vec<u8>,
    pub instructions: Vec<Instruction>,
    pub tagged_cache_address: u32,
    pub legacy_scan_address: u32,
    pub store_address: u32,
    pub finish_address: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct NameEntryNicknameCompanionReport {
    pub routine_file_byte_range: [usize; 2],
    pub routine_runtime_address_range: [String; 2],
    pub overwritten_byte_count: usize,
    pub source_sha256: String,
    pub replacement_sha256: String,
    pub typed_instruction_count: usize,
    pub nickname_cache_code_range: [String; 2],
    pub tagged_slots_use_persistent_cache_codes: bool,
    pub legacy_mapping_preserved: bool,
    pub invalid_or_unmapped_values_terminate: bool,
    pub source_instructions_verified: bool,
    pub installed: bool,
    pub runtime_execution_verified: bool,
}

pub(super) fn install_name_entry_nickname_companion(
    source: &[u8],
    patched: &mut [u8],
    consumer_layout: &NameGlyphConsumerLayout,
) -> Result<NameEntryNicknameCompanionReport> {
    let end = ROUTINE_OFFSET + ROUTINE_BYTE_COUNT;
    let source_routine = source
        .get(ROUTINE_OFFSET..end)
        .context("nickname companion source routine is truncated")?;
    ensure!(
        sha256_bytes(source_routine) == SOURCE_ROUTINE_SHA256,
        "nickname companion source routine changed"
    );
    let source_instructions = verify_placed_program(source_routine, routine_address())?;
    ensure!(
        source_instructions.len() * 4 == ROUTINE_BYTE_COUNT,
        "nickname companion source routine lost complete typed decoding"
    );
    ensure!(
        patched
            .get(ROUTINE_OFFSET..end)
            .context("nickname companion destination is truncated")?
            == source_routine,
        "another name-entry writer changed the nickname companion routine"
    );

    let nickname_cache_codes = consumer_layout.cache_codes_for_field(NameField::Nickname)?;
    let nickname_cache_code_start = nickname_cache_codes[0];
    let program = build_nickname_companion_program(consumer_layout)?;
    let (replacement_byte_count, replacement_sha256, replacement_instruction_count) = {
        let replacement = patched
            .get_mut(ROUTINE_OFFSET..end)
            .context("nickname companion destination is truncated")?;
        replacement.copy_from_slice(&program.bytes);
        ensure!(
            replacement == program.bytes,
            "nickname companion replacement readback changed"
        );
        let replacement_instructions = verify_placed_program(replacement, routine_address())?;
        ensure!(
            replacement_instructions.len() * 4 == ROUTINE_BYTE_COUNT,
            "nickname companion replacement lost complete typed decoding"
        );
        (
            replacement.len(),
            sha256_bytes(replacement),
            replacement_instructions.len(),
        )
    };

    let nickname_cache_code_end = nickname_cache_code_start
        .checked_add(3)
        .context("nickname cache code range overflow")?;
    let legacy_mapping_preserved = verify_nickname_companion_lookup_preserved(source, patched)?;
    Ok(NameEntryNicknameCompanionReport {
        routine_file_byte_range: [ROUTINE_OFFSET, end],
        routine_runtime_address_range: [
            format!("0x{:08x}", routine_address()),
            format!("0x{:08x}", routine_address() + ROUTINE_BYTE_COUNT as u32),
        ],
        overwritten_byte_count: replacement_byte_count,
        source_sha256: SOURCE_ROUTINE_SHA256.to_string(),
        replacement_sha256,
        typed_instruction_count: replacement_instruction_count,
        nickname_cache_code_range: [
            format!("0x{nickname_cache_code_start:04x}"),
            format!("0x{nickname_cache_code_end:04x}"),
        ],
        tagged_slots_use_persistent_cache_codes: true,
        legacy_mapping_preserved,
        invalid_or_unmapped_values_terminate: true,
        source_instructions_verified: true,
        installed: true,
        runtime_execution_verified: false,
    })
}

pub(super) fn verify_nickname_companion_lookup_preserved(
    source: &[u8],
    patched: &[u8],
) -> Result<bool> {
    let start = usize::try_from(
        NICKNAME_COMPANION_LOOKUP_ADDRESS
            .checked_sub(OVERLAY_RUNTIME_BASE)
            .context("nickname companion lookup precedes the overlay")?,
    )?;
    let end = start
        .checked_add(NICKNAME_COMPANION_LOOKUP_BYTE_COUNT)
        .context("nickname companion lookup range overflow")?;
    let source_lookup = source
        .get(start..end)
        .context("nickname companion source lookup is truncated")?;
    let patched_lookup = patched
        .get(start..end)
        .context("nickname companion patched lookup is truncated")?;
    ensure!(
        patched_lookup == source_lookup,
        "another name-entry writer changed the nickname companion lookup"
    );
    Ok(true)
}

pub(super) fn build_nickname_companion_program(
    consumer_layout: &NameGlyphConsumerLayout,
) -> Result<NicknameCompanionProgram> {
    let nickname_cache_codes = consumer_layout.cache_codes_for_field(NameField::Nickname)?;
    let nickname_cache_code_start = nickname_cache_codes[0];
    let nickname_cache_code_end = nickname_cache_code_start
        .checked_add(3)
        .context("nickname cache code range overflow")?;
    ensure!(
        nickname_cache_code_end & TAG_MASK == 0,
        "nickname cache codes overlap the tagged record domain"
    );
    let matched_output_offset = NICKNAME_COMPANION_LOOKUP_ADDRESS
        .checked_sub(NAME_CODE_LOOKUP_ADDRESS)
        .and_then(|offset| offset.checked_sub(2))
        .context("nickname companion lookup displacement underflow")?;
    let matched_output_offset = i16::try_from(matched_output_offset)
        .context("nickname companion lookup displacement exceeds one load")?;

    let mut assembler = Assembler::new();
    assembler
        .emit(Instruction::Addiu {
            rt: Register::A0,
            rs: Register::A0,
            immediate: NICKNAME_RECORD_OFFSET,
        })
        .emit(Instruction::Lui {
            rt: Register::A3,
            immediate: (NICKNAME_COMPANION_BUFFER_ADDRESS >> 16) as u16,
        })
        .emit(Instruction::Ori {
            rt: Register::A3,
            rs: Register::A3,
            immediate: NICKNAME_COMPANION_BUFFER_ADDRESS as u16,
        })
        .emit(Instruction::Ori {
            rt: Register::T4,
            rs: Register::ZERO,
            immediate: nickname_cache_code_start,
        })
        .emit(Instruction::Ori {
            rt: Register::T1,
            rs: Register::ZERO,
            immediate: MESSAGE_END_CODE,
        })
        .emit(Instruction::Ori {
            rt: Register::T3,
            rs: Register::ZERO,
            immediate: LOOKUP_END_CODE,
        })
        .label("read_nickname_slot")
        .emit(Instruction::Lhu {
            rt: Register::A2,
            base: Register::A0,
            offset: 0,
        })
        .emit(Instruction::Addiu {
            rt: Register::A0,
            rs: Register::A0,
            immediate: 2,
        })
        .beq(Register::A2, Register::T1, "finish_nickname_companion")
        .emit(Instruction::Andi {
            rt: Register::T0,
            rs: Register::A2,
            immediate: TAG_MASK,
        })
        .emit(Instruction::Ori {
            rt: Register::V0,
            rs: Register::ZERO,
            immediate: INVALID_TAG,
        })
        .beq(Register::T0, Register::V0, "finish_nickname_companion")
        .emit(Instruction::Lui {
            rt: Register::A1,
            immediate: (NAME_CODE_LOOKUP_ADDRESS >> 16) as u16,
        })
        .bne(Register::T0, Register::ZERO, "use_tagged_nickname_cache")
        // The low half is 0xad14: ADDIU would sign-extend it and read the
        // previous RAM bank instead of the resident name-code table.
        .emit(Instruction::Ori {
            rt: Register::A1,
            rs: Register::A1,
            immediate: NAME_CODE_LOOKUP_ADDRESS as u16,
        })
        .label("scan_legacy_nickname_code")
        .emit(Instruction::Lhu {
            rt: Register::V1,
            base: Register::A1,
            offset: 0,
        })
        .emit(Instruction::Addiu {
            rt: Register::A1,
            rs: Register::A1,
            immediate: 2,
        })
        .beq(Register::V1, Register::A2, "legacy_nickname_code_found")
        .emit(Instruction::nop())
        .beq(Register::V1, Register::T3, "finish_nickname_companion")
        .emit(Instruction::nop())
        .jump("scan_legacy_nickname_code")
        .emit(Instruction::nop())
        .label("legacy_nickname_code_found")
        .emit(Instruction::Lhu {
            rt: Register::V0,
            base: Register::A1,
            offset: matched_output_offset,
        })
        .jump("store_nickname_companion_code")
        .emit(Instruction::nop())
        .label("use_tagged_nickname_cache")
        .emit(Instruction::Addu {
            rd: Register::V0,
            rs: Register::T4,
            rt: Register::ZERO,
        })
        .label("store_nickname_companion_code")
        .emit(Instruction::Sh {
            rt: Register::V0,
            base: Register::A3,
            offset: 0,
        })
        .emit(Instruction::Addiu {
            rt: Register::T4,
            rs: Register::T4,
            immediate: 1,
        })
        .jump("read_nickname_slot")
        .emit(Instruction::Addiu {
            rt: Register::A3,
            rs: Register::A3,
            immediate: 2,
        })
        .label("finish_nickname_companion")
        .emit(Instruction::Sh {
            rt: Register::T1,
            base: Register::A3,
            offset: 0,
        })
        .emit(Instruction::Jr { rs: Register::RA })
        .emit(Instruction::nop());

    let placed = assembler
        .assemble(routine_address())
        .context("failed to assemble typed nickname companion producer")?;
    ensure!(
        placed.bytes().len() == ROUTINE_BYTE_COUNT,
        "typed nickname companion producer must replace exactly the source routine"
    );
    ensure!(
        placed.instruction_spans().len() == placed.instructions().len(),
        "nickname companion producer lost typed instruction placement evidence"
    );
    let instructions = placed.instructions();
    let tagged_cache_address = instructions
        .iter()
        .find_map(|instruction| match instruction {
            Instruction::Bne {
                rs: Register::T0,
                rt: Register::ZERO,
                target,
            } => Some(*target),
            _ => None,
        })
        .context("nickname companion tagged-cache branch disappeared")?;
    let finish_address = instructions
        .iter()
        .find_map(|instruction| match instruction {
            Instruction::Beq {
                rs: Register::T0,
                rt: Register::V0,
                target,
            } => Some(*target),
            _ => None,
        })
        .context("nickname companion invalid-tag branch disappeared")?;
    let store_address = instructions
        .windows(2)
        .find_map(|window| match window {
            [
                Instruction::Lhu {
                    rt: Register::V0,
                    base: Register::A1,
                    ..
                },
                Instruction::J { target },
            ] => Some(*target),
            _ => None,
        })
        .context("nickname companion legacy-store jump disappeared")?;
    let legacy_scan_address = instructions
        .iter()
        .find_map(|instruction| match instruction {
            Instruction::J { target } if *target > routine_address() && *target < store_address => {
                Some(*target)
            }
            _ => None,
        })
        .context("nickname companion legacy scan loop disappeared")?;

    Ok(NicknameCompanionProgram {
        bytes: placed.bytes().to_vec(),
        instructions: placed.instructions().to_vec(),
        tagged_cache_address,
        legacy_scan_address,
        store_address,
        finish_address,
    })
}

pub(super) const fn routine_address() -> u32 {
    OVERLAY_RUNTIME_BASE + ROUTINE_OFFSET as u32
}
