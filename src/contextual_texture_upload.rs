use anyhow::{Context, Result, ensure};
use psx_r3000a::{Assembler, Instruction, Register, load_address, verify_placed_program};
use serde::{Deserialize, Serialize};

use crate::tim::Cell;

pub(crate) const TEXTURE_UPLOAD_DESCRIPTOR_BYTE_COUNT: usize = 12;
pub(crate) const VRAM_UPLOAD_ROUTINE_ADDRESS: u32 = 0x8006_2630;
pub(crate) const VRAM_UPLOAD_SYNC_ROUTINE_ADDRESS: u32 = 0x8006_2384;

#[derive(Debug, Clone)]
pub(crate) struct ContextualMenuGlyph {
    pub(crate) role: String,
    pub(crate) character: char,
    pub(crate) code: u16,
    pub(crate) cell: Cell,
    pub(crate) payload: Vec<u8>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ContextualMenuGlyphUploadReport {
    pub context: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload_encoding: Option<String>,
    pub entry_count: usize,
    pub global_menu_write_count: usize,
    pub source_menu_graphics_preserved: bool,
    pub hook_path: String,
    pub hook_offset: String,
    pub hook_runtime_address: String,
    pub native_call_target: String,
    pub storage_path: String,
    pub program_offset: String,
    pub program_runtime_address: String,
    pub program_byte_count: usize,
    pub program_byte_capacity: usize,
    pub program_sha256: String,
    pub typed_instruction_count: usize,
    pub descriptor_offset: String,
    pub descriptor_runtime_address: String,
    pub descriptor_byte_count: usize,
    pub payload_byte_ranges: Vec<[usize; 2]>,
    pub payload_byte_count: usize,
    pub source_storage_padding_verified: bool,
    pub typed_program_verified: bool,
    pub runtime_execution_verified: bool,
    pub upload_routine_address: String,
    pub entries: Vec<ContextualMenuGlyphUploadEntry>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ContextualMenuGlyphUploadEntry {
    pub role: String,
    pub character: char,
    pub code: String,
    pub cell: Cell,
    pub vram_rect: [u16; 4],
    pub payload_offset: String,
    pub payload_runtime_address: String,
    pub payload_sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu_payload_sha256: Option<String>,
}

pub(crate) struct ContextualTextureUploadProgram {
    pub(crate) bytes: Vec<u8>,
    pub(crate) typed_instruction_count: usize,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum NativeCallTarget {
    Address(u32),
}

pub(crate) fn assemble_upload_after_native_call(
    program_runtime_address: u32,
    native_call_target: NativeCallTarget,
    descriptor_runtime_address: u32,
    entry_count: usize,
    loop_label: &'static str,
    context: &'static str,
) -> Result<ContextualTextureUploadProgram> {
    ensure!(
        entry_count > 0,
        "{context} runtime texture upload has no entries"
    );
    let mut assembler = Assembler::new();
    assembler
        .emit(Instruction::Addiu {
            rt: Register::SP,
            rs: Register::SP,
            immediate: -32,
        })
        .emit(Instruction::Sw {
            rt: Register::RA,
            base: Register::SP,
            offset: 28,
        })
        .emit(Instruction::Sw {
            rt: Register::S0,
            base: Register::SP,
            offset: 24,
        })
        .emit(Instruction::Sw {
            rt: Register::S1,
            base: Register::SP,
            offset: 20,
        })
        .emit(match native_call_target {
            NativeCallTarget::Address(target) => Instruction::Jal { target },
        })
        .emit(Instruction::nop())
        .emit(Instruction::Sw {
            rt: Register::V0,
            base: Register::SP,
            offset: 16,
        })
        .emit_all(load_address(Register::S0, descriptor_runtime_address))
        .emit(Instruction::Ori {
            rt: Register::S1,
            rs: Register::ZERO,
            immediate: u16::try_from(entry_count)?,
        })
        .label(loop_label)
        .emit(Instruction::Addu {
            rd: Register::A0,
            rs: Register::S0,
            rt: Register::ZERO,
        })
        .emit(Instruction::Lw {
            rt: Register::A1,
            base: Register::S0,
            offset: 8,
        })
        .emit(Instruction::Jal {
            target: VRAM_UPLOAD_ROUTINE_ADDRESS,
        })
        .emit(Instruction::Addiu {
            rt: Register::S0,
            rs: Register::S0,
            immediate: TEXTURE_UPLOAD_DESCRIPTOR_BYTE_COUNT as i16,
        })
        .emit(Instruction::Addiu {
            rt: Register::S1,
            rs: Register::S1,
            immediate: -1,
        })
        .bne(Register::S1, Register::ZERO, loop_label)
        .emit(Instruction::nop())
        .emit(Instruction::Lw {
            rt: Register::V0,
            base: Register::SP,
            offset: 16,
        })
        .emit(Instruction::Lw {
            rt: Register::RA,
            base: Register::SP,
            offset: 28,
        })
        .emit(Instruction::Lw {
            rt: Register::S0,
            base: Register::SP,
            offset: 24,
        })
        .emit(Instruction::Lw {
            rt: Register::S1,
            base: Register::SP,
            offset: 20,
        })
        .emit(Instruction::Jr { rs: Register::RA })
        .emit(Instruction::Addiu {
            rt: Register::SP,
            rs: Register::SP,
            immediate: 32,
        });
    let program = assembler
        .assemble(program_runtime_address)
        .with_context(|| format!("failed to assemble {context} runtime texture uploader"))?;
    ensure!(
        verify_placed_program(program.bytes(), program_runtime_address)? == program.instructions(),
        "{context} runtime texture upload program failed typed placement verification"
    );
    Ok(ContextualTextureUploadProgram {
        bytes: program.bytes().to_vec(),
        typed_instruction_count: program.instruction_spans().len(),
    })
}

pub(crate) fn menu_atlas_vram_rect(cell: Cell) -> Result<[u16; 4]> {
    ensure!(
        cell.x.is_multiple_of(4) && cell.width.is_multiple_of(4),
        "contextual texture is not aligned to PlayStation VRAM words"
    );
    Ok([
        u16::try_from(768 + cell.x / 4)?,
        u16::try_from(cell.y)?,
        u16::try_from(cell.width / 4)?,
        u16::try_from(cell.height)?,
    ])
}

pub(crate) fn encode_texture_upload_descriptor(
    output: &mut Vec<u8>,
    rect: [u16; 4],
    source_address: u32,
) {
    for value in rect {
        output.extend_from_slice(&value.to_le_bytes());
    }
    output.extend_from_slice(&source_address.to_le_bytes());
}

pub(crate) fn pack_4bpp_pixels(pixels: &[u8]) -> Result<Vec<u8>> {
    let (pairs, remainder) = pixels.as_chunks::<2>();
    ensure!(
        remainder.is_empty(),
        "contextual texture has an odd pixel count"
    );
    ensure!(
        pixels.iter().all(|pixel| *pixel < 16),
        "contextual texture contains a non-4-bpp index"
    );
    Ok(pairs.iter().map(|pair| pair[0] | pair[1] << 4).collect())
}
