use psx_r3000a::{Assembler, Instruction, Register};

pub(super) const SELECTED_GLYPH_VALIDATOR_LABEL: &str = "validate_selected_name_glyph";

pub(super) fn emit_selected_glyph_validator(
    assembler: &mut Assembler,
    component_resolver_address: u32,
) {
    assembler
        .label(SELECTED_GLYPH_VALIDATOR_LABEL)
        .emit(Instruction::Addiu {
            rt: Register::SP,
            rs: Register::SP,
            immediate: -32,
        })
        .emit(Instruction::Sw {
            rt: Register::RA,
            base: Register::SP,
            offset: 28,
        })
        .emit(Instruction::Sw {
            rt: Register::T3,
            base: Register::SP,
            offset: 16,
        })
        .emit(Instruction::Sw {
            rt: Register::T4,
            base: Register::SP,
            offset: 12,
        })
        .emit(Instruction::Sw {
            rt: Register::T1,
            base: Register::SP,
            offset: 8,
        })
        .emit(Instruction::Sw {
            rt: Register::T5,
            base: Register::SP,
            offset: 20,
        })
        .emit(Instruction::Jal {
            target: component_resolver_address,
        })
        .emit(Instruction::nop())
        .emit(Instruction::Lw {
            rt: Register::T0,
            base: Register::SP,
            offset: 28,
        })
        .emit(Instruction::Lw {
            rt: Register::T3,
            base: Register::SP,
            offset: 16,
        })
        .emit(Instruction::Lw {
            rt: Register::T4,
            base: Register::SP,
            offset: 12,
        })
        .emit(Instruction::Lw {
            rt: Register::T1,
            base: Register::SP,
            offset: 8,
        })
        .emit(Instruction::Lw {
            rt: Register::T5,
            base: Register::SP,
            offset: 20,
        })
        .emit(Instruction::Addiu {
            rt: Register::SP,
            rs: Register::SP,
            immediate: 32,
        })
        .emit(Instruction::Jr { rs: Register::T0 })
        .emit(Instruction::Addu {
            rd: Register::RA,
            rs: Register::T9,
            rt: Register::ZERO,
        });
}

// The installed stored-atlas reader only clobbers V0 and T0..T3. Save the
// pending glyph/cursor and local return address in argument registers instead
// of allocating a stack frame. T4/T5/T8/T9 keep the edit transaction unchanged.
pub(super) fn emit_selected_band_glyph_validator(a: &mut Assembler, reader: u32, membership: u16) {
    use Instruction::*;
    use Register as R;
    a.label(SELECTED_GLYPH_VALIDATOR_LABEL);
    for (rd, rs) in [
        (R::A1, R::T3),
        (R::A2, R::T1),
        (R::A3, R::RA),
        (R::T6, R::A0),
    ] {
        a.emit(Addu {
            rd,
            rs,
            rt: R::ZERO,
        });
    }
    a.emit(Sltiu {
        rt: R::T0,
        rs: R::A0,
        immediate: 11172,
    })
    .beq(R::T0, R::ZERO, "selected_band_validation_return")
    .emit(Ori {
        rt: R::V0,
        rs: R::ZERO,
        immediate: 65535,
    })
    .emit(Srl {
        rd: R::A0,
        rt: R::A0,
        shift: 3,
    })
    .emit(Jal { target: reader })
    .emit(Addiu {
        rt: R::A0,
        rs: R::A0,
        immediate: membership as i16,
    })
    .emit(Andi {
        rt: R::T0,
        rs: R::T6,
        immediate: 7,
    })
    .emit(Srlv {
        rd: R::V0,
        rt: R::V0,
        rs: R::T0,
    })
    .emit(Andi {
        rt: R::V0,
        rs: R::V0,
        immediate: 1,
    })
    .emit(Addiu {
        rt: R::V0,
        rs: R::V0,
        immediate: -1,
    })
    .emit(Andi {
        rt: R::V0,
        rs: R::V0,
        immediate: 65535,
    })
    .label("selected_band_validation_return")
    .emit(Addu {
        rd: R::T3,
        rs: R::A1,
        rt: R::ZERO,
    })
    .emit(Addu {
        rd: R::T1,
        rs: R::A2,
        rt: R::ZERO,
    })
    .emit(Jr { rs: R::A3 })
    .emit(Addu {
        rd: R::RA,
        rs: R::T9,
        rt: R::ZERO,
    });
}
