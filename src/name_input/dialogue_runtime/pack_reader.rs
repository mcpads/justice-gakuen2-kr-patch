use psx_r3000a::{Assembler, Instruction, Register, load_address};

use super::super::NAME_GLYPH_PACK_STORAGE_BYTES;

const PACK_CELL_BYTES: u16 = 200;
const FIRST_STORAGE_CELL_COUNT: i16 = 28;
const FIRST_TWO_STORAGE_CELL_COUNT: i16 = 58;
const FIRST_SEQUENCE_BIAS: i16 = 56;
const SECOND_SEQUENCE_BIAS: i16 = 108;
const THIRD_SEQUENCE_BIAS: i16 = 133;
const SHARED_CODE_START: i16 = 0x0305;

pub(super) fn emit_dialogue_pack_byte_reader(
    assembler: &mut Assembler,
    current_atlas_pointer_address: u32,
) {
    assembler
        .label("read_name_glyph_pack_byte")
        .emit(Instruction::Sltiu {
            rt: Register::T0,
            rs: Register::A0,
            immediate: NAME_GLYPH_PACK_STORAGE_BYTES as i16,
        })
        .bne(
            Register::T0,
            Register::ZERO,
            "dialogue_pack_offset_in_range",
        )
        .emit(Instruction::nop())
        .emit(Instruction::Addu {
            rd: Register::V0,
            rs: Register::ZERO,
            rt: Register::ZERO,
        })
        .emit(Instruction::Jr { rs: Register::RA })
        .emit(Instruction::nop())
        .label("dialogue_pack_offset_in_range")
        .emit(Instruction::Ori {
            rt: Register::T0,
            rs: Register::ZERO,
            immediate: PACK_CELL_BYTES,
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
        .bne(Register::T0, Register::ZERO, "dialogue_pack_first_range")
        .emit(Instruction::nop())
        .emit(Instruction::Sltiu {
            rt: Register::T0,
            rs: Register::T1,
            immediate: FIRST_TWO_STORAGE_CELL_COUNT,
        })
        .bne(Register::T0, Register::ZERO, "dialogue_pack_second_range")
        .emit(Instruction::nop())
        .emit(Instruction::Addiu {
            rt: Register::T1,
            rs: Register::T1,
            immediate: THIRD_SEQUENCE_BIAS + SHARED_CODE_START,
        })
        .jump("dialogue_pack_code_ready")
        .emit(Instruction::nop())
        .label("dialogue_pack_first_range")
        .emit(Instruction::Addiu {
            rt: Register::T1,
            rs: Register::T1,
            immediate: FIRST_SEQUENCE_BIAS + SHARED_CODE_START,
        })
        .jump("dialogue_pack_code_ready")
        .emit(Instruction::nop())
        .label("dialogue_pack_second_range")
        .emit(Instruction::Addiu {
            rt: Register::T1,
            rs: Register::T1,
            immediate: SECOND_SEQUENCE_BIAS + SHARED_CODE_START,
        })
        .label("dialogue_pack_code_ready")
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
        .emit(Instruction::Sll {
            rd: Register::T0,
            rt: Register::T0,
            shift: 3,
        })
        .emit(Instruction::Addu {
            rd: Register::T0,
            rs: Register::T0,
            rt: Register::T1,
        })
        .emit(Instruction::Sll {
            rd: Register::T0,
            rt: Register::T0,
            shift: 3,
        })
        .emit_all(load_address(Register::T1, current_atlas_pointer_address))
        .emit(Instruction::Lw {
            rt: Register::T1,
            base: Register::T1,
            offset: 0,
        })
        .emit(Instruction::Addu {
            rd: Register::T0,
            rs: Register::T0,
            rt: Register::T2,
        })
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
