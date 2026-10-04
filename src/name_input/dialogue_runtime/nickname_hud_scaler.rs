use psx_r3000a::{Assembler, Instruction, Register};

use super::super::NicknameHudGlyphLayout;

const GLYPH_CELL_WORD_COUNT: u16 = 50;
const GLYPH_CELL_ROW_BYTES: u16 = 10;

pub(super) fn emit_nickname_hud_glyph_scaler(
    assembler: &mut Assembler,
    layout: &NicknameHudGlyphLayout,
) {
    let [source_x, source_y, source_width, source_height] = layout.source_bounds;
    let [target_x, target_y, target_width, target_height] = layout.target_bounds;
    let target_right = target_x + target_width;
    let target_bottom = target_y + target_height;

    assembler
        .label("scale_nickname_hud_glyph_cell")
        .emit(Instruction::Addu {
            rd: Register::T0,
            rs: Register::A2,
            rt: Register::ZERO,
        })
        .emit(Instruction::Ori {
            rt: Register::T1,
            rs: Register::ZERO,
            immediate: GLYPH_CELL_WORD_COUNT,
        })
        .label("clear_scaled_nickname_hud_word")
        .emit(Instruction::Sw {
            rt: Register::ZERO,
            base: Register::T0,
            offset: 0,
        })
        .emit(Instruction::Addiu {
            rt: Register::T0,
            rs: Register::T0,
            immediate: 4,
        })
        .emit(Instruction::Addiu {
            rt: Register::T1,
            rs: Register::T1,
            immediate: -1,
        })
        .bgtz(Register::T1, "clear_scaled_nickname_hud_word")
        .emit(Instruction::nop())
        .emit(Instruction::Ori {
            rt: Register::T0,
            rs: Register::ZERO,
            immediate: target_y as u16,
        })
        .label("scale_nickname_hud_row")
        .emit(Instruction::Addiu {
            rt: Register::T1,
            rs: Register::T0,
            immediate: -(target_y as i16),
        })
        .emit(Instruction::Ori {
            rt: Register::T2,
            rs: Register::ZERO,
            immediate: source_height as u16,
        })
        .emit(Instruction::Multu {
            rs: Register::T1,
            rt: Register::T2,
        })
        .emit(Instruction::Mflo { rd: Register::T1 })
        .emit(Instruction::Ori {
            rt: Register::T2,
            rs: Register::ZERO,
            immediate: target_height as u16,
        })
        .emit(Instruction::Divu {
            rs: Register::T1,
            rt: Register::T2,
        })
        .emit(Instruction::Mflo { rd: Register::T1 })
        .emit(Instruction::Addiu {
            rt: Register::T1,
            rs: Register::T1,
            immediate: source_y as i16,
        })
        .emit(Instruction::Multu {
            rs: Register::T1,
            rt: Register::A1,
        })
        .emit(Instruction::Mflo { rd: Register::T1 })
        .emit(Instruction::Addu {
            rd: Register::T1,
            rs: Register::A0,
            rt: Register::T1,
        })
        .emit(Instruction::Ori {
            rt: Register::T2,
            rs: Register::ZERO,
            immediate: GLYPH_CELL_ROW_BYTES,
        })
        .emit(Instruction::Multu {
            rs: Register::T0,
            rt: Register::T2,
        })
        .emit(Instruction::Mflo { rd: Register::T2 })
        .emit(Instruction::Addu {
            rd: Register::T2,
            rs: Register::A2,
            rt: Register::T2,
        })
        .emit(Instruction::Ori {
            rt: Register::T3,
            rs: Register::ZERO,
            immediate: target_x as u16,
        })
        .label("scale_nickname_hud_pixel")
        .emit(Instruction::Addiu {
            rt: Register::T4,
            rs: Register::T3,
            immediate: -(target_x as i16),
        })
        .emit(Instruction::Ori {
            rt: Register::T5,
            rs: Register::ZERO,
            immediate: source_width as u16,
        })
        .emit(Instruction::Multu {
            rs: Register::T4,
            rt: Register::T5,
        })
        .emit(Instruction::Mflo { rd: Register::T4 })
        .emit(Instruction::Ori {
            rt: Register::T5,
            rs: Register::ZERO,
            immediate: target_width as u16,
        })
        .emit(Instruction::Divu {
            rs: Register::T4,
            rt: Register::T5,
        })
        .emit(Instruction::Mflo { rd: Register::T4 })
        .emit(Instruction::Addiu {
            rt: Register::T4,
            rs: Register::T4,
            immediate: source_x as i16,
        })
        .emit(Instruction::Srl {
            rd: Register::T5,
            rt: Register::T4,
            shift: 1,
        })
        .emit(Instruction::Addu {
            rd: Register::T5,
            rs: Register::T1,
            rt: Register::T5,
        })
        .emit(Instruction::Lbu {
            rt: Register::T5,
            base: Register::T5,
            offset: 0,
        })
        .emit(Instruction::Andi {
            rt: Register::T6,
            rs: Register::T4,
            immediate: 1,
        })
        .emit(Instruction::Sll {
            rd: Register::T6,
            rt: Register::T6,
            shift: 2,
        })
        .emit(Instruction::Srlv {
            rd: Register::T5,
            rt: Register::T5,
            rs: Register::T6,
        })
        .emit(Instruction::Andi {
            rt: Register::T5,
            rs: Register::T5,
            immediate: 0x000f,
        })
        .emit(Instruction::Srl {
            rd: Register::T6,
            rt: Register::T3,
            shift: 1,
        })
        .emit(Instruction::Addu {
            rd: Register::T7,
            rs: Register::T2,
            rt: Register::T6,
        })
        .emit(Instruction::Lbu {
            rt: Register::T8,
            base: Register::T7,
            offset: 0,
        })
        .emit(Instruction::Andi {
            rt: Register::T9,
            rs: Register::T3,
            immediate: 1,
        })
        .emit(Instruction::Sll {
            rd: Register::T9,
            rt: Register::T9,
            shift: 2,
        })
        .emit(Instruction::Sllv {
            rd: Register::T5,
            rt: Register::T5,
            rs: Register::T9,
        })
        .emit(Instruction::Or {
            rd: Register::T8,
            rs: Register::T8,
            rt: Register::T5,
        })
        .emit(Instruction::Sb {
            rt: Register::T8,
            base: Register::T7,
            offset: 0,
        })
        .emit(Instruction::Addiu {
            rt: Register::T3,
            rs: Register::T3,
            immediate: 1,
        })
        .emit(Instruction::Sltiu {
            rt: Register::T4,
            rs: Register::T3,
            immediate: target_right as i16,
        })
        .bne(Register::T4, Register::ZERO, "scale_nickname_hud_pixel")
        .emit(Instruction::nop())
        .emit(Instruction::Addiu {
            rt: Register::T0,
            rs: Register::T0,
            immediate: 1,
        })
        .emit(Instruction::Sltiu {
            rt: Register::T1,
            rs: Register::T0,
            immediate: target_bottom as i16,
        })
        .bne(Register::T1, Register::ZERO, "scale_nickname_hud_row")
        .emit(Instruction::nop())
        .emit(Instruction::Jr { rs: Register::RA })
        .emit(Instruction::Ori {
            rt: Register::V0,
            rs: Register::ZERO,
            immediate: 1,
        });
}
