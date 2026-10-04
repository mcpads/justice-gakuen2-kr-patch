//! Source-owned SOLO state prompts in the common OVER.TIZ texture.

use anyhow::{Context, Result, ensure};

use crate::character_select_graphics::model::{
    CharacterSelectConsumerReferenceKind, CharacterSelectFixedStripAllocation,
    CharacterSelectFixedStripWriteMode, CharacterSelectFontRole, CharacterSelectLocalizedSource,
    CharacterSelectRouteOccurrence, CharacterSelectTextFlow, CharacterSelectTextSelection,
    CharacterSelectTextureSurface,
};
use crate::character_select_graphics::route_census::{
    bound_occurrence, consumer_target, producer_target,
};
use crate::character_select_graphics::texture_targets::{OVER_PATH, SOLO_STATE_PROMPT_TIM_OFFSET};
use crate::pipeline::sha256_bytes;
use crate::tim::{Cell, read_indexed_cell_in_prefix};

const CONSUMER_RECORD: &str = "SLPS_021.20";

#[derive(Clone, Copy)]
struct SoloStatePrompt {
    source_ui_id: &'static str,
    source_text: &'static str,
    occurrence_id: &'static str,
    physical_region_id: &'static str,
    cell: Cell,
    source_indexed_sha256: &'static str,
    primitive_offsets: &'static [usize],
    runtime_state: &'static str,
}

const COMMON_PRIMITIVE_OFFSETS: [usize; 6] = [
    0x0a438, // state setter entry at runtime 0x80019c38
    0x0a46c, // common prompt constructor at runtime 0x80019c6c
    0x0a5fc, // descriptor loads at runtime 0x80019dfc
    0x0a61c, // UV write at runtime 0x80019e1c
    0x0a66c, // XY and size write at runtime 0x80019e6c
    0x0a684, // ordering-table insertion at runtime 0x80019e84
];

const CHALLENGE_PRIMITIVE_OFFSETS: [usize; 12] = [
    0x07a128, // state 6 descriptor: UV (0, 100), size delta (88, 11)
    0x09d38,
    0x09d70, // state 6 immediate
    0x09d74, // state setter call
    0x09de0, // state 6 fallback immediate in the press-start selector
    0x09de4, // shared selector state setter call
    COMMON_PRIMITIVE_OFFSETS[0],
    COMMON_PRIMITIVE_OFFSETS[1],
    COMMON_PRIMITIVE_OFFSETS[2],
    COMMON_PRIMITIVE_OFFSETS[3],
    COMMON_PRIMITIVE_OFFSETS[4],
    COMMON_PRIMITIVE_OFFSETS[5],
];

const PRESS_START_PRIMITIVE_OFFSETS: [usize; 12] = [
    0x07a114, // state 1 descriptor: UV (0, 116), size delta (112, 11)
    0x07a124, // state 5 aliases the same descriptor
    0x09d98,
    0x09db8, // state 5 immediate
    0x09dd0, // state 1 immediate
    0x09de4, // state setter call
    COMMON_PRIMITIVE_OFFSETS[0],
    COMMON_PRIMITIVE_OFFSETS[1],
    COMMON_PRIMITIVE_OFFSETS[2],
    COMMON_PRIMITIVE_OFFSETS[3],
    COMMON_PRIMITIVE_OFFSETS[4],
    COMMON_PRIMITIVE_OFFSETS[5],
];

const WAIT_PRIMITIVE_OFFSETS: [usize; 10] = [
    0x07a130, // state 8 descriptor: UV (0, 196), size delta (80, 11)
    0x0a068,
    0x0a090, // state setter call
    0x0a094, // state 8 immediate in the call delay slot
    COMMON_PRIMITIVE_OFFSETS[0],
    COMMON_PRIMITIVE_OFFSETS[1],
    COMMON_PRIMITIVE_OFFSETS[2],
    COMMON_PRIMITIVE_OFFSETS[3],
    COMMON_PRIMITIVE_OFFSETS[4],
    COMMON_PRIMITIVE_OFFSETS[5],
];

const SOLO_STATE_PROMPTS: [SoloStatePrompt; 3] = [
    SoloStatePrompt {
        source_ui_id: "solo_challenge_prompt",
        source_text: "かかって来なさい!",
        occurrence_id: "selp1-solo-challenge-prompt",
        physical_region_id: "solo-challenge-source-region",
        cell: Cell {
            x: 0,
            y: 100,
            width: 88,
            height: 12,
        },
        source_indexed_sha256: "ed12b483d469429e7a59a9ab4f5f516d01361afff48621a3b975a3ddb2123d3d",
        primitive_offsets: &CHALLENGE_PRIMITIVE_OFFSETS,
        runtime_state: "solo-select/prompt-state-6/challenge",
    },
    SoloStatePrompt {
        source_ui_id: "solo_press_start_prompt",
        source_text: "スタートボタンを押して!",
        occurrence_id: "selp1-solo-press-start-prompt",
        physical_region_id: "solo-press-start-source-region",
        cell: Cell {
            x: 0,
            y: 116,
            width: 112,
            height: 12,
        },
        source_indexed_sha256: "ad796b7e12d53c27d7df39c4633d2c3b99ff9ce48468b307137d2720ced21dd3",
        primitive_offsets: &PRESS_START_PRIMITIVE_OFFSETS,
        runtime_state: "solo-select/prompt-state-5-or-1/press-start",
    },
    SoloStatePrompt {
        source_ui_id: "solo_wait_prompt",
        source_text: "ちょっと待って!",
        occurrence_id: "selp1-solo-wait-prompt",
        physical_region_id: "solo-wait-source-region",
        cell: Cell {
            x: 0,
            y: 196,
            width: 80,
            height: 12,
        },
        source_indexed_sha256: "b55a10f2807147e67a18ecfd60f1d9dbb31ee4c52409c9ce9a7fcb49d455a179",
        primitive_offsets: &WAIT_PRIMITIVE_OFFSETS,
        runtime_state: "solo-select/prompt-state-8/wait",
    },
];

pub(super) struct SoloStatePromptPlan {
    pub(super) fixed_strips: Vec<CharacterSelectFixedStripAllocation>,
    pub(super) occurrences: Vec<CharacterSelectRouteOccurrence>,
}

pub(super) fn plan_solo_state_prompts(
    source_decoded: Option<&[u8]>,
    localized_sources: &[CharacterSelectLocalizedSource],
) -> Result<SoloStatePromptPlan> {
    let requested = SOLO_STATE_PROMPTS
        .iter()
        .filter_map(|spec| {
            localized_sources
                .iter()
                .find(|entry| entry.source_ui_id == spec.source_ui_id)
                .map(|entry| (spec, entry))
        })
        .collect::<Vec<_>>();
    if requested.is_empty() {
        return Ok(SoloStatePromptPlan {
            fixed_strips: Vec::new(),
            occurrences: Vec::new(),
        });
    }
    ensure!(
        requested.len() == SOLO_STATE_PROMPTS.len(),
        "solo state-prompt translations must be supplied as one complete state set"
    );
    let source_decoded = source_decoded.context("OVER.TIZ is required by solo state prompts")?;
    validate_source_tim(source_decoded)?;

    let mut fixed_strips = Vec::with_capacity(requested.len());
    let mut occurrences = Vec::with_capacity(requested.len());
    for (spec, entry) in requested {
        ensure!(
            entry.source_text == spec.source_text,
            "solo state prompt {} source identity changed",
            spec.source_ui_id
        );
        ensure!(
            !entry.korean_text.trim().is_empty() && entry.korean_text.lines().count() == 1,
            "solo state prompt {} must remain one line",
            spec.source_ui_id
        );
        let source_pixels =
            read_indexed_cell_in_prefix(source_decoded, SOLO_STATE_PROMPT_TIM_OFFSET, spec.cell)?;
        let source_sha256 = sha256_bytes(&source_pixels);
        ensure!(
            source_sha256 == spec.source_indexed_sha256,
            "solo state prompt {} source cell changed: found {source_sha256}",
            spec.source_ui_id
        );
        fixed_strips.push(CharacterSelectFixedStripAllocation {
            physical_text_region_id: spec.physical_region_id.to_string(),
            source_ui_ids: vec![spec.source_ui_id.to_string()],
            translation_id: entry.translation_id.clone(),
            text_selection: CharacterSelectTextSelection::Entire,
            text_flow: CharacterSelectTextFlow::Horizontal,
            write_mode: CharacterSelectFixedStripWriteMode::ReplaceRegion,
            font_role: CharacterSelectFontRole::SoloStatePrompt,
            surface: CharacterSelectTextureSurface::SoloStatePromptAtlas,
            tim_offset: SOLO_STATE_PROMPT_TIM_OFFSET,
            cell: spec.cell,
            text_cell: spec.cell,
            clear_index: 0,
        });
        occurrences.push(bound_occurrence(
            spec.occurrence_id,
            "over_solo_state_prompts",
            [spec.source_ui_id],
            vec![producer_target(
                OVER_PATH,
                Some(SOLO_STATE_PROMPT_TIM_OFFSET),
                Some(CharacterSelectTextureSurface::SoloStatePromptAtlas),
                [spec.physical_region_id],
            )],
            vec![consumer_target(
                OVER_PATH,
                CONSUMER_RECORD,
                CharacterSelectConsumerReferenceKind::PrimitiveSetup,
                spec.primitive_offsets.iter().copied(),
                [spec.runtime_state],
            )],
        ));
    }
    Ok(SoloStatePromptPlan {
        fixed_strips,
        occurrences,
    })
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
        "OVER.TIZ solo prompt TIM geometry changed: image={}x{} at ({}, {}), CLUT={}x{} at ({}, {}), TIM size={:#x}, decoded size={:#x}",
        tim.pixel_width(),
        tim.image_height,
        tim.image_x,
        tim.image_y,
        tim.clut_width,
        tim.clut_height,
        tim.clut_x,
        tim.clut_y,
        tim.total_size,
        source_decoded.len()
    );
    Ok(())
}
