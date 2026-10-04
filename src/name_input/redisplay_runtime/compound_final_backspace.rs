use psx_r3000a::{Assembler, Instruction::*, Register as R, load_address};

// Called for a complete syllable in the current slot. Removing its medial
// exposes the initial in that same slot instead of erasing the whole syllable.
pub(super) fn emit_compound_final_backspace(
    a: &mut Assembler,
    simple: u32,
    compound: u32,
    medial_backspace: Option<u32>,
) {
    a.label("backspace_compound_name_final")
        .emit(Andi {
            rt: R::T6,
            rs: R::T3,
            immediate: 0x3fff,
        })
        .emit(Ori {
            rt: R::T0,
            rs: R::ZERO,
            immediate: 28,
        })
        .emit(Divu {
            rs: R::T6,
            rt: R::T0,
        })
        .emit(Mfhi { rd: R::A1 })
        .beq(R::A1, R::ZERO, "backspace_name_medial")
        .emit(Addu {
            rd: R::T9,
            rs: R::RA,
            rt: R::ZERO,
        })
        .call("split_name_final")
        .emit(Subu {
            rd: R::T4,
            rs: R::T3,
            rt: R::A1,
        })
        .emit(Addu {
            rd: R::RA,
            rs: R::T9,
            rt: R::ZERO,
        })
        .jump("write_backspaced_name")
        .emit(Addu {
            rd: R::T3,
            rs: R::T4,
            rt: R::V1,
        })
        .label("backspace_name_medial");
    if let Some(target) = medial_backspace {
        a.emit(J { target }).emit(psx_r3000a::Instruction::nop());
    } else {
        a.emit(Ori {
            rt: R::T0,
            rs: R::ZERO,
            immediate: 588,
        })
        .emit(Divu {
            rs: R::T6,
            rt: R::T0,
        })
        .emit(Mflo { rd: R::T3 })
        .emit(Ori {
            rt: R::T3,
            rs: R::T3,
            immediate: 0xc000,
        });
    }
    a.label("write_backspaced_name")
        .emit(Sh {
            rt: R::T3,
            base: R::T2,
            offset: 0,
        })
        .emit(J {
            target: 0x8018_2c08,
        })
        .emit(Ori {
            rt: R::A0,
            rs: R::ZERO,
            immediate: 0x0092,
        })
        // A1: nonzero final; V0: next initial; V1: final remaining in old slot.
        // Preserve the pending record, cursor, vowel and return address.
        .label("split_name_final")
        .emit_all(load_address(R::T0, simple))
        .emit(Addu {
            rd: R::V0,
            rs: R::ZERO,
            rt: R::ZERO,
        })
        .label("scan_simple_name_final")
        .emit(Lbu {
            rt: R::T5,
            base: R::T0,
            offset: 0,
        })
        .emit(Addiu {
            rt: R::T0,
            rs: R::T0,
            immediate: 1,
        })
        .beq(R::T5, R::A1, "return_split_name_final")
        .emit(Addu {
            rd: R::V1,
            rs: R::ZERO,
            rt: R::ZERO,
        })
        .emit(Addiu {
            rt: R::V0,
            rs: R::V0,
            immediate: 1,
        })
        .emit(Sltiu {
            rt: R::T5,
            rs: R::V0,
            immediate: 19,
        })
        .bne(R::T5, R::ZERO, "scan_simple_name_final")
        .emit(psx_r3000a::Instruction::nop())
        .emit_all(load_address(R::T0, compound))
        .label("scan_split_compound_final")
        .emit(Lhu {
            rt: R::T5,
            base: R::T0,
            offset: 0,
        })
        .emit(Addiu {
            rt: R::T0,
            rs: R::T0,
            immediate: 2,
        })
        .emit(Srl {
            rd: R::T6,
            rt: R::T5,
            shift: 10,
        })
        .bne(R::T6, R::A1, "scan_split_compound_final")
        .emit(Srl {
            rd: R::V0,
            rt: R::T5,
            shift: 5,
        })
        .emit(Andi {
            rt: R::V0,
            rs: R::V0,
            immediate: 31,
        })
        .emit(Andi {
            rt: R::V1,
            rs: R::T5,
            immediate: 31,
        })
        .label("return_split_name_final")
        .emit(Jr { rs: R::RA })
        .emit(psx_r3000a::Instruction::nop());
}
