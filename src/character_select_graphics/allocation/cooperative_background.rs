//! Preserves the cooperative background art while replacing its two text regions.

use anyhow::{Context, Result, ensure};

use crate::character_select_graphics::model::{
    CharacterSelectConsumerReferenceKind, CharacterSelectFixedStripAllocation,
    CharacterSelectFixedStripWriteMode, CharacterSelectFontRole, CharacterSelectLocalizedSource,
    CharacterSelectRouteOccurrence, CharacterSelectTextFlow, CharacterSelectTextSelection,
    CharacterSelectTextureSurface,
};
use crate::character_select_graphics::resource_loads::resource_load_byte_offsets;
use crate::character_select_graphics::route_census::{
    bound_occurrence, consumer_target, pending_occurrence, producer_target,
};
use crate::character_select_graphics::texture_targets::COOPERATIVE_BACKGROUND_TIM_OFFSET;
use crate::pipeline::sha256_bytes;
use crate::tim::{Cell, read_indexed_cell_in_prefix};

const SOURCE_TIM_SHA256: &str = "bf0ad6e23e1d2eea7c1c5aa0831142cab20d1f40ffb0b1a90ea151fa7c649026";
const SOURCE_UI_ID: &str = "cooperative_background_gedo_emblem";
const SOURCE_TEXT: &str = "外道";
const CLEAR_INDEX: u8 = 10;

struct TextRegionSpec {
    physical_text_region_id: &'static str,
    translation_character_index: usize,
    cell: Cell,
    text_cell: Cell,
    source_bright_ink_count: usize,
}

const TEXT_REGIONS: [TextRegionSpec; 2] = [
    TextRegionSpec {
        physical_text_region_id: "cooperative-background-emblem-left-character",
        translation_character_index: 0,
        cell: Cell {
            x: 2,
            y: 0,
            width: 31,
            height: 96,
        },
        text_cell: Cell {
            x: 2,
            y: 27,
            width: 31,
            height: 41,
        },
        source_bright_ink_count: 602,
    },
    TextRegionSpec {
        physical_text_region_id: "cooperative-background-emblem-right-character",
        translation_character_index: 1,
        cell: Cell {
            x: 109,
            y: 0,
            width: 33,
            height: 96,
        },
        text_cell: Cell {
            x: 109,
            y: 27,
            width: 33,
            height: 41,
        },
        source_bright_ink_count: 788,
    },
];

pub(super) struct CooperativeBackgroundPlan {
    pub(super) fixed_strips: Vec<CharacterSelectFixedStripAllocation>,
    pub(super) occurrences: Vec<CharacterSelectRouteOccurrence>,
}

pub(super) fn plan_cooperative_background(
    source_decoded: Option<&[u8]>,
    localized_sources: &[CharacterSelectLocalizedSource],
) -> Result<CooperativeBackgroundPlan> {
    let Some(entry) = localized_sources
        .iter()
        .find(|entry| entry.source_ui_id == SOURCE_UI_ID)
    else {
        return Ok(empty_plan());
    };
    ensure!(
        entry.source_text == SOURCE_TEXT,
        "cooperative background source identity changed"
    );
    ensure!(
        entry.korean_text.chars().count() == TEXT_REGIONS.len(),
        "cooperative background translation must have exactly two characters"
    );
    let Some(source_decoded) = source_decoded else {
        return Ok(CooperativeBackgroundPlan {
            fixed_strips: Vec::new(),
            occurrences: vec![background_occurrence(false)?],
        });
    };
    validate_source_tim(source_decoded)?;

    let mut fixed_strips = Vec::with_capacity(TEXT_REGIONS.len());
    for spec in TEXT_REGIONS {
        validate_source_text_region(source_decoded, &spec)?;
        fixed_strips.push(CharacterSelectFixedStripAllocation {
            physical_text_region_id: spec.physical_text_region_id.to_string(),
            source_ui_ids: vec![SOURCE_UI_ID.to_string()],
            translation_id: entry.translation_id.clone(),
            text_selection: CharacterSelectTextSelection::Character {
                index: spec.translation_character_index,
            },
            text_flow: CharacterSelectTextFlow::Horizontal,
            write_mode: CharacterSelectFixedStripWriteMode::ReplaceRegion,
            font_role: CharacterSelectFontRole::CooperativeEmblemCharacter,
            surface: CharacterSelectTextureSurface::CooperativeBackgroundEmblem,
            tim_offset: COOPERATIVE_BACKGROUND_TIM_OFFSET,
            cell: spec.cell,
            text_cell: spec.text_cell,
            clear_index: CLEAR_INDEX,
        });
    }
    Ok(CooperativeBackgroundPlan {
        fixed_strips,
        occurrences: vec![background_occurrence(true)?],
    })
}

fn background_occurrence(bound: bool) -> Result<CharacterSelectRouteOccurrence> {
    let producer_targets = vec![producer_target(
        "DAT2/SELP5.BIZ",
        Some(COOPERATIVE_BACKGROUND_TIM_OFFSET),
        Some(CharacterSelectTextureSurface::CooperativeBackgroundEmblem),
        TEXT_REGIONS.map(|region| region.physical_text_region_id),
    )];
    let byte_offsets = resource_load_byte_offsets("DAT2/SELP5.BIZ", "DAT1/PLSEL5.BIN")
        .context("cooperative background lost its SELP5 resource-load relation")?;
    let consumer_targets = vec![consumer_target(
        "DAT2/SELP5.BIZ",
        "DAT1/PLSEL5.BIN",
        CharacterSelectConsumerReferenceKind::ResourceLoad,
        byte_offsets.iter().copied(),
        ["cooperative-character-select-background"],
    )];
    if bound {
        Ok(bound_occurrence(
            "selp5-cooperative-background-emblem",
            "selp5_cooperative_background_emblem",
            [SOURCE_UI_ID],
            producer_targets,
            consumer_targets,
        ))
    } else {
        Ok(pending_occurrence(
            "selp5-cooperative-background-emblem",
            "selp5_cooperative_background_emblem",
            [SOURCE_UI_ID],
            producer_targets,
            consumer_targets,
        ))
    }
}

fn empty_plan() -> CooperativeBackgroundPlan {
    CooperativeBackgroundPlan {
        fixed_strips: Vec::new(),
        occurrences: Vec::new(),
    }
}

fn validate_source_tim(source_decoded: &[u8]) -> Result<()> {
    let source = source_decoded
        .get(COOPERATIVE_BACKGROUND_TIM_OFFSET..)
        .context("SELP5 cooperative background TIM offset moved")?;
    let tim = crate::tim::parse_4bpp_prefix(source)?;
    ensure!(
        tim.pixel_width() == 144
            && tim.image_height == 96
            && tim.image_x == 896
            && tim.image_y == 256
            && tim.clut_x == 0
            && tim.clut_y == 510
            && tim.clut_width * tim.clut_height / 16 == 1,
        "SELP5 cooperative background TIM geometry changed"
    );
    ensure!(
        sha256_bytes(&source[..tim.total_size]) == SOURCE_TIM_SHA256,
        "SELP5 cooperative background source TIM identity changed"
    );
    Ok(())
}

fn validate_source_text_region(source_decoded: &[u8], spec: &TextRegionSpec) -> Result<()> {
    let pixels =
        read_indexed_cell_in_prefix(source_decoded, COOPERATIVE_BACKGROUND_TIM_OFFSET, spec.cell)?;
    ensure!(
        pixels.iter().filter(|pixel| **pixel >= 12).count() == spec.source_bright_ink_count,
        "SELP5 cooperative background text region {} changed",
        spec.physical_text_region_id
    );
    ensure!(
        dominant_index(&pixels) == CLEAR_INDEX,
        "SELP5 cooperative background text region {} changed its background index",
        spec.physical_text_region_id
    );
    Ok(())
}

fn dominant_index(pixels: &[u8]) -> u8 {
    let mut counts = [0usize; 16];
    for &pixel in pixels {
        counts[usize::from(pixel)] += 1;
    }
    counts
        .iter()
        .enumerate()
        .max_by_key(|(_, count)| *count)
        .map(|(index, _)| index as u8)
        .expect("a cooperative background text region has pixels")
}
