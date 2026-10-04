use psx_r3000a::{Assembler, Instruction, Register, load_address};

use super::{
    NICKNAME_HUD_CACHE_SLOT_COUNT, NICKNAME_HUD_CACHE_SLOT_START, NICKNAME_HUD_GLYPH_CELL_BYTES,
    NICKNAME_HUD_MAGIC, NICKNAME_HUD_MAGIC_ADDRESS, NICKNAME_HUD_PERSISTENT_CELL_ORIGIN,
};

pub(super) fn emit_nickname_hud_glyph_store(
    assembler: &mut Assembler,
    nickname_hud_scaler_address: u32,
) {
    assembler
        .label("store_nickname_hud_glyph_cell")
        .emit(Instruction::Addiu {
            rt: Register::T0,
            rs: Register::A0,
            immediate: -(NICKNAME_HUD_CACHE_SLOT_START as i16),
        })
        .emit(Instruction::Sltiu {
            rt: Register::T1,
            rs: Register::T0,
            immediate: NICKNAME_HUD_CACHE_SLOT_COUNT as i16,
        })
        .beq(Register::T1, Register::ZERO, "nickname_hud_store_complete")
        .emit(Instruction::nop())
        .emit_all(load_address(
            Register::T1,
            NICKNAME_HUD_PERSISTENT_CELL_ORIGIN,
        ))
        .emit(Instruction::Ori {
            rt: Register::T2,
            rs: Register::ZERO,
            immediate: NICKNAME_HUD_GLYPH_CELL_BYTES as u16,
        })
        .emit(Instruction::Multu {
            rs: Register::T0,
            rt: Register::T2,
        })
        .emit(Instruction::Mflo { rd: Register::T3 })
        .emit(Instruction::Addu {
            rd: Register::T1,
            rs: Register::T1,
            rt: Register::T3,
        })
        .emit(Instruction::Addiu {
            rt: Register::SP,
            rs: Register::SP,
            immediate: -8,
        })
        .emit(Instruction::Sw {
            rt: Register::RA,
            base: Register::SP,
            offset: 4,
        })
        .emit(Instruction::Addu {
            rd: Register::A0,
            rs: Register::A1,
            rt: Register::ZERO,
        })
        .emit(Instruction::Addu {
            rd: Register::A1,
            rs: Register::A2,
            rt: Register::ZERO,
        })
        .emit(Instruction::Addu {
            rd: Register::A2,
            rs: Register::T1,
            rt: Register::ZERO,
        })
        .emit(Instruction::Jal {
            target: nickname_hud_scaler_address,
        })
        .emit(Instruction::nop())
        .emit_all(load_address(Register::T0, NICKNAME_HUD_MAGIC_ADDRESS))
        .emit_all(load_address(Register::T1, NICKNAME_HUD_MAGIC))
        .emit(Instruction::Sw {
            rt: Register::T1,
            base: Register::T0,
            offset: 0,
        })
        .emit(Instruction::Lw {
            rt: Register::RA,
            base: Register::SP,
            offset: 4,
        })
        .emit(Instruction::Addiu {
            rt: Register::SP,
            rs: Register::SP,
            immediate: 8,
        })
        .label("nickname_hud_store_complete")
        .emit(Instruction::Jr { rs: Register::RA })
        .emit(Instruction::Ori {
            rt: Register::V0,
            rs: Register::ZERO,
            immediate: 1,
        });
}
