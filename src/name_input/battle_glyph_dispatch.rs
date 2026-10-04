//! Decode battle record words before any glyph supplier is allowed to index data.
use super::selector_loader::{overlaps, ram_range};
use anyhow::{Result, ensure};
use psx_r3000a::{Assembler, Instruction::*, Register as R, load_address, verify_placed_program};

/// A0: stored word; A1: native record index (0..16). Return V0 scratch or zero;
/// V1 is 0 for shared glyph palette and 1 for native palette. Callee-saved
/// registers are preserved. Only the temporary Diary record 16 can resolve a
/// nickname cache alias through the canonical profile, preventing another saved
/// character from accidentally borrowing the current Diary player's name.
pub fn build_battle_glyph_dispatch(
    origin: u32,
    capacity: usize,
    hangul: u32,
    ascii: u32,
    legacy: u32,
) -> Result<Vec<u8>> {
    let code = ram_range(origin, capacity)?;
    ensure!(origin.is_multiple_of(4), "unaligned battle glyph dispatch");
    for target in [hangul, ascii, legacy] {
        ensure!(
            target.is_multiple_of(4) && !overlaps(&code, &ram_range(target, 4)?),
            "battle glyph dispatch overlaps its supplier"
        );
    }
    let mut a = Assembler::new();
    macro_rules! nop {
        () => {
            a.emit(psx_r3000a::Instruction::nop());
        };
    }
    a.emit(Sltiu {
        rt: R::T0,
        rs: R::A1,
        immediate: 17,
    })
    .beq(R::T0, R::ZERO, "reject");
    nop!();
    // Only four nickname aliases, never arbitrary low-domain glyph indexes.
    a.emit(Addiu {
        rt: R::T0,
        rs: R::A0,
        immediate: -0x339,
    })
    .emit(Sltiu {
        rt: R::T1,
        rs: R::T0,
        immediate: 4,
    })
    .beq(R::T1, R::ZERO, "decode")
    .emit(Addiu {
        rt: R::T1,
        rs: R::ZERO,
        immediate: 16,
    })
    .bne(R::A1, R::T1, "reject");
    nop!();
    a.emit_all(load_address(R::T1, 0x801f1896))
        .emit(Sll {
            rd: R::T0,
            rt: R::T0,
            shift: 1,
        })
        .emit(Addu {
            rd: R::T1,
            rs: R::T1,
            rt: R::T0,
        })
        .emit(Lhu {
            rt: R::A0,
            base: R::T1,
            offset: 0,
        });
    nop!();
    a.label("decode")
        .emit(Andi {
            rt: R::T0,
            rs: R::A0,
            immediate: 0xc000,
        })
        .emit(Ori {
            rt: R::T1,
            rs: R::ZERO,
            immediate: 0x8000,
        })
        .beq(R::T0, R::T1, "hangul");
    nop!();
    a.emit(Andi {
        rt: R::T0,
        rs: R::A0,
        immediate: 0xff80,
    })
    .emit(Ori {
        rt: R::T1,
        rs: R::ZERO,
        immediate: 0x4000,
    })
    .beq(R::T0, R::T1, "ascii");
    nop!();
    a.emit(Sltiu {
        rt: R::T0,
        rs: R::A0,
        immediate: 176,
    })
    .beq(R::T0, R::ZERO, "reject")
    .emit(Addiu {
        rt: R::T0,
        rs: R::ZERO,
        immediate: 0xa8,
    })
    .bne(R::A0, R::T0, "legacy");
    nop!();
    a.emit(Addiu {
        rt: R::A0,
        rs: R::ZERO,
        immediate: 0x54,
    });
    a.label("legacy")
        .emit_all(load_address(R::T9, legacy))
        .emit(Addiu {
            rt: R::T0,
            rs: R::ZERO,
            immediate: 1,
        })
        .beq(R::ZERO, R::ZERO, "call");
    nop!();
    a.label("hangul")
        .emit_all(load_address(R::T9, hangul))
        .emit(Andi {
            rt: R::A0,
            rs: R::A0,
            immediate: 0x3fff,
        })
        .beq(R::ZERO, R::ZERO, "shared");
    nop!();
    a.label("ascii")
        .emit_all(load_address(R::T9, ascii))
        .emit(Andi {
            rt: R::A0,
            rs: R::A0,
            immediate: 0x7f,
        });
    a.label("shared").emit(Addu {
        rd: R::T0,
        rs: R::ZERO,
        rt: R::ZERO,
    });
    a.label("call")
        .emit(Addiu {
            rt: R::SP,
            rs: R::SP,
            immediate: -24,
        })
        .emit(Sw {
            rt: R::RA,
            base: R::SP,
            offset: 20,
        })
        .emit(Sw {
            rt: R::T0,
            base: R::SP,
            offset: 16,
        })
        .emit(Jalr {
            rd: R::RA,
            rs: R::T9,
        });
    nop!();
    a.emit(Lw {
        rt: R::V1,
        base: R::SP,
        offset: 16,
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
    .emit(Jr { rs: R::RA });
    nop!();
    a.label("reject")
        .emit(Addu {
            rd: R::V0,
            rs: R::ZERO,
            rt: R::ZERO,
        })
        .emit(Jr { rs: R::RA })
        .emit(Addu {
            rd: R::V1,
            rs: R::ZERO,
            rt: R::ZERO,
        });
    let bytes = a.assemble(origin)?.bytes().to_vec();
    ensure!(
        bytes.len() <= capacity,
        "battle glyph dispatch exceeds capacity"
    );
    crate::psx_machine_code_sources::verify_r3000a_load_delays(
        &verify_placed_program(&bytes, origin)?,
        origin,
        "battle glyph dispatch",
    )?;
    Ok(bytes)
}

#[cfg(test)]
#[path = "battle_glyph_dispatch_tests.rs"]
mod tests;
