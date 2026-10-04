use psx_r3000a::{Assembler, Instruction, Register};

const FAILURE: u16 = u16::MAX;

pub(in crate::name_input) fn emit_prefixed_membership_rank(assembler: &mut Assembler) {
    assembler
        .label("rank_name_glyph_membership")
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
            rd: Register::T4,
            rs: Register::A0,
            rt: Register::ZERO,
        })
        .emit(Instruction::Addu {
            rd: Register::T5,
            rs: Register::A1,
            rt: Register::ZERO,
        })
        .emit(Instruction::Addu {
            rd: Register::T6,
            rs: Register::A2,
            rt: Register::ZERO,
        })
        .emit(Instruction::Srl {
            rd: Register::T0,
            rt: Register::T6,
            shift: 3,
        })
        .emit(Instruction::Addu {
            rd: Register::A0,
            rs: Register::T4,
            rt: Register::T0,
        })
        .call("read_name_glyph_pack_byte")
        .emit(Instruction::nop())
        .emit(Instruction::Addu {
            rd: Register::T7,
            rs: Register::V0,
            rt: Register::ZERO,
        })
        .emit(Instruction::Andi {
            rt: Register::T0,
            rs: Register::T6,
            immediate: 7,
        })
        .emit(Instruction::Ori {
            rt: Register::T1,
            rs: Register::ZERO,
            immediate: 1,
        })
        .emit(Instruction::Sllv {
            rd: Register::T1,
            rt: Register::T1,
            rs: Register::T0,
        })
        .emit(Instruction::And {
            rd: Register::T1,
            rs: Register::T7,
            rt: Register::T1,
        })
        .beq(Register::T1, Register::ZERO, "membership_rank_failed")
        .emit(Instruction::Srl {
            rd: Register::T0,
            rt: Register::T6,
            shift: 3,
        })
        .emit(Instruction::Ori {
            rt: Register::T1,
            rs: Register::ZERO,
            immediate: 1,
        })
        .beq(
            Register::A3,
            Register::T1,
            "membership_rank_one_byte_prefix",
        )
        .emit(Instruction::nop())
        .emit(Instruction::Sll {
            rd: Register::T0,
            rt: Register::T0,
            shift: 1,
        })
        .emit(Instruction::Addu {
            rd: Register::A0,
            rs: Register::T5,
            rt: Register::T0,
        })
        .call("read_name_glyph_pack_byte")
        .emit(Instruction::nop())
        .emit(Instruction::Addu {
            rd: Register::T4,
            rs: Register::V0,
            rt: Register::ZERO,
        })
        .emit(Instruction::Srl {
            rd: Register::T0,
            rt: Register::T6,
            shift: 2,
        })
        .emit(Instruction::Andi {
            rt: Register::T0,
            rs: Register::T0,
            immediate: 0xfffe,
        })
        .emit(Instruction::Addu {
            rd: Register::A0,
            rs: Register::T5,
            rt: Register::T0,
        })
        .emit(Instruction::Addiu {
            rt: Register::A0,
            rs: Register::A0,
            immediate: 1,
        })
        .call("read_name_glyph_pack_byte")
        .emit(Instruction::nop())
        .emit(Instruction::Sll {
            rd: Register::V0,
            rt: Register::V0,
            shift: 8,
        })
        .jump("membership_rank_prefix_ready")
        .emit(Instruction::Or {
            rd: Register::T4,
            rs: Register::T4,
            rt: Register::V0,
        })
        .label("membership_rank_one_byte_prefix")
        .emit(Instruction::Addu {
            rd: Register::A0,
            rs: Register::T5,
            rt: Register::T0,
        })
        .call("read_name_glyph_pack_byte")
        .emit(Instruction::nop())
        .emit(Instruction::Addu {
            rd: Register::T4,
            rs: Register::V0,
            rt: Register::ZERO,
        })
        .label("membership_rank_prefix_ready")
        .emit(Instruction::Andi {
            rt: Register::T0,
            rs: Register::T6,
            immediate: 7,
        })
        .emit(Instruction::Ori {
            rt: Register::T1,
            rs: Register::ZERO,
            immediate: 1,
        })
        .emit(Instruction::Sllv {
            rd: Register::T1,
            rt: Register::T1,
            rs: Register::T0,
        })
        .emit(Instruction::Addiu {
            rt: Register::T1,
            rs: Register::T1,
            immediate: -1,
        })
        .emit(Instruction::And {
            rd: Register::V0,
            rs: Register::T7,
            rt: Register::T1,
        })
        .call("count_byte_population")
        .emit(Instruction::nop())
        .emit(Instruction::Addu {
            rd: Register::V0,
            rs: Register::T4,
            rt: Register::V0,
        })
        .jump("membership_rank_complete")
        .emit(Instruction::nop())
        .label("membership_rank_failed")
        .emit(Instruction::Ori {
            rt: Register::V0,
            rs: Register::ZERO,
            immediate: FAILURE,
        })
        .label("membership_rank_complete")
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
        .emit(Instruction::Jr { rs: Register::RA })
        .emit(Instruction::nop());

    emit_byte_population_count(assembler);
}

fn emit_byte_population_count(assembler: &mut Assembler) {
    assembler
        .label("count_byte_population")
        .emit(Instruction::Srl {
            rd: Register::T0,
            rt: Register::V0,
            shift: 1,
        })
        .emit(Instruction::Andi {
            rt: Register::T0,
            rs: Register::T0,
            immediate: 0x55,
        })
        .emit(Instruction::Subu {
            rd: Register::V0,
            rs: Register::V0,
            rt: Register::T0,
        })
        .emit(Instruction::Andi {
            rt: Register::T0,
            rs: Register::V0,
            immediate: 0x33,
        })
        .emit(Instruction::Srl {
            rd: Register::T1,
            rt: Register::V0,
            shift: 2,
        })
        .emit(Instruction::Andi {
            rt: Register::T1,
            rs: Register::T1,
            immediate: 0x33,
        })
        .emit(Instruction::Addu {
            rd: Register::V0,
            rs: Register::T0,
            rt: Register::T1,
        })
        .emit(Instruction::Srl {
            rd: Register::T0,
            rt: Register::V0,
            shift: 4,
        })
        .emit(Instruction::Addu {
            rd: Register::V0,
            rs: Register::V0,
            rt: Register::T0,
        })
        .emit(Instruction::Andi {
            rt: Register::V0,
            rs: Register::V0,
            immediate: 0x0f,
        })
        .emit(Instruction::Jr { rs: Register::RA })
        .emit(Instruction::nop());
}
