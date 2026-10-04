use psx_r3000a::{Assembler, Instruction::*, Register as R, load_address};

use super::super::{HANGUL_NAME_TAG, NAME_GLYPH_CODE_START};
use super::selected_glyph_validation::SELECTED_GLYPH_VALIDATOR_LABEL;

// The record cursor identifies the composing syllable, including its initial.
// T1 requests a new slot; T4 optionally replaces the old syllable after a split.
// No record or cursor writes occur until membership and field bounds pass.
pub(super) fn emit_selected_key_handler(
    a: &mut Assembler,
    simple: u32,
    compound: u32,
    medial: Option<u32>,
) {
    a.label("handle_selected_name_key")
        .emit(Addu {
            rd: R::T8,
            rs: R::V0,
            rt: R::ZERO,
        })
        .emit(Lhu {
            rt: R::T3,
            base: R::V0,
            offset: 0,
        })
        .emit(Lbu {
            rt: R::T0,
            base: R::S0,
            offset: 9,
        })
        .emit(Andi {
            rt: R::T6,
            rs: R::T3,
            immediate: 0xc000,
        })
        .bne(R::T0, R::ZERO, "select_direct_name_key")
        .emit(Addu {
            rd: R::T4,
            rs: R::ZERO,
            rt: R::ZERO,
        })
        .emit(Addiu {
            rt: R::T5,
            rs: R::V1,
            immediate: -(NAME_GLYPH_CODE_START as i16),
        })
        .emit(Sltiu {
            rt: R::T0,
            rs: R::T5,
            immediate: 40,
        })
        .beq(R::T0, R::ZERO, "return_name_key")
        .emit(Addu {
            rd: R::V0,
            rs: R::ZERO,
            rt: R::ZERO,
        })
        .emit(Andi {
            rt: R::T2,
            rs: R::T3,
            immediate: 0x3fff,
        })
        .emit(Ori {
            rt: R::T0,
            rs: R::ZERO,
            immediate: 28,
        })
        .emit(Divu {
            rs: R::T2,
            rt: R::T0,
        })
        .emit(Mfhi { rd: R::T7 })
        .emit(Sltiu {
            rt: R::T0,
            rs: R::T5,
            immediate: 19,
        })
        .beq(R::T0, R::ZERO, "select_name_vowel")
        .emit(Addu {
            rd: R::T1,
            rs: R::ZERO,
            rt: R::ZERO,
        })
        .emit(Ori {
            rt: R::T0,
            rs: R::ZERO,
            immediate: HANGUL_NAME_TAG,
        })
        .bne(R::T6, R::T0, "store_name_initial")
        .emit(Addu {
            rd: R::T3,
            rs: R::T5,
            rt: R::ZERO,
        })
        .bne(R::T7, R::ZERO, "select_compound_name_final")
        .emit(Sll {
            rd: R::T6,
            rt: R::T5,
            shift: 5,
        })
        .emit_all(load_address(R::T0, simple))
        .emit(Addu {
            rd: R::T0,
            rs: R::T0,
            rt: R::T5,
        })
        .emit(Lbu {
            rt: R::T6,
            base: R::T0,
            offset: 0,
        })
        .emit(Ori {
            rt: R::T0,
            rs: R::ZERO,
            immediate: 255,
        })
        .beq(R::T6, R::T0, "start_next_name_initial")
        .emit(psx_r3000a::Instruction::nop())
        .jump("compose_name_final")
        .emit(psx_r3000a::Instruction::nop())
        .label("select_compound_name_final")
        .emit(Or {
            rd: R::T6,
            rs: R::T6,
            rt: R::T7,
        })
        .emit_all(load_address(R::T0, compound))
        .label("scan_name_final")
        .emit(Lhu {
            rt: R::T3,
            base: R::T0,
            offset: 0,
        })
        .emit(Addiu {
            rt: R::T0,
            rs: R::T0,
            immediate: 2,
        })
        .beq(R::T3, R::ZERO, "start_next_name_initial")
        .emit(Andi {
            rt: R::V1,
            rs: R::T3,
            immediate: 0x03ff,
        })
        .bne(R::V1, R::T6, "scan_name_final")
        .emit(psx_r3000a::Instruction::nop())
        .emit(Srl {
            rd: R::T6,
            rt: R::T3,
            shift: 10,
        })
        .emit(Subu {
            rd: R::T2,
            rs: R::T2,
            rt: R::T7,
        })
        .label("compose_name_final")
        .emit(Addu {
            rd: R::T2,
            rs: R::T2,
            rt: R::T6,
        })
        .jump("validate_name_syllable")
        .emit(psx_r3000a::Instruction::nop())
        .label("start_next_name_initial")
        .emit(Ori {
            rt: R::T1,
            rs: R::ZERO,
            immediate: 1,
        })
        .emit(Addu {
            rd: R::T3,
            rs: R::T5,
            rt: R::ZERO,
        })
        .label("store_name_initial")
        .jump("store_name_stage")
        .emit(Ori {
            rt: R::T3,
            rs: R::T3,
            immediate: 0xc000,
        })
        .label("select_name_vowel")
        .emit(Addiu {
            rt: R::T1,
            rs: R::T5,
            immediate: -19,
        })
        .emit(Ori {
            rt: R::T0,
            rs: R::ZERO,
            immediate: 0xc000,
        })
        .beq(R::T6, R::T0, "compose_name_medial")
        .emit(Addu {
            rd: R::T9,
            rs: R::RA,
            rt: R::ZERO,
        })
        .emit(Ori {
            rt: R::T0,
            rs: R::ZERO,
            immediate: HANGUL_NAME_TAG,
        })
        .bne(R::T6, R::T0, "return_name_key")
        .emit(psx_r3000a::Instruction::nop())
        .beq(
            R::T7,
            R::ZERO,
            if medial.is_some() {
                "change_name_medial"
            } else {
                "return_name_key"
            },
        )
        .emit(psx_r3000a::Instruction::nop())
        .emit(Subu {
            rd: R::T4,
            rs: R::T3,
            rt: R::T7,
        })
        .call("split_name_final")
        .emit(Addu {
            rd: R::A1,
            rs: R::T7,
            rt: R::ZERO,
        })
        .emit(Addu {
            rd: R::RA,
            rs: R::T9,
            rt: R::ZERO,
        })
        .emit(Addu {
            rd: R::T4,
            rs: R::T4,
            rt: R::V1,
        })
        .emit(Addu {
            rd: R::T2,
            rs: R::V0,
            rt: R::ZERO,
        })
        .label("compose_name_medial")
        .emit(Ori {
            rt: R::T0,
            rs: R::ZERO,
            immediate: 588,
        })
        .emit(Multu {
            rs: R::T2,
            rt: R::T0,
        })
        .emit(Mflo { rd: R::T2 })
        .emit(Ori {
            rt: R::T0,
            rs: R::ZERO,
            immediate: 28,
        })
        .emit(Multu {
            rs: R::T1,
            rt: R::T0,
        })
        .emit(Mflo { rd: R::T1 })
        .emit(Addu {
            rd: R::T2,
            rs: R::T2,
            rt: R::T1,
        })
        .emit(Sltu {
            rd: R::T1,
            rs: R::ZERO,
            rt: R::T4,
        })
        .label("validate_name_syllable")
        .emit(Ori {
            rt: R::T3,
            rs: R::T2,
            immediate: HANGUL_NAME_TAG,
        })
        .emit(Addu {
            rd: R::T9,
            rs: R::RA,
            rt: R::ZERO,
        })
        .call(SELECTED_GLYPH_VALIDATOR_LABEL)
        .emit(Addu {
            rd: R::A0,
            rs: R::T2,
            rt: R::ZERO,
        })
        .emit(Ori {
            rt: R::T0,
            rs: R::ZERO,
            immediate: 65535,
        })
        .bne(R::V0, R::T0, "store_name_stage")
        .emit(Sltiu {
            rt: R::T0,
            rs: R::T5,
            immediate: 19,
        })
        // A consonant that cannot form an admitted final starts the next
        // syllable. Vowels still reject unsupported completed syllables.
        .beq(R::T0, R::ZERO, "reject_name_stage")
        .emit(psx_r3000a::Instruction::nop())
        .jump("start_next_name_initial")
        .emit(psx_r3000a::Instruction::nop())
        .label("select_direct_name_key")
        .emit(Ori {
            rt: R::T0,
            rs: R::ZERO,
            immediate: 0xc000,
        })
        .beq(R::T6, R::T0, "reject_name_stage")
        .emit(Addu {
            rd: R::T3,
            rs: R::V1,
            rt: R::ZERO,
        })
        .emit(Xori {
            rt: R::T1,
            rs: R::T6,
            immediate: HANGUL_NAME_TAG,
        })
        .emit(Sltiu {
            rt: R::T1,
            rs: R::T1,
            immediate: 1,
        })
        .label("store_name_stage")
        .beq(R::T1, R::ZERO, "write_name_stage")
        .emit(psx_r3000a::Instruction::nop())
        .emit(Lbu {
            rt: R::T0,
            base: R::S0,
            offset: 15,
        })
        .emit(Lbu {
            rt: R::T5,
            base: R::S0,
            offset: 10,
        })
        .emit(Sltiu {
            rt: R::T0,
            rs: R::T0,
            immediate: 2,
        })
        .emit(Sll {
            rd: R::T0,
            rt: R::T0,
            shift: 1,
        })
        .emit(Addiu {
            rt: R::T0,
            rs: R::T0,
            immediate: 3,
        })
        .emit(Sltu {
            rd: R::T0,
            rs: R::T5,
            rt: R::T0,
        })
        .beq(R::T0, R::ZERO, "reject_name_stage")
        .emit(Addiu {
            rt: R::T5,
            rs: R::T5,
            immediate: 1,
        })
        .emit(Sb {
            rt: R::T5,
            base: R::S0,
            offset: 10,
        })
        .beq(R::T4, R::ZERO, "write_name_stage")
        .emit(Addiu {
            rt: R::T8,
            rs: R::T8,
            immediate: 2,
        })
        .emit(Sh {
            rt: R::T4,
            base: R::T8,
            offset: -2,
        })
        .label("write_name_stage")
        .emit(Sh {
            rt: R::T3,
            base: R::T8,
            offset: 0,
        })
        .emit(Lbu {
            rt: R::V0,
            base: R::S0,
            offset: 9,
        })
        .emit(psx_r3000a::Instruction::nop())
        .jump("return_name_key")
        .emit(Sltu {
            rd: R::V0,
            rs: R::ZERO,
            rt: R::V0,
        })
        .label("reject_name_stage")
        .emit(Addu {
            rd: R::V0,
            rs: R::ZERO,
            rt: R::ZERO,
        })
        .label("return_name_key")
        .emit(Jr { rs: R::RA })
        .emit(psx_r3000a::Instruction::nop());
    if let Some(target) = medial {
        // LO still contains syllable / 28 from the initial stage dispatch.
        a.label("change_name_medial")
            .emit(Mflo { rd: R::A0 })
            .emit(Ori {
                rt: R::T0,
                rs: R::ZERO,
                immediate: 21,
            })
            .emit(Divu {
                rs: R::A0,
                rt: R::T0,
            })
            .emit(Mfhi { rd: R::A0 })
            .emit(Mflo { rd: R::T2 })
            .emit(Jal { target })
            .emit(Addu {
                rd: R::A1,
                rs: R::T1,
                rt: R::ZERO,
            })
            .emit(Addu {
                rd: R::RA,
                rs: R::T9,
                rt: R::ZERO,
            })
            .jump("compose_name_medial")
            .emit(Addu {
                rd: R::T1,
                rs: R::V0,
                rt: R::ZERO,
            });
    }
}
