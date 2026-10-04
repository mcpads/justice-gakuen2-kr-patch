//! Shared tagged-glyph call used by roster-specific packet/position adapters.
use super::selector_loader::{overlaps, ram_range};
use anyhow::{Result, ensure};
use psx_r3000a::{Assembler, Instruction::*, Register as R, load_address, verify_placed_program};

pub struct RosterGlyphLayout {
    pub origin: u32,
    pub capacity: usize,
    /// First nickname word of the native 17-record, 40-byte-stride table.
    pub names: u32,
    pub hangul: u32,
    pub ascii: u32,
    /// Must admit all 68 record-character cells; residency belongs to installer.
    pub uploader: u32,
    pub sprite: u32,
}

/// A0=canonical word, A1=packet, A2=packed XY, A3=ordering-table entry.
/// Preserve S0..S8/SP/RA. Return sprite result, or zero without publishing a
/// packet for an unsupported/missing glyph or failed upload. The slot is the
/// first matching record-character position, so repeated names share pixels
/// while all distinct names stay resident throughout a frame. Records are read
/// only; no temporary recoding of persistent names is required.
pub fn build_roster_glyph(l: &RosterGlyphLayout) -> Result<Vec<u8>> {
    let code = ram_range(l.origin, l.capacity)?;
    let names = ram_range(l.names, 16 * 40 + 8)?;
    ensure!(
        l.origin.is_multiple_of(4) && l.names.is_multiple_of(2),
        "unaligned roster glyph placement"
    );
    ensure!(
        !overlaps(&code, &names),
        "roster glyph overlaps name records"
    );
    for target in [l.hangul, l.ascii, l.uploader, l.sprite] {
        let call = ram_range(target, 4)?;
        ensure!(
            target.is_multiple_of(4) && !overlaps(&code, &call) && !overlaps(&names, &call),
            "roster glyph overlaps callee or records"
        );
    }
    let mut a = Assembler::new();
    a.emit(Andi {
        rt: R::T0,
        rs: R::A0,
        immediate: 0xc000,
    })
    .emit(Ori {
        rt: R::T1,
        rs: R::ZERO,
        immediate: 0x8000,
    })
    .emit_all(load_address(R::T9, l.hangul))
    .beq(R::T0, R::T1, "find_roster_glyph")
    .emit(Andi {
        rt: R::T0,
        rs: R::A0,
        immediate: 0xff80,
    })
    .emit(Ori {
        rt: R::T1,
        rs: R::ZERO,
        immediate: 0x4000,
    })
    .bne(R::T0, R::T1, "roster_glyph_missing")
    .emit(psx_r3000a::Instruction::nop())
    .emit_all(load_address(R::T9, l.ascii))
    .label("find_roster_glyph")
    .emit_all(load_address(R::T0, l.names))
    .emit(Ori {
        rt: R::T1,
        rs: R::ZERO,
        immediate: 17,
    })
    .emit(Addu {
        rd: R::T3,
        rs: R::ZERO,
        rt: R::ZERO,
    })
    .label("roster_record")
    .emit(Ori {
        rt: R::T2,
        rs: R::ZERO,
        immediate: 4,
    })
    .label("roster_word")
    .emit(Lhu {
        rt: R::T4,
        base: R::T0,
        offset: 0,
    })
    .emit(Addiu {
        rt: R::T0,
        rs: R::T0,
        immediate: 2,
    })
    .beq(R::T4, R::A0, "render_roster_glyph")
    .emit(psx_r3000a::Instruction::nop())
    .emit(Addiu {
        rt: R::T3,
        rs: R::T3,
        immediate: 1,
    })
    .emit(Addiu {
        rt: R::T2,
        rs: R::T2,
        immediate: -1,
    })
    .bne(R::T2, R::ZERO, "roster_word")
    .emit(psx_r3000a::Instruction::nop())
    .emit(Addiu {
        rt: R::T1,
        rs: R::T1,
        immediate: -1,
    })
    .bne(R::T1, R::ZERO, "roster_record")
    .emit(Addiu {
        rt: R::T0,
        rs: R::T0,
        immediate: 32,
    })
    .label("roster_glyph_missing")
    .emit(Jr { rs: R::RA })
    .emit(Addu {
        rd: R::V0,
        rs: R::ZERO,
        rt: R::ZERO,
    })
    .label("render_roster_glyph")
    .emit(Addiu {
        rt: R::SP,
        rs: R::SP,
        immediate: -40,
    });
    for (r, offset) in [
        (R::A1, 16),
        (R::A2, 20),
        (R::A3, 24),
        (R::T3, 28),
        (R::RA, 36),
    ] {
        a.emit(Sw {
            rt: r,
            base: R::SP,
            offset,
        });
    }
    a.emit(Jalr {
        rd: R::RA,
        rs: R::T9,
    })
    .emit(Andi {
        rt: R::A0,
        rs: R::A0,
        immediate: 0x3fff,
    })
    .beq(R::V0, R::ZERO, "roster_glyph_return")
    .emit(psx_r3000a::Instruction::nop())
    .emit(Lw {
        rt: R::A0,
        base: R::SP,
        offset: 28,
    })
    .emit(Jal { target: l.uploader })
    .emit(psx_r3000a::Instruction::nop())
    .emit(Addiu {
        rt: R::T0,
        rs: R::V0,
        immediate: 1,
    })
    .beq(R::T0, R::ZERO, "roster_upload_failed")
    .emit(psx_r3000a::Instruction::nop());
    for (r, offset) in [(R::A0, 28), (R::A1, 16), (R::A2, 20), (R::A3, 24)] {
        a.emit(Lw {
            rt: r,
            base: R::SP,
            offset,
        });
    }
    a.emit(Jal { target: l.sprite })
        .emit(psx_r3000a::Instruction::nop())
        .beq(R::ZERO, R::ZERO, "roster_glyph_return")
        .emit(psx_r3000a::Instruction::nop())
        .label("roster_upload_failed")
        .emit(Addu {
            rd: R::V0,
            rs: R::ZERO,
            rt: R::ZERO,
        })
        .label("roster_glyph_return")
        .emit(Lw {
            rt: R::RA,
            base: R::SP,
            offset: 36,
        })
        .emit(Addiu {
            rt: R::SP,
            rs: R::SP,
            immediate: 40,
        })
        .emit(Jr { rs: R::RA })
        .emit(psx_r3000a::Instruction::nop());
    let bytes = a.assemble(l.origin)?.bytes().to_vec();
    ensure!(bytes.len() <= l.capacity, "roster glyph exceeds capacity");
    crate::psx_machine_code_sources::verify_r3000a_load_delays(
        &verify_placed_program(&bytes, l.origin)?,
        l.origin,
        "roster glyph",
    )?;
    Ok(bytes)
}

#[cfg(test)]
#[path = "roster_glyph_tests.rs"]
mod tests;
