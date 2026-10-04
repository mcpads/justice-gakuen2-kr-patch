//! Source-owned pause menu shared by practical-exam overlays.

use std::collections::BTreeMap;

use anyhow::{Context, Result, ensure};

use crate::character_select_graphics::model::{
    CharacterSelectConsumerReferenceKind, CharacterSelectFixedStripAllocation,
    CharacterSelectFixedStripSegment, CharacterSelectFixedStripWriteMode, CharacterSelectFontRole,
    CharacterSelectLocalizedSource, CharacterSelectRouteOccurrence,
    CharacterSelectSegmentedFixedStripAllocation, CharacterSelectTextFlow,
    CharacterSelectTextSelection, CharacterSelectTextureSurface,
};
use crate::character_select_graphics::pause_menu_consumer::{
    CONSUMER_RECORD, PRIMITIVE_LAYOUT_EVIDENCE_OFFSETS,
};
use crate::character_select_graphics::route_census::{
    bound_occurrence, consumer_target, producer_target,
};
use crate::character_select_graphics::texture_targets::{OVER_PATH, SOLO_STATE_PROMPT_TIM_OFFSET};
use crate::pipeline::sha256_bytes;
use crate::tim::{Cell, read_indexed_cell_in_prefix};

const HEADER_SOURCE_UI_ID: &str = "common_pause_header";
const CONTINUE_SOURCE_UI_ID: &str = "common_pause_continue";
const BACK_SOURCE_UI_ID: &str = "common_pause_back";
const SOURCE_UI_IDS: [&str; 3] = [
    HEADER_SOURCE_UI_ID,
    CONTINUE_SOURCE_UI_ID,
    BACK_SOURCE_UI_ID,
];

const HEADER_REGION_IDS: [&str; 4] = [
    "common-pause-header-digit",
    "common-pause-header-player",
    "common-pause-header-word",
    "common-pause-header-last",
];
const CONTINUE_REGION_IDS: [&str; 3] = [
    "common-pause-continue-first",
    "common-pause-continue-middle",
    "common-pause-shared-suffix",
];
const BACK_REGION_IDS: [&str; 2] = ["common-pause-back-first", "common-pause-shared-suffix"];

const MODE_RETURN_ID: &str = "common_pause_mode_return";
const MODE_RETURN_CELLS: [SourceCellSpec; 4] = [
    source_cell(
        "battle-pause-return-first",
        192,
        84,
        48,
        "2fb5d618dc17367174e3d146b210e5ab11fac4895635c6fe2b8c3460ada2925f",
    ),
    source_cell(
        "battle-pause-return-second",
        192,
        12,
        48,
        "242e8598905f79a1b86c58b7d2906e200b1c958f054ddf44a04622ca1133a4dd",
    ),
    source_cell(
        "battle-pause-return-third",
        216,
        72,
        24,
        "1b1df0fe431116ea3087b30ac0760e58a9fae77a1f892dd2770f1b0c59536ddf",
    ),
    source_cell(
        "battle-pause-return-last",
        192,
        96,
        24,
        "4efe41ba1fd1adbc9f55fc984fd390bc11b9e9bb20cfa696502c943f6938d155",
    ),
];

pub(crate) fn register_battle_pause_return(
    source: &[u8],
    build: &super::super::model::CharacterSelectAtlasBuild,
    plan: &mut crate::decoded_record_write_plan::DecodedRecordWritePlan<'_>,
) -> Result<()> {
    use crate::decoded_record_write_plan::DecodedDataClaim;
    let strips = build
        .report
        .fixed_strips
        .iter()
        .filter(|strip| strip.source_ui_ids.iter().any(|id| id == MODE_RETURN_ID))
        .collect::<Vec<_>>();
    if strips.is_empty() {
        return Ok(());
    }
    ensure!(
        strips.len() == MODE_RETURN_CELLS.len()
            && MODE_RETURN_CELLS
                .iter()
                .all(
                    |cell| strips.iter().any(|strip| strip.physical_text_region_id
                        == cell.physical_region_id
                        && strip.cell == cell.cell)
                ),
        "battle pause return consumer requires all composed texture segments"
    );
    let start = 0x7a89c;
    let original = [
        10, 8, 0, 7, 1, 1, 2, 1, 1, 7, 1, 0, 1, 2, 1, 6, 1, 1, 2, 1, 0, 8, 2, 0, 5, 1,
    ];
    ensure!(
        source.get(start..start + original.len()) == Some(&original),
        "battle pause return descriptor changed"
    );
    // Six 24-pixel units, four segments. The native renderer centers the row
    // from the first byte; its surrounding pointers and menu actions stay intact.
    let mut replacement = [0u8; 26];
    replacement[..14].copy_from_slice(&[6, 4, 0, 7, 2, 0, 1, 2, 1, 6, 1, 0, 8, 1]);
    let mut candidate = source.to_vec();
    candidate[start..start + 26].copy_from_slice(&replacement);
    plan.register_data_candidate(
        "battle pause return",
        crate::source_disc::MAIN_EXECUTABLE_SHA256,
        &candidate,
        &DecodedDataClaim::from_ranges(
            "battle-pause-return",
            "read complete Korean mode-return strip",
            [[start, start + 26]],
        ),
    )
}

#[derive(Clone, Copy)]
struct SourceCellSpec {
    physical_region_id: &'static str,
    cell: Cell,
    source_indexed_sha256: &'static str,
}

const SOURCE_CELLS: [SourceCellSpec; 8] = [
    source_cell(
        HEADER_REGION_IDS[0],
        192,
        0,
        24,
        "f3fb1be6d51f27742b4f93e8b160a211f89c1a38776cc9ecee8011d2bf362368",
    ),
    source_cell(
        HEADER_REGION_IDS[2],
        192,
        24,
        48,
        "a26e26e66aac199ebe56910a5d188d59da8a5a63c2c9be8abcb3d2f6fedf4b8a",
    ),
    source_cell(
        HEADER_REGION_IDS[3],
        192,
        36,
        24,
        "05ae8138d15a1e7cf58378d717cbdddbbc99ac19dbb4d645e8736a2e15f5fe87",
    ),
    source_cell(
        CONTINUE_REGION_IDS[1],
        192,
        48,
        48,
        "ca4203d5e1d6c5294f214fad72e871f195ffca2bdd31ccdb74aace7840f1d4f2",
    ),
    source_cell(
        CONTINUE_REGION_IDS[2],
        192,
        60,
        24,
        "e95949280446acfd6c2e8e42c2b3cc5dd3a635da360f57459dba74cbd1d64b0c",
    ),
    source_cell(
        HEADER_REGION_IDS[1],
        192,
        108,
        24,
        "5e4b49b75c833507952ccd877e60889145f3c777ae34fc451ce87b428d7edb82",
    ),
    source_cell(
        CONTINUE_REGION_IDS[0],
        216,
        36,
        24,
        "b4ef6f5a333695db74099c722a1c209dce5fa7ce3bdcee7d28420a432e8c2090",
    ),
    source_cell(
        BACK_REGION_IDS[0],
        216,
        96,
        24,
        "79e4fbd8469950f8948cc5820e0be26af7e98ba29f5697b53c1edaea73b97348",
    ),
];

const fn source_cell(
    physical_region_id: &'static str,
    x: usize,
    y: usize,
    width: usize,
    source_indexed_sha256: &'static str,
) -> SourceCellSpec {
    SourceCellSpec {
        physical_region_id,
        cell: Cell {
            x,
            y,
            width,
            height: 12,
        },
        source_indexed_sha256,
    }
}

pub(super) struct CommonPauseMenuPlan {
    pub(super) fixed_strips: Vec<CharacterSelectFixedStripAllocation>,
    pub(super) segmented_fixed_strips: Vec<CharacterSelectSegmentedFixedStripAllocation>,
    pub(super) occurrences: Vec<CharacterSelectRouteOccurrence>,
}

pub(super) fn plan_common_pause_menu(
    source_decoded: Option<&[u8]>,
    localized_sources: &[CharacterSelectLocalizedSource],
) -> Result<CommonPauseMenuPlan> {
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
        "common pause-menu translations must be supplied as one complete menu"
    );
    validate_translation_sources(&entries)?;
    let source_decoded = source_decoded.context("OVER.TIZ is required by the common pause menu")?;
    validate_source_tim(source_decoded)?;
    validate_source_cells(source_decoded)?;

    let header = entries[HEADER_SOURCE_UI_ID];
    let continue_entry = entries[CONTINUE_SOURCE_UI_ID];
    let back = entries[BACK_SOURCE_UI_ID];
    let fixed_strips = Vec::new();
    let mut segmented_fixed_strips = vec![
        segmented_strip(
            "common-pause-header",
            header,
            96,
            0.0,
            1,
            vec![
                segment(HEADER_REGION_IDS[1], SOURCE_CELLS[5].cell, 0),
                segment(HEADER_REGION_IDS[2], SOURCE_CELLS[1].cell, 24),
                segment(HEADER_REGION_IDS[3], SOURCE_CELLS[2].cell, 72),
            ],
        ),
        segmented_strip(
            "common-pause-continue",
            continue_entry,
            96,
            0.0,
            2,
            vec![
                segment(CONTINUE_REGION_IDS[0], SOURCE_CELLS[6].cell, 0),
                segment(CONTINUE_REGION_IDS[1], SOURCE_CELLS[3].cell, 24),
                segment(CONTINUE_REGION_IDS[2], SOURCE_CELLS[4].cell, 72),
            ],
        ),
        segmented_strip(
            "common-pause-back",
            back,
            48,
            0.0,
            2,
            vec![segment(BACK_REGION_IDS[0], SOURCE_CELLS[7].cell, 0)],
        ),
    ];

    // The native descriptor selects either the original 1 or 2 sprite before
    // this shared suffix. Baking the digit into it duplicates the 1 on 2P.
    segmented_fixed_strips[0].text_selection =
        CharacterSelectTextSelection::Suffix { skip_characters: 1 };
    segmented_fixed_strips[0].text_flow = CharacterSelectTextFlow::HorizontalLeft;

    let mut occurrences = vec![
        pause_occurrence(
            "sikeng22-common-pause-header",
            HEADER_SOURCE_UI_ID,
            &HEADER_REGION_IDS[1..],
        ),
        pause_occurrence(
            "sikeng22-common-pause-continue",
            CONTINUE_SOURCE_UI_ID,
            &CONTINUE_REGION_IDS,
        ),
        pause_occurrence(
            "sikeng22-common-pause-back",
            BACK_SOURCE_UI_ID,
            &BACK_REGION_IDS,
        ),
    ];
    if let Some(entry) = localized_sources
        .iter()
        .find(|entry| entry.source_ui_id == MODE_RETURN_ID)
    {
        ensure!(
            entry.source_text == "モードメニューに戻る",
            "battle pause return source changed"
        );
        for spec in MODE_RETURN_CELLS {
            let pixels = read_indexed_cell_in_prefix(source_decoded, 0, spec.cell)?;
            ensure!(
                sha256_bytes(&pixels) == spec.source_indexed_sha256,
                "battle pause return source cell changed"
            );
        }
        segmented_fixed_strips.push(segmented_strip(
            "common-pause-mode-return",
            entry,
            144,
            0.0,
            1,
            MODE_RETURN_CELLS
                .iter()
                .zip([0, 48, 96, 120])
                .map(|(spec, x)| segment(spec.physical_region_id, spec.cell, x))
                .collect(),
        ));
        occurrences.push(bound_occurrence(
            "battle-pause-mode-return",
            "over_common_pause_menu",
            [MODE_RETURN_ID],
            vec![producer_target(
                OVER_PATH,
                Some(0),
                Some(CharacterSelectTextureSurface::CommonPauseMenuAtlas),
                MODE_RETURN_CELLS.iter().map(|s| s.physical_region_id),
            )],
            vec![consumer_target(
                OVER_PATH,
                "SLPS_021.20",
                CharacterSelectConsumerReferenceKind::PrimitiveLayout,
                [0x7a89c, 0x7a89e],
                ["battle/pause/mode-return"],
            )],
        ));
    }
    Ok(CommonPauseMenuPlan {
        fixed_strips,
        segmented_fixed_strips,
        occurrences,
    })
}

fn empty_plan() -> CommonPauseMenuPlan {
    CommonPauseMenuPlan {
        fixed_strips: Vec::new(),
        segmented_fixed_strips: Vec::new(),
        occurrences: Vec::new(),
    }
}

fn segmented_strip(
    logical_text_region_id: &str,
    entry: &CharacterSelectLocalizedSource,
    logical_width: usize,
    tracking_px: f32,
    horizontal_scale: usize,
    segments: Vec<CharacterSelectFixedStripSegment>,
) -> CharacterSelectSegmentedFixedStripAllocation {
    CharacterSelectSegmentedFixedStripAllocation {
        logical_text_region_id: logical_text_region_id.to_string(),
        source_ui_ids: vec![entry.source_ui_id.clone()],
        translation_id: entry.translation_id.clone(),
        text_selection: CharacterSelectTextSelection::Entire,
        text_flow: CharacterSelectTextFlow::Horizontal,
        write_mode: CharacterSelectFixedStripWriteMode::ReplaceRegion,
        font_role: CharacterSelectFontRole::CommonPauseMenu,
        surface: CharacterSelectTextureSurface::CommonPauseMenuAtlas,
        tim_offset: SOLO_STATE_PROMPT_TIM_OFFSET,
        logical_width,
        logical_height: 12,
        tracking_px,
        horizontal_scale,
        segments,
        clear_index: 0,
    }
}

const fn segment(
    physical_text_region_id: &'static str,
    cell: Cell,
    logical_x: usize,
) -> CharacterSelectFixedStripSegment {
    CharacterSelectFixedStripSegment {
        physical_text_region_id,
        cell,
        logical_origin: [logical_x, 0],
    }
}

fn pause_occurrence(
    occurrence_id: &str,
    source_ui_id: &str,
    physical_region_ids: &[&str],
) -> CharacterSelectRouteOccurrence {
    bound_occurrence(
        occurrence_id,
        "over_common_pause_menu",
        [source_ui_id],
        vec![producer_target(
            OVER_PATH,
            Some(SOLO_STATE_PROMPT_TIM_OFFSET),
            Some(CharacterSelectTextureSurface::CommonPauseMenuAtlas),
            physical_region_ids.iter().copied(),
        )],
        vec![consumer_target(
            OVER_PATH,
            CONSUMER_RECORD,
            CharacterSelectConsumerReferenceKind::PrimitiveLayout,
            PRIMITIVE_LAYOUT_EVIDENCE_OFFSETS,
            ["practical-basics/pause", "first-term-exam/pause"],
        )],
    )
}

fn validate_translation_sources(
    entries: &BTreeMap<&str, &CharacterSelectLocalizedSource>,
) -> Result<()> {
    for (source_ui_id, source_text) in [
        (HEADER_SOURCE_UI_ID, "1Pポーズ"),
        (CONTINUE_SOURCE_UI_ID, "つづける"),
        (BACK_SOURCE_UI_ID, "戻る"),
    ] {
        ensure!(
            entries[source_ui_id].source_text == source_text,
            "common pause-menu source identity changed for {source_ui_id}"
        );
    }
    let header = &entries[HEADER_SOURCE_UI_ID].korean_text;
    ensure!(
        header.lines().count() == 1 && header.starts_with("1P ") && header.chars().count() > 3,
        "common pause header must be one non-empty line with its player prefix"
    );
    ensure!(
        entries[CONTINUE_SOURCE_UI_ID].korean_text.chars().count() == 4,
        "common pause continue label must occupy its four proven source slots"
    );
    ensure!(
        entries[BACK_SOURCE_UI_ID].korean_text.chars().count() == 2,
        "common pause back label must occupy its two proven source slots"
    );
    ensure!(
        entries[CONTINUE_SOURCE_UI_ID].korean_text.chars().nth(3)
            == entries[BACK_SOURCE_UI_ID].korean_text.chars().nth(1),
        "common pause labels must share the same final glyph in their shared source cell"
    );
    Ok(())
}

fn validate_source_tim(source_decoded: &[u8]) -> Result<()> {
    let tim = crate::tim::parse_4bpp_prefix(source_decoded)?;
    ensure!(
        tim.pixel_width() == 256
            && tim.image_height == 240
            && tim.image_x == 960
            && tim.image_y == 256
            && tim.clut_x == 0
            && tim.clut_y == 511
            && tim.clut_width == 464
            && tim.clut_height == 1
            && tim.total_size == source_decoded.len(),
        "OVER.TIZ common pause-menu TIM geometry changed"
    );
    Ok(())
}

fn validate_source_cells(source_decoded: &[u8]) -> Result<()> {
    for spec in SOURCE_CELLS {
        let source_pixels =
            read_indexed_cell_in_prefix(source_decoded, SOLO_STATE_PROMPT_TIM_OFFSET, spec.cell)?;
        let source_sha256 = sha256_bytes(&source_pixels);
        ensure!(
            source_sha256 == spec.source_indexed_sha256,
            "common pause-menu source cell {} changed: found {source_sha256}",
            spec.physical_region_id
        );
    }
    Ok(())
}
