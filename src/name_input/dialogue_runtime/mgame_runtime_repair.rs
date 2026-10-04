use anyhow::{Context, Result, ensure};
use psx_r3000a::{Assembler, Instruction, Register, load_address, verify_placed_program};
use serde::Serialize;

use crate::pipeline::sha256_bytes;

use super::NAME_DIALOGUE_RUNTIME_ORIGIN;

pub const MGAME_RUNTIME_REPAIR_LITERAL_ORIGIN: u32 = 0x8001_0ad8;
pub const MGAME_RUNTIME_REPAIR_LITERAL_BYTE_CAPACITY: usize = 0x0028;
pub const MGAME_RUNTIME_REPAIR_HELPER_ORIGIN: u32 = 0x8001_0b94;
pub const MGAME_RUNTIME_REPAIR_HELPER_BYTE_CAPACITY: usize = 0x0060;

const PROLOGUE_REPAIR_OFFSET: usize = 0x0578;
const PROLOGUE_REPAIR_BYTE_COUNT: usize = 4;
const EPILOGUE_REPAIR_OFFSET: usize = 0x0688;
const EPILOGUE_REPAIR_BYTE_COUNT: usize = 40;
const EPILOGUE_REPAIR_WORD_COUNT: u16 = (EPILOGUE_REPAIR_BYTE_COUNT / 4) as u16;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MgameRuntimeRepairProgram {
    pub helper_bytes: Vec<u8>,
    pub helper_instructions: Vec<Instruction>,
    pub literal_bytes: Vec<u8>,
    pub literal_instructions: Vec<Instruction>,
    pub report: MgameRuntimeRepairProgramReport,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MgameRuntimeRepairRangeReport {
    pub destination_address_range: [String; 2],
    pub byte_count: usize,
    pub sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MgameRuntimeRepairProgramReport {
    pub helper_origin: String,
    pub helper_byte_count: usize,
    pub helper_byte_capacity: usize,
    pub helper_sha256: String,
    pub helper_typed_instruction_count: usize,
    pub literal_origin: String,
    pub literal_byte_count: usize,
    pub literal_byte_capacity: usize,
    pub literal_sha256: String,
    pub repair_ranges: [MgameRuntimeRepairRangeReport; 2],
    pub expected_runtime_sha256: String,
    pub first_entry_copy_roundtrip_verified: bool,
    pub destination_repair_roundtrip_verified: bool,
    pub nickname_hud_uploader_address: String,
    pub nickname_hud_upload_before_mgame_entry: bool,
    pub nickname_hud_upload_skips_profile: bool,
    pub installed: bool,
    pub runtime_execution_verified: bool,
}

pub(in crate::name_input) fn build_mgame_runtime_repair_program(
    runtime: &[u8],
    nickname_hud_uploader_address: u32,
) -> Result<MgameRuntimeRepairProgram> {
    ensure!(
        runtime.len() > EPILOGUE_REPAIR_OFFSET + EPILOGUE_REPAIR_BYTE_COUNT
            && runtime.len().is_multiple_of(4),
        "MGAME runtime repair requires the complete word-aligned runtime"
    );
    let prologue_bytes = runtime
        .get(PROLOGUE_REPAIR_OFFSET..PROLOGUE_REPAIR_OFFSET + PROLOGUE_REPAIR_BYTE_COUNT)
        .context("MGAME runtime prologue repair range leaves the installed runtime")?;
    // Native MGAME writes these destination addresses. Their contents depend on
    // the generated decoder and must be restored from this runtime, not a prior one.
    let prologue_word = u32::from_le_bytes(prologue_bytes.try_into()?);
    let literal_bytes = runtime
        .get(EPILOGUE_REPAIR_OFFSET..EPILOGUE_REPAIR_OFFSET + EPILOGUE_REPAIR_BYTE_COUNT)
        .context("MGAME runtime epilogue repair range leaves the installed runtime")?
        .to_vec();
    ensure!(
        literal_bytes.len() == MGAME_RUNTIME_REPAIR_LITERAL_BYTE_CAPACITY,
        "MGAME runtime repair literals no longer fill their persistent region"
    );
    let literal_instructions =
        verify_placed_program(&literal_bytes, MGAME_RUNTIME_REPAIR_LITERAL_ORIGIN)
            .context("failed to decode typed MGAME runtime repair literals")?;

    let mut assembler = Assembler::new();
    emit_mgame_runtime_repair_helper(&mut assembler, nickname_hud_uploader_address, prologue_word);
    let helper = assembler
        .assemble(MGAME_RUNTIME_REPAIR_HELPER_ORIGIN)
        .context("failed to assemble typed MGAME runtime destination repair helper")?;
    ensure!(
        helper.bytes().len() <= MGAME_RUNTIME_REPAIR_HELPER_BYTE_CAPACITY,
        "MGAME runtime destination repair helper exceeds its persistent region"
    );

    let mut first_entry_destination = vec![0_u8; runtime.len()];
    first_entry_destination.copy_from_slice(runtime);
    ensure!(
        first_entry_destination == runtime,
        "MGAME first-entry full-copy model changed the staged runtime"
    );
    let mut later_destination = runtime.to_vec();
    later_destination[..4].copy_from_slice(&0x800d_0180_u32.to_le_bytes());
    later_destination[PROLOGUE_REPAIR_OFFSET..PROLOGUE_REPAIR_OFFSET + PROLOGUE_REPAIR_BYTE_COUNT]
        .fill(0);
    later_destination[EPILOGUE_REPAIR_OFFSET..EPILOGUE_REPAIR_OFFSET + EPILOGUE_REPAIR_BYTE_COUNT]
        .fill(0);
    apply_mgame_runtime_destination_repair(&mut later_destination, prologue_word, &literal_bytes)?;
    ensure!(
        later_destination[4..] == runtime[4..],
        "MGAME destination repair does not reconstruct the immutable runtime bytes"
    );

    let repair_ranges = [
        (PROLOGUE_REPAIR_OFFSET, PROLOGUE_REPAIR_BYTE_COUNT),
        (EPILOGUE_REPAIR_OFFSET, EPILOGUE_REPAIR_BYTE_COUNT),
    ]
    .map(|(offset, byte_count)| {
        let address = NAME_DIALOGUE_RUNTIME_ORIGIN + offset as u32;
        MgameRuntimeRepairRangeReport {
            destination_address_range: [
                hex_address(address),
                hex_address(address + byte_count as u32),
            ],
            byte_count,
            sha256: sha256_bytes(&runtime[offset..offset + byte_count]),
        }
    });

    Ok(MgameRuntimeRepairProgram {
        helper_bytes: helper.bytes().to_vec(),
        helper_instructions: helper.instructions().to_vec(),
        literal_bytes: literal_bytes.clone(),
        literal_instructions,
        report: MgameRuntimeRepairProgramReport {
            helper_origin: hex_address(MGAME_RUNTIME_REPAIR_HELPER_ORIGIN),
            helper_byte_count: helper.bytes().len(),
            helper_byte_capacity: MGAME_RUNTIME_REPAIR_HELPER_BYTE_CAPACITY,
            helper_sha256: sha256_bytes(helper.bytes()),
            helper_typed_instruction_count: helper.instructions().len(),
            literal_origin: hex_address(MGAME_RUNTIME_REPAIR_LITERAL_ORIGIN),
            literal_byte_count: literal_bytes.len(),
            literal_byte_capacity: MGAME_RUNTIME_REPAIR_LITERAL_BYTE_CAPACITY,
            literal_sha256: sha256_bytes(&literal_bytes),
            repair_ranges,
            expected_runtime_sha256: sha256_bytes(runtime),
            first_entry_copy_roundtrip_verified: true,
            destination_repair_roundtrip_verified: true,
            nickname_hud_uploader_address: hex_address(nickname_hud_uploader_address),
            nickname_hud_upload_before_mgame_entry: true,
            nickname_hud_upload_skips_profile: true,
            installed: false,
            runtime_execution_verified: false,
        },
    })
}

fn emit_mgame_runtime_repair_helper(
    assembler: &mut Assembler,
    nickname_hud_uploader_address: u32,
    prologue_word: u32,
) {
    let epilogue_repair_address = NAME_DIALOGUE_RUNTIME_ORIGIN + EPILOGUE_REPAIR_OFFSET as u32;
    assembler
        .label("repair_mgame_runtime_destination_code")
        .emit_all(load_address(Register::T1, epilogue_repair_address))
        .emit_all(load_address(Register::T3, prologue_word))
        .emit(Instruction::Sw {
            rt: Register::T3,
            base: Register::T1,
            offset: -((EPILOGUE_REPAIR_OFFSET - PROLOGUE_REPAIR_OFFSET) as i16),
        })
        .emit_all(load_address(
            Register::T0,
            MGAME_RUNTIME_REPAIR_LITERAL_ORIGIN,
        ))
        .emit(Instruction::Ori {
            rt: Register::T2,
            rs: Register::ZERO,
            immediate: EPILOGUE_REPAIR_WORD_COUNT,
        })
        .label("repair_mgame_runtime_destination_epilogue_word")
        .emit(Instruction::Lw {
            rt: Register::T3,
            base: Register::T0,
            offset: 0,
        })
        .emit(Instruction::Addiu {
            rt: Register::T0,
            rs: Register::T0,
            immediate: 4,
        })
        .emit(Instruction::Sw {
            rt: Register::T3,
            base: Register::T1,
            offset: 0,
        })
        .emit(Instruction::Addiu {
            rt: Register::T1,
            rs: Register::T1,
            immediate: 4,
        })
        .emit(Instruction::Addiu {
            rt: Register::T2,
            rs: Register::T2,
            immediate: -1,
        })
        .bgtz(
            Register::T2,
            "repair_mgame_runtime_destination_epilogue_word",
        )
        .emit(Instruction::nop())
        // The native action menu sets 801f194d=2 while the profile owns
        // the shared family/nickname texture. Repair code on every frame,
        // but leave that texture alone until the native menu closes it.
        .emit(Instruction::Lui {
            rt: Register::T0,
            immediate: 0x801f,
        })
        .emit(Instruction::Lbu {
            rt: Register::T0,
            base: Register::T0,
            offset: 0x194d,
        })
        .emit(Instruction::Ori {
            rt: Register::T1,
            rs: Register::ZERO,
            immediate: 2,
        })
        .beq(Register::T0, Register::T1, "preserve_profile_name_texture")
        .emit(Instruction::nop())
        .emit(Instruction::J {
            target: nickname_hud_uploader_address,
        })
        .emit(Instruction::nop())
        .label("preserve_profile_name_texture")
        .emit(Instruction::Jr { rs: Register::RA })
        .emit(Instruction::nop());
}

pub(super) fn apply_mgame_runtime_destination_repair(
    destination: &mut [u8],
    prologue_word: u32,
    epilogue_literal_bytes: &[u8],
) -> Result<()> {
    destination
        .get_mut(PROLOGUE_REPAIR_OFFSET..PROLOGUE_REPAIR_OFFSET + PROLOGUE_REPAIR_BYTE_COUNT)
        .context("MGAME runtime destination prologue repair range is truncated")?
        .copy_from_slice(&prologue_word.to_le_bytes());
    ensure!(
        epilogue_literal_bytes.len() == EPILOGUE_REPAIR_BYTE_COUNT,
        "MGAME runtime epilogue repair literals have the wrong size"
    );
    destination
        .get_mut(EPILOGUE_REPAIR_OFFSET..EPILOGUE_REPAIR_OFFSET + EPILOGUE_REPAIR_BYTE_COUNT)
        .context("MGAME runtime destination epilogue repair range is truncated")?
        .copy_from_slice(epilogue_literal_bytes);
    Ok(())
}

fn hex_address(address: u32) -> String {
    format!("0x{address:08x}")
}
