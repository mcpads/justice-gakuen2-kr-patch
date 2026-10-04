use anyhow::{Context, Result, ensure};
use psx_r3000a::{Assembler, Instruction, Register, verify_placed_program};
use serde::Serialize;

use crate::pipeline::sha256_bytes;

use super::name_entry::OVERLAY_RUNTIME_BASE;

pub(super) const ROUTINE_OFFSET: usize = 0x72a4;
pub(super) const ROUTINE_BYTE_COUNT: usize = 0x00a0;

const ROUTINE_SOURCE_SHA256: &str =
    "45427d38160ff6c033d3839a3043e493eca7c1f8e81ba6527eea7d10aacc2332";
const NEXT_SLOT_INSTRUCTION_OFFSET: usize = 13;
const VALIDATOR_INSTRUCTION_OFFSET: usize = 29;
const FIELD_INDEX_OBJECT_OFFSET: i16 = 15;
const RECORD_SLOT_INDEX_OBJECT_OFFSET: i16 = 10;
const FAMILY_NAME_RECORD_OFFSET: i16 = 0x12;
const RECORD_FIELD_STRIDE_SHIFT: u8 = 4;
const RECORD_SLOT_STRIDE_SHIFT: u8 = 1;
const FAMILY_OR_GIVEN_FIELD_COUNT: i16 = 2;
const NICKNAME_LAST_SLOT_INDEX: i16 = 3;
const TAG_MASK: u16 = 0xc000;
const INCOMPLETE_INITIAL_TAG: u16 = 0xc000;
const MOVE_SOUND_ID: u16 = 0x0091;
const PLAY_SOUND_ADDRESS: u32 = 0x8018_2c08;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct NameEntrySlotNavigationGuardProgram {
    pub(super) bytes: Vec<u8>,
    pub(super) instructions: Vec<Instruction>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct NameEntrySlotNavigationGuardReport {
    pub routine_file_offset: String,
    pub routine_runtime_address: String,
    pub byte_count: usize,
    pub previous_slot_entry_address: String,
    pub next_slot_entry_address: String,
    pub current_slot_validator_address: String,
    pub source_sha256: String,
    pub replacement_sha256: String,
    pub typed_instruction_count: usize,
    pub source_identity_verified: bool,
    pub installed: bool,
    pub runtime_execution_verified: bool,
}

pub(super) fn install_name_entry_slot_navigation_guard(
    source_overlay: &[u8],
    patched_overlay: &mut [u8],
) -> Result<NameEntrySlotNavigationGuardReport> {
    let source = routine_slice(source_overlay)?;
    ensure!(
        sha256_bytes(source) == ROUTINE_SOURCE_SHA256,
        "name-entry slot navigation source changed"
    );
    ensure!(
        routine_slice(patched_overlay)? == source,
        "name-entry slot navigation was claimed before its typed replacement"
    );
    verify_placed_program(source, previous_slot_entry_address())
        .context("name-entry slot navigation contains an unsupported source instruction")?;

    let program = build_name_entry_slot_navigation_guard_program()?;
    routine_slice_mut(patched_overlay)?.copy_from_slice(&program.bytes);
    let readback = routine_slice(patched_overlay)?;
    ensure!(
        readback == program.bytes,
        "name-entry slot navigation guard readback changed"
    );
    ensure!(
        verify_placed_program(readback, previous_slot_entry_address())?.len()
            == program.instructions.len(),
        "name-entry slot navigation guard lost typed instruction placement evidence"
    );

    Ok(NameEntrySlotNavigationGuardReport {
        routine_file_offset: format!("0x{ROUTINE_OFFSET:04x}"),
        routine_runtime_address: format!("0x{:08x}", previous_slot_entry_address()),
        byte_count: program.bytes.len(),
        previous_slot_entry_address: format!("0x{:08x}", previous_slot_entry_address()),
        next_slot_entry_address: format!("0x{:08x}", next_slot_entry_address()),
        current_slot_validator_address: format!("0x{:08x}", current_slot_validator_address()),
        source_sha256: ROUTINE_SOURCE_SHA256.to_string(),
        replacement_sha256: sha256_bytes(&program.bytes),
        typed_instruction_count: program.instructions.len(),
        source_identity_verified: true,
        installed: true,
        runtime_execution_verified: false,
    })
}

pub(super) fn build_name_entry_slot_navigation_guard_program()
-> Result<NameEntrySlotNavigationGuardProgram> {
    let mut assembler = Assembler::new();
    assembler
        .label("move_to_previous_name_slot")
        .emit(Instruction::Addu {
            rd: Register::T9,
            rs: Register::RA,
            rt: Register::ZERO,
        })
        .call("validate_current_name_slot")
        .emit(Instruction::nop())
        .beq(
            Register::V0,
            Register::ZERO,
            "return_from_previous_name_slot",
        )
        .emit(Instruction::nop())
        .beq(
            Register::T1,
            Register::ZERO,
            "return_from_previous_name_slot",
        )
        .emit(Instruction::Addiu {
            rt: Register::T1,
            rs: Register::T1,
            immediate: -1,
        })
        .emit(Instruction::Sb {
            rt: Register::T1,
            base: Register::A0,
            offset: RECORD_SLOT_INDEX_OBJECT_OFFSET,
        })
        .emit(Instruction::Addu {
            rd: Register::RA,
            rs: Register::T9,
            rt: Register::ZERO,
        })
        .emit(Instruction::J {
            target: PLAY_SOUND_ADDRESS,
        })
        .emit(Instruction::Ori {
            rt: Register::A0,
            rs: Register::ZERO,
            immediate: MOVE_SOUND_ID,
        })
        .label("return_from_previous_name_slot")
        .emit(Instruction::Jr { rs: Register::T9 })
        .emit(Instruction::nop())
        .label("move_to_next_name_slot")
        .emit(Instruction::Addu {
            rd: Register::T9,
            rs: Register::RA,
            rt: Register::ZERO,
        })
        .call("validate_current_name_slot")
        .emit(Instruction::nop())
        .beq(Register::V0, Register::ZERO, "return_from_next_name_slot")
        .emit(Instruction::Sltiu {
            rt: Register::T0,
            rs: Register::T0,
            immediate: FAMILY_OR_GIVEN_FIELD_COUNT,
        })
        .emit(Instruction::Sll {
            rd: Register::T0,
            rt: Register::T0,
            shift: 1,
        })
        .emit(Instruction::Addiu {
            rt: Register::T0,
            rs: Register::T0,
            immediate: NICKNAME_LAST_SLOT_INDEX,
        })
        .emit(Instruction::Sltu {
            rd: Register::T0,
            rs: Register::T1,
            rt: Register::T0,
        })
        .beq(Register::T0, Register::ZERO, "return_from_next_name_slot")
        .emit(Instruction::Addiu {
            rt: Register::T1,
            rs: Register::T1,
            immediate: 1,
        })
        .emit(Instruction::Sb {
            rt: Register::T1,
            base: Register::A0,
            offset: RECORD_SLOT_INDEX_OBJECT_OFFSET,
        })
        .emit(Instruction::Addu {
            rd: Register::RA,
            rs: Register::T9,
            rt: Register::ZERO,
        })
        .emit(Instruction::J {
            target: PLAY_SOUND_ADDRESS,
        })
        .emit(Instruction::Ori {
            rt: Register::A0,
            rs: Register::ZERO,
            immediate: MOVE_SOUND_ID,
        })
        .label("return_from_next_name_slot")
        .emit(Instruction::Jr { rs: Register::T9 })
        .emit(Instruction::nop())
        .label("validate_current_name_slot")
        .emit(Instruction::Lbu {
            rt: Register::T0,
            base: Register::A0,
            offset: FIELD_INDEX_OBJECT_OFFSET,
        })
        .emit(Instruction::Lbu {
            rt: Register::T1,
            base: Register::A0,
            offset: RECORD_SLOT_INDEX_OBJECT_OFFSET,
        })
        .emit(Instruction::Sll {
            rd: Register::T3,
            rt: Register::T0,
            shift: RECORD_FIELD_STRIDE_SHIFT,
        })
        .emit(Instruction::Sll {
            rd: Register::T2,
            rt: Register::T1,
            shift: RECORD_SLOT_STRIDE_SHIFT,
        })
        .emit(Instruction::Addu {
            rd: Register::T3,
            rs: Register::T3,
            rt: Register::T2,
        })
        .emit(Instruction::Addu {
            rd: Register::T3,
            rs: Register::T3,
            rt: Register::A0,
        })
        .emit(Instruction::Lhu {
            rt: Register::V0,
            base: Register::T3,
            offset: FAMILY_NAME_RECORD_OFFSET,
        })
        .emit(Instruction::nop())
        .emit(Instruction::Andi {
            rt: Register::V0,
            rs: Register::V0,
            immediate: TAG_MASK,
        })
        .emit(Instruction::Jr { rs: Register::RA })
        .emit(Instruction::Xori {
            rt: Register::V0,
            rs: Register::V0,
            immediate: INCOMPLETE_INITIAL_TAG,
        });

    let program = assembler
        .assemble(previous_slot_entry_address())
        .context("failed to assemble typed name-entry slot navigation guard")?;
    ensure!(
        program.bytes().len() == ROUTINE_BYTE_COUNT,
        "typed name-entry slot navigation guard requires {:#x} bytes but owns {ROUTINE_BYTE_COUNT:#x}",
        program.bytes().len()
    );
    ensure!(
        program
            .instructions()
            .get(NEXT_SLOT_INSTRUCTION_OFFSET)
            .is_some()
            && previous_slot_entry_address()
                + u32::try_from(NEXT_SLOT_INSTRUCTION_OFFSET * 4).unwrap()
                == next_slot_entry_address(),
        "typed name-entry next-slot entry moved"
    );
    ensure!(
        previous_slot_entry_address() + u32::try_from(VALIDATOR_INSTRUCTION_OFFSET * 4).unwrap()
            == current_slot_validator_address(),
        "typed name-entry current-slot validator moved"
    );
    ensure!(
        program.instruction_spans().len() == program.instructions().len(),
        "name-entry slot navigation guard lost typed instruction placement evidence"
    );
    Ok(NameEntrySlotNavigationGuardProgram {
        bytes: program.bytes().to_vec(),
        instructions: program.instructions().to_vec(),
    })
}

fn routine_slice(bytes: &[u8]) -> Result<&[u8]> {
    bytes
        .get(ROUTINE_OFFSET..ROUTINE_OFFSET + ROUTINE_BYTE_COUNT)
        .context("name-entry slot navigation routine is truncated")
}

fn routine_slice_mut(bytes: &mut [u8]) -> Result<&mut [u8]> {
    bytes
        .get_mut(ROUTINE_OFFSET..ROUTINE_OFFSET + ROUTINE_BYTE_COUNT)
        .context("name-entry slot navigation destination is truncated")
}

pub(super) const fn previous_slot_entry_address() -> u32 {
    OVERLAY_RUNTIME_BASE + ROUTINE_OFFSET as u32
}

pub(super) const fn next_slot_entry_address() -> u32 {
    previous_slot_entry_address() + NEXT_SLOT_INSTRUCTION_OFFSET as u32 * 4
}

pub(super) const fn current_slot_validator_address() -> u32 {
    previous_slot_entry_address() + VALIDATOR_INSTRUCTION_OFFSET as u32 * 4
}
