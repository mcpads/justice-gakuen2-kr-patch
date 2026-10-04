use anyhow::{Context, Result, ensure};
use psx_r3000a::{Assembler, Instruction, Register, verify_placed_program};
use serde::Serialize;

use crate::name_input::EMPTY_NAME_SLOT;
use crate::pipeline::sha256_bytes;

use super::name_entry::OVERLAY_RUNTIME_BASE;

pub(super) const DELETE_ROUTINE_OFFSET: usize = 0x70f0;
pub(super) const DELETE_ROUTINE_BYTE_COUNT: usize = 0x00a4;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct NameEntryDeleteHandlerProgram {
    pub(super) bytes: Vec<u8>,
    pub(super) instructions: Vec<Instruction>,
}

const DELETE_ROUTINE_SOURCE_SHA256: &str =
    "9975c2b2af4b1420d915ad1fc4434d533f09268cf582bfd0c0fe034d7fcd6908";
const FIELD_INDEX_OBJECT_OFFSET: i16 = 15;
const RECORD_SLOT_INDEX_OBJECT_OFFSET: i16 = 10;
const FAMILY_NAME_RECORD_OFFSET: i16 = 0x12;
const RECORD_FIELD_STRIDE_SHIFT: u8 = 4;
const INCOMPLETE_INITIAL_TAG: u16 = 0xc000;
const TAG_MASK: u16 = 0xc000;
const DELETE_SOUND_ID: u16 = 0x0092;
const PLAY_SOUND_ADDRESS: u32 = 0x8018_2c08;
const CONFIRMATION_EPILOGUE_ADDRESS: u32 = 0x8017_bf54;
const CONFIRMATION_ENTRY_INSTRUCTION_OFFSET: usize = 2;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct NameEntryDeleteHandlerReport {
    pub routine_file_offset: String,
    pub routine_runtime_address: String,
    pub byte_count: usize,
    pub source_sha256: String,
    pub replacement_sha256: String,
    pub typed_instruction_count: usize,
    pub confirmation_entry_address: String,
    pub compound_final_backspace_address: String,
    pub source_identity_verified: bool,
    pub installed: bool,
    pub runtime_execution_verified: bool,
}

pub(super) fn install_name_entry_delete_handler(
    source_overlay: &[u8],
    patched_overlay: &mut [u8],
    compound_final_backspace_address: u32,
) -> Result<NameEntryDeleteHandlerReport> {
    let source = routine_slice(source_overlay)?;
    ensure!(
        sha256_bytes(source) == DELETE_ROUTINE_SOURCE_SHA256,
        "name-entry delete routine source changed"
    );
    ensure!(
        routine_slice(patched_overlay)? == source,
        "name-entry delete routine was claimed before its typed replacement"
    );
    verify_placed_program(source, routine_runtime_address())
        .context("name-entry delete routine contains an unsupported source instruction")?;

    let program = build_name_entry_delete_handler_program(compound_final_backspace_address)?;
    routine_slice_mut(patched_overlay)?.copy_from_slice(&program.bytes);
    let readback = routine_slice(patched_overlay)?;
    ensure!(
        readback == program.bytes,
        "name-entry delete handler readback changed"
    );
    ensure!(
        verify_placed_program(readback, routine_runtime_address())?.len()
            == program.instructions.len(),
        "name-entry delete handler lost typed instruction placement evidence"
    );

    Ok(NameEntryDeleteHandlerReport {
        routine_file_offset: format!("0x{DELETE_ROUTINE_OFFSET:04x}"),
        routine_runtime_address: format!("0x{:08x}", routine_runtime_address()),
        byte_count: program.bytes.len(),
        source_sha256: DELETE_ROUTINE_SOURCE_SHA256.to_string(),
        replacement_sha256: sha256_bytes(&program.bytes),
        typed_instruction_count: program.instructions.len(),
        confirmation_entry_address: format!("0x{:08x}", confirmation_entry_address()),
        compound_final_backspace_address: format!("0x{compound_final_backspace_address:08x}"),
        source_identity_verified: true,
        installed: true,
        runtime_execution_verified: false,
    })
}

#[cfg(test)]
pub(super) fn build_name_entry_delete_handler(
    compound_final_backspace_address: u32,
) -> Result<(Vec<u8>, usize)> {
    let program = build_name_entry_delete_handler_program(compound_final_backspace_address)?;
    let instruction_count = program.instructions.len();
    Ok((program.bytes, instruction_count))
}

pub(super) fn build_name_entry_delete_handler_program(
    compound_final_backspace_address: u32,
) -> Result<NameEntryDeleteHandlerProgram> {
    ensure!(
        compound_final_backspace_address.is_multiple_of(4),
        "compound-final backspace address is unaligned"
    );
    let mut assembler = Assembler::new();
    assembler
        .label("delete_name_input_stage")
        .jump("locate_name_input_stage")
        .emit(Instruction::Addu {
            rd: Register::T7,
            rs: Register::ZERO,
            rt: Register::ZERO,
        })
        .label("confirm_name_input_stage")
        .emit(Instruction::Ori {
            rt: Register::T7,
            rs: Register::ZERO,
            immediate: 1,
        })
        .label("locate_name_input_stage")
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
            rd: Register::T0,
            rt: Register::T0,
            shift: RECORD_FIELD_STRIDE_SHIFT,
        })
        .emit(Instruction::Sll {
            rd: Register::T6,
            rt: Register::T1,
            shift: 1,
        })
        .emit(Instruction::Addu {
            rd: Register::T0,
            rs: Register::T0,
            rt: Register::A0,
        })
        .emit(Instruction::Addu {
            rd: Register::T2,
            rs: Register::T0,
            rt: Register::T6,
        })
        .emit(Instruction::Addiu {
            rt: Register::T2,
            rs: Register::T2,
            immediate: FAMILY_NAME_RECORD_OFFSET,
        })
        .label("read_name_stage")
        .emit(Instruction::Lhu {
            rt: Register::T3,
            base: Register::T2,
            offset: 0,
        })
        .emit(Instruction::Ori {
            rt: Register::T5,
            rs: Register::ZERO,
            immediate: EMPTY_NAME_SLOT,
        })
        .emit(Instruction::Andi {
            rt: Register::T4,
            rs: Register::T3,
            immediate: TAG_MASK,
        })
        .emit(Instruction::Ori {
            rt: Register::T6,
            rs: Register::ZERO,
            immediate: INCOMPLETE_INITIAL_TAG,
        })
        .beq(Register::T7, Register::ZERO, "delete_current_name_stage")
        .emit(Instruction::nop())
        .emit(Instruction::Beq {
            rs: Register::T4,
            rt: Register::T6,
            target: CONFIRMATION_EPILOGUE_ADDRESS,
        })
        .emit(Instruction::Addu {
            rd: Register::V0,
            rs: Register::ZERO,
            rt: Register::ZERO,
        })
        .jump("finish_name_confirmation")
        .emit(Instruction::nop())
        .label("delete_current_name_stage")
        .beq(Register::T3, Register::T5, "delete_empty_name_slot")
        .emit(Instruction::Ori {
            rt: Register::T0,
            rs: Register::ZERO,
            immediate: 0x8000,
        })
        .bne(Register::T4, Register::T0, "clear_name_stage")
        .emit(Instruction::nop())
        .emit(Instruction::J {
            target: compound_final_backspace_address,
        })
        .emit(Instruction::nop())
        .label("delete_empty_name_slot")
        .beq(Register::T1, Register::ZERO, "play_name_delete_sound")
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
        .jump("read_name_stage")
        .emit(Instruction::Addiu {
            rt: Register::T2,
            rs: Register::T2,
            immediate: -2,
        })
        .label("clear_name_stage")
        .emit(Instruction::Sh {
            rt: Register::T5,
            base: Register::T2,
            offset: 0,
        })
        .label("play_name_delete_sound")
        .emit(Instruction::J {
            target: PLAY_SOUND_ADDRESS,
        })
        .emit(Instruction::Ori {
            rt: Register::A0,
            rs: Register::ZERO,
            immediate: DELETE_SOUND_ID,
        })
        .label("finish_name_confirmation")
        .emit(Instruction::J {
            target: CONFIRMATION_EPILOGUE_ADDRESS,
        })
        .emit(Instruction::Ori {
            rt: Register::V0,
            rs: Register::ZERO,
            immediate: 1,
        });

    let program = assembler
        .assemble(routine_runtime_address())
        .context("failed to assemble typed name-entry delete handler")?;
    ensure!(
        program.bytes().len() <= DELETE_ROUTINE_BYTE_COUNT,
        "typed name-entry delete handler requires {:#x} bytes but owns {DELETE_ROUTINE_BYTE_COUNT:#x}",
        program.bytes().len()
    );
    ensure!(
        program.instruction_spans().len() == program.instructions().len(),
        "name-entry delete handler lost typed instruction placement evidence"
    );
    let mut instructions = program.instructions().to_vec();
    let mut bytes = program.bytes().to_vec();
    instructions.resize(DELETE_ROUTINE_BYTE_COUNT / 4, Instruction::nop());
    bytes.resize(DELETE_ROUTINE_BYTE_COUNT, 0);
    ensure!(
        verify_placed_program(&bytes, routine_runtime_address())?.len()
            == DELETE_ROUTINE_BYTE_COUNT / 4,
        "padded name-entry delete handler is not a complete typed program"
    );
    Ok(NameEntryDeleteHandlerProgram {
        bytes,
        instructions,
    })
}

fn routine_slice(bytes: &[u8]) -> Result<&[u8]> {
    bytes
        .get(DELETE_ROUTINE_OFFSET..DELETE_ROUTINE_OFFSET + DELETE_ROUTINE_BYTE_COUNT)
        .context("name-entry delete routine is truncated")
}

fn routine_slice_mut(bytes: &mut [u8]) -> Result<&mut [u8]> {
    bytes
        .get_mut(DELETE_ROUTINE_OFFSET..DELETE_ROUTINE_OFFSET + DELETE_ROUTINE_BYTE_COUNT)
        .context("name-entry delete routine destination is truncated")
}

pub(super) const fn routine_runtime_address() -> u32 {
    OVERLAY_RUNTIME_BASE + DELETE_ROUTINE_OFFSET as u32
}

pub(super) const fn confirmation_entry_address() -> u32 {
    routine_runtime_address() + CONFIRMATION_ENTRY_INSTRUCTION_OFFSET as u32 * 4
}
