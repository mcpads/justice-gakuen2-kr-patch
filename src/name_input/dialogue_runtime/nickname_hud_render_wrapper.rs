use psx_r3000a::{Assembler, Instruction, Register, load_address};

use super::super::dialogue_runtime_bootstrap::{NICKNAME_HUD_MAGIC, NICKNAME_HUD_MAGIC_ADDRESS};

const WRAPPER_STACK_BYTES: i16 = 24;
const SAVED_RA_OFFSET: i16 = 20;

pub(super) fn emit_nickname_hud_render_wrapper(
    assembler: &mut Assembler,
    atlas_descriptor_address: u32,
    nickname_record_address: u32,
    name_consumer_address: u32,
    nickname_hud_uploader_address: u32,
) {
    assembler
        .label("load_nickname_hud_texture")
        .emit(Instruction::Addiu {
            rt: Register::SP,
            rs: Register::SP,
            immediate: -WRAPPER_STACK_BYTES,
        })
        .emit(Instruction::Sw {
            rt: Register::RA,
            base: Register::SP,
            offset: SAVED_RA_OFFSET,
        })
        .emit(Instruction::Jalr {
            rd: Register::RA,
            rs: Register::V0,
        })
        .emit(Instruction::nop())
        // S2 retains the native input record. A profile family/given load shares
        // this palette loader but must not overwrite its texture with nickname.
        // The independent LUI also observes the native return's load delay.
        .emit_all(load_address(Register::T0, nickname_record_address))
        .bne(Register::S2, Register::T0, "nickname_hud_render_complete")
        .emit(Instruction::Lui {
            rt: Register::T0,
            immediate: (NICKNAME_HUD_MAGIC_ADDRESS >> 16) as u16,
        })
        .emit(Instruction::Ori {
            rt: Register::T0,
            rs: Register::T0,
            immediate: NICKNAME_HUD_MAGIC_ADDRESS as u16,
        })
        // A native card load can change the canonical name while preserving
        // this resident buffer. Rebuild for each nickname texture load; magic
        // only admits the result of this call, never a preceding character.
        .emit(Instruction::Sw {
            rt: Register::ZERO,
            base: Register::T0,
            offset: 0,
        })
        .emit_all(load_address(Register::A0, atlas_descriptor_address))
        .emit_all(load_address(Register::A1, nickname_record_address))
        .emit(Instruction::Jal {
            target: name_consumer_address,
        })
        .emit(Instruction::Addu {
            rd: Register::A2,
            rs: Register::ZERO,
            rt: Register::ZERO,
        })
        // An empty or malformed record can leave the persistent cells
        // unmaterialized. Never upload that empty fallback buffer.
        .emit_all(load_address(Register::T0, NICKNAME_HUD_MAGIC_ADDRESS))
        .emit(Instruction::Lw {
            rt: Register::T0,
            base: Register::T0,
            offset: 0,
        })
        .emit_all(load_address(Register::T1, NICKNAME_HUD_MAGIC))
        .bne(Register::T0, Register::T1, "nickname_hud_render_complete")
        .emit(Instruction::nop())
        .label("upload_persistent_nickname_hud_glyph_cells")
        .emit(Instruction::Jal {
            target: nickname_hud_uploader_address,
        })
        .emit(Instruction::nop())
        .label("nickname_hud_render_complete")
        .emit(Instruction::Lw {
            rt: Register::RA,
            base: Register::SP,
            offset: SAVED_RA_OFFSET,
        })
        .emit(Instruction::Addiu {
            rt: Register::SP,
            rs: Register::SP,
            immediate: WRAPPER_STACK_BYTES,
        })
        .emit(Instruction::Jr { rs: Register::RA })
        .emit(Instruction::nop());
}
