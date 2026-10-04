use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};

use crate::bonus_confirmation::bonus_confirmation_reserved_glyph_codes;
use crate::bonus_inventory_action_labels::bonus_action_label_reserved_glyph_codes;
use crate::bonus_inventory_card_acquisition::bonus_card_acquisition_reserved_glyph_codes;
use crate::bonus_inventory_memory_card_swap::bonus_memory_card_swap_reserved_glyph_codes;
use crate::bonus_inventory_stock_label::bonus_stock_label_reserved_glyph_codes;
use crate::bonus_j_bank_return_label::bonus_j_bank_return_label_reserved_glyph_codes;
use crate::bonus_page_indicator::bonus_page_indicator_reserved_glyph_codes;
use crate::tim::cells_overlap;

use super::consumer::{
    DIRECT_SELECTOR_REGION, OVERLAY_RUNTIME_BASE, POINTER_TABLE_END, POINTER_TABLE_OFFSET,
};
use super::glyph_slots::glyph_cell;

const EXPECTED_PHYSICAL_ALIAS_CODES: [u16; 3] = [0x03b3, 0x03b5, 0x03bf];

#[derive(Debug)]
pub(super) struct GlyphOwnershipValidation {
    pub(super) declared_physical_alias_set_matches: bool,
    pub(super) existing_bonus_component_allocations_disjoint: bool,
    pub(super) pointer_command_table_range: [usize; 2],
    pub(super) pointer_command_table_parsed_glyphs_disjoint: bool,
    pub(super) declared_direct_selector_byte_region: [usize; 2],
    pub(super) declared_direct_selector_byte_region_scan_disjoint: bool,
}

pub(super) fn validate_allocated_glyph_ownership(
    source: &[u8],
) -> Result<GlyphOwnershipValidation> {
    let aliases = allocated_physical_alias_codes();
    let declared_physical_alias_set_matches =
        aliases == EXPECTED_PHYSICAL_ALIAS_CODES.into_iter().collect();
    ensure!(
        declared_physical_alias_set_matches,
        "KOUBAI1 J-BANK return-label physical alias set changed"
    );

    let existing = bonus_action_label_reserved_glyph_codes()
        .chain(bonus_page_indicator_reserved_glyph_codes())
        .chain(bonus_confirmation_reserved_glyph_codes())
        .chain(bonus_stock_label_reserved_glyph_codes())
        .chain(bonus_card_acquisition_reserved_glyph_codes())
        .chain(bonus_memory_card_swap_reserved_glyph_codes())
        .collect::<BTreeSet<_>>();
    let existing_aliases = (0x0300_u16..=0x03ff)
        .filter(|candidate| {
            existing
                .iter()
                .any(|allocated| cells_overlap(glyph_cell(*candidate), glyph_cell(*allocated)))
        })
        .collect::<BTreeSet<_>>();
    let existing_bonus_component_allocations_disjoint = aliases.is_disjoint(&existing_aliases);
    ensure!(
        existing_bonus_component_allocations_disjoint,
        "KOUBAI1 J-BANK return-label allocation aliases another bonus component"
    );

    let referenced = pointer_table_glyph_codes(source)?;
    let pointer_command_table_parsed_glyphs_disjoint = aliases.is_disjoint(&referenced);
    ensure!(
        pointer_command_table_parsed_glyphs_disjoint,
        "KOUBAI1 J-BANK return-label allocation aliases a source command glyph"
    );

    let direct_selector_bytes = source
        .get(DIRECT_SELECTOR_REGION[0]..DIRECT_SELECTOR_REGION[1])
        .context("KOUBAI2 declared direct-selector byte region is truncated")?;
    let declared_direct_selector_byte_region_scan_disjoint = aliases.iter().all(|code| {
        let selector = sprite_selector(*code);
        !direct_selector_bytes
            .windows(selector.len())
            .any(|bytes| bytes == selector)
    });
    ensure!(
        declared_direct_selector_byte_region_scan_disjoint,
        "KOUBAI1 J-BANK return-label selector occurs in the declared direct-selector byte region"
    );

    Ok(GlyphOwnershipValidation {
        declared_physical_alias_set_matches,
        existing_bonus_component_allocations_disjoint,
        pointer_command_table_range: [POINTER_TABLE_OFFSET, POINTER_TABLE_END],
        pointer_command_table_parsed_glyphs_disjoint,
        declared_direct_selector_byte_region: DIRECT_SELECTOR_REGION,
        declared_direct_selector_byte_region_scan_disjoint,
    })
}

pub(super) fn allocated_physical_alias_codes() -> BTreeSet<u16> {
    let reserved = bonus_j_bank_return_label_reserved_glyph_codes().collect::<Vec<_>>();
    (0x0300_u16..=0x03ff)
        .filter(|candidate| {
            reserved
                .iter()
                .any(|allocated| cells_overlap(glyph_cell(*candidate), glyph_cell(*allocated)))
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
