use anyhow::{Context, Result, ensure};
use psx_r3000a::{
    Assembler, Instruction, Register, decode, encode, load_address, verify_placed_program,
};

use super::{UPLOAD_ROUTINE_ADDRESS, UploadProgram};
use crate::options::source::OVERLAY_RUNTIME_BASE;
use crate::source_disc::{MAIN_TEXT_RUNTIME_BASE, main_executable_text};

pub(crate) const OPTIONS_HOOK_OFFSET: usize = 0x10c0;
pub(crate) const OPTIONS_HOOK_RUNTIME_ADDRESS: u32 =
    OVERLAY_RUNTIME_BASE + OPTIONS_HOOK_OFFSET as u32;
pub(crate) const OPTIONS_INITIALIZER_ADDRESS: u32 = 0x800a_3924;

pub(crate) const OPTIONS_PROGRAM_OFFSET: usize = 0x3530;
pub(crate) const OPTIONS_PROGRAM_RUNTIME_ADDRESS: u32 =
    OVERLAY_RUNTIME_BASE + OPTIONS_PROGRAM_OFFSET as u32;
pub(crate) const OPTIONS_PROGRAM_BYTE_CAPACITY: usize = 0x200;

pub(crate) const RUNTIME_DECOMPRESSOR_ADDRESS: u32 = 0x8001_5eac;
pub(crate) const MENU_LOADER_ADDRESS: u32 = 0x8001_5414;
pub(crate) const MENU_TIM_PARSER_ADDRESS: u32 = 0x8001_5fcc;
pub(crate) const MENU_CATALOG_INDEX: u16 = 42;
pub(crate) const MENU_LOAD_DESTINATION: u32 = 0x800d_4000;
pub(crate) const MENU_TIM_RUNTIME_ADDRESSES: [u32; 3] =
    [MENU_LOAD_DESTINATION, 0x800f_4800, 0x800f_6800];
pub(crate) const MENU_LOAD_CALL_RUNTIME_ADDRESS: u32 = 0x8001_3aac;
pub(crate) const MENU_TIM_PARSER_CALL_RUNTIME_ADDRESSES: [u32; 3] =
    [0x8001_3ab8, 0x8001_3ac4, 0x8001_3ad0];

#[derive(Debug, Clone, Copy)]
pub(crate) struct PayloadDecompression {
    pub(crate) input_start: u32,
    pub(crate) input_end: u32,
    pub(crate) output_start: u32,
}

pub(crate) fn build_options_upload_program(
    descriptor_runtime_address: u32,
    entry_count: usize,
    streams: &[PayloadDecompression],
) -> Result<UploadProgram> {
    ensure!(entry_count > 0, "OPTIONS contextual upload has no entries");
    ensure!(
        !streams.is_empty(),
        "OPTIONS contextual upload has no compressed payload streams"
    );
    ensure!(
        streams
            .iter()
            .all(|stream| stream.input_start < stream.input_end),
        "OPTIONS contextual upload has an empty compressed payload stream"
    );

    let mut assembler = Assembler::new();
    assembler
        .emit(Instruction::Addiu {
            rt: Register::SP,
            rs: Register::SP,
            immediate: -40,
        })
        .emit(Instruction::Sw {
            rt: Register::RA,
            base: Register::SP,
            offset: 36,
        })
        .emit(Instruction::Sw {
            rt: Register::S0,
            base: Register::SP,
            offset: 32,
        })
        .emit(Instruction::Sw {
            rt: Register::S1,
            base: Register::SP,
            offset: 28,
        })
        .emit(Instruction::Jal {
            target: OPTIONS_INITIALIZER_ADDRESS,
        })
        .emit(Instruction::nop())
        .emit(Instruction::Sw {
            rt: Register::V0,
            base: Register::SP,
            offset: 24,
        });
    for stream in streams {
        assembler
            .emit_all(load_address(Register::A0, stream.input_start))
            .emit_all(load_address(Register::A1, stream.input_end))
            .emit_all(load_address(Register::A2, stream.output_start))
            .emit(Instruction::Jal {
                target: RUNTIME_DECOMPRESSOR_ADDRESS,
            })
            .emit(Instruction::nop());
    }
    assembler
        .emit_all(load_address(Register::S0, descriptor_runtime_address))
        .emit(Instruction::Ori {
            rt: Register::S1,
            rs: Register::ZERO,
            immediate: u16::try_from(entry_count)?,
        })
        .label("upload_options_contextual_texture")
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
            target: UPLOAD_ROUTINE_ADDRESS,
        })
        .emit(Instruction::Addiu {
            rt: Register::S0,
            rs: Register::S0,
            immediate: super::super::model::DESCRIPTOR_BYTE_COUNT as i16,
        })
        .emit(Instruction::Addiu {
            rt: Register::S1,
            rs: Register::S1,
            immediate: -1,
        })
        .bne(
            Register::S1,
            Register::ZERO,
            "upload_options_contextual_texture",
        )
        .emit(Instruction::nop())
        .emit(Instruction::Lw {
            rt: Register::V0,
            base: Register::SP,
            offset: 24,
        })
        .emit(Instruction::Lw {
            rt: Register::RA,
            base: Register::SP,
            offset: 36,
        })
        .emit(Instruction::Lw {
            rt: Register::S0,
            base: Register::SP,
            offset: 32,
        })
        .emit(Instruction::Lw {
            rt: Register::S1,
            base: Register::SP,
            offset: 28,
        })
        .emit(Instruction::Jr { rs: Register::RA })
        .emit(Instruction::Addiu {
            rt: Register::SP,
            rs: Register::SP,
            immediate: 40,
        });
    let program = assembler
        .assemble(OPTIONS_PROGRAM_RUNTIME_ADDRESS)
        .context("failed to assemble OPTIONS contextual texture uploader")?;
    ensure!(
        program.bytes().len() <= OPTIONS_PROGRAM_BYTE_CAPACITY,
        "OPTIONS runtime texture upload program exceeds its fixed storage"
    );
    ensure!(
        verify_placed_program(program.bytes(), OPTIONS_PROGRAM_RUNTIME_ADDRESS)?
            == program.instructions(),
        "OPTIONS runtime texture upload program failed typed placement verification"
    );
    Ok(UploadProgram {
        bytes: program.bytes().to_vec(),
        typed_instruction_count: program.instruction_spans().len(),
    })
}

pub(crate) fn verify_native_menu_restore_sequence(source: &[u8]) -> Result<()> {
    let expected = [
        (
            MENU_LOAD_CALL_RUNTIME_ADDRESS - 8,
            Instruction::Lui {
                rt: Register::A0,
                immediate: (MENU_LOAD_DESTINATION >> 16) as u16,
            },
        ),
        (
            MENU_LOAD_CALL_RUNTIME_ADDRESS - 4,
            Instruction::Ori {
                rt: Register::A0,
                rs: Register::A0,
                immediate: MENU_LOAD_DESTINATION as u16,
            },
        ),
        (
            MENU_LOAD_CALL_RUNTIME_ADDRESS,
            Instruction::Jal {
                target: MENU_LOADER_ADDRESS,
            },
        ),
        (
            MENU_LOAD_CALL_RUNTIME_ADDRESS + 4,
            Instruction::Addiu {
                rt: Register::A1,
                rs: Register::ZERO,
                immediate: MENU_CATALOG_INDEX as i16,
            },
        ),
    ];
    for (address, instruction) in expected {
        ensure!(
            decode_main_instruction(source, address)? == instruction,
            "native MENU reload source changed at {address:#010x}"
        );
    }
    for (call_address, tim_address) in MENU_TIM_PARSER_CALL_RUNTIME_ADDRESSES
        .into_iter()
        .zip(MENU_TIM_RUNTIME_ADDRESSES)
    {
        ensure!(
            decode_main_instruction(source, call_address - 4)?
                == Instruction::Lui {
                    rt: Register::A0,
                    immediate: (tim_address >> 16) as u16,
                }
                && decode_main_instruction(source, call_address)?
                    == Instruction::Jal {
                        target: MENU_TIM_PARSER_ADDRESS,
                    }
                && decode_main_instruction(source, call_address + 4)?
                    == Instruction::Ori {
                        rt: Register::A0,
                        rs: Register::A0,
                        immediate: tim_address as u16,
                    },
            "native MENU TIM parser source changed at {call_address:#010x}"
        );
    }
    Ok(())
}

pub(crate) fn patch_options_initializer_call(source: &[u8], output: &mut [u8]) -> Result<()> {
    ensure!(
        decode_overlay_instruction(source, OPTIONS_HOOK_OFFSET)?
            == Instruction::Jal {
                target: OPTIONS_INITIALIZER_ADDRESS,
            }
            && decode_overlay_instruction(source, OPTIONS_HOOK_OFFSET + 4)? == Instruction::nop(),
        "NEWOPT OPTIONS initializer hook source changed"
    );
    ensure!(
        output.get(OPTIONS_HOOK_OFFSET..OPTIONS_HOOK_OFFSET + 4)
            == source.get(OPTIONS_HOOK_OFFSET..OPTIONS_HOOK_OFFSET + 4),
        "NEWOPT OPTIONS initializer hook overlaps another writer"
    );
    let instruction = Instruction::Jal {
        target: OPTIONS_PROGRAM_RUNTIME_ADDRESS,
    };
    let encoded = encode(&instruction, OPTIONS_HOOK_RUNTIME_ADDRESS)?;
    output[OPTIONS_HOOK_OFFSET..OPTIONS_HOOK_OFFSET + 4].copy_from_slice(&encoded.to_le_bytes());
    ensure!(
        decode_overlay_instruction(output, OPTIONS_HOOK_OFFSET)? == instruction
            && decode_overlay_instruction(output, OPTIONS_HOOK_OFFSET + 4)? == Instruction::nop(),
        "NEWOPT OPTIONS initializer hook failed final decode verification"
    );
    Ok(())
}

fn decode_overlay_instruction(bytes: &[u8], offset: usize) -> Result<Instruction> {
    let word = u32::from_le_bytes(
        bytes
            .get(offset..offset + 4)
            .context("NEWOPT OPTIONS instruction is truncated")?
            .try_into()
            .expect("four-byte instruction"),
    );
    decode(word, OVERLAY_RUNTIME_BASE + u32::try_from(offset)?)
        .context("failed to decode NEWOPT OPTIONS instruction")
}

fn decode_main_instruction(bytes: &[u8], address: u32) -> Result<Instruction> {
    let text = main_executable_text(bytes)?;
    let offset = address
        .checked_sub(MAIN_TEXT_RUNTIME_BASE)
        .context("native MENU instruction precedes the main executable")?;
    let offset = usize::try_from(offset)?;
    let word = u32::from_le_bytes(
        text.get(offset..offset + 4)
            .context("native MENU instruction is truncated")?
            .try_into()
            .expect("four-byte instruction"),
    );
    decode(word, address).context("failed to decode native MENU instruction")
}
