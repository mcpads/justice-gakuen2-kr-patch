//! Audited SELP1 glyph allocation for the complete versus-stage table.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Result, ensure};

use crate::tim::{Cell, cells_overlap};

use crate::character_select_graphics::consumer::CharacterSelectDynamicTextBinding;
use crate::character_select_graphics::model::{
    CharacterSelectFixedStripAllocation, CharacterSelectFontRole, CharacterSelectGlyphAllocation,
    CharacterSelectLocalizedSource, CharacterSelectSourceCellOccupancy,
    CharacterSelectTextureSurface,
};

use super::{SHARED_ATLAS_OFFSET, SourceInkMap, available_cells, pack_non_overlapping};

pub(super) fn allocate_stage_label_glyphs(
    localized_sources: &[CharacterSelectLocalizedSource],
    dynamic_bindings: &[CharacterSelectDynamicTextBinding],
    source_ink: &SourceInkMap,
    reclaimable_cells: &[Cell],
    protected_cells: &[Cell],
    fixed_strips: &[CharacterSelectFixedStripAllocation],
    allocations: &mut Vec<CharacterSelectGlyphAllocation>,
) -> Result<()> {
    let stage_source_ui_ids = dynamic_bindings
        .iter()
        .filter(|binding| binding.font_role == CharacterSelectFontRole::StageLabel)
        .map(|binding| binding.source_ui_id)
        .collect::<BTreeSet<_>>();
    let localized_by_id = localized_sources
        .iter()
        .map(|source| (source.source_ui_id.as_str(), source))
        .collect::<BTreeMap<_, _>>();
    let active_source_ui_ids = stage_source_ui_ids
        .iter()
        .filter(|source_ui_id| localized_by_id.contains_key(**source_ui_id))
        .copied()
        .collect::<BTreeSet<_>>();
    if active_source_ui_ids.is_empty() {
        return Ok(());
    }
    ensure!(
        active_source_ui_ids == stage_source_ui_ids,
        "stage-label allocation requires all {} stage translations, found {}",
        stage_source_ui_ids.len(),
        active_source_ui_ids.len()
    );

    let mut requested = BTreeSet::new();
    for source_ui_id in &stage_source_ui_ids {
        let source = localized_by_id[source_ui_id];
        for character in source.korean_text.chars() {
            ensure!(
                character == ' ' || !character.is_whitespace(),
                "stage-label translation {} contains unsupported whitespace {:?}",
                source.source_ui_id,
                character
            );
            if character != ' ' {
                requested.insert(character);
            }
        }
    }
    ensure!(
        !requested.is_empty(),
        "stage-label translations contain no glyphs"
    );

    let mut candidates = available_cells(
        source_ink,
        CharacterSelectFontRole::StageLabel,
        protected_cells,
    );
    for cell in reclaimable_cells {
        ensure!(
            cell.width == 20
                && cell.height == 20
                && cell.x < source_ink.width
                && cell.x % 256 <= 220
                && cell.y <= 220
                && cell.x % 256 % 20 == 0
                && cell.y % 20 == 0,
            "stage-label reclaim candidate escaped the canonical texture-page grid: {cell:?}"
        );
        let texture_page_index = u8::try_from(cell.x / 256)?;
        let texture_uv = [u8::try_from(cell.x % 256)?, u8::try_from(cell.y)?];
        candidates.push((texture_page_index, texture_uv, *cell));
    }
    let candidates = candidates
        .into_iter()
        .filter(|candidate| {
            protected_cells
                .iter()
                .all(|cell| !cells_overlap(*cell, candidate.2))
        })
        .filter(|cell| {
            let cell = cell.2;
            allocations.iter().all(|allocation| {
                !allocation
                    .surface
                    .shares_physical_texture(CharacterSelectTextureSurface::StageLabelAtlas)
                    || !cells_overlap(allocation.cell, cell)
            })
        })
        .filter(|cell| {
            let cell = cell.2;
            fixed_strips.iter().all(|strip| {
                !strip
                    .surface
                    .shares_physical_texture(CharacterSelectTextureSurface::StageLabelAtlas)
                    || !cells_overlap(strip.cell, cell)
            })
        })
        .collect::<Vec<_>>();
    let available_count = pack_non_overlapping(candidates.clone(), usize::MAX).len();
    let selected = pack_non_overlapping(candidates, requested.len());
    ensure!(
        selected.len() == requested.len(),
        "stage labels need {} unique glyphs but only {} audited SELP1 cells are available",
        requested.len(),
        available_count
    );

    for (character, (texture_page_index, texture_uv, cell)) in requested.into_iter().zip(selected) {
        ensure!(
            cell.width == 20
                && cell.height == 20
                && cell.x / 256 == usize::from(texture_page_index)
                && cell.x % 256 <= 220
                && cell.y <= 220
                && cell.x % 256 % 20 == 0
                && cell.y % 20 == 0,
            "stage-label candidate escaped the canonical texture-page grid: {cell:?}"
        );
        let source_cell_occupancy = if source_ink.is_blank(cell) {
            CharacterSelectSourceCellOccupancy::Blank
        } else {
            CharacterSelectSourceCellOccupancy::Inked
        };
        allocations.push(CharacterSelectGlyphAllocation {
            font_role: CharacterSelectFontRole::StageLabel,
            character,
            surface: CharacterSelectTextureSurface::StageLabelAtlas,
            tim_offset: SHARED_ATLAS_OFFSET,
            texture_page_index,
            texture_uv,
            cell,
            source_cell_occupancy,
        });
    }
    Ok(())
}
