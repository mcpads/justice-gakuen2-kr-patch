use anyhow::{Context, Result, ensure};
use psx_r3000a::{Instruction, Register, decode};

use crate::tim::{Tim4bpp, parse_4bpp_prefix};

pub const DIARY_HEADER_PATH: &str = "DAT2/MGCOCK.TIZ";
pub(super) const SOURCE_STORED_SHA256: &str =
    "a3af55cefe56dde6e068ca2c079fe4c97df00dadc59b35138ce2d20a9e4f3570";
pub(super) const SOURCE_DECODED_SHA256: &str =
    "326d1c8205c62f5f78953a846a04a9888699b10d1509d7911e843e288e5807ee";

pub(super) fn validate_diary_header_loader(mgame: &[u8]) -> Result<()> {
    use Instruction::*;
    use Register as R;

    // MGAME +0xef3c loads file-table record 0x2e6 into 0x80137000 through
    // the paged file-loader table, then submits that same buffer as a TIM.
    let instructions = [
        Addiu {
            rt: R::SP,
            rs: R::SP,
            immediate: -24,
        },
        Lui {
            rt: R::A0,
            immediate: 0x8013,
        },
        Lui {
            rt: R::V0,
            immediate: 0x801f,
        },
        Lw {
            rt: R::V0,
            base: R::V0,
            offset: 0x6374,
        },
        Ori {
            rt: R::A0,
            rs: R::A0,
            immediate: 0x7000,
        },
        Sw {
            rt: R::RA,
            base: R::SP,
            offset: 16,
        },
        Lw {
            rt: R::V0,
            base: R::V0,
            offset: 0x14c,
        },
        Instruction::nop(),
        Jalr {
            rd: R::RA,
            rs: R::V0,
        },
        Addiu {
            rt: R::A1,
            rs: R::ZERO,
            immediate: 0x2e6,
        },
        Lui {
            rt: R::V0,
            immediate: 0x801f,
        },
        Lw {
            rt: R::V0,
            base: R::V0,
            offset: 0x6360,
        },
        Lui {
            rt: R::A0,
            immediate: 0x8013,
        },
        Lw {
            rt: R::V0,
            base: R::V0,
            offset: 0x150,
        },
        Instruction::nop(),
        Jalr {
            rd: R::RA,
            rs: R::V0,
        },
        Ori {
            rt: R::A0,
            rs: R::A0,
            immediate: 0x7000,
        },
        Lw {
            rt: R::RA,
            base: R::SP,
            offset: 16,
        },
        Addiu {
            rt: R::SP,
            rs: R::SP,
            immediate: 24,
        },
        Jr { rs: R::RA },
        Instruction::nop(),
    ];
    for (index, expected) in instructions.into_iter().enumerate() {
        let offset = 0xef3c + index * 4;
        let bytes = mgame
            .get(offset..offset + 4)
            .context("truncated MGCOCK loader")?;
        ensure!(
            decode(
                u32::from_le_bytes(bytes.try_into()?),
                0x800a_2000 + offset as u32
            )? == expected,
            "MGCOCK paged-loader binding changed at MGAME +{offset:#x}"
        );
    }
    Ok(())
}

pub(super) fn parse_diary_header_tim(decoded: &[u8]) -> Result<Tim4bpp> {
    let tim = parse_4bpp_prefix(decoded)?;
    ensure!(
        tim.total_size == decoded.len(),
        "MGCOCK TIM has trailing bytes"
    );
    ensure!(
        tim.clut_x == 0 && tim.clut_y == 480 && tim.clut_width == 512 && tim.clut_height == 3,
        "MGCOCK CLUT geometry changed"
    );
    ensure!(
        tim.image_x == 512
            && tim.image_y == 256
            && tim.pixel_width() == 1024
            && tim.image_height == 256,
        "MGCOCK image geometry changed"
    );
    Ok(tim)
}
