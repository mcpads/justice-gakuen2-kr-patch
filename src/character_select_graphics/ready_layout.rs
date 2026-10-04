//! Same ready-text ink and 20px advance, stored without two blank side columns.
use anyhow::{Result, ensure};
use psx_r3000a::{Instruction, Register, decode, encode};

pub(super) const CELL_WIDTH: usize = 16;
pub(super) const CELL_HEIGHT: usize = 20;
pub(super) const SCREEN_X: i16 = 198;

pub(super) fn rewrite_sprite_width(
    source_path: &str,
    source: &[u8],
    output: &mut [u8],
) -> Result<Vec<usize>> {
    let start = match source_path {
        "DAT1/PLSEL3.BIN" => 0x8720,
        "DAT1/PLSEL4.BIN" => 0x9bf4,
        "DAT1/PLSEL5.BIN" => 0xb3cc,
        _ => return Ok(Vec::new()),
    };
    let sh = |rt, offset| Instruction::Sh {
        rt,
        base: Register::S0,
        offset,
    };
    // Move the X store into the old width-store slot. Use the freed slot to
    // load the new width, then store it in the existing function-load delay
    // slot. Height, 20px advance, UV loads and call order stay unchanged.
    let patches = [
        (start, sh(Register::V0, 0x18), sh(Register::S4, 0x10)),
        (
            start + 8,
            sh(Register::S4, 0x10),
            Instruction::Addiu {
                rt: Register::T0,
                rs: Register::ZERO,
                immediate: CELL_WIDTH as i16,
            },
        ),
        (start + 0x24, decode(0, 0)?, sh(Register::T0, 0x18)),
    ];
    for (offset, expected, replacement) in &patches {
        let pc = 0x800a2000 + *offset as u32;
        let actual = decode(
            u32::from_le_bytes(source[*offset..*offset + 4].try_into()?),
            pc,
        )?;
        ensure!(
            actual == *expected,
            "ready sprite layout changed at {source_path}+{offset:x}"
        );
        output[*offset..*offset + 4].copy_from_slice(&encode(replacement, pc)?.to_le_bytes());
    }
    Ok(patches.iter().map(|p| p.0).collect())
}
