//! Binds the completed tournament bracket's two source-owned champion glyph cells.

use anyhow::{Context, Result, ensure};

use crate::character_select_graphics::model::{
    CharacterSelectConsumerReferenceKind, CharacterSelectFixedStripAllocation,
    CharacterSelectFixedStripWriteMode, CharacterSelectFontRole, CharacterSelectLocalizedSource,
    CharacterSelectRouteOccurrence, CharacterSelectTextFlow, CharacterSelectTextSelection,
    CharacterSelectTextureSurface,
};
use crate::character_select_graphics::route_census::{
    bound_occurrence, consumer_target, pending_occurrence, producer_target,
};
use crate::character_select_graphics::texture_targets::SHARED_ATLAS_OFFSET;
use crate::pipeline::sha256_bytes;
use crate::tim::{Cell, read_indexed_cell_in_prefix};

const SOURCE_UI_ID: &str = "tournament_champion_label";
const SOURCE_TEXT: &str = "優勝";
const ROUTE_OCCURRENCE_ID: &str = "plsel4-completed-bracket-champion-label";

struct SourceGlyphCell {
    physical_region_id: &'static str,
    translation_character_index: usize,
    cell: Cell,
}

const SOURCE_GLYPH_CELLS: [SourceGlyphCell; 2] = [
    SourceGlyphCell {
        physical_region_id: "completed-bracket-champion-left",
        translation_character_index: 0,
        cell: Cell {
            x: 328,
            y: 224,
            width: 32,
            height: 32,
        },
    },
    SourceGlyphCell {
        physical_region_id: "completed-bracket-champion-right",
        translation_character_index: 1,
        cell: Cell {
            x: 256,
            y: 0,
            width: 32,
            height: 32,
        },
    },
];

const EXPECTED_SOURCE_CELL_SHA256: [&str; 2] = [
    "af3df8b082e616cb5ab6447320db6d76b9c5bbe69677cb8c13cc6f6820ac8bef",
    "9aa9ececa82bd4f95cfcb4573356c37ba13feca296a76feb2b4fb555d015c07c",
];

pub(super) struct TournamentBracketPlan {
    pub(super) fixed_strips: Vec<CharacterSelectFixedStripAllocation>,
    pub(super) occurrences: Vec<CharacterSelectRouteOccurrence>,
}

pub(super) fn plan_tournament_bracket_label(
    source_decoded: Option<&[u8]>,
    localized_sources: &[CharacterSelectLocalizedSource],
) -> Result<TournamentBracketPlan> {
    let Some(entry) = localized_sources
        .iter()
        .find(|entry| entry.source_ui_id == SOURCE_UI_ID)
    else {
        return Ok(empty_plan());
    };
    ensure!(
        entry.source_text == SOURCE_TEXT,
        "completed tournament bracket champion source identity changed"
    );
    ensure!(
        entry.korean_text.chars().count() == SOURCE_GLYPH_CELLS.len(),
        "completed tournament bracket champion translation must have exactly two characters"
    );
    let Some(source_decoded) = source_decoded else {
        return Ok(TournamentBracketPlan {
            fixed_strips: Vec::new(),
            occurrences: vec![bracket_occurrence(false)],
        });
    };
    validate_source_cells(source_decoded)?;
    let fixed_strips = SOURCE_GLYPH_CELLS
        .iter()
        .map(|spec| CharacterSelectFixedStripAllocation {
            physical_text_region_id: spec.physical_region_id.to_string(),
            source_ui_ids: vec![SOURCE_UI_ID.to_string()],
            translation_id: entry.translation_id.clone(),
            text_selection: CharacterSelectTextSelection::Character {
                index: spec.translation_character_index,
            },
            text_flow: CharacterSelectTextFlow::Horizontal,
            write_mode: CharacterSelectFixedStripWriteMode::ReplaceRegion,
            font_role: CharacterSelectFontRole::TournamentBracketLabel,
            surface: CharacterSelectTextureSurface::TournamentBracketLabelAtlas,
            tim_offset: SHARED_ATLAS_OFFSET,
            cell: spec.cell,
            text_cell: spec.cell,
            clear_index: 0,
        })
        .collect();
    Ok(TournamentBracketPlan {
        fixed_strips,
        occurrences: vec![bracket_occurrence(true)],
    })
}

fn validate_source_cells(source_decoded: &[u8]) -> Result<()> {
    let source_tim = source_decoded
        .get(SHARED_ATLAS_OFFSET..)
        .context("SELP4 shared atlas offset moved")?;
    let tim = crate::tim::parse_4bpp_prefix(source_tim)?;
    ensure!(
        tim.pixel_width() == 1024
            && tim.image_height == 256
            && tim.image_x == 768
            && tim.image_y == 0,
        "SELP4 completed-bracket source TIM geometry changed"
    );
    let identities = SOURCE_GLYPH_CELLS
        .iter()
        .map(|spec| {
            Ok((
                spec.physical_region_id,
                sha256_bytes(&read_indexed_cell_in_prefix(
                    source_decoded,
                    SHARED_ATLAS_OFFSET,
                    spec.cell,
                )?),
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    ensure!(
        identities
            .iter()
            .map(|(_, sha256)| sha256.as_str())
            .eq(EXPECTED_SOURCE_CELL_SHA256),
        "SELP4 completed-bracket champion source cells changed: found {identities:?}"
    );
    Ok(())
}

fn bracket_occurrence(bound: bool) -> CharacterSelectRouteOccurrence {
    let producer_targets = vec![producer_target(
        "DAT2/SELP4.BIZ",
        Some(SHARED_ATLAS_OFFSET),
        Some(CharacterSelectTextureSurface::TournamentBracketLabelAtlas),
        SOURCE_GLYPH_CELLS.map(|spec| spec.physical_region_id),
    )];
    let consumer_targets = vec![consumer_target(
        "DAT2/SELP4.BIZ",
        "DAT1/PLSEL4.BIN",
        CharacterSelectConsumerReferenceKind::PrimitiveSetup,
        [0x6968, 0x6990, 0x6a44, 0x6ac4, 0x6b54],
        ["top-level-state-1/inner-state-8/completed-bracket"],
    )];
    if bound {
        bound_occurrence(
            ROUTE_OCCURRENCE_ID,
            "selp4_completed_tournament_bracket",
            [SOURCE_UI_ID],
            producer_targets,
            consumer_targets,
        )
    } else {
        pending_occurrence(
            ROUTE_OCCURRENCE_ID,
            "selp4_completed_tournament_bracket",
            [SOURCE_UI_ID],
            producer_targets,
            consumer_targets,
        )
    }
}

fn empty_plan() -> TournamentBracketPlan {
    TournamentBracketPlan {
        fixed_strips: Vec::new(),
        occurrences: Vec::new(),
    }
}
