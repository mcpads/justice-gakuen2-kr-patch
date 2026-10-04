use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};

use super::consumer::OVERLAY_RUNTIME_BASE;
use super::glyph_slots::reserved_glyph_codes;

const POINTER_TABLE_OFFSET: usize = 0x11e0;
const POINTER_TABLE_END: usize = 0x1418;
const DIRECT_SPRITE_TABLE_START: usize = POINTER_TABLE_END;
const OVERLAY_ENTRYPOINT_OFFSET: usize = 0x3afc;
const DECLARED_NEIGHBOR_CODES: [u16; 13] = [
    0x017a, 0x0271, 0x026a, 0x0320, 0x0321, 0x0322, 0x0323, 0x0324, 0x0341, 0x034a, 0x034d, 0x034e,
    0x0325,
];

pub(super) fn validate_allocated_glyph_ownership(source: &[u8]) -> Result<()> {
    let aliases = allocated_physical_alias_codes();
    let neighbors = DECLARED_NEIGHBOR_CODES.into_iter().collect::<BTreeSet<_>>();
    ensure!(
        aliases.is_disjoint(&neighbors),
        "KOUBAI1 confirmation allocation aliases another bonus overlay allocation"
    );

    let referenced = pointer_table_glyph_codes(source)?;
    ensure!(
        aliases.is_disjoint(&referenced),
        "KOUBAI1 confirmation allocation aliases a source command glyph"
    );

    let direct_table = source
        .get(DIRECT_SPRITE_TABLE_START..OVERLAY_ENTRYPOINT_OFFSET)
        .context("KOUBAI2 direct-sprite table region is truncated")?;
    for code in aliases {
        let selector = sprite_selector(code);
        ensure!(
            !direct_table
                .windows(selector.len())
                .any(|bytes| bytes == selector),
            "KOUBAI1 confirmation allocation aliases source direct-sprite glyph 0x{code:04x}"
        );
    }
    Ok(())
}

pub(super) fn allocated_physical_alias_codes() -> BTreeSet<u16> {
    let reserved = reserved_glyph_codes().collect::<Vec<_>>();
    (0x0300_u16..=0x03ff)
        .filter(|candidate| {
            reserved
                .iter()
                .any(|allocated| wrapped_cells_overlap(*candidate, *allocated))
        })
        .collect()
}

fn pointer_table_glyph_codes(source: &[u8]) -> Result<BTreeSet<u16>> {
    let mut codes = BTreeSet::new();
    for pointer_offset in (POINTER_TABLE_OFFSET..POINTER_TABLE_END).step_by(4) {
        let pointer = read_u32(source, pointer_offset)?;
        ensure!(
            (OVERLAY_RUNTIME_BASE..OVERLAY_RUNTIME_BASE + POINTER_TABLE_OFFSET as u32)
                .contains(&pointer),
            "KOUBAI2 command pointer left its source string region"
        );
        let mut cursor = usize::try_from(pointer - OVERLAY_RUNTIME_BASE)?;
        loop {
            let opcode = *source
                .get(cursor)
                .context("KOUBAI2 command sequence is unterminated")?;
            match opcode {
                0x81 => break,
                0x80 => cursor += 1,
                0x63 => {
                    ensure!(
                        source.get(cursor..cursor + 3) == Some(&[0x63; 3]),
                        "KOUBAI2 blank command changed"
                    );
                    cursor += 3;
                }
                page => {
                    let column = *source.get(cursor + 1).context("truncated glyph command")?;
                    let row = *source.get(cursor + 2).context("truncated glyph command")?;
                    ensure!(
                        page <= 3 && column <= 15 && row <= 15,
                        "invalid KOUBAI2 glyph command"
                    );
                    codes.insert(u16::from(page) << 8 | u16::from(row) << 4 | u16::from(column));
                    cursor += 3;
                }
            }
        }
    }
    Ok(codes)
}

fn wrapped_cells_overlap(left: u16, right: u16) -> bool {
    left >> 8 == right >> 8
        && wrapped_axis_overlap(usize::from(left & 0x0f), usize::from(right & 0x0f))
        && wrapped_axis_overlap(
            usize::from((left >> 4) & 0x0f),
            usize::from((right >> 4) & 0x0f),
        )
}

fn wrapped_axis_overlap(left: usize, right: usize) -> bool {
    let mut occupied = [false; 256];
    for local in 0..20 {
        occupied[(left * 20 + local) & 0xff] = true;
    }
    (0..20).any(|local| occupied[(right * 20 + local) & 0xff])
}

fn sprite_selector(code: u16) -> [u8; 3] {
    [
        8 + (code >> 8) as u8,
        (code & 0x000f) as u8,
        ((code >> 4) & 0x000f) as u8,
    ]
}

fn read_u32(source: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        source
            .get(offset..offset + 4)
            .with_context(|| format!("truncated KOUBAI2 word at +0x{offset:04x}"))?
            .try_into()?,
    ))
}
