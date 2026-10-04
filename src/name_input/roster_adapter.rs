//! Adapt measured roster-loop locals to the shared tagged-glyph renderer.
use super::selector_loader::{overlaps, ram_range};
use anyhow::{Result, ensure};
use psx_r3000a::{Assembler, Instruction::*, Register as R, load_address, verify_placed_program};

// The narrowest roster columns are 56 pixels apart. Keep four character
// advances within that pitch; the shared sprite retains its complete outline.
pub(super) const ROSTER_ADVANCE: i16 = 14;
pub(super) fn roster_text_width(length: R, output: R) -> [psx_r3000a::Instruction; 3] {
    [
        Sll {
            rd: output,
            rt: length,
            shift: 3,
        },
        Subu {
            rd: output,
            rs: output,
            rt: length,
        },
        Sll {
            rd: output,
            rt: output,
            shift: 1,
        },
    ]
}

#[derive(Clone, Copy)]
pub enum RosterValue {
    Register(R),
    StackWord(i16),
    StackHalf(i16),
    Constant(i16),
}

pub struct RosterAdapterLayout {
    pub origin: u32,
    pub capacity: usize,
    pub renderer: u32,
    pub legacy: u32,
    pub next: u32,
    pub packet_base: i16,
    pub packet_extra: Option<i16>,
    /// League standings add (first stack word - second stack word) * 32.
    pub packet_row: Option<[i16; 2]>,
    pub length: R,
    pub x: RosterValue,
    pub y: RosterValue,
    pub y_bias: i16,
    pub ot_offset: i16,
}

fn value(a: &mut Assembler, to: R, from: RosterValue) {
    a.emit(match from {
        RosterValue::Register(rs) => Addu {
            rd: to,
            rs,
            rt: R::ZERO,
        },
        RosterValue::StackWord(offset) => Lw {
            rt: to,
            base: R::SP,
            offset,
        },
        RosterValue::StackHalf(offset) => Lhu {
            rt: to,
            base: R::SP,
            offset,
        },
        RosterValue::Constant(immediate) => Addiu {
            rt: to,
            rs: R::ZERO,
            immediate,
        },
    });
    // Keep the same schedule for register/constant and delayed-load sources.
    a.emit(psx_r3000a::Instruction::nop());
}

/// Entry at the original S4 < 256 guard, before its A1=0 delay slot.
/// S5 is the native double-buffered packet stride; S6 is the character advance.
/// The installer must change native centering/advance to 14 for mixed names.
/// This adapter computes context without changing source loop saved registers.
pub fn build_roster_adapter(l: &RosterAdapterLayout) -> Result<Vec<u8>> {
    let code = ram_range(l.origin, l.capacity)?;
    ensure!(l.origin.is_multiple_of(4), "unaligned roster adapter");
    for target in [l.renderer, l.legacy, l.next] {
        ensure!(
            target.is_multiple_of(4) && !overlaps(&code, &ram_range(target, 4)?),
            "roster adapter overlaps continuation"
        );
    }
    ensure!(
        matches!(l.length, R::S2 | R::S3),
        "unbound roster length register"
    );
    for v in [l.x, l.y] {
        if let RosterValue::Register(r) = v {
            ensure!(
                matches!(
                    r,
                    R::S0 | R::S1 | R::S2 | R::S3 | R::S4 | R::S5 | R::S6 | R::S7 | R::FP
                ),
                "roster coordinate must use a saved source register"
            );
        }
    }
    let mut a = Assembler::new();
    a.emit(Sltiu {
        rt: R::V0,
        rs: R::S4,
        immediate: 256,
    })
    .beq(R::V0, R::ZERO, "roster_tagged_context")
    .emit(Addu {
        rd: R::A1,
        rs: R::ZERO,
        rt: R::ZERO,
    })
    .emit(J { target: l.legacy })
    .emit(psx_r3000a::Instruction::nop())
    .label("roster_tagged_context");
    value(&mut a, R::A1, RosterValue::StackWord(l.packet_base));
    if let Some(offset) = l.packet_extra {
        value(&mut a, R::T0, RosterValue::StackWord(offset));
        a.emit(Addu {
            rd: R::A1,
            rs: R::A1,
            rt: R::T0,
        });
    }
    if let Some([first, second]) = l.packet_row {
        value(&mut a, R::T0, RosterValue::StackWord(first));
        value(&mut a, R::T1, RosterValue::StackWord(second));
        a.emit(Subu {
            rd: R::T0,
            rs: R::T0,
            rt: R::T1,
        })
        .emit(Sll {
            rd: R::T0,
            rt: R::T0,
            shift: 5,
        })
        .emit(Addu {
            rd: R::A1,
            rs: R::A1,
            rt: R::T0,
        });
    }
    a.emit_all(load_address(R::T0, 0x801f608c))
        .emit(Lw {
            rt: R::T0,
            base: R::T0,
            offset: 0,
        })
        .emit(Addu {
            rd: R::A1,
            rs: R::A1,
            rt: R::S5,
        })
        .emit(Sll {
            rd: R::T1,
            rt: R::T0,
            shift: 3,
        })
        .emit(Subu {
            rd: R::T1,
            rs: R::T1,
            rt: R::T0,
        })
        .emit(Sll {
            rd: R::T1,
            rt: R::T1,
            shift: 2,
        })
        .emit(Addu {
            rd: R::A1,
            rs: R::A1,
            rt: R::T1,
        });
    value(&mut a, R::T0, l.x);
    a.emit_all(roster_text_width(l.length, R::T1))
        .emit(Addiu {
            rt: R::T2,
            rs: R::ZERO,
            immediate: 48,
        })
        .emit(Subu {
            rd: R::T1,
            rs: R::T2,
            rt: R::T1,
        })
        .emit(Sra {
            rd: R::T1,
            rt: R::T1,
            shift: 1,
        })
        .emit(Addu {
            rd: R::T0,
            rs: R::T0,
            rt: R::T1,
        })
        .emit(Addu {
            rd: R::T0,
            rs: R::T0,
            rt: R::S6,
        })
        .emit(Andi {
            rt: R::A2,
            rs: R::T0,
            immediate: 0xffff,
        });
    value(&mut a, R::T0, l.y);
    a.emit(Addiu {
        rt: R::T0,
        rs: R::T0,
        immediate: l.y_bias,
    })
    .emit(Sll {
        rd: R::T0,
        rt: R::T0,
        shift: 16,
    })
    .emit(Or {
        rd: R::A2,
        rs: R::A2,
        rt: R::T0,
    })
    .emit_all(load_address(R::A3, 0x801f6090))
    .emit(Lw {
        rt: R::A3,
        base: R::A3,
        offset: 0,
    })
    .emit(Addiu {
        rt: R::SP,
        rs: R::SP,
        immediate: -24,
    })
    .emit(Addiu {
        rt: R::A3,
        rs: R::A3,
        immediate: l.ot_offset,
    })
    .emit(Sw {
        rt: R::RA,
        base: R::SP,
        offset: 20,
    })
    .emit(Jal { target: l.renderer })
    .emit(Addu {
        rd: R::A0,
        rs: R::S4,
        rt: R::ZERO,
    })
    .emit(Lw {
        rt: R::RA,
        base: R::SP,
        offset: 20,
    })
    .emit(Addiu {
        rt: R::SP,
        rs: R::SP,
        immediate: 24,
    })
    .emit(J { target: l.next })
    .emit(psx_r3000a::Instruction::nop());
    let bytes = a.assemble(l.origin)?.bytes().to_vec();
    ensure!(bytes.len() <= l.capacity, "roster adapter exceeds capacity");
    crate::psx_machine_code_sources::verify_r3000a_load_delays(
        &verify_placed_program(&bytes, l.origin)?,
        l.origin,
        "roster adapter",
    )?;
    Ok(bytes)
}

#[cfg(test)]
#[path = "roster_adapter_tests.rs"]
mod tests;
