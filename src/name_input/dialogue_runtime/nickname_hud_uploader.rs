use psx_r3000a::{Assembler, Instruction, Register, load_address};

use super::super::dialogue_runtime_bootstrap::NICKNAME_HUD_PERSISTENT_CELL_ORIGIN;
use super::super::redisplay_runtime::{
    ENGINE_RUNTIME_TABLE_POINTER_ADDRESS, TIM_LOADER_FUNCTION_OFFSET,
};

const GLYPH_CELL_BYTES: i16 = 200;
const GLYPH_CELL_WORD_COUNT: u16 = 50;
const UPLOAD_TIM_HEADER_BYTES: i16 = 20;
const UPLOAD_TIM_PIXEL_BYTES: u16 = 200;
const UPLOAD_TIM_BLOCK_BYTES: u16 = 12 + UPLOAD_TIM_PIXEL_BYTES;
const UPLOAD_STACK_BYTES: i16 = 248;

const SAVED_S4_OFFSET: i16 = 224;
const SAVED_S3_OFFSET: i16 = 228;
const SAVED_S2_OFFSET: i16 = 232;
const SAVED_S1_OFFSET: i16 = 236;
const SAVED_S0_OFFSET: i16 = 240;
const SAVED_RA_OFFSET: i16 = 244;

pub(in crate::name_input) const NICKNAME_HUD_VRAM_CELL_RECTS: [[u16; 4]; 4] = [
    [768, 492, 5, 20],
    [773, 492, 5, 20],
    [778, 492, 5, 20],
    [783, 492, 5, 20],
];

pub(super) fn emit_nickname_hud_glyph_uploader(assembler: &mut Assembler) {
    let [first_x, y, width, height] = NICKNAME_HUD_VRAM_CELL_RECTS[0];

    assembler
        .label("upload_nickname_hud_glyph_cells")
        .emit(Instruction::Addiu {
            rt: Register::SP,
            rs: Register::SP,
            immediate: -UPLOAD_STACK_BYTES,
        })
        .emit(Instruction::Sw {
            rt: Register::RA,
            base: Register::SP,
            offset: SAVED_RA_OFFSET,
        })
        .emit(Instruction::Sw {
            rt: Register::S0,
            base: Register::SP,
            offset: SAVED_S0_OFFSET,
        })
        .emit(Instruction::Sw {
            rt: Register::S1,
            base: Register::SP,
            offset: SAVED_S1_OFFSET,
        })
        .emit(Instruction::Sw {
            rt: Register::S2,
            base: Register::SP,
            offset: SAVED_S2_OFFSET,
        })
        .emit(Instruction::Sw {
            rt: Register::S3,
            base: Register::SP,
            offset: SAVED_S3_OFFSET,
        })
        .emit(Instruction::Sw {
            rt: Register::S4,
            base: Register::SP,
            offset: SAVED_S4_OFFSET,
        })
        // The wrapper validates or materializes the persistent cells before
        // every call. Each native HUD texture load can replace the same VRAM
        // rectangle, so uploading here must not be suppressed by a global
        // one-shot counter.
        .emit_all(load_address(
            Register::S0,
            NICKNAME_HUD_PERSISTENT_CELL_ORIGIN,
        ))
        .emit(Instruction::Ori {
            rt: Register::S1,
            rs: Register::ZERO,
            immediate: first_x,
        })
        .emit(Instruction::Ori {
            rt: Register::S2,
            rs: Register::ZERO,
            immediate: NICKNAME_HUD_VRAM_CELL_RECTS.len() as u16,
        })
        .emit(Instruction::Addu {
            rd: Register::S3,
            rs: Register::SP,
            rt: Register::ZERO,
        })
        .emit_all(load_address(
            Register::T0,
            ENGINE_RUNTIME_TABLE_POINTER_ADDRESS,
        ))
        .emit(Instruction::Lw {
            rt: Register::S4,
            base: Register::T0,
            offset: 0,
        })
        .emit(Instruction::nop())
        .emit(Instruction::Lw {
            rt: Register::S4,
            base: Register::S4,
            offset: TIM_LOADER_FUNCTION_OFFSET,
        })
        .emit(Instruction::nop())
        .emit(Instruction::Ori {
            rt: Register::T0,
            rs: Register::ZERO,
            immediate: 0x10,
        })
        .emit(Instruction::Sw {
            rt: Register::T0,
            base: Register::S3,
            offset: 0,
        })
        .emit(Instruction::Sw {
            rt: Register::ZERO,
            base: Register::S3,
            offset: 4,
        })
        .emit(Instruction::Ori {
            rt: Register::T0,
            rs: Register::ZERO,
            immediate: UPLOAD_TIM_BLOCK_BYTES,
        })
        .emit(Instruction::Sw {
            rt: Register::T0,
            base: Register::S3,
            offset: 8,
        })
        .emit(Instruction::Ori {
            rt: Register::T0,
            rs: Register::ZERO,
            immediate: y,
        })
        .emit(Instruction::Sh {
            rt: Register::T0,
            base: Register::S3,
            offset: 14,
        })
        .emit(Instruction::Ori {
            rt: Register::T0,
            rs: Register::ZERO,
            immediate: width,
        })
        .emit(Instruction::Sh {
            rt: Register::T0,
            base: Register::S3,
            offset: 16,
        })
        .emit(Instruction::Ori {
            rt: Register::T0,
            rs: Register::ZERO,
            immediate: height,
        })
        .emit(Instruction::Sh {
            rt: Register::T0,
            base: Register::S3,
            offset: 18,
        })
        .label("upload_nickname_hud_cell")
        .emit(Instruction::Sh {
            rt: Register::S1,
            base: Register::S3,
            offset: 12,
        })
        .emit(Instruction::Addu {
            rd: Register::T0,
            rs: Register::S0,
            rt: Register::ZERO,
        })
        .emit(Instruction::Addiu {
            rt: Register::T1,
            rs: Register::S3,
            immediate: UPLOAD_TIM_HEADER_BYTES,
        })
        .emit(Instruction::Ori {
            rt: Register::T2,
            rs: Register::ZERO,
            immediate: GLYPH_CELL_WORD_COUNT,
        })
        .label("copy_nickname_hud_upload_word")
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
        .bgtz(Register::T2, "copy_nickname_hud_upload_word")
        .emit(Instruction::nop())
        .emit(Instruction::Jalr {
            rd: Register::RA,
            rs: Register::S4,
        })
        .emit(Instruction::Addu {
            rd: Register::A0,
            rs: Register::S3,
            rt: Register::ZERO,
        })
        .emit(Instruction::Addiu {
            rt: Register::S0,
            rs: Register::S0,
            immediate: GLYPH_CELL_BYTES,
        })
        .emit(Instruction::Addiu {
            rt: Register::S1,
            rs: Register::S1,
            immediate: width as i16,
        })
        .emit(Instruction::Addiu {
            rt: Register::S2,
            rs: Register::S2,
            immediate: -1,
        })
        .bgtz(Register::S2, "upload_nickname_hud_cell")
        .emit(Instruction::nop())
        .emit(Instruction::Lw {
            rt: Register::S4,
            base: Register::SP,
            offset: SAVED_S4_OFFSET,
        })
        .emit(Instruction::Lw {
            rt: Register::S3,
            base: Register::SP,
            offset: SAVED_S3_OFFSET,
        })
        .emit(Instruction::Lw {
            rt: Register::S2,
            base: Register::SP,
            offset: SAVED_S2_OFFSET,
        })
        .emit(Instruction::Lw {
            rt: Register::S1,
            base: Register::SP,
            offset: SAVED_S1_OFFSET,
        })
        .emit(Instruction::Lw {
            rt: Register::S0,
            base: Register::SP,
            offset: SAVED_S0_OFFSET,
        })
        .emit(Instruction::Lw {
            rt: Register::RA,
            base: Register::SP,
            offset: SAVED_RA_OFFSET,
        })
        .emit(Instruction::Addiu {
            rt: Register::SP,
            rs: Register::SP,
            immediate: UPLOAD_STACK_BYTES,
        })
        .emit(Instruction::Jr { rs: Register::RA })
        .emit(Instruction::nop());
}
