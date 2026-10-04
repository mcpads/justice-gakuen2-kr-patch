//! Record-owned win/loss cells consumed as literal sprites by PLSEL3 standings.

use anyhow::{Result, ensure};

use crate::character_select_graphics::model::{
    CharacterSelectConsumerReferenceKind, CharacterSelectFixedStripAllocation,
    CharacterSelectFixedStripWriteMode, CharacterSelectFontRole, CharacterSelectLocalizedSource,
    CharacterSelectRouteOccurrence, CharacterSelectTextFlow, CharacterSelectTextSelection,
    CharacterSelectTextureSurface,
};
use crate::character_select_graphics::route_census::{
    bound_occurrence, consumer_target, producer_target,
};
use crate::character_select_graphics::texture_targets::SHARED_ATLAS_OFFSET;
use crate::pipeline::sha256_bytes;
use crate::tim::{Cell, read_indexed_cell_in_prefix};

const SOURCE_RECORD: &str = "DAT2/SELP3.BIZ";
const CONSUMER_RECORD: &str = "DAT1/PLSEL3.BIN";
const RUNTIME_STATE: &str = "league-standings";

#[derive(Clone, Copy)]
struct LeagueStandingLabel {
    source_ui_id: &'static str,
    source_text: &'static str,
    occurrence_id: &'static str,
    physical_region_id: &'static str,
    cell: Cell,
    source_indexed_sha256: &'static str,
    primitive_offsets: &'static [usize],
}

const LEAGUE_STANDING_LABELS: [LeagueStandingLabel; 2] = [
    LeagueStandingLabel {
        source_ui_id: "league_win_label",
        source_text: "勝",
        occurrence_id: "selp3-league-win-label",
        physical_region_id: "league-win-label-source-region",
        cell: Cell {
            x: 672,
            y: 96,
            width: 16,
            height: 16,
        },
        source_indexed_sha256: "f81d07dfd29b03ca6c37362bc7b9e99ba60be50a44e07262cf51f6146ac7e981",
        primitive_offsets: &[
            0x48cc, 0x48d0, 0x48d4, 0x48dc, 0x48f0, 0x48f4, 0x48f8, 0x48fc,
        ],
    },
    LeagueStandingLabel {
        source_ui_id: "league_loss_label",
        source_text: "敗",
        occurrence_id: "selp3-league-loss-label",
        physical_region_id: "league-loss-label-source-region",
        cell: Cell {
            x: 688,
            y: 96,
            width: 16,
            height: 16,
        },
        source_indexed_sha256: "092befc80a970042615d7d1bfc47d8c6b9c764de37bbe82b9d11bb010f87792a",
        primitive_offsets: &[
            0x4b20, 0x4b24, 0x4b28, 0x4b30, 0x4b44, 0x4b48, 0x4b4c, 0x4b50,
        ],
    },
];

pub(super) struct LeagueStandingsPlan {
    pub(super) fixed_strips: Vec<CharacterSelectFixedStripAllocation>,
    pub(super) occurrences: Vec<CharacterSelectRouteOccurrence>,
}

pub(super) fn plan_league_standings(
    shared_source_decoded: &[u8],
    localized_sources: &[CharacterSelectLocalizedSource],
) -> Result<LeagueStandingsPlan> {
    let requested = LEAGUE_STANDING_LABELS
        .iter()
        .filter_map(|spec| {
            localized_sources
                .iter()
                .find(|entry| entry.source_ui_id == spec.source_ui_id)
                .map(|entry| (spec, entry))
        })
        .collect::<Vec<_>>();
    if requested.is_empty() {
        return Ok(LeagueStandingsPlan {
            fixed_strips: Vec::new(),
            occurrences: Vec::new(),
        });
    }
    ensure!(
        requested.len() == LEAGUE_STANDING_LABELS.len(),
        "league standings translations must be supplied as one complete label pair"
    );

    let mut fixed_strips = Vec::with_capacity(requested.len());
    let mut occurrences = Vec::with_capacity(requested.len());
    for (spec, entry) in requested {
        ensure!(
            entry.source_text == spec.source_text,
            "league standings {} source identity changed",
            spec.source_ui_id
        );
        ensure!(
            entry.korean_text.chars().count() == 1,
            "league standings {} must remain one visible glyph",
            spec.source_ui_id
        );
        let source_pixels =
            read_indexed_cell_in_prefix(shared_source_decoded, SHARED_ATLAS_OFFSET, spec.cell)?;
        let source_sha256 = sha256_bytes(&source_pixels);
        ensure!(
            source_sha256 == spec.source_indexed_sha256,
            "league standings {} source cell changed: found {source_sha256}",
            spec.source_ui_id
        );

        fixed_strips.push(CharacterSelectFixedStripAllocation {
            physical_text_region_id: spec.physical_region_id.to_string(),
            source_ui_ids: vec![spec.source_ui_id.to_string()],
            translation_id: entry.translation_id.clone(),
            text_selection: CharacterSelectTextSelection::Entire,
            text_flow: CharacterSelectTextFlow::Horizontal,
            write_mode: CharacterSelectFixedStripWriteMode::ReplaceRegion,
            font_role: CharacterSelectFontRole::LeagueStandingLabel,
            surface: CharacterSelectTextureSurface::LeagueStandingsAtlas,
            tim_offset: SHARED_ATLAS_OFFSET,
            cell: spec.cell,
            text_cell: spec.cell,
            clear_index: 0,
        });
        occurrences.push(bound_occurrence(
            spec.occurrence_id,
            "selp3_league_standings",
            [spec.source_ui_id],
            vec![producer_target(
                SOURCE_RECORD,
                Some(SHARED_ATLAS_OFFSET),
                Some(CharacterSelectTextureSurface::LeagueStandingsAtlas),
                [spec.physical_region_id],
            )],
            vec![consumer_target(
                SOURCE_RECORD,
                CONSUMER_RECORD,
                CharacterSelectConsumerReferenceKind::PrimitiveSetup,
                spec.primitive_offsets.iter().copied(),
                [RUNTIME_STATE],
            )],
        ));
    }

    Ok(LeagueStandingsPlan {
        fixed_strips,
        occurrences,
    })
}
