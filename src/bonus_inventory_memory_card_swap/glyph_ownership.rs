use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};

use crate::bonus_confirmation::bonus_confirmation_reserved_glyph_codes;
use crate::bonus_inventory_action_labels::bonus_action_label_reserved_glyph_codes;
use crate::bonus_inventory_card_acquisition::bonus_card_acquisition_reserved_glyph_codes;
use crate::bonus_inventory_memory_card_swap::bonus_memory_card_swap_reserved_glyph_codes;
use crate::bonus_inventory_stock_label::bonus_stock_label_reserved_glyph_codes;
use crate::bonus_page_indicator::bonus_page_indicator_reserved_glyph_codes;
use crate::tim::cells_overlap;

use super::consumer::{OVERLAY_RUNTIME_BASE, POINTER_TABLE_END, POINTER_TABLE_OFFSET};
use super::glyph_slots::glyph_cell;

const DECLARED_DIRECT_SELECTOR_BYTE_REGION_START: usize = POINTER_TABLE_END;
const DECLARED_DIRECT_SELECTOR_BYTE_REGION_END: usize = 0x3afc;
const EXPECTED_PHYSICAL_ALIAS_CODES: [u16; 26] = [
    0x0380, 0x0381, 0x0382, 0x0383, 0x0384, 0x0385, 0x0386, 0x0387, 0x0388, 0x0389, 0x038a, 0x038b,
    0x038d, 0x038e, 0x038f, 0x0390, 0x0391, 0x0392, 0x0393, 0x0394, 0x0395, 0x0396, 0x0397, 0x039d,
    0x039e, 0x039f,
];

#[derive(Debug, Clone)]
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
        "KOUBAI1 memory-card-swap physical alias set changed"
    );

    let existing = bonus_action_label_reserved_glyph_codes()
        .chain(bonus_page_indicator_reserved_glyph_codes())
        .chain(bonus_confirmation_reserved_glyph_codes())
        .chain(bonus_stock_label_reserved_glyph_codes())
        .chain(bonus_card_acquisition_reserved_glyph_codes())
        .collect::<BTreeSet<_>>();
    let existing_bonus_component_allocations_disjoint = aliases.is_disjoint(&existing);
    ensure!(
        existing_bonus_component_allocations_disjoint,
        "KOUBAI1 memory-card-swap allocation aliases another bonus component"
    );

    let referenced = pointer_table_glyph_codes(source)?;
    let pointer_command_table_parsed_glyphs_disjoint = aliases.is_disjoint(&referenced);
    ensure!(
        pointer_command_table_parsed_glyphs_disjoint,
        "KOUBAI1 memory-card-swap allocation aliases a source command glyph"
    );

    let declared_direct_selector_bytes = source
        .get(DECLARED_DIRECT_SELECTOR_BYTE_REGION_START..DECLARED_DIRECT_SELECTOR_BYTE_REGION_END)
        .context("KOUBAI2 declared direct-selector byte region is truncated")?;
    let declared_direct_selector_byte_region_scan_disjoint = aliases.iter().all(|code| {
        let selector = sprite_selector(*code);
        !declared_direct_selector_bytes
            .windows(selector.len())
            .any(|bytes| bytes == selector)
    });
    ensure!(
        declared_direct_selector_byte_region_scan_disjoint,
        "KOUBAI1 memory-card-swap allocation selector occurs in the declared direct-selector byte region"
    );

    Ok(GlyphOwnershipValidation {
        declared_physical_alias_set_matches,
        existing_bonus_component_allocations_disjoint,
        pointer_command_table_range: [POINTER_TABLE_OFFSET, POINTER_TABLE_END],
        pointer_command_table_parsed_glyphs_disjoint,
        declared_direct_selector_byte_region: [
            DECLARED_DIRECT_SELECTOR_BYTE_REGION_START,
            DECLARED_DIRECT_SELECTOR_BYTE_REGION_END,
        ],
        declared_direct_selector_byte_region_scan_disjoint,
    })
}

pub(super) fn allocated_physical_alias_codes() -> BTreeSet<u16> {
    let reserved = bonus_memory_card_swap_reserved_glyph_codes().collect::<Vec<_>>();
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
