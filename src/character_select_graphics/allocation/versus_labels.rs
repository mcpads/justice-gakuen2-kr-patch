//! Glyph placement for labels owned by versus-screen consumers.

use anyhow::{Result, ensure};

use crate::pipeline::sha256_bytes;
use crate::tim::{Cell, read_indexed_cell_in_prefix};

use super::SHARED_ATLAS_OFFSET;
use crate::character_select_graphics::model::{
    CharacterSelectFontRole, CharacterSelectGlyphAllocation, CharacterSelectLocalizedSource,
    CharacterSelectSourceCellOccupancy, CharacterSelectTextureSurface,
};

const WIN_COUNT_CELLS: [Cell; 3] = [
    Cell {
        x: 160,
        y: 200,
        width: 20,
        height: 20,
    },
    Cell {
        x: 180,
        y: 200,
        width: 20,
        height: 20,
    },
    Cell {
        x: 60,
        y: 100,
        width: 20,
        height: 20,
    },
];

const HANDICAP_SOURCE_REGION: Cell = Cell {
    x: 0,
    y: 60,
    width: 120,
    height: 40,
};
const HANDICAP_SOURCE_REGION_SHA256: &str =
    "640246bc979896418ceb08f1477465f941edab23a58e278179f3471676781ab9";
const HANDICAP_KOREAN_CELLS: [Cell; 3] = [
    Cell {
        x: 0,
        y: 80,
        width: 20,
        height: 20,
    },
    Cell {
        x: 20,
        y: 80,
        width: 20,
        height: 20,
    },
    Cell {
        x: 40,
        y: 80,
        width: 20,
        height: 20,
    },
];

pub(super) fn allocate_versus_label_glyphs(
    localized_sources: &[CharacterSelectLocalizedSource],
    selp1_source_decoded: &[u8],
    allocations: &mut Vec<CharacterSelectGlyphAllocation>,
) -> Result<()> {
    if let Some(win_count) = localized_sources
        .iter()
        .find(|entry| entry.source_ui_id == "win_count_label")
    {
        let characters = visible_characters(&win_count.korean_text);
        ensure!(
            characters.len() == WIN_COUNT_CELLS.len(),
            "win-count label must occupy exactly {} owned cells",
            WIN_COUNT_CELLS.len()
        );
        for (&character, &cell) in characters.iter().zip(&WIN_COUNT_CELLS) {
            allocations.push(CharacterSelectGlyphAllocation {
                font_role: CharacterSelectFontRole::Label,
                character,
                surface: CharacterSelectTextureSurface::VersusLabelAtlas,
                tim_offset: SHARED_ATLAS_OFFSET,
                texture_page_index: 0,
                texture_uv: [u8::try_from(cell.x)?, u8::try_from(cell.y)?],
                cell,
                source_cell_occupancy: CharacterSelectSourceCellOccupancy::Inked,
            });
        }
    }

    let Some(handicap) = localized_sources
        .iter()
        .find(|entry| entry.source_ui_id == "versus_handicap_label")
    else {
        return Ok(());
    };
    let characters = visible_characters(&handicap.korean_text);
    ensure!(
        characters.len() == HANDICAP_KOREAN_CELLS.len(),
        "versus handicap must occupy exactly {} owned source cells",
        HANDICAP_KOREAN_CELLS.len()
    );
    let source_region = read_indexed_cell_in_prefix(
        selp1_source_decoded,
        SHARED_ATLAS_OFFSET,
        HANDICAP_SOURCE_REGION,
    )?;
    ensure!(
        sha256_bytes(&source_region) == HANDICAP_SOURCE_REGION_SHA256,
        "SELP1 versus-handicap source glyph region changed"
    );
    for (&character, &cell) in characters.iter().zip(&HANDICAP_KOREAN_CELLS) {
        allocations.push(CharacterSelectGlyphAllocation {
            font_role: CharacterSelectFontRole::Label,
            character,
            surface: CharacterSelectTextureSurface::VersusHandicapAtlas,
            tim_offset: SHARED_ATLAS_OFFSET,
            texture_page_index: 0,
            texture_uv: [u8::try_from(cell.x)?, u8::try_from(cell.y)?],
            cell,
            source_cell_occupancy: CharacterSelectSourceCellOccupancy::Inked,
        });
    }
    Ok(())
}

fn visible_characters(text: &str) -> Vec<char> {
    text.chars()
        .filter(|character| !character.is_whitespace())
        .collect()
}
