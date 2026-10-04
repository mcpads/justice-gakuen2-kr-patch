//! Allocations bound to exact source cells owned by record-specific label consumers.

use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};

use crate::tim::Cell;

use super::{SHARED_ATLAS_OFFSET, SourceInkMap, cells_overlap, pack_on_one_texture_page};
use crate::character_select_graphics::model::{
    CharacterSelectFontRole, CharacterSelectGlyphAllocation, CharacterSelectLocalizedSource,
    CharacterSelectSourceCellOccupancy, CharacterSelectTextureSurface,
};

const PARTICIPANT_LABEL_CELLS: [Cell; 8] = [
    Cell {
        x: 120,
        y: 100,
        width: 20,
        height: 20,
    },
    Cell {
        x: 140,
        y: 100,
        width: 20,
        height: 20,
    },
    Cell {
        x: 160,
        y: 100,
        width: 20,
        height: 20,
    },
    Cell {
        x: 180,
        y: 100,
        width: 20,
        height: 20,
    },
    Cell {
        x: 140,
        y: 20,
        width: 20,
        height: 20,
    },
    Cell {
        x: 160,
        y: 20,
        width: 20,
        height: 20,
    },
    Cell {
        x: 180,
        y: 20,
        width: 20,
        height: 20,
    },
    Cell {
        x: 200,
        y: 20,
        width: 20,
        height: 20,
    },
];

pub(super) fn allocate_battle_ready_label_glyphs(
    localized_sources: &[CharacterSelectLocalizedSource],
    source_ink: &SourceInkMap,
    protected_fixed_cells: &[Cell],
    allocations: &mut Vec<CharacterSelectGlyphAllocation>,
) -> Result<()> {
    let Some(entry) = localized_sources
        .iter()
        .find(|entry| entry.source_ui_id == "battle_ready_label")
    else {
        return Ok(());
    };
    let visible = visible_characters(&entry.korean_text, false);
    ensure!(
        visible.len() == 6,
        "battle-ready label must occupy exactly six sprite slots"
    );
    let requested = visible.into_iter().collect::<BTreeSet<_>>();
    let selected = pack_on_one_texture_page(
        available_battle_ready_cells(source_ink, protected_fixed_cells, allocations),
        requested.len(),
    );
    ensure!(
        selected.len() == requested.len(),
        "battle-ready label needs {} glyphs but only {} cells fit on one protected texture page",
        requested.len(),
        selected.len()
    );
    for (character, (texture_page_index, texture_uv, cell)) in requested.into_iter().zip(selected) {
        allocations.push(CharacterSelectGlyphAllocation {
            font_role: CharacterSelectFontRole::Label,
            character,
            surface: CharacterSelectTextureSurface::BattleReadyAtlas,
            tim_offset: SHARED_ATLAS_OFFSET,
            texture_page_index,
            texture_uv,
            cell,
            source_cell_occupancy: CharacterSelectSourceCellOccupancy::Blank,
        });
    }
    Ok(())
}

pub(super) fn allocate_participant_label_glyphs(
    localized_sources: &[CharacterSelectLocalizedSource],
    allocations: &mut Vec<CharacterSelectGlyphAllocation>,
) -> Result<()> {
    let owned_ids = [
        "team_participant_count_prompt",
        "league_participant_count_prompt",
    ];
    if !localized_sources
        .iter()
        .any(|entry| owned_ids.contains(&entry.source_ui_id.as_str()))
    {
        return Ok(());
    }
    let team = localized_source(localized_sources, "team_participant_count_prompt")?;
    let league = localized_source(localized_sources, "league_participant_count_prompt")?;
    let team_characters = visible_characters(&team.korean_text, true);
    let league_characters = visible_characters(&league.korean_text, false);
    ensure!(
        team_characters == league_characters,
        "team and league participant prompts must share one physical glyph sequence"
    );
    ensure!(
        league_characters.len() == PARTICIPANT_LABEL_CELLS.len(),
        "participant prompt must occupy exactly {} owned cells",
        PARTICIPANT_LABEL_CELLS.len()
    );
    append_owned_glyphs(
        allocations,
        CharacterSelectTextureSurface::ParticipantLabelAtlas,
        &league_characters,
        &PARTICIPANT_LABEL_CELLS,
    )
}

pub(super) fn allocate_selection_help_glyphs(
    localized_sources: &[CharacterSelectLocalizedSource],
    source_ink: &SourceInkMap,
    protected_fixed_cells: &[Cell],
    allocations: &mut Vec<CharacterSelectGlyphAllocation>,
) -> Result<()> {
    if !localized_sources
        .iter()
        .any(|entry| entry.source_ui_id == "selection_back_help")
    {
        return Ok(());
    }
    let entry = localized_source(localized_sources, "selection_back_help")?;
    let requested = entry.korean_text.chars().collect::<BTreeSet<_>>();
    let mut occupied = allocations
        .iter()
        .filter(|allocation| {
            allocation
                .surface
                .shares_physical_texture(CharacterSelectTextureSurface::SelectionHelpAtlas)
        })
        .map(|allocation| allocation.cell)
        .collect::<Vec<_>>();
    occupied.extend_from_slice(protected_fixed_cells);
    let candidates = available_selection_help_cells(source_ink, &occupied);
    let selected = pack_on_one_texture_page(candidates, requested.len());
    ensure!(
        selected.len() == requested.len(),
        "selection help needs {} glyphs including its blank cell but only {} fit on one page",
        requested.len(),
        selected.len()
    );
    for (character, (texture_page_index, texture_uv, cell)) in requested.into_iter().zip(selected) {
        allocations.push(CharacterSelectGlyphAllocation {
            font_role: CharacterSelectFontRole::SelectionHelp,
            character,
            surface: CharacterSelectTextureSurface::SelectionHelpAtlas,
            tim_offset: SHARED_ATLAS_OFFSET,
            texture_page_index,
            texture_uv,
            cell,
            source_cell_occupancy: CharacterSelectSourceCellOccupancy::Blank,
        });
    }
    Ok(())
}

fn available_battle_ready_cells(
    source_ink: &SourceInkMap,
    protected_fixed_cells: &[Cell],
    allocations: &[CharacterSelectGlyphAllocation],
) -> Vec<(u8, [u8; 2], Cell)> {
    let occupied = protected_fixed_cells
        .iter()
        .copied()
        .chain(
            allocations
                .iter()
                .filter(|a| {
                    a.surface
                        .shares_physical_texture(CharacterSelectTextureSurface::BattleReadyAtlas)
                })
                .map(|a| a.cell),
        )
        .collect::<Vec<_>>();
    // The descriptor and GetTPage consumers follow the allocated page together.
    // Transparent parts of existing sprites are never fallback storage.
    super::available_rectangles(
        source_ink,
        [
            crate::character_select_graphics::ready_layout::CELL_WIDTH,
            crate::character_select_graphics::ready_layout::CELL_HEIGHT,
        ],
        4,
        &occupied,
    )
}

fn localized_source<'a>(
    entries: &'a [CharacterSelectLocalizedSource],
    source_ui_id: &str,
) -> Result<&'a CharacterSelectLocalizedSource> {
    entries
        .iter()
        .find(|entry| entry.source_ui_id == source_ui_id)
        .with_context(|| format!("missing character-select translation for {source_ui_id}"))
}

fn visible_characters(text: &str, trim_question_mark: bool) -> Vec<char> {
    text.chars()
        .filter(|character| {
            !character.is_whitespace() && !(trim_question_mark && *character == '?')
        })
        .collect()
}

fn append_owned_glyphs(
    allocations: &mut Vec<CharacterSelectGlyphAllocation>,
    surface: CharacterSelectTextureSurface,
    characters: &[char],
    cells: &[Cell],
) -> Result<()> {
    for (&character, &cell) in characters.iter().zip(cells) {
        allocations.push(CharacterSelectGlyphAllocation {
            font_role: CharacterSelectFontRole::Label,
            character,
            surface,
            tim_offset: SHARED_ATLAS_OFFSET,
            texture_page_index: 0,
            texture_uv: [u8::try_from(cell.x)?, u8::try_from(cell.y)?],
            cell,
            source_cell_occupancy: CharacterSelectSourceCellOccupancy::Inked,
        });
    }
    Ok(())
}

fn available_selection_help_cells(
    source_ink: &SourceInkMap,
    occupied: &[Cell],
) -> Vec<(u8, [u8; 2], Cell)> {
    let [width, height] = CharacterSelectFontRole::SelectionHelp.cell_size();
    let mut cells = Vec::new();
    let page = 3;
    if source_ink.width < (page + 1) * 256 {
        return cells;
    }
    let page_width = (source_ink.width - page * 256).min(256);
    for y in (96..=source_ink.height - height).step_by(height) {
        for u in (0..=page_width - width).step_by(width) {
            let cell = Cell {
                x: page * 256 + u,
                y,
                width,
                height,
            };
            if source_ink.is_blank(cell)
                && occupied
                    .iter()
                    .all(|protected| !cells_overlap(*protected, cell))
            {
                cells.push((page as u8, [u as u8, y as u8], cell));
            }
        }
    }
    cells
}
