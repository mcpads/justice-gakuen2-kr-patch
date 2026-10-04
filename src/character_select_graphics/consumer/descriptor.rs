//! Encoding Korean text into the descriptor grammars consumed by PLSEL overlays.

use std::collections::BTreeMap;

use anyhow::{Context, Result, ensure};

use super::map::{DescriptorEncoding, DescriptorSpec};
use crate::character_select_graphics::model::{
    CharacterSelectFontRole, CharacterSelectGlyphAllocation, CharacterSelectLocalizedSource,
    CharacterSelectTextureSurface,
};

type AllocationMap<'a> = BTreeMap<
    (CharacterSelectTextureSurface, CharacterSelectFontRole, char),
    &'a CharacterSelectGlyphAllocation,
>;

pub(super) fn encode_descriptor(
    entry: &CharacterSelectLocalizedSource,
    spec: &DescriptorSpec,
    allocations: &AllocationMap<'_>,
) -> Result<Vec<u8>> {
    match spec.encoding {
        DescriptorEncoding::DirectUv => encode_direct_uv(entry, spec, allocations),
        DescriptorEncoding::DirectLabelGrid => encode_label_grid(entry, spec, allocations, false),
        DescriptorEncoding::CountedLabelGrid => encode_label_grid(entry, spec, allocations, true),
        DescriptorEncoding::ParticipantRuns => encode_participant_runs(entry, spec, allocations),
        DescriptorEncoding::SelectionHelp => encode_selection_help(entry, spec, allocations),
        DescriptorEncoding::ColumnRow | DescriptorEncoding::DirectURow => {
            encode_heading_descriptor(entry, spec, allocations)
        }
    }
}

fn encode_direct_uv(
    entry: &CharacterSelectLocalizedSource,
    spec: &DescriptorSpec,
    allocations: &AllocationMap<'_>,
) -> Result<Vec<u8>> {
    let characters = entry
        .korean_text
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<Vec<_>>();
    let capacity = spec.expected.len() / 2;
    ensure!(
        spec.expected.len().is_multiple_of(2) && characters.len() == capacity,
        "character-select translation {} needs exactly {} direct UV slots, found {}",
        entry.source_ui_id,
        capacity,
        characters.len()
    );
    let mut encoded = Vec::with_capacity(spec.expected.len());
    let mut page = None;
    for character in characters {
        let allocation =
            allocation_for(entry, spec.surface, spec.font_role, character, allocations)?;
        ensure_one_page(entry, &mut page, allocation.texture_page_index)?;
        encoded.extend_from_slice(&allocation.texture_uv);
    }
    Ok(encoded)
}

fn encode_heading_descriptor(
    entry: &CharacterSelectLocalizedSource,
    spec: &DescriptorSpec,
    allocations: &AllocationMap<'_>,
) -> Result<Vec<u8>> {
    ensure!(
        spec.expected.len() % 2 == 1,
        "counted character-select descriptor has an invalid source length"
    );
    let capacity = (spec.expected.len() - 1) / 2;
    let characters = entry.korean_text.chars().collect::<Vec<_>>();
    ensure!(
        characters.len() <= capacity,
        "character-select translation {} needs {} slots but its consumer holds {}",
        entry.source_ui_id,
        characters.len(),
        capacity
    );
    let mut encoded = vec![0_u8; spec.expected.len()];
    let mut cursor = 1;
    encoded[0] = u8::try_from(characters.len())?;
    let mut page = None;
    for character in characters {
        // Spaces are drawn quads too. UV (0, 0) can contain native artwork on
        // the selected page; use the allocator-owned transparent glyph instead.
        let allocation = allocations
            .get(&(spec.surface, spec.font_role, character))
            .with_context(|| {
                format!(
                    "character-select translation {} has no allocation for {:?}",
                    entry.source_ui_id, character
                )
            })?;
        ensure!(
            page.replace(allocation.texture_page_index)
                .is_none_or(|existing| existing == allocation.texture_page_index),
            "character-select translation {} spans texture pages",
            entry.source_ui_id
        );
        let [u, v] = allocation.texture_uv;
        let pair = match spec.encoding {
            DescriptorEncoding::ColumnRow => {
                ensure!(
                    u % 32 == 0 && v % 32 == 0,
                    "character-select translation {} is not representable as column/row",
                    entry.source_ui_id
                );
                [u / 32, v / 32]
            }
            DescriptorEncoding::DirectURow => {
                ensure!(
                    v % 32 == 0,
                    "character-select translation {} has a non-row-aligned V coordinate",
                    entry.source_ui_id
                );
                [u, v / 32]
            }
            _ => unreachable!("non-heading encoding was dispatched before heading encoding"),
        };
        encoded[cursor..cursor + 2].copy_from_slice(&pair);
        cursor += 2;
    }
    Ok(encoded)
}

fn encode_label_grid(
    entry: &CharacterSelectLocalizedSource,
    spec: &DescriptorSpec,
    allocations: &AllocationMap<'_>,
    counted: bool,
) -> Result<Vec<u8>> {
    let characters = entry
        .korean_text
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<Vec<_>>();
    let header_size = usize::from(counted);
    let capacity = (spec.expected.len() - header_size) / 2;
    if counted {
        ensure!(
            characters.len() <= capacity,
            "character-select translation {} needs {} visible slots but its counted grid holds {}",
            entry.source_ui_id,
            characters.len(),
            capacity
        );
    } else {
        ensure!(
            characters.len() == capacity,
            "character-select translation {} needs exactly {} visible slots, found {}",
            entry.source_ui_id,
            capacity,
            characters.len()
        );
    }
    let mut encoded = vec![0_u8; spec.expected.len()];
    if counted {
        encoded[0] = u8::try_from(characters.len())?;
    }
    let mut cursor = header_size;
    let mut page = None;
    for character in characters {
        let allocation =
            allocation_for(entry, spec.surface, spec.font_role, character, allocations)?;
        ensure_one_page(entry, &mut page, allocation.texture_page_index)?;
        let [u, v] = allocation.texture_uv;
        ensure!(
            u % 20 == 0 && v % 20 == 0,
            "character-select translation {} is not representable on the 20px label grid",
            entry.source_ui_id
        );
        encoded[cursor..cursor + 2].copy_from_slice(&[u / 20, v / 20]);
        cursor += 2;
    }
    Ok(encoded)
}

fn encode_participant_runs(
    entry: &CharacterSelectLocalizedSource,
    spec: &DescriptorSpec,
    allocations: &AllocationMap<'_>,
) -> Result<Vec<u8>> {
    let characters = entry
        .korean_text
        .chars()
        .filter(|character| !character.is_whitespace() && *character != '?')
        .collect::<Vec<_>>();
    ensure!(
        characters.len() == 8 && entry.korean_text.trim_end().ends_with('?'),
        "team participant prompt must contain eight visible glyphs followed by a question mark"
    );
    let mut cells = Vec::with_capacity(characters.len());
    let mut page = None;
    for character in characters {
        let allocation =
            allocation_for(entry, spec.surface, spec.font_role, character, allocations)?;
        ensure_one_page(entry, &mut page, allocation.texture_page_index)?;
        let [u, v] = allocation.texture_uv;
        ensure!(
            u % 20 == 0 && v % 20 == 0,
            "team participant prompt is not representable on the 20px label grid"
        );
        cells.push((u / 20, v / 20));
    }
    let mut runs = Vec::<(u8, u8, u8)>::new();
    for (column, row) in cells {
        if let Some((start, existing_row, width)) = runs.last_mut()
            && *existing_row == row
            && start.checked_add(*width) == Some(column)
        {
            *width += 1;
        } else {
            runs.push((column, row, 1));
        }
    }
    ensure!(
        runs.len() <= 2,
        "team participant prompt needs {} texture runs but its consumer holds two",
        runs.len()
    );
    let mut encoded = Vec::with_capacity(spec.expected.len());
    encoded.push(u8::try_from(runs.len() + 1)?);
    for (column, row, width) in runs {
        encoded.extend_from_slice(&[column, row, width]);
    }
    encoded.extend_from_slice(&[0x06, 0x01, 0x01]);
    ensure!(
        encoded.len() == spec.expected.len(),
        "team participant descriptor changed size"
    );
    Ok(encoded)
}

fn encode_selection_help(
    entry: &CharacterSelectLocalizedSource,
    spec: &DescriptorSpec,
    allocations: &AllocationMap<'_>,
) -> Result<Vec<u8>> {
    let capacity = spec.expected.len() / 2;
    let characters = entry.korean_text.chars().collect::<Vec<_>>();
    ensure!(
        characters.len() == capacity,
        "selection help needs exactly {} slots; got {}",
        capacity,
        characters.len()
    );
    let mut encoded = Vec::with_capacity(spec.expected.len());
    let mut page = None;
    for character in characters {
        let allocation =
            allocation_for(entry, spec.surface, spec.font_role, character, allocations)?;
        ensure_one_page(entry, &mut page, allocation.texture_page_index)?;
        let [u, v] = allocation.texture_uv;
        ensure!(
            u % 12 == 0 && v >= 96 && (v - 96) % 16 == 0,
            "selection help glyph {character:?} is not representable by its 12x16 consumer"
        );
        encoded.extend_from_slice(&[u / 12, (v - 96) / 16]);
    }
    Ok(encoded)
}

fn allocation_for<'a>(
    entry: &CharacterSelectLocalizedSource,
    surface: CharacterSelectTextureSurface,
    role: CharacterSelectFontRole,
    character: char,
    allocations: &'a AllocationMap<'a>,
) -> Result<&'a CharacterSelectGlyphAllocation> {
    allocations
        .get(&(surface, role, character))
        .copied()
        .with_context(|| {
            format!(
                "character-select translation {} has no allocation for {:?}",
                entry.source_ui_id, character
            )
        })
}

fn ensure_one_page(
    entry: &CharacterSelectLocalizedSource,
    page: &mut Option<u8>,
    candidate: u8,
) -> Result<()> {
    ensure!(
        page.replace(candidate)
            .is_none_or(|existing| existing == candidate),
        "character-select translation {} spans texture pages",
        entry.source_ui_id
    );
    Ok(())
}
