use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};

use crate::bonus_confirmation::bonus_confirmation_reserved_glyph_codes;
pub(super) use crate::tim::cells_overlap;

use super::consumer::{OVERLAY_RUNTIME_BASE, SOURCE_GLYPH_CODES};
use super::source::{STOCK_LABEL_GLYPHS, glyph_cell};

const POINTER_TABLE_OFFSET: usize = 0x11e0;
const POINTER_TABLE_END: usize = 0x1418;
const DIRECT_SPRITE_TABLE_START: usize = POINTER_TABLE_END;
const OVERLAY_ENTRYPOINT_OFFSET: usize = 0x3afc;
const EXPECTED_PHYSICAL_ALIAS_CODES: [u16; 4] = [0x0341, 0x034a, 0x034d, 0x034e];
const ACTION_LABEL_CODES: [u16; 5] = [0x0320, 0x0321, 0x0322, 0x0323, 0x0324];
const PAGE_INDICATOR_CODE: u16 = 0x017a;

pub(super) fn validate_allocated_glyph_ownership(source: &[u8]) -> Result<()> {
    let aliases = allocated_physical_alias_codes();
    ensure!(
        aliases == EXPECTED_PHYSICAL_ALIAS_CODES.into_iter().collect(),
        "KOUBAI1 stock-label physical alias set changed"
    );
    ensure!(
        aliases.is_disjoint(&SOURCE_GLYPH_CODES.into_iter().collect()),
        "KOUBAI1 stock-label allocation aliases the original computed glyphs"
    );

    let referenced = pointer_table_glyph_codes(source)?;
    ensure!(
        aliases.is_disjoint(&referenced),
        "KOUBAI1 stock-label allocation aliases a source command glyph"
    );
    let direct_table = source
        .get(DIRECT_SPRITE_TABLE_START..OVERLAY_ENTRYPOINT_OFFSET)
        .context("KOUBAI2 direct-sprite table region is truncated")?;
    for code in &aliases {
        let selector = sprite_selector(*code);
        ensure!(
            !direct_table
                .windows(selector.len())
                .any(|bytes| bytes == selector),
            "KOUBAI1 stock-label allocation aliases source direct-sprite glyph 0x{code:04x}"
        );
    }

    let existing_cells = ACTION_LABEL_CODES
        .into_iter()
        .chain([PAGE_INDICATOR_CODE])
        .chain(bonus_confirmation_reserved_glyph_codes())
        .map(glyph_cell)
        .collect::<Vec<_>>();
    for (_, code, stock_cell) in STOCK_LABEL_GLYPHS {
        ensure!(
            existing_cells
                .iter()
                .all(|existing| !cells_overlap(stock_cell, *existing)),
            "KOUBAI1 stock-label cell 0x{code:04x} overlaps another bonus component"
        );
    }
    Ok(())
}

pub(super) fn allocated_physical_alias_codes() -> BTreeSet<u16> {
    let stock_cells = STOCK_LABEL_GLYPHS.map(|(_, _, cell)| cell);
    (0x0300_u16..=0x03ff)
        .filter(|code| {
            let candidate = glyph_cell(*code);
            stock_cells
                .iter()
                .any(|stock| cells_overlap(candidate, *stock))
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
