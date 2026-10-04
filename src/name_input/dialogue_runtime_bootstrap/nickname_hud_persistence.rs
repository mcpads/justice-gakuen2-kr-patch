use anyhow::{Context, Result, ensure};
use psx_r3000a::{Assembler, Instruction, Register};

use super::super::dialogue_runtime::{
    NAME_DIALOGUE_RUNTIME_STORAGE_BYTE_CAPACITY, NAME_DIALOGUE_RUNTIME_STORAGE_ORIGIN,
};

#[path = "nickname_hud_persistence/glyph_store.rs"]
mod glyph_store;

use glyph_store::emit_nickname_hud_glyph_store;

pub const NICKNAME_HUD_GLYPH_STORE_ORIGIN: u32 = 0x8001_0b00;
pub const DIRECT_NAME_HUD_STORE_ORIGIN: u32 = 0x8001_0b68;

pub const NICKNAME_HUD_STORAGE_OFFSET: usize = 0x09d0;
pub const NICKNAME_HUD_MAGIC_ADDRESS: u32 =
    NAME_DIALOGUE_RUNTIME_STORAGE_ORIGIN + NICKNAME_HUD_STORAGE_OFFSET as u32;
pub const NICKNAME_HUD_PERSISTENT_CELL_ORIGIN: u32 = NICKNAME_HUD_MAGIC_ADDRESS + 0x10;

const NICKNAME_HUD_CACHE_SLOT_START: u16 = 12;
const NICKNAME_HUD_CACHE_SLOT_COUNT: u16 = 4;
const NICKNAME_HUD_REUSED_SCENE_ATLAS_ADDRESS: u32 = 0x8014_d148;
const NICKNAME_HUD_FIRST_REUSED_SCENE_GLYPH_INDEX: u16 = 0x0203;
pub(in crate::name_input) const NICKNAME_HUD_GLYPH_CELL_BYTES: u32 = 200;
const NICKNAME_HUD_GLYPH_STORAGE_BYTES: usize =
    NICKNAME_HUD_GLYPH_CELL_BYTES as usize * NICKNAME_HUD_CACHE_SLOT_COUNT as usize;
pub(in crate::name_input) const NICKNAME_HUD_MAGIC: u32 = u32::from_le_bytes(*b"NHUD");

pub(super) struct NicknameHudPersistencePrograms {
    pub(super) store_bytes: Vec<u8>,
    pub(super) store_instructions: Vec<Instruction>,
}

pub(in crate::name_input) fn nickname_hud_reused_scene_cell_address(
    cache_slot: u16,
) -> Option<u32> {
    let relative_slot = cache_slot.checked_sub(NICKNAME_HUD_CACHE_SLOT_START)?;
    if relative_slot >= NICKNAME_HUD_CACHE_SLOT_COUNT {
        return None;
    }
    let skipped_source_cell = u16::from(relative_slot >= 3);
    let glyph_index = NICKNAME_HUD_FIRST_REUSED_SCENE_GLYPH_INDEX
        .checked_add(relative_slot)?
        .checked_add(skipped_source_cell)?;
    NICKNAME_HUD_REUSED_SCENE_ATLAS_ADDRESS
        .checked_add(u32::from(glyph_index) * NICKNAME_HUD_GLYPH_CELL_BYTES)
}

pub(in crate::name_input) fn nickname_hud_persistent_cell_address(cache_slot: u16) -> Option<u32> {
    let relative_slot = cache_slot.checked_sub(NICKNAME_HUD_CACHE_SLOT_START)?;
    if relative_slot >= NICKNAME_HUD_CACHE_SLOT_COUNT {
        return None;
    }
    NICKNAME_HUD_PERSISTENT_CELL_ORIGIN
        .checked_add(u32::from(relative_slot) * NICKNAME_HUD_GLYPH_CELL_BYTES)
}

pub(super) fn build_nickname_hud_persistence_programs(
    nickname_hud_scaler_address: u32,
) -> Result<NicknameHudPersistencePrograms> {
    ensure!(
        NICKNAME_HUD_STORAGE_OFFSET + 0x10 + NICKNAME_HUD_GLYPH_STORAGE_BYTES
            == NAME_DIALOGUE_RUNTIME_STORAGE_BYTE_CAPACITY,
        "nickname HUD storage no longer owns the tail of the persistent runtime region"
    );

    let mut store = Assembler::new();
    emit_nickname_hud_glyph_store(&mut store, nickname_hud_scaler_address);
    let store_end = NICKNAME_HUD_GLYPH_STORE_ORIGIN
        + u32::try_from(
            store
                .assemble(NICKNAME_HUD_GLYPH_STORE_ORIGIN)?
                .bytes()
                .len(),
        )?;
    ensure!(
        store_end == DIRECT_NAME_HUD_STORE_ORIGIN,
        "direct HUD supplier must immediately follow the persistent store"
    );
    // A0 is the field cache slot, A1 the resolved legacy code, A2 the current
    // dialogue atlas pixel base. The store ignores slots outside nickname.
    store
        .emit(Instruction::Ori {
            rt: Register::T0,
            rs: Register::ZERO,
            immediate: 200,
        })
        .emit(Instruction::Multu {
            rs: Register::A1,
            rt: Register::T0,
        })
        .emit(Instruction::Mflo { rd: Register::A1 })
        .emit(Instruction::Addu {
            rd: Register::A1,
            rs: Register::A1,
            rt: Register::A2,
        })
        .emit(Instruction::J {
            target: NICKNAME_HUD_GLYPH_STORE_ORIGIN,
        })
        .emit(Instruction::Ori {
            rt: Register::A2,
            rs: Register::ZERO,
            immediate: 10,
        });
    let store = store
        .assemble(NICKNAME_HUD_GLYPH_STORE_ORIGIN)
        .context("failed to assemble typed nickname HUD glyph store")?;
    Ok(NicknameHudPersistencePrograms {
        store_bytes: store.bytes().to_vec(),
        store_instructions: store.instructions().to_vec(),
    })
}
