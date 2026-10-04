//! Applies validated sparse corrections to a 20x20 packed 4-bpp fill buffer.
use super::NameGlyphFillCorrections;
use anyhow::{Result, ensure};
use psx_r3000a::{Assembler, Instruction::*, Register, verify_placed_program};

/// A0: untagged Hangul syllable offset; A1: 200-byte fill-only cell;
/// A2/A3: validated correction stream start/end. Returns V0=1 on a matching
/// entry, else 0. Clobbers A2 and T0..T7; preserves the other arguments and RA.
/// The caller must generate the outline after this routine, not before it.
/// `origin` is supplied by the eventual storage owner; this claims no RAM.
pub fn build_name_fill_correction_program(origin: u32, crop: [usize; 4]) -> Result<Vec<u8>> {
    NameGlyphFillCorrections::parse(Vec::new(), crop)?;
    ensure!(
        origin.is_multiple_of(4),
        "correction program origin is unaligned"
    );
    let [x, y, width, _] = crop;
    let mut a = Assembler::new();
    a.emit(Addu {
        rd: Register::V0,
        rs: Register::ZERO,
        rt: Register::ZERO,
    })
    .label("scan")
    .emit(Sltu {
        rd: Register::T0,
        rs: Register::A2,
        rt: Register::A3,
    })
    .beq(Register::T0, Register::ZERO, "return")
    .emit(psx_r3000a::Instruction::nop())
    // Variable-length entries need byte loads, not unaligned halfwords.
    .emit(Lbu {
        rt: Register::T1,
        base: Register::A2,
        offset: 0,
    })
    .emit(Lbu {
        rt: Register::T2,
        base: Register::A2,
        offset: 1,
    })
    .emit(psx_r3000a::Instruction::nop())
    .emit(Sll {
        rd: Register::T2,
        rt: Register::T2,
        shift: 8,
    })
    .emit(Or {
        rd: Register::T1,
        rs: Register::T1,
        rt: Register::T2,
    })
    .emit(Sltu {
        rd: Register::T0,
        rs: Register::A0,
        rt: Register::T1,
    })
    .bne(Register::T0, Register::ZERO, "return")
    .emit(psx_r3000a::Instruction::nop())
    .emit(Lbu {
        rt: Register::T2,
        base: Register::A2,
        offset: 2,
    })
    .emit(Addiu {
        rt: Register::A2,
        rs: Register::A2,
        immediate: 3,
    })
    .emit(Addu {
        rd: Register::T3,
        rs: Register::A2,
        rt: Register::T2,
    })
    .beq(Register::A0, Register::T1, "match")
    .emit(psx_r3000a::Instruction::nop())
    .emit(Addu {
        rd: Register::A2,
        rs: Register::T3,
        rt: Register::ZERO,
    })
    .jump("scan")
    .emit(psx_r3000a::Instruction::nop())
    .label("match")
    .emit(Ori {
        rt: Register::V0,
        rs: Register::ZERO,
        immediate: 1,
    })
    .label("coordinate")
    .emit(Lbu {
        rt: Register::T0,
        base: Register::A2,
        offset: 0,
    })
    .emit(Addu {
        rd: Register::T1,
        rs: Register::ZERO,
        rt: Register::ZERO,
    })
    .label("row")
    .emit(Sltiu {
        rt: Register::T4,
        rs: Register::T0,
        immediate: width as i16,
    })
    .bne(Register::T4, Register::ZERO, "position")
    .emit(psx_r3000a::Instruction::nop())
    .emit(Addiu {
        rt: Register::T0,
        rs: Register::T0,
        immediate: -(width as i16),
    })
    .emit(Addiu {
        rt: Register::T1,
        rs: Register::T1,
        immediate: 1,
    })
    .jump("row")
    .emit(psx_r3000a::Instruction::nop())
    .label("position")
    .emit(Addiu {
        rt: Register::T0,
        rs: Register::T0,
        immediate: x as i16,
    })
    .emit(Addiu {
        rt: Register::T1,
        rs: Register::T1,
        immediate: y as i16,
    })
    // Ten bytes per row, two pixels per byte, low nibble first.
    .emit(Sll {
        rd: Register::T4,
        rt: Register::T1,
        shift: 3,
    })
    .emit(Sll {
        rd: Register::T5,
        rt: Register::T1,
        shift: 1,
    })
    .emit(Addu {
        rd: Register::T4,
        rs: Register::T4,
        rt: Register::T5,
    })
    .emit(Srl {
        rd: Register::T5,
        rt: Register::T0,
        shift: 1,
    })
    .emit(Addu {
        rd: Register::T4,
        rs: Register::T4,
        rt: Register::T5,
    })
    .emit(Addu {
        rd: Register::T4,
        rs: Register::T4,
        rt: Register::A1,
    })
    .emit(Andi {
        rt: Register::T5,
        rs: Register::T0,
        immediate: 1,
    })
    .emit(Sll {
        rd: Register::T5,
        rt: Register::T5,
        shift: 2,
    })
    .emit(Ori {
        rt: Register::T6,
        rs: Register::ZERO,
        immediate: 13,
    })
    .emit(Sllv {
        rd: Register::T6,
        rt: Register::T6,
        rs: Register::T5,
    })
    .emit(Lbu {
        rt: Register::T7,
        base: Register::T4,
        offset: 0,
    })
    .emit(psx_r3000a::Instruction::nop())
    .emit(Xor {
        rd: Register::T7,
        rs: Register::T7,
        rt: Register::T6,
    })
    .emit(Sb {
        rt: Register::T7,
        base: Register::T4,
        offset: 0,
    })
    .emit(Addiu {
        rt: Register::A2,
        rs: Register::A2,
        immediate: 1,
    })
    .bne(Register::A2, Register::T3, "coordinate")
    .emit(psx_r3000a::Instruction::nop())
    .label("return")
    .emit(Jr { rs: Register::RA })
    .emit(psx_r3000a::Instruction::nop());
    let placed = a.assemble(origin)?;
    ensure!(
        verify_placed_program(placed.bytes(), origin)? == placed.instructions(),
        "correction program readback differs"
    );
    crate::psx_machine_code_sources::verify_r3000a_load_delays(
        placed.instructions(),
        origin,
        "name fill corrections",
    )?;
    Ok(placed.bytes().to_vec())
}

#[cfg(test)]
#[path = "correction_runtime_tests.rs"]
mod tests;
