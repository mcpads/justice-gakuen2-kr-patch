use anyhow::{Context, Result, ensure};
use psx_r3000a::{Assembler, Instruction, Register};
use serde::Serialize;

use crate::pipeline::sha256_bytes;

// The old 0x8008c22c placement overlapped live battle floor descriptors.
// Keep this helper in the existing resident bootstrap region, after the MGAME
// repair helper and before its reload wrapper. The staging-copy region is
// overwritten when Diary loads and cannot hold callable code.
pub const SHARED_NAME_OUTLINE_RUNTIME_ORIGIN: u32 = 0x8001_0c00;
pub const SHARED_NAME_OUTLINE_RUNTIME_BYTE_CAPACITY: usize =
    (super::dialogue_runtime::MGAME_RELOAD_WRAPPER_ORIGIN - SHARED_NAME_OUTLINE_RUNTIME_ORIGIN)
        as usize;

const CELL_ROW_BYTES: i16 = 10;
const CELL_HEIGHT: i16 = 20;
const FILL_NIBBLE_FLAG_MASK: u16 = 0x88;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SharedNameOutlineRuntimeProgram {
    pub bytes: Vec<u8>,
    pub instructions: Vec<Instruction>,
    pub outline_pixel_address: u32,
    pub outline_cleanup_address: u32,
    pub report: SharedNameOutlineRuntimeProgramReport,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SharedNameOutlineRuntimeProgramReport {
    pub origin: String,
    pub byte_count: usize,
    pub byte_capacity: usize,
    pub sha256: String,
    pub typed_instruction_count: usize,
    pub outline_pixel_address: String,
    pub outline_cleanup_address: String,
    pub row_stride_register: String,
    pub fits_main_executable_region: bool,
    pub installed: bool,
    pub runtime_execution_verified: bool,
}

pub fn build_shared_name_outline_runtime_program() -> Result<SharedNameOutlineRuntimeProgram> {
    let mut assembler = Assembler::new();
    emit_outline_pixel(&mut assembler);
    let pixel = assembler
        .assemble(SHARED_NAME_OUTLINE_RUNTIME_ORIGIN)
        .context("failed to place typed shared-name outline writer")?;
    let outline_cleanup_address = SHARED_NAME_OUTLINE_RUNTIME_ORIGIN
        .checked_add(u32::try_from(pixel.bytes().len())?)
        .context("shared-name outline cleanup address overflow")?;
    emit_outline_cleanup(&mut assembler);
    let placed = assembler
        .assemble(SHARED_NAME_OUTLINE_RUNTIME_ORIGIN)
        .context("failed to assemble typed shared-name outline runtime")?;
    ensure!(
        placed.bytes().len() <= SHARED_NAME_OUTLINE_RUNTIME_BYTE_CAPACITY,
        "shared-name outline runtime exceeds its main-executable region"
    );
    ensure!(
        placed.instruction_spans().len() == placed.instructions().len(),
        "shared-name outline runtime lost typed instruction placement evidence"
    );
    let bytes = placed.bytes().to_vec();
    let instructions = placed.instructions().to_vec();

    Ok(SharedNameOutlineRuntimeProgram {
        bytes: bytes.clone(),
        instructions: instructions.clone(),
        outline_pixel_address: SHARED_NAME_OUTLINE_RUNTIME_ORIGIN,
        outline_cleanup_address,
        report: SharedNameOutlineRuntimeProgramReport {
            origin: format!("0x{SHARED_NAME_OUTLINE_RUNTIME_ORIGIN:08x}"),
            byte_count: bytes.len(),
            byte_capacity: SHARED_NAME_OUTLINE_RUNTIME_BYTE_CAPACITY,
            sha256: sha256_bytes(&bytes),
            typed_instruction_count: instructions.len(),
            outline_pixel_address: format!("0x{SHARED_NAME_OUTLINE_RUNTIME_ORIGIN:08x}"),
            outline_cleanup_address: format!("0x{outline_cleanup_address:08x}"),
            row_stride_register: "t8".to_string(),
            fits_main_executable_region: true,
            installed: false,
            runtime_execution_verified: false,
        },
    })
}

fn emit_outline_pixel(assembler: &mut Assembler) {
    assembler
        .label("outline_name_glyph_pixel")
        .bne(Register::A1, Register::ZERO, "outline_odd_pixel")
        .emit(Instruction::Subu {
            rd: Register::A0,
            rs: Register::A0,
            rt: Register::T8,
        })
        .emit(Instruction::Addiu {
            rt: Register::A0,
            rs: Register::A0,
            immediate: -1,
        })
        .jump("outline_pair_ready")
        .emit(Instruction::Ori {
            rt: Register::A1,
            rs: Register::ZERO,
            immediate: 0x3330,
        })
        .label("outline_odd_pixel")
        .emit(Instruction::Ori {
            rt: Register::A1,
            rs: Register::ZERO,
            immediate: 0x0333,
        })
        .label("outline_pair_ready")
        .call("or_name_outline_pair")
        .emit(Instruction::nop())
        .emit(Instruction::Addu {
            rd: Register::A0,
            rs: Register::A0,
            rt: Register::T8,
        })
        .call("or_name_outline_pair")
        .emit(Instruction::nop())
        .emit(Instruction::Addu {
            rd: Register::A0,
            rs: Register::A0,
            rt: Register::T8,
        })
        .call("or_name_outline_pair")
        .emit(Instruction::nop())
        .emit(Instruction::Jr { rs: Register::A2 })
        .emit(Instruction::nop())
        .label("or_name_outline_pair")
        .emit(Instruction::Lbu {
            rt: Register::T0,
            base: Register::A0,
            offset: 0,
        })
        .emit(Instruction::Lbu {
            rt: Register::T1,
            base: Register::A0,
            offset: 1,
        })
        .emit(Instruction::Andi {
            rt: Register::T2,
            rs: Register::A1,
            immediate: 0x00ff,
        })
        .emit(Instruction::Srl {
            rd: Register::T3,
            rt: Register::A1,
            shift: 8,
        })
        .emit(Instruction::Or {
            rd: Register::T0,
            rs: Register::T0,
            rt: Register::T2,
        })
        .emit(Instruction::Or {
            rd: Register::T1,
            rs: Register::T1,
            rt: Register::T3,
        })
        .emit(Instruction::Sb {
            rt: Register::T0,
            base: Register::A0,
            offset: 0,
        })
        .emit(Instruction::Sb {
            rt: Register::T1,
            base: Register::A0,
            offset: 1,
        })
        .emit(Instruction::Jr { rs: Register::RA })
        .emit(Instruction::nop());
}

fn emit_outline_cleanup(assembler: &mut Assembler) {
    assembler
        .label("clean_name_glyph_fill_nibbles")
        .emit(Instruction::Ori {
            rt: Register::T0,
            rs: Register::ZERO,
            immediate: CELL_HEIGHT as u16,
        })
        .label("clean_name_glyph_row")
        .emit(Instruction::Ori {
            rt: Register::T1,
            rs: Register::ZERO,
            immediate: CELL_ROW_BYTES as u16,
        })
        .label("clean_name_glyph_byte")
        .emit(Instruction::Lbu {
            rt: Register::T2,
            base: Register::A0,
            offset: 0,
        })
        .emit(Instruction::Addiu {
            rt: Register::T1,
            rs: Register::T1,
            immediate: -1,
        })
        .emit(Instruction::Andi {
            rt: Register::T3,
            rs: Register::T2,
            immediate: FILL_NIBBLE_FLAG_MASK,
        })
        .emit(Instruction::Srl {
            rd: Register::T3,
            rt: Register::T3,
            shift: 2,
        })
        .emit(Instruction::Xor {
            rd: Register::T2,
            rs: Register::T2,
            rt: Register::T3,
        })
        .emit(Instruction::Sb {
            rt: Register::T2,
            base: Register::A0,
            offset: 0,
        })
        .bgtz(Register::T1, "clean_name_glyph_byte")
        .emit(Instruction::Addiu {
            rt: Register::A0,
            rs: Register::A0,
            immediate: 1,
        })
        .emit(Instruction::Addiu {
            rt: Register::T0,
            rs: Register::T0,
            immediate: -1,
        })
        .emit(Instruction::Addiu {
            rt: Register::A0,
            rs: Register::A0,
            immediate: -CELL_ROW_BYTES,
        })
        .bgtz(Register::T0, "clean_name_glyph_row")
        .emit(Instruction::Addu {
            rd: Register::A0,
            rs: Register::A0,
            rt: Register::T8,
        })
        .emit(Instruction::Jr { rs: Register::RA })
        .emit(Instruction::nop());
}
