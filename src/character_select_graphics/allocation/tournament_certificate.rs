//! Plans the decorated tournament certificate without baking its dynamic A-H team marker.

use std::collections::BTreeMap;

use anyhow::{Context, Result, ensure};

use crate::character_select_graphics::model::{
    CharacterSelectConsumerReferenceKind, CharacterSelectFixedStripAllocation,
    CharacterSelectFixedStripWriteMode, CharacterSelectFontRole, CharacterSelectLocalizedSource,
    CharacterSelectRouteOccurrence, CharacterSelectSourceInkCleanupAllocation,
    CharacterSelectTextFlow, CharacterSelectTextSelection, CharacterSelectTextureSurface,
};
use crate::character_select_graphics::route_census::{
    bound_occurrence, consumer_target, pending_occurrence, producer_target,
};
use crate::character_select_graphics::texture_targets::TOURNAMENT_CERTIFICATE_TIM_OFFSETS;
use crate::pipeline::sha256_bytes;
use crate::tim::{Cell, read_indexed_cell_in_prefix};

const TITLE_SOURCE_UI_ID: &str = "tournament_certificate_title";
const CHAMPION_SOURCE_UI_ID: &str = "tournament_champion_label";
const BODY_SOURCE_UI_ID: &str = "tournament_certificate_body";
const SOURCE_UI_IDS: [&str; 3] = [CHAMPION_SOURCE_UI_ID, TITLE_SOURCE_UI_ID, BODY_SOURCE_UI_ID];
const TITLE_SOURCE_TEXT: &str = "表彰状";
const CHAMPION_SOURCE_TEXT: &str = "優勝";
const BODY_SOURCE_TEXT: &str = "チーム\nあなたはトーナメント戦において優秀な成績をおさめられました\nよってここに表彰いたします\n私立ジャスティス学園\n学長 忌野雷蔵";
const CLEAR_INDEX: u8 = 15;
const FIRST_SOURCE_INK_INDEX: u8 = 0;
const LAST_SOURCE_INK_INDEX: u8 = 8;
const CONSUMED_CERTIFICATE_CELL: Cell = Cell {
    x: 0,
    y: 0,
    width: 446,
    height: 393,
};
const DYNAMIC_TEAM_MARKER_ATLAS_CELL: Cell = Cell {
    x: 448,
    y: 0,
    width: 32,
    height: 128,
};

const CHAMPION_LABEL_CELL: Cell = cell(165, 140, 70, 40);
const TEAM_SUFFIX_CELL: Cell = cell(275, 140, 35, 40);
// PLSEL4 places every certificate slice at (32,32). Its independent 16x32
// A-H sprite uses screen coordinates, so the paper origin must be added once.
const CERTIFICATE_SCREEN_ORIGIN: usize = 32;
const TEAM_MARKER_CELL: Cell = cell(
    (CHAMPION_LABEL_CELL.x + CHAMPION_LABEL_CELL.width + TEAM_SUFFIX_CELL.x - 16) / 2,
    CHAMPION_LABEL_CELL.y + (CHAMPION_LABEL_CELL.height - 32) / 2,
    16,
    32,
);
pub(in crate::character_select_graphics) const TEAM_MARKER_SCREEN_X: i16 =
    (CERTIFICATE_SCREEN_ORIGIN + TEAM_MARKER_CELL.x) as i16;
pub(in crate::character_select_graphics) const TEAM_MARKER_SCREEN_Y: i16 =
    (CERTIFICATE_SCREEN_ORIGIN + TEAM_MARKER_CELL.y) as i16;

struct SourceTimSpec {
    offset: usize,
    sha256: &'static str,
    height: usize,
}

const SOURCE_TIMS: [SourceTimSpec; 2] = [
    SourceTimSpec {
        offset: TOURNAMENT_CERTIFICATE_TIM_OFFSETS[0],
        sha256: "9cf925f12ae9b7c3fe8cb6c99f74dbe6f25e9292deaec38aaf908cc92639e1a2",
        height: 512,
    },
    SourceTimSpec {
        offset: TOURNAMENT_CERTIFICATE_TIM_OFFSETS[1],
        sha256: "2e7efa2fb6304a5b746a154cae20b3c03120da0ae134b3613fdda14862bb7c3c",
        height: 396,
    },
];

struct CleanupSpec {
    id: &'static str,
    cell: Cell,
    expected_source_ink_count: usize,
}

const CLEANUPS: [CleanupSpec; 9] = [
    cleanup("principal", 70, 80, 30, 270, 1_173),
    cleanup("school", 100, 80, 30, 270, 664),
    cleanup("body-left", 135, 75, 30, 275, 998),
    cleanup("body-mid-left", 165, 75, 30, 275, 540),
    cleanup("body-mid", 195, 75, 30, 275, 1_251),
    cleanup("body-mid-right", 225, 75, 30, 275, 1_966),
    cleanup("body-right", 255, 75, 30, 275, 1_258),
    cleanup("champion-team", 290, 100, 45, 250, 860),
    cleanup("title", 340, 90, 50, 180, 1_147),
];

const fn cleanup(
    id: &'static str,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    expected_source_ink_count: usize,
) -> CleanupSpec {
    CleanupSpec {
        id,
        cell: Cell {
            x,
            y,
            width,
            height,
        },
        expected_source_ink_count,
    }
}

struct TextSpec {
    id: &'static str,
    source_ui_id: &'static str,
    selection: CharacterSelectTextSelection,
    role: CharacterSelectFontRole,
    cell: Cell,
}

const TEXTS: [TextSpec; 8] = [
    text(
        "title",
        TITLE_SOURCE_UI_ID,
        CharacterSelectTextSelection::Entire,
        CharacterSelectFontRole::TournamentCertificateTitle,
        cell(145, 80, 230, 48),
    ),
    text(
        "champion",
        CHAMPION_SOURCE_UI_ID,
        CharacterSelectTextSelection::Entire,
        CharacterSelectFontRole::TournamentCertificateLabel,
        CHAMPION_LABEL_CELL,
    ),
    text(
        "team-suffix",
        BODY_SOURCE_UI_ID,
        CharacterSelectTextSelection::Line { index: 0 },
        CharacterSelectFontRole::TournamentCertificateLabel,
        TEAM_SUFFIX_CELL,
    ),
    text(
        "body-right",
        BODY_SOURCE_UI_ID,
        CharacterSelectTextSelection::Line { index: 1 },
        CharacterSelectFontRole::TournamentCertificateBody,
        cell(90, 195, 270, 30),
    ),
    text(
        "body-mid-right",
        BODY_SOURCE_UI_ID,
        CharacterSelectTextSelection::Line { index: 2 },
        CharacterSelectFontRole::TournamentCertificateBody,
        cell(75, 230, 300, 30),
    ),
    text(
        "body-mid",
        BODY_SOURCE_UI_ID,
        CharacterSelectTextSelection::Line { index: 3 },
        CharacterSelectFontRole::TournamentCertificateBody,
        cell(130, 265, 190, 30),
    ),
    text(
        "body-left",
        BODY_SOURCE_UI_ID,
        CharacterSelectTextSelection::Line { index: 4 },
        CharacterSelectFontRole::TournamentCertificateBody,
        cell(115, 295, 220, 30),
    ),
    text(
        "school-principal",
        BODY_SOURCE_UI_ID,
        CharacterSelectTextSelection::Line { index: 5 },
        CharacterSelectFontRole::TournamentCertificateBody,
        cell(105, 325, 240, 30),
    ),
];

const fn text(
    id: &'static str,
    source_ui_id: &'static str,
    selection: CharacterSelectTextSelection,
    role: CharacterSelectFontRole,
    cell: Cell,
) -> TextSpec {
    TextSpec {
        id,
        source_ui_id,
        selection,
        role,
        cell,
    }
}

const fn cell(x: usize, y: usize, width: usize, height: usize) -> Cell {
    Cell {
        x,
        y,
        width,
        height,
    }
}

pub(super) struct TournamentCertificatePlan {
    pub(super) fixed_strips: Vec<CharacterSelectFixedStripAllocation>,
    pub(super) source_ink_cleanups: Vec<CharacterSelectSourceInkCleanupAllocation>,
    pub(super) occurrences: Vec<CharacterSelectRouteOccurrence>,
}

pub(super) fn plan_tournament_certificate(
    source_decoded: Option<&[u8]>,
    localized_sources: &[CharacterSelectLocalizedSource],
) -> Result<TournamentCertificatePlan> {
    let entries = SOURCE_UI_IDS
        .iter()
        .filter_map(|source_ui_id| {
            localized_sources
                .iter()
                .find(|entry| entry.source_ui_id == *source_ui_id)
                .map(|entry| (*source_ui_id, entry))
        })
        .collect::<BTreeMap<_, _>>();
    if entries.is_empty() {
        return Ok(empty_plan());
    }
    ensure!(
        entries.len() == SOURCE_UI_IDS.len(),
        "tournament certificate translations are only partially present"
    );
    validate_translation_sources(&entries)?;
    let Some(source_decoded) = source_decoded else {
        return Ok(TournamentCertificatePlan {
            fixed_strips: Vec::new(),
            source_ink_cleanups: Vec::new(),
            occurrences: certificate_occurrences(false),
        });
    };
    validate_source_tims(source_decoded)?;

    let mut source_ink_cleanups = Vec::with_capacity(SOURCE_TIMS.len() * CLEANUPS.len());
    let mut fixed_strips = Vec::with_capacity(SOURCE_TIMS.len() * TEXTS.len());
    for tim in SOURCE_TIMS {
        for cleanup in CLEANUPS {
            source_ink_cleanups.push(CharacterSelectSourceInkCleanupAllocation {
                physical_region_id: physical_id(tim.offset, cleanup.id),
                surface: CharacterSelectTextureSurface::TournamentCertificate,
                tim_offset: tim.offset,
                cell: cleanup.cell,
                first_ink_index: FIRST_SOURCE_INK_INDEX,
                last_ink_index: LAST_SOURCE_INK_INDEX,
                replacement_index: CLEAR_INDEX,
                expected_source_ink_count: cleanup.expected_source_ink_count,
            });
        }
        for text in TEXTS {
            let entry = entries
                .get(text.source_ui_id)
                .context("tournament certificate translation disappeared")?;
            fixed_strips.push(CharacterSelectFixedStripAllocation {
                physical_text_region_id: physical_id(tim.offset, text.id),
                source_ui_ids: vec![text.source_ui_id.to_string()],
                translation_id: entry.translation_id.clone(),
                text_selection: text.selection,
                text_flow: CharacterSelectTextFlow::Horizontal,
                write_mode: CharacterSelectFixedStripWriteMode::OverlayNonClearPixels,
                font_role: text.role,
                surface: CharacterSelectTextureSurface::TournamentCertificate,
                tim_offset: tim.offset,
                cell: text.cell,
                text_cell: text.cell,
                clear_index: CLEAR_INDEX,
            });
        }
    }
    validate_plan_geometry(&fixed_strips, &source_ink_cleanups)?;
    Ok(TournamentCertificatePlan {
        fixed_strips,
        source_ink_cleanups,
        occurrences: certificate_occurrences(true),
    })
}

fn certificate_occurrences(bound: bool) -> Vec<CharacterSelectRouteOccurrence> {
    SOURCE_UI_IDS
        .into_iter()
        .map(|source_ui_id| {
            let physical_region_ids = SOURCE_TIMS
                .iter()
                .flat_map(|tim| {
                    TEXTS
                        .iter()
                        .filter(move |text| text.source_ui_id == source_ui_id)
                        .map(move |text| physical_id(tim.offset, text.id))
                })
                .collect::<Vec<_>>();
            let producer_targets = SOURCE_TIMS
                .iter()
                .map(|tim| {
                    producer_target(
                        "DAT2/TOROFY.BIZ",
                        Some(tim.offset),
                        Some(CharacterSelectTextureSurface::TournamentCertificate),
                        physical_region_ids
                            .iter()
                            .filter(|region_id| {
                                region_id.starts_with(&format!(
                                    "tournament-certificate-{:05x}-",
                                    tim.offset
                                ))
                            })
                            .cloned(),
                    )
                })
                .collect::<Vec<_>>();
            let consumer_targets = vec![consumer_target(
                "DAT2/TOROFY.BIZ",
                "DAT1/PLSEL4.BIN",
                CharacterSelectConsumerReferenceKind::PrimitiveLayout,
                [0x0780],
                ["tournament-award-certificate"],
            )];
            let occurrence_id = format!("torofy-certificate:{source_ui_id}");
            if bound {
                bound_occurrence(
                    occurrence_id,
                    "torofy_tournament_certificate",
                    [source_ui_id],
                    producer_targets,
                    consumer_targets,
                )
            } else {
                pending_occurrence(
                    occurrence_id,
                    "torofy_tournament_certificate",
                    [source_ui_id],
                    producer_targets,
                    consumer_targets,
                )
            }
        })
        .collect()
}

fn validate_plan_geometry(
    fixed_strips: &[CharacterSelectFixedStripAllocation],
    cleanups: &[CharacterSelectSourceInkCleanupAllocation],
) -> Result<()> {
    for (index, left) in cleanups.iter().enumerate() {
        ensure!(
            cleanups[index + 1..].iter().all(|right| {
                left.tim_offset != right.tim_offset
                    || !crate::tim::cells_overlap(left.cell, right.cell)
            }),
            "TOROFY source-ink cleanup regions overlap"
        );
    }
    for (index, strip) in fixed_strips.iter().enumerate() {
        ensure!(
            !crate::tim::cells_overlap(strip.cell, TEAM_MARKER_CELL),
            "TOROFY text overlaps the dynamic team marker"
        );
        ensure!(
            cell_contains(CONSUMED_CERTIFICATE_CELL, strip.cell),
            "TOROFY Korean text region {} escaped the fixed certificate",
            strip.physical_text_region_id
        );
        ensure!(
            fixed_strips[index + 1..].iter().all(|right| {
                strip.tim_offset != right.tim_offset
                    || !crate::tim::cells_overlap(strip.cell, right.cell)
            }),
            "TOROFY Korean text regions overlap"
        );
    }
    Ok(())
}

fn validate_translation_sources(
    entries: &BTreeMap<&str, &CharacterSelectLocalizedSource>,
) -> Result<()> {
    for (source_ui_id, expected_source) in [
        (TITLE_SOURCE_UI_ID, TITLE_SOURCE_TEXT),
        (CHAMPION_SOURCE_UI_ID, CHAMPION_SOURCE_TEXT),
        (BODY_SOURCE_UI_ID, BODY_SOURCE_TEXT),
    ] {
        ensure!(
            entries
                .get(source_ui_id)
                .context("tournament certificate source entry disappeared")?
                .source_text
                == expected_source,
            "tournament certificate source identity changed for {source_ui_id}"
        );
    }
    ensure!(
        entries[BODY_SOURCE_UI_ID].korean_text.lines().count() == 6,
        "tournament certificate body must contain the team suffix and five body lines"
    );
    Ok(())
}

fn validate_source_tims(source_decoded: &[u8]) -> Result<()> {
    for spec in SOURCE_TIMS {
        let source = source_decoded
            .get(spec.offset..)
            .context("TOROFY certificate TIM offset moved")?;
        let tim = crate::tim::parse_4bpp_prefix(source)?;
        ensure!(
            tim.pixel_width() == 512
                && tim.image_height == spec.height
                && tim.image_x == 512
                && tim.image_y == 0
                && tim.clut_x == 0
                && tim.clut_y == 483
                && tim.clut_width * tim.clut_height / 16 == 1,
            "TOROFY certificate TIM geometry changed at +0x{:05x}",
            spec.offset
        );
        ensure!(
            sha256_bytes(&source[..tim.total_size]) == spec.sha256,
            "TOROFY certificate source TIM identity changed at +0x{:05x}",
            spec.offset
        );
    }
    ensure!(
        read_indexed_cell_in_prefix(
            source_decoded,
            SOURCE_TIMS[0].offset,
            CONSUMED_CERTIFICATE_CELL,
        )? == read_indexed_cell_in_prefix(
            source_decoded,
            SOURCE_TIMS[1].offset,
            CONSUMED_CERTIFICATE_CELL,
        )?,
        "TOROFY certificate source copies diverged"
    );
    ensure!(
        read_indexed_cell_in_prefix(
            source_decoded,
            SOURCE_TIMS[0].offset,
            DYNAMIC_TEAM_MARKER_ATLAS_CELL,
        )? == read_indexed_cell_in_prefix(
            source_decoded,
            SOURCE_TIMS[1].offset,
            DYNAMIC_TEAM_MARKER_ATLAS_CELL,
        )?,
        "TOROFY A-H dynamic team marker atlases diverged"
    );
    Ok(())
}

fn cell_contains(outer: Cell, inner: Cell) -> bool {
    inner.x >= outer.x
        && inner.y >= outer.y
        && inner.x + inner.width <= outer.x + outer.width
        && inner.y + inner.height <= outer.y + outer.height
}

fn physical_id(tim_offset: usize, region: &str) -> String {
    format!("tournament-certificate-{tim_offset:05x}-{region}")
}

fn empty_plan() -> TournamentCertificatePlan {
    TournamentCertificatePlan {
        fixed_strips: Vec::new(),
        source_ink_cleanups: Vec::new(),
        occurrences: Vec::new(),
    }
}
