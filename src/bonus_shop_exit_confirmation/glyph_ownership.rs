use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};

use crate::bonus_shop_source::{POINTER_COMMAND_RECORD_RANGES, cells_overlap, sprite_selector};

use super::consumer::{DIRECT_SELECTOR_REGION, OVERLAY_RUNTIME_BASE, POINTER_COMMAND_TABLE_RANGES};
use super::glyph_slots::{EXPECTED_PHYSICAL_ALIAS_CODES, glyph_cell};
use super::source::{FIXED_UI_TIM_OFFSET, FIXED_UI_TIM_SIZE, GLYPH_TIM_OFFSET};

#[derive(Debug)]
pub(super) struct GlyphOwnershipValidation {
    pub(super) declared_physical_alias_set_matches: bool,
    pub(super) fixed_ui_tim_disjoint: bool,
    pub(super) pointer_command_record_count: usize,
    pub(super) pointer_command_unique_glyph_count: usize,
    pub(super) pointer_command_table_parsed_glyphs_disjoint: bool,
    pub(super) declared_direct_selector_byte_region: [usize; 2],
    pub(super) declared_direct_selector_byte_region_scan_disjoint: bool,
}

pub(super) fn validate_allocated_glyph_ownership(
    source: &[u8],
) -> Result<GlyphOwnershipValidation> {
    let aliases = allocated_physical_alias_codes();
    let expected = EXPECTED_PHYSICAL_ALIAS_CODES.into_iter().collect();
    let declared_physical_alias_set_matches = aliases == expected;
    ensure!(
        declared_physical_alias_set_matches,
        "KOUBAI shop exit-confirmation physical alias set changed"
    );

    let fixed_ui_tim_disjoint = FIXED_UI_TIM_OFFSET + FIXED_UI_TIM_SIZE <= GLYPH_TIM_OFFSET;
    ensure!(
        fixed_ui_tim_disjoint,
        "KOUBAI fixed shop-UI writer overlaps the exit-confirmation glyph atlas"
    );

    let (referenced, pointer_command_record_count) = pointer_table_glyph_codes(source)?;
    let pointer_command_unique_glyph_count = referenced.len();
    let pointer_command_table_parsed_glyphs_disjoint = aliases.is_disjoint(&referenced);
    ensure!(
        pointer_command_table_parsed_glyphs_disjoint,
        "KOUBAI shop exit-confirmation allocation aliases a source command glyph"
    );

    let direct_selector_bytes = source
        .get(DIRECT_SELECTOR_REGION[0]..DIRECT_SELECTOR_REGION[1])
        .context("KOUBAI declared direct-selector byte region is truncated")?;
    let declared_direct_selector_byte_region_scan_disjoint = aliases.iter().all(|code| {
        let selector = sprite_selector(*code);
        !direct_selector_bytes
            .windows(selector.len())
            .any(|bytes| bytes == selector)
    });
    ensure!(
        declared_direct_selector_byte_region_scan_disjoint,
        "KOUBAI shop exit-confirmation selector occurs in the direct-selector byte region"
    );

    Ok(GlyphOwnershipValidation {
        declared_physical_alias_set_matches,
        fixed_ui_tim_disjoint,
        pointer_command_record_count,
        pointer_command_unique_glyph_count,
        pointer_command_table_parsed_glyphs_disjoint,
        declared_direct_selector_byte_region: DIRECT_SELECTOR_REGION,
        declared_direct_selector_byte_region_scan_disjoint,
    })
}

pub(super) fn allocated_physical_alias_codes() -> BTreeSet<u16> {
    let reserved = crate::bonus_shop_exit_confirmation::bonus_shop_exit_reserved_glyph_codes()
        .collect::<Vec<_>>();
    (0x0300_u16..=0x03ff)
        .filter(|candidate| {
            reserved
                .iter()
                .any(|allocated| cells_overlap(glyph_cell(*candidate), glyph_cell(*allocated)))
        })
        .collect()
}

fn pointer_table_glyph_codes(source: &[u8]) -> Result<(BTreeSet<u16>, usize)> {
    let mut codes = BTreeSet::new();
    let mut record_count = 0;
    for (pointer_range, record_range) in POINTER_COMMAND_TABLE_RANGES
        .into_iter()
        .zip(POINTER_COMMAND_RECORD_RANGES)
    {
        ensure!(
            (pointer_range[1] - pointer_range[0]).is_multiple_of(4),
            "KOUBAI pointer-command table alignment changed"
        );
        for pointer_offset in (pointer_range[0]..pointer_range[1]).step_by(4) {
            let pointer = read_u32(source, pointer_offset)?;
            let record_offset = usize::try_from(
                pointer
                    .checked_sub(OVERLAY_RUNTIME_BASE)
                    .context("KOUBAI command pointer precedes its runtime base")?,
            )?;
            ensure!(
                (record_range[0]..record_range[1]).contains(&record_offset),
                "KOUBAI command pointer left its typed source record region"
            );
            parse_command_record(source, record_offset, record_range[1], &mut codes)?;
            record_count += 1;
        }
    }
    Ok((codes, record_count))
}

fn parse_command_record(
    source: &[u8],
    mut cursor: usize,
    record_region_end: usize,
    codes: &mut BTreeSet<u16>,
) -> Result<()> {
    loop {
        ensure!(
            cursor < record_region_end,
            "KOUBAI command sequence is unterminated within its record region"
        );
        let opcode = *source.get(cursor).context("truncated KOUBAI command")?;
        match opcode {
            0x81 => return Ok(()),
            0x80 => cursor += 1,
            0x63 => {
                ensure!(
                    source.get(cursor..cursor + 3) == Some(&[0x63; 3]),
                    "KOUBAI blank command changed"
                );
                cursor += 3;
            }
            page => {
                let column = *source.get(cursor + 1).context("truncated glyph column")?;
                let row = *source.get(cursor + 2).context("truncated glyph row")?;
                ensure!(
                    page <= 3 && column <= 15 && row <= 15,
                    "invalid KOUBAI page/column/row glyph command"
                );
                codes.insert(u16::from(page) << 8 | u16::from(row) << 4 | u16::from(column));
                cursor += 3;
            }
        }
    }
}

fn read_u32(source: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        source
            .get(offset..offset + 4)
            .with_context(|| format!("truncated KOUBAI word at +0x{offset:04x}"))?
            .try_into()?,
    ))
}
