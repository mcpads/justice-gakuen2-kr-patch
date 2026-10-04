use anyhow::{Context, Result, ensure};
use psx_r3000a::{Assembler, Instruction, Register, encode_le_bytes, load_address};
use serde::Serialize;

use crate::pipeline::sha256_bytes;

use super::{
    MGAME_RELOAD_WRAPPER_ORIGIN, MGAME_RUNTIME_REPAIR_HELPER_ORIGIN,
    MGAME_RUNTIME_REPAIR_LITERAL_ORIGIN, MgameRuntimeRepairProgram, NAME_DIALOGUE_RUNTIME_ORIGIN,
    NAME_DIALOGUE_RUNTIME_STORAGE_ORIGIN,
};

#[path = "dialogue_runtime_bootstrap/nickname_hud_persistence.rs"]
mod nickname_hud_persistence;

use nickname_hud_persistence::build_nickname_hud_persistence_programs;
pub use nickname_hud_persistence::{
    DIRECT_NAME_HUD_STORE_ORIGIN, NICKNAME_HUD_GLYPH_STORE_ORIGIN, NICKNAME_HUD_MAGIC_ADDRESS,
    NICKNAME_HUD_PERSISTENT_CELL_ORIGIN, NICKNAME_HUD_STORAGE_OFFSET,
};
pub(super) use nickname_hud_persistence::{
    NICKNAME_HUD_MAGIC, nickname_hud_persistent_cell_address,
    nickname_hud_reused_scene_cell_address,
};

pub const NAME_DIALOGUE_RUNTIME_BOOTSTRAP_ORIGIN: u32 = 0x8001_0aa4;
pub const NAME_DIALOGUE_RUNTIME_BOOTSTRAP_BYTE_CAPACITY: usize = 0x0284;

const ENTRY_DELAY_SLOT_IMMEDIATE: i16 = -26_680;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NameDialogueRuntimeBootstrapProgram {
    pub bytes: Vec<u8>,
    pub instructions: Vec<Instruction>,
    pub nickname_hud_scaler_address: u32,
    pub nickname_hud_store_address: u32,
    pub report: NameDialogueRuntimeBootstrapProgramReport,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct NameDialogueRuntimeBootstrapProgramReport {
    pub origin: String,
    pub byte_count: usize,
    pub byte_capacity: usize,
    pub sha256: String,
    pub typed_instruction_count: usize,
    pub source_address: String,
    pub destination_address: String,
    pub copied_byte_count: usize,
    pub nickname_hud_store_address: String,
    pub nickname_hud_scaler_address: String,
    pub nickname_hud_magic_address: String,
    pub nickname_hud_persistent_cell_addresses: [String; 4],
    pub nickname_hud_reused_scene_cell_addresses: [String; 4],
    pub nickname_hud_writes_reused_scene_cells: bool,
    pub restores_entry_delay_slot: bool,
    pub installed: bool,
    pub runtime_execution_verified: bool,
}

pub fn build_name_dialogue_runtime_bootstrap_program(
    dialogue_runtime_byte_count: usize,
    nickname_hud_scaler_address: u32,
    mgame_runtime_repair: &MgameRuntimeRepairProgram,
) -> Result<NameDialogueRuntimeBootstrapProgram> {
    ensure!(
        dialogue_runtime_byte_count > 0 && dialogue_runtime_byte_count.is_multiple_of(4),
        "dialogue runtime bootstrap requires a nonempty word-aligned payload"
    );
    ensure!(
        dialogue_runtime_byte_count <= NICKNAME_HUD_STORAGE_OFFSET,
        "dialogue runtime overlaps the persistent nickname HUD cells"
    );
    ensure!(
        NAME_DIALOGUE_RUNTIME_ORIGIN.is_multiple_of(4),
        "dialogue runtime destination must be word-aligned"
    );
    let copy_counter_start = u16::try_from(dialogue_runtime_byte_count / 4)?
        .checked_sub(1)
        .context("dialogue runtime bootstrap word count overflow")?;

    let mut assembler = Assembler::new();
    assembler
        .emit_all(load_address(
            Register::T0,
            NAME_DIALOGUE_RUNTIME_STORAGE_ORIGIN,
        ))
        .emit_all(load_address(Register::T1, NAME_DIALOGUE_RUNTIME_ORIGIN))
        .emit(Instruction::Ori {
            rt: Register::T2,
            rs: Register::ZERO,
            immediate: copy_counter_start,
        })
        .label("copy_dialogue_runtime_word")
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
        .bgtz(Register::T2, "copy_dialogue_runtime_word")
        .emit(Instruction::Addiu {
            rt: Register::T2,
            rs: Register::T2,
            immediate: -1,
        })
        .emit(Instruction::Jr { rs: Register::RA })
        .emit(Instruction::Addiu {
            rt: Register::V0,
            rs: Register::V0,
            immediate: ENTRY_DELAY_SLOT_IMMEDIATE,
        });
    let bootstrap = assembler
        .assemble(NAME_DIALOGUE_RUNTIME_BOOTSTRAP_ORIGIN)
        .context("failed to assemble typed dialogue runtime bootstrap")?;
    ensure!(
        NAME_DIALOGUE_RUNTIME_BOOTSTRAP_ORIGIN + u32::try_from(bootstrap.bytes().len())?
            <= MGAME_RUNTIME_REPAIR_LITERAL_ORIGIN
            && MGAME_RUNTIME_REPAIR_LITERAL_ORIGIN
                + u32::try_from(mgame_runtime_repair.literal_bytes.len())?
                <= NICKNAME_HUD_GLYPH_STORE_ORIGIN,
        "dialogue runtime bootstrap or MGAME repair literals overlap the nickname HUD store"
    );
    let persistence = build_nickname_hud_persistence_programs(nickname_hud_scaler_address)?;
    ensure!(
        NICKNAME_HUD_GLYPH_STORE_ORIGIN + u32::try_from(persistence.store_bytes.len())?
            <= MGAME_RUNTIME_REPAIR_HELPER_ORIGIN
            && MGAME_RUNTIME_REPAIR_HELPER_ORIGIN
                + u32::try_from(mgame_runtime_repair.helper_bytes.len())?
                <= MGAME_RELOAD_WRAPPER_ORIGIN,
        "nickname HUD store or MGAME repair helper overlaps the reload wrapper"
    );
    let mut bytes = vec![0_u8; NAME_DIALOGUE_RUNTIME_BOOTSTRAP_BYTE_CAPACITY];
    place_program(
        &mut bytes,
        NAME_DIALOGUE_RUNTIME_BOOTSTRAP_ORIGIN,
        NAME_DIALOGUE_RUNTIME_BOOTSTRAP_ORIGIN,
        bootstrap.bytes(),
    )?;
    place_program(
        &mut bytes,
        NAME_DIALOGUE_RUNTIME_BOOTSTRAP_ORIGIN,
        MGAME_RUNTIME_REPAIR_LITERAL_ORIGIN,
        &mgame_runtime_repair.literal_bytes,
    )?;
    place_program(
        &mut bytes,
        NAME_DIALOGUE_RUNTIME_BOOTSTRAP_ORIGIN,
        NICKNAME_HUD_GLYPH_STORE_ORIGIN,
        &persistence.store_bytes,
    )?;
    let byte_count = place_program(
        &mut bytes,
        NAME_DIALOGUE_RUNTIME_BOOTSTRAP_ORIGIN,
        MGAME_RUNTIME_REPAIR_HELPER_ORIGIN,
        &mgame_runtime_repair.helper_bytes,
    )?;
    bytes.truncate(byte_count);
    ensure!(
        bytes.len() <= NAME_DIALOGUE_RUNTIME_BOOTSTRAP_BYTE_CAPACITY,
        "dialogue runtime bootstrap exceeds its main-executable region"
    );
    ensure!(
        bootstrap.instruction_spans().len() == bootstrap.instructions().len(),
        "dialogue runtime bootstrap lost typed instruction placement evidence"
    );
    ensure!(
        bytes.len().is_multiple_of(4),
        "dialogue runtime bootstrap has a partial instruction"
    );
    let mut instructions = vec![Instruction::nop(); bytes.len() / 4];
    place_instruction_source(
        &mut instructions,
        NAME_DIALOGUE_RUNTIME_BOOTSTRAP_ORIGIN,
        NAME_DIALOGUE_RUNTIME_BOOTSTRAP_ORIGIN,
        bootstrap.instructions(),
    )?;
    place_instruction_source(
        &mut instructions,
        NAME_DIALOGUE_RUNTIME_BOOTSTRAP_ORIGIN,
        MGAME_RUNTIME_REPAIR_LITERAL_ORIGIN,
        &mgame_runtime_repair.literal_instructions,
    )?;
    place_instruction_source(
        &mut instructions,
        NAME_DIALOGUE_RUNTIME_BOOTSTRAP_ORIGIN,
        NICKNAME_HUD_GLYPH_STORE_ORIGIN,
        &persistence.store_instructions,
    )?;
    place_instruction_source(
        &mut instructions,
        NAME_DIALOGUE_RUNTIME_BOOTSTRAP_ORIGIN,
        MGAME_RUNTIME_REPAIR_HELPER_ORIGIN,
        &mgame_runtime_repair.helper_instructions,
    )?;
    ensure!(
        encode_le_bytes(&instructions, NAME_DIALOGUE_RUNTIME_BOOTSTRAP_ORIGIN)? == bytes,
        "dialogue runtime bootstrap typed source differs from its installed bytes"
    );
    let typed_instruction_count = instructions.len();

    Ok(NameDialogueRuntimeBootstrapProgram {
        instructions,
        nickname_hud_scaler_address,
        nickname_hud_store_address: NICKNAME_HUD_GLYPH_STORE_ORIGIN,
        report: NameDialogueRuntimeBootstrapProgramReport {
            origin: hex_address(NAME_DIALOGUE_RUNTIME_BOOTSTRAP_ORIGIN),
            byte_count: bytes.len(),
            byte_capacity: NAME_DIALOGUE_RUNTIME_BOOTSTRAP_BYTE_CAPACITY,
            sha256: sha256_bytes(&bytes),
            typed_instruction_count,
            source_address: hex_address(NAME_DIALOGUE_RUNTIME_STORAGE_ORIGIN),
            destination_address: hex_address(NAME_DIALOGUE_RUNTIME_ORIGIN),
            copied_byte_count: dialogue_runtime_byte_count,
            nickname_hud_store_address: hex_address(NICKNAME_HUD_GLYPH_STORE_ORIGIN),
            nickname_hud_scaler_address: hex_address(nickname_hud_scaler_address),
            nickname_hud_magic_address: hex_address(NICKNAME_HUD_MAGIC_ADDRESS),
            nickname_hud_persistent_cell_addresses: [12, 13, 14, 15]
                .map(|slot| hex_address(nickname_hud_persistent_cell_address(slot).unwrap())),
            nickname_hud_reused_scene_cell_addresses: [12, 13, 14, 15]
                .map(|slot| hex_address(nickname_hud_reused_scene_cell_address(slot).unwrap())),
            nickname_hud_writes_reused_scene_cells: false,
            restores_entry_delay_slot: true,
            installed: false,
            runtime_execution_verified: false,
        },
        bytes,
    })
}

fn place_instruction_source(
    region: &mut [Instruction],
    region_origin: u32,
    program_origin: u32,
    program: &[Instruction],
) -> Result<()> {
    let byte_offset = usize::try_from(
        program_origin
            .checked_sub(region_origin)
            .context("bootstrap instruction source precedes its region")?,
    )?;
    ensure!(
        byte_offset.is_multiple_of(4),
        "bootstrap instruction source is not word-aligned"
    );
    let start = byte_offset / 4;
    let end = start
        .checked_add(program.len())
        .context("bootstrap instruction source range overflow")?;
    region
        .get_mut(start..end)
        .context("bootstrap instruction source leaves its installed extent")?
        .clone_from_slice(program);
    Ok(())
}

fn place_program(
    region: &mut [u8],
    region_origin: u32,
    program_origin: u32,
    program: &[u8],
) -> Result<usize> {
    let offset = usize::try_from(
        program_origin
            .checked_sub(region_origin)
            .context("nickname HUD program precedes the bootstrap region")?,
    )?;
    let end = offset
        .checked_add(program.len())
        .context("nickname HUD program range overflow")?;
    let destination = region
        .get_mut(offset..end)
        .context("nickname HUD program exceeds the bootstrap region")?;
    ensure!(
        destination.iter().all(|byte| *byte == 0),
        "nickname HUD programs overlap inside the bootstrap region"
    );
    destination.copy_from_slice(program);
    Ok(end)
}

fn hex_address(address: u32) -> String {
    format!("0x{address:08x}")
}
