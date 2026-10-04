//! Typed JP tile-ID to source-coordinate projection witnesses.

use anyhow::{Context, Result, ensure};
use psx_r3000a::{Instruction, Register, decode};

use super::super::super::projection_model::PracticalResultTileProjectionWitness;
use super::super::source_evidence::{checked_end, parse_hex};

#[derive(Debug, Clone, Copy)]
pub(super) struct ValidatedTileProjectionGeometry {
    pub(super) column_count: usize,
    pub(super) tile_width: usize,
    pub(super) tile_height: usize,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum TileProjectionKind {
    Static,
    Dynamic,
}

pub(super) fn validate_tile_projection_witness(
    overlay: &[u8],
    base: u32,
    function_offset_text: &str,
    function_size: usize,
    witness: &PracticalResultTileProjectionWitness,
    kind: TileProjectionKind,
    role: &str,
) -> Result<ValidatedTileProjectionGeometry> {
    let function_offset = parse_hex(function_offset_text, "tile renderer function offset")?;
    let function_end = checked_end(function_offset, function_size, "tile renderer function")?;
    let source_u_offset = parse_hex(
        &witness.source_u_projection_offset,
        "source U projection offset",
    )?;
    let source_v_offset = parse_hex(
        &witness.source_v_projection_offset,
        "source V projection offset",
    )?;
    let increment_offset = parse_hex(
        &witness.tile_id_increment_offset,
        "tile ID increment offset",
    )?;
    let (tile_register, stream_register) = match kind {
        TileProjectionKind::Static => (Register::S1, Register::S2),
        TileProjectionKind::Dynamic => (Register::S2, Register::S3),
    };
    let tile_load_offsets = witness
        .tile_id_load_offsets
        .iter()
        .map(|offset| parse_hex(offset, "tile ID load offset"))
        .collect::<Result<Vec<_>>>()?;
    ensure!(
        tile_load_offsets.len() == 2,
        "{role} tile ID load denominator changed"
    );
    let mut witness_instruction_offsets = tile_load_offsets.clone();
    for offset in &tile_load_offsets {
        ensure!(
            decode_instruction(overlay, base, *offset, role)?
                == (Instruction::Lbu {
                    rt: tile_register,
                    base: stream_register,
                    offset: 0,
                }),
            "{role} tile ID load changed at +0x{offset:04x}"
        );
    }

    let coordinate_copy_offset = source_u_offset
        .checked_sub(0x14)
        .context("source coordinate copy offset underflow")?;
    let u_shift_offset = source_u_offset
        .checked_add(4)
        .context("source U shift offset overflow")?;
    let nonnegative_branch_offset = source_u_offset
        .checked_add(0x0c)
        .context("source U branch offset overflow")?;
    let u_store_offset = source_u_offset
        .checked_add(0x10)
        .context("source U store offset overflow")?;
    let negative_adjust_offset = source_u_offset
        .checked_add(0x14)
        .context("source coordinate adjustment offset overflow")?;
    let branch_target = base
        .checked_add(u32::try_from(nonnegative_branch_offset)?)
        .and_then(|address| address.checked_add(0x0c))
        .context("source U branch target overflow")?;
    ensure!(
        decode_instruction(overlay, base, coordinate_copy_offset, role)?
            == (Instruction::Addu {
                rd: Register::V1,
                rs: tile_register,
                rt: Register::ZERO,
            })
            && decode_instruction(overlay, base, source_u_offset, role)?
                == (Instruction::Andi {
                    rt: Register::V0,
                    rs: tile_register,
                    immediate: 7,
                })
            && decode_instruction(overlay, base, u_shift_offset, role)?
                == (Instruction::Sll {
                    rd: Register::V0,
                    rt: Register::V0,
                    shift: 5,
                })
            && decode_instruction(overlay, base, nonnegative_branch_offset, role)?
                == (Instruction::Bgez {
                    rs: tile_register,
                    target: branch_target,
                })
            && decode_instruction(overlay, base, u_store_offset, role)?
                == (Instruction::Sb {
                    rt: Register::V0,
                    base: Register::S0,
                    offset: 0x14,
                })
            && decode_instruction(overlay, base, negative_adjust_offset, role)?
                == (Instruction::Addiu {
                    rt: Register::V1,
                    rs: tile_register,
                    immediate: 7,
                }),
        "{role} source U projection changed"
    );

    let v_scale_offset = source_v_offset
        .checked_add(4)
        .context("source V scale offset overflow")?;
    let v_store_offset = source_v_offset
        .checked_add(8)
        .context("source V store offset overflow")?;
    let width_literal_offset = source_v_offset
        .checked_add(0x0c)
        .context("tile width literal offset overflow")?;
    let width_store_offset = source_v_offset
        .checked_add(0x10)
        .context("tile width store offset overflow")?;
    let height_literal_offset = source_v_offset
        .checked_add(0x14)
        .context("tile height literal offset overflow")?;
    let height_store_offset = source_v_offset
        .checked_add(0x18)
        .context("tile height store offset overflow")?;
    ensure!(
        decode_instruction(overlay, base, source_v_offset, role)?
            == (Instruction::Sra {
                rd: Register::V0,
                rt: Register::V1,
                shift: 3,
            })
            && decode_instruction(overlay, base, v_scale_offset, role)?
                == (Instruction::Sll {
                    rd: Register::V0,
                    rt: Register::V0,
                    shift: 4,
                })
            && decode_instruction(overlay, base, v_store_offset, role)?
                == (Instruction::Sb {
                    rt: Register::V0,
                    base: Register::S0,
                    offset: 0x15,
                })
            && decode_instruction(overlay, base, width_literal_offset, role)?
                == (Instruction::Addiu {
                    rt: Register::V0,
                    rs: Register::ZERO,
                    immediate: 0x20,
                })
            && decode_instruction(overlay, base, width_store_offset, role)?
                == (Instruction::Sh {
                    rt: Register::V0,
                    base: Register::S0,
                    offset: 0x18,
                })
            && decode_instruction(overlay, base, height_literal_offset, role)?
                == (Instruction::Addiu {
                    rt: Register::V0,
                    rs: Register::ZERO,
                    immediate: 0x10,
                })
            && decode_instruction(overlay, base, height_store_offset, role)?
                == (Instruction::Sh {
                    rt: Register::V0,
                    base: Register::S0,
                    offset: 0x1a,
                }),
        "{role} source V projection or primitive size changed"
    );
    ensure!(
        decode_instruction(overlay, base, increment_offset, role)?
            == (Instruction::Addiu {
                rt: tile_register,
                rs: tile_register,
                immediate: 1,
            }),
        "{role} tile ID increment changed"
    );

    match (kind, &witness.dynamic_header_transfer_offset) {
        (TileProjectionKind::Static, None) => {}
        (TileProjectionKind::Dynamic, Some(header_offset)) => {
            let header_offset = parse_hex(header_offset, "dynamic header transfer offset")?;
            let header_increment_offset = header_offset
                .checked_add(4)
                .context("dynamic header increment offset overflow")?;
            let header_store_offset = header_offset
                .checked_add(8)
                .context("dynamic header store offset overflow")?;
            witness_instruction_offsets.extend([
                header_offset,
                header_increment_offset,
                header_store_offset,
            ]);
            ensure!(
                decode_instruction(overlay, base, header_offset, role)?
                    == (Instruction::Lbu {
                        rt: Register::V0,
                        base: Register::S3,
                        offset: 0,
                    })
                    && decode_instruction(overlay, base, header_increment_offset, role)?
                        == (Instruction::Addiu {
                            rt: Register::S3,
                            rs: Register::S3,
                            immediate: 1,
                        })
                    && decode_instruction(overlay, base, header_store_offset, role)?
                        == (Instruction::Sb {
                            rt: Register::V0,
                            base: Register::A1,
                            offset: 0x16,
                        }),
                "{role} dynamic header transfer changed"
            );
        }
        _ => anyhow::bail!("{role} dynamic-header witness kind changed"),
    }

    witness_instruction_offsets.extend([
        coordinate_copy_offset,
        source_u_offset,
        u_shift_offset,
        nonnegative_branch_offset,
        u_store_offset,
        negative_adjust_offset,
        source_v_offset,
        v_scale_offset,
        v_store_offset,
        width_literal_offset,
        width_store_offset,
        height_literal_offset,
        height_store_offset,
        increment_offset,
    ]);
    ensure!(
        witness_instruction_offsets
            .iter()
            .all(|offset| offset % 4 == 0
                && *offset >= function_offset
                && offset
                    .checked_add(4)
                    .is_some_and(|instruction_end| instruction_end <= function_end)),
        "{role} tile-projection witness escapes its exact function"
    );
    Ok(ValidatedTileProjectionGeometry {
        column_count: 8,
        tile_width: 32,
        tile_height: 16,
    })
}

fn decode_instruction(overlay: &[u8], base: u32, offset: usize, role: &str) -> Result<Instruction> {
    let bytes = overlay
        .get(offset..offset + 4)
        .with_context(|| format!("{role} instruction at +0x{offset:04x} is truncated"))?;
    let pc = base
        .checked_add(u32::try_from(offset)?)
        .context("tile renderer instruction address overflow")?;
    decode(u32::from_le_bytes(bytes.try_into()?), pc)
        .with_context(|| format!("failed to decode {role} instruction at +0x{offset:04x}"))
}
