use psx_r3000a::{Assembler, Instruction, Register, load_address};

use super::super::NAME_GLYPH_PACK_STORAGE_BYTES;
use super::NAME_INPUT_RUNTIME_ORIGIN;

const NAME_LAYOUT_TABLE_ADDRESS: u32 = 0x8017_aeea;
const NAME_FONT_PIXEL_ADDRESS: u32 = 0x800e_92e0;
const FIRST_STORAGE_CELL_COUNT: i16 = 28;
const FIRST_TWO_STORAGE_CELL_COUNT: i16 = 58;
const FIRST_LAYOUT_INDEX_BIAS: i16 = 56;
const SECOND_LAYOUT_INDEX_BIAS: i16 = 108;
const THIRD_LAYOUT_INDEX_BIAS: i16 = 133;

pub(super) fn emit_table_pack_byte_reader(assembler: &mut Assembler) {
    assembler
        .label("read_name_glyph_pack_byte")
        .emit(Instruction::Sltiu {
            rt: Register::T0,
            rs: Register::A0,
            immediate: NAME_GLYPH_PACK_STORAGE_BYTES as i16,
        })
        .bne(Register::T0, Register::ZERO, "table_pack_offset_in_range")
        .emit(Instruction::nop())
        .emit(Instruction::Addu {
            rd: Register::V0,
            rs: Register::ZERO,
            rt: Register::ZERO,
        })
        .emit(Instruction::Jr { rs: Register::RA })
        .emit(Instruction::nop())
        .label("table_pack_offset_in_range")
        .emit(Instruction::Ori {
            rt: Register::T0,
            rs: Register::ZERO,
            immediate: 200,
        })
        .emit(Instruction::Divu {
            rs: Register::A0,
            rt: Register::T0,
        })
        .emit(Instruction::Mflo { rd: Register::T1 })
        .emit(Instruction::Mfhi { rd: Register::T2 })
        .emit(Instruction::Sll {
            rd: Register::T1,
            rt: Register::T1,
            shift: 1,
        })
        .emit_all(load_address(Register::T0, NAME_INPUT_RUNTIME_ORIGIN))
        .emit(Instruction::Addu {
            rd: Register::T0,
            rs: Register::T0,
            rt: Register::T1,
        })
        .emit(Instruction::Lhu {
            rt: Register::T1,
            base: Register::T0,
            offset: 0,
        })
        .emit(Instruction::Ori {
            rt: Register::T0,
            rs: Register::ZERO,
            immediate: 10,
        })
        .emit(Instruction::Divu {
            rs: Register::T2,
            rt: Register::T0,
        })
        .emit(Instruction::Mflo { rd: Register::T3 })
        .emit(Instruction::Mfhi { rd: Register::T2 })
        .emit(Instruction::Sll {
            rd: Register::T0,
            rt: Register::T3,
            shift: 8,
        })
        .emit(Instruction::Sll {
            rd: Register::T3,
            rt: Register::T3,
            shift: 7,
        })
        .emit(Instruction::Addu {
            rd: Register::T3,
            rs: Register::T0,
            rt: Register::T3,
        })
        .emit(Instruction::Addu {
            rd: Register::T1,
            rs: Register::T1,
            rt: Register::T3,
        })
        .emit(Instruction::Addu {
            rd: Register::T1,
            rs: Register::T1,
            rt: Register::T2,
        })
        .emit_all(load_address(Register::T0, NAME_FONT_PIXEL_ADDRESS))
        .emit(Instruction::Addu {
            rd: Register::T0,
            rs: Register::T0,
            rt: Register::T1,
        })
        .emit(Instruction::Lbu {
            rt: Register::V0,
            base: Register::T0,
            offset: 0,
        })
        .emit(Instruction::Jr { rs: Register::RA })
        .emit(Instruction::nop());
}

pub(super) fn emit_stored_table_pack_byte_reader(
    assembler: &mut Assembler,
    lookup_cell_base_byte_offset: u16,
) {
    assembler
        .label("read_name_glyph_pack_byte")
        .emit(Instruction::Sltiu {
            rt: Register::T0,
            rs: Register::A0,
            immediate: NAME_GLYPH_PACK_STORAGE_BYTES as i16,
        })
        .bne(Register::T0, Register::ZERO, "stored_pack_offset_in_range")
        .emit(Instruction::nop())
        .emit(Instruction::Addu {
            rd: Register::V0,
            rs: Register::ZERO,
            rt: Register::ZERO,
        })
        .emit(Instruction::Jr { rs: Register::RA })
        .emit(Instruction::nop())
        .label("stored_pack_offset_in_range")
        .emit(Instruction::Ori {
            rt: Register::T0,
            rs: Register::ZERO,
            immediate: 200,
        })
        .emit(Instruction::Divu {
            rs: Register::A0,
            rt: Register::T0,
        })
        .emit(Instruction::Mflo { rd: Register::T1 })
        .emit(Instruction::Mfhi { rd: Register::T2 })
        .emit(Instruction::Sll {
            rd: Register::T1,
            rt: Register::T1,
            shift: 1,
        })
        .emit(Instruction::Ori {
            rt: Register::T0,
            rs: Register::ZERO,
            immediate: 10,
        })
        .emit(Instruction::Divu {
            rs: Register::T1,
            rt: Register::T0,
        })
        .emit(Instruction::Mflo { rd: Register::T3 })
        .emit(Instruction::Mfhi { rd: Register::T1 })
        .emit(Instruction::Sll {
            rd: Register::T0,
            rt: Register::T3,
            shift: 8,
        })
        .emit(Instruction::Sll {
            rd: Register::T3,
            rt: Register::T3,
            shift: 7,
        })
        .emit(Instruction::Addu {
            rd: Register::T3,
            rs: Register::T0,
            rt: Register::T3,
        })
        .emit(Instruction::Addu {
            rd: Register::T3,
            rs: Register::T3,
            rt: Register::T1,
        })
        .emit_all(load_address(
            Register::T0,
            NAME_FONT_PIXEL_ADDRESS + u32::from(lookup_cell_base_byte_offset),
        ))
        .emit(Instruction::Addu {
            rd: Register::T0,
            rs: Register::T0,
            rt: Register::T3,
        })
        .emit(Instruction::Lhu {
            rt: Register::T1,
            base: Register::T0,
            offset: 0,
        })
        .emit(Instruction::Ori {
            rt: Register::T0,
            rs: Register::ZERO,
            immediate: 10,
        })
        .emit(Instruction::Divu {
            rs: Register::T2,
            rt: Register::T0,
        })
        .emit(Instruction::Mflo { rd: Register::T3 })
        .emit(Instruction::Mfhi { rd: Register::T2 })
        .emit(Instruction::Sll {
            rd: Register::T0,
            rt: Register::T3,
            shift: 8,
        })
        .emit(Instruction::Sll {
            rd: Register::T3,
            rt: Register::T3,
            shift: 7,
        })
        .emit(Instruction::Addu {
            rd: Register::T3,
            rs: Register::T0,
            rt: Register::T3,
        })
        .emit(Instruction::Addu {
            rd: Register::T1,
            rs: Register::T1,
            rt: Register::T3,
        })
        .emit(Instruction::Addu {
            rd: Register::T1,
            rs: Register::T1,
            rt: Register::T2,
        })
        .emit_all(load_address(Register::T0, NAME_FONT_PIXEL_ADDRESS))
        .emit(Instruction::Addu {
            rd: Register::T0,
            rs: Register::T0,
            rt: Register::T1,
        })
        .emit(Instruction::Lbu {
            rt: Register::V0,
            base: Register::T0,
            offset: 0,
        })
        .emit(Instruction::Jr { rs: Register::RA })
        .emit(Instruction::nop());
}

pub(super) fn emit_pack_byte_reader(assembler: &mut Assembler) {
    assembler
        .label("read_name_glyph_pack_byte")
        .emit(Instruction::Sltiu {
            rt: Register::T0,
            rs: Register::A0,
            immediate: NAME_GLYPH_PACK_STORAGE_BYTES as i16,
        })
        .bne(Register::T0, Register::ZERO, "pack_offset_in_range")
        .emit(Instruction::nop())
        .emit(Instruction::Addu {
            rd: Register::V0,
            rs: Register::ZERO,
            rt: Register::ZERO,
        })
        .emit(Instruction::Jr { rs: Register::RA })
        .emit(Instruction::nop())
        .label("pack_offset_in_range")
        .emit(Instruction::Ori {
            rt: Register::T0,
            rs: Register::ZERO,
            immediate: 200,
        })
        .emit(Instruction::Divu {
            rs: Register::A0,
            rt: Register::T0,
        })
        .emit(Instruction::Mflo { rd: Register::T1 })
        .emit(Instruction::Mfhi { rd: Register::T2 })
        .emit(Instruction::Sltiu {
            rt: Register::T0,
            rs: Register::T1,
            immediate: FIRST_STORAGE_CELL_COUNT,
        })
        .bne(Register::T0, Register::ZERO, "first_storage_range")
        .emit(Instruction::nop())
        .emit(Instruction::Sltiu {
            rt: Register::T0,
            rs: Register::T1,
            immediate: FIRST_TWO_STORAGE_CELL_COUNT,
        })
        .bne(Register::T0, Register::ZERO, "second_storage_range")
        .emit(Instruction::nop())
        .emit(Instruction::Addiu {
            rt: Register::T1,
            rs: Register::T1,
            immediate: THIRD_LAYOUT_INDEX_BIAS,
        })
        .jump("layout_index_ready")
        .emit(Instruction::nop())
        .label("first_storage_range")
        .emit(Instruction::Addiu {
            rt: Register::T1,
            rs: Register::T1,
            immediate: FIRST_LAYOUT_INDEX_BIAS,
        })
        .jump("layout_index_ready")
        .emit(Instruction::nop())
        .label("second_storage_range")
        .emit(Instruction::Addiu {
            rt: Register::T1,
            rs: Register::T1,
            immediate: SECOND_LAYOUT_INDEX_BIAS,
        })
        .label("layout_index_ready")
        .emit(Instruction::Sll {
            rd: Register::T0,
            rt: Register::T1,
            shift: 1,
        })
        .emit(Instruction::Addu {
            rd: Register::T0,
            rs: Register::T0,
            rt: Register::T1,
        })
        .emit_all(load_address(Register::T1, NAME_LAYOUT_TABLE_ADDRESS))
        .emit(Instruction::Addu {
            rd: Register::T1,
            rs: Register::T1,
            rt: Register::T0,
        })
        .emit(Instruction::Lbu {
            rt: Register::T3,
            base: Register::T1,
            offset: 0,
        })
        .emit(Instruction::Lbu {
            rt: Register::T4,
            base: Register::T1,
            offset: 1,
        })
        .emit(Instruction::Lbu {
            rt: Register::T5,
            base: Register::T1,
            offset: 2,
        })
        .emit(Instruction::Addiu {
            rt: Register::T3,
            rs: Register::T3,
            immediate: -12,
        })
        .emit(Instruction::Sll {
            rd: Register::T3,
            rt: Register::T3,
            shift: 7,
        })
        .emit(Instruction::Sll {
            rd: Register::T0,
            rt: Register::T4,
            shift: 3,
        })
        .emit(Instruction::Sll {
            rd: Register::T4,
            rt: Register::T4,
            shift: 1,
        })
        .emit(Instruction::Addu {
            rd: Register::T4,
            rs: Register::T0,
            rt: Register::T4,
        })
        .emit(Instruction::Sll {
            rd: Register::T0,
            rt: Register::T5,
            shift: 13,
        })
        .emit(Instruction::Sll {
            rd: Register::T5,
            rt: Register::T5,
            shift: 9,
        })
        .emit(Instruction::Subu {
            rd: Register::T5,
            rs: Register::T0,
            rt: Register::T5,
        })
        .emit(Instruction::Ori {
            rt: Register::T0,
            rs: Register::ZERO,
            immediate: 10,
        })
        .emit(Instruction::Divu {
            rs: Register::T2,
            rt: Register::T0,
        })
        .emit(Instruction::Mflo { rd: Register::T6 })
        .emit(Instruction::Mfhi { rd: Register::T2 })
        .emit(Instruction::Sll {
            rd: Register::T0,
            rt: Register::T6,
            shift: 9,
        })
        .emit(Instruction::Sll {
            rd: Register::T6,
            rt: Register::T6,
            shift: 7,
        })
        .emit(Instruction::Subu {
            rd: Register::T6,
            rs: Register::T0,
            rt: Register::T6,
        })
        .emit(Instruction::Addu {
            rd: Register::T3,
            rs: Register::T3,
            rt: Register::T4,
        })
        .emit(Instruction::Addu {
            rd: Register::T3,
            rs: Register::T3,
            rt: Register::T5,
        })
        .emit(Instruction::Addu {
            rd: Register::T3,
            rs: Register::T3,
            rt: Register::T6,
        })
        .emit(Instruction::Addu {
            rd: Register::T3,
            rs: Register::T3,
            rt: Register::T2,
        })
        .emit_all(load_address(Register::T0, NAME_FONT_PIXEL_ADDRESS))
        .emit(Instruction::Addu {
            rd: Register::T0,
            rs: Register::T0,
            rt: Register::T3,
        })
        .emit(Instruction::Lbu {
            rt: Register::V0,
            base: Register::T0,
            offset: 0,
        })
        .emit(Instruction::Jr { rs: Register::RA })
        .emit(Instruction::nop());
}
