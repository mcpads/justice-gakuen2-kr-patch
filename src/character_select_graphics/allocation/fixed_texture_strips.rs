//! Source-owned atlas strips consumed without a PLSEL descriptor rewrite.

use anyhow::{Context, Result, ensure};

use crate::tim::{Cell, read_indexed_cell_in_prefix};

use super::{SHARED_ATLAS_OFFSET, cells_overlap};
use crate::character_select_graphics::model::{
    CharacterSelectConsumerReferenceKind, CharacterSelectFixedStripAllocation,
    CharacterSelectFontRole, CharacterSelectGlyphAllocation, CharacterSelectLocalizedSource,
    CharacterSelectRouteOccurrence, CharacterSelectTextureSurface,
};
use crate::character_select_graphics::resource_loads::resource_load_byte_offsets;
use crate::character_select_graphics::route_census::{
    bound_occurrence, consumer_target, producer_target,
};

#[path = "roster_names.rs"]
mod roster_names;
#[path = "shared_prompts.rs"]
mod shared_prompts;

use roster_names::ROSTER_NAME_STRIPS;
use shared_prompts::PROMPT_STRIPS;

#[derive(Clone, Copy)]
pub(super) struct FixedStripSpec {
    primary_source_ui_id: &'static str,
    additional_source_ui_ids: &'static [&'static str],
    source_text: &'static str,
    font_role: CharacterSelectFontRole,
    surface: CharacterSelectTextureSurface,
    texture_page_index: u8,
    texture_uv: [u8; 2],
    cell: Cell,
    preserve_source_background: bool,
}

impl FixedStripSpec {
    fn source_ui_ids(self) -> impl Iterator<Item = &'static str> {
        std::iter::once(self.primary_source_ui_id)
            .chain(self.additional_source_ui_ids.iter().copied())
    }
}

const fn strip(
    source_ui_id: &'static str,
    source_text: &'static str,
    font_role: CharacterSelectFontRole,
    x: usize,
    y: usize,
    width: usize,
) -> FixedStripSpec {
    FixedStripSpec {
        primary_source_ui_id: source_ui_id,
        additional_source_ui_ids: &[],
        source_text,
        font_role,
        surface: CharacterSelectTextureSurface::SharedFixedStripAtlas,
        texture_page_index: (x / 256) as u8,
        texture_uv: [(x % 256) as u8, y as u8],
        cell: Cell {
            x,
            y,
            width,
            height: 20,
        },
        preserve_source_background: false,
    }
}

pub(super) const fn roster_strip(
    source_ui_id: &'static str,
    source_text: &'static str,
    texture_page_index: u8,
    texture_u: u8,
    texture_v: u8,
    width: usize,
    height: usize,
) -> FixedStripSpec {
    FixedStripSpec {
        primary_source_ui_id: source_ui_id,
        additional_source_ui_ids: &[],
        source_text,
        font_role: CharacterSelectFontRole::RosterName,
        surface: CharacterSelectTextureSurface::SharedFixedStripAtlas,
        texture_page_index,
        texture_uv: [texture_u, texture_v],
        cell: Cell {
            x: texture_page_index as usize * 256 + texture_u as usize,
            y: texture_v as usize,
            width,
            height,
        },
        preserve_source_background: false,
    }
}

pub(super) const fn prompt_strip(
    primary_source_ui_id: &'static str,
    additional_source_ui_ids: &'static [&'static str],
    source_text: &'static str,
    font_role: CharacterSelectFontRole,
    placement: FixedStripPlacement,
) -> FixedStripSpec {
    FixedStripSpec {
        primary_source_ui_id,
        additional_source_ui_ids,
        source_text,
        font_role,
        surface: placement.surface,
        texture_page_index: placement.texture_page_index,
        texture_uv: placement.texture_uv,
        cell: Cell {
            x: placement.texture_page_index as usize * 256 + placement.texture_uv[0] as usize,
            y: placement.texture_uv[1] as usize,
            width: placement.size[0],
            height: placement.size[1],
        },
        preserve_source_background: placement.preserve_source_background,
    }
}

#[derive(Clone, Copy)]
pub(super) struct FixedStripPlacement {
    pub(super) surface: CharacterSelectTextureSurface,
    pub(super) texture_page_index: u8,
    pub(super) texture_uv: [u8; 2],
    pub(super) size: [usize; 2],
    pub(super) preserve_source_background: bool,
}

const GENERAL_FIXED_STRIPS: &[FixedStripSpec] = &[
    strip(
        "protagonist_label",
        "主役",
        CharacterSelectFontRole::Label,
        0,
        220,
        40,
    ),
    strip(
        "partner_label",
        "相棒",
        CharacterSelectFontRole::Label,
        40,
        220,
        40,
    ),
];

pub(super) struct FixedTextureStripPlan {
    pub(super) allocations: Vec<CharacterSelectFixedStripAllocation>,
    pub(super) occurrences: Vec<CharacterSelectRouteOccurrence>,
}

pub(super) fn plan_fixed_texture_strips(
    source_decoded: &[u8],
    localized_sources: &[CharacterSelectLocalizedSource],
    roster_names_required: bool,
) -> Result<FixedTextureStripPlan> {
    if roster_names_required {
        for spec in ROSTER_NAME_STRIPS {
            ensure!(
                localized_sources
                    .iter()
                    .any(|entry| entry.source_ui_id == spec.primary_source_ui_id),
                "required roster-name source binding {} is missing from the translation assets",
                spec.primary_source_ui_id
            );
        }
    }
    let mut allocations = Vec::new();
    let mut occurrences = Vec::new();
    for spec in GENERAL_FIXED_STRIPS
        .iter()
        .chain(PROMPT_STRIPS)
        .chain(ROSTER_NAME_STRIPS)
    {
        let entries = localized_sources
            .iter()
            .filter(|entry| spec.source_ui_ids().any(|id| id == entry.source_ui_id))
            .collect::<Vec<_>>();
        if entries.is_empty() {
            continue;
        }
        for entry in &entries {
            ensure!(
                entry.source_text == spec.source_text,
                "fixed character-select strip {} changed its source identity",
                entry.source_ui_id
            );
        }
        let translation_id = entries[0].translation_id.as_str();
        ensure!(
            entries
                .iter()
                .all(|entry| entry.translation_id == translation_id),
            "shared fixed character-select strip {} has conflicting translations",
            spec.primary_source_ui_id
        );
        ensure!(
            spec.cell.x
                == usize::from(spec.texture_page_index) * 256 + usize::from(spec.texture_uv[0])
                && spec.cell.y == usize::from(spec.texture_uv[1])
                && usize::from(spec.texture_uv[0]) + spec.cell.width <= 256
                && usize::from(spec.texture_uv[1]) + spec.cell.height <= 256,
            "fixed character-select strip {} escaped its source texture-page consumer cell",
            spec.primary_source_ui_id
        );
        let source_pixels =
            read_indexed_cell_in_prefix(source_decoded, SHARED_ATLAS_OFFSET, spec.cell)?;
        ensure!(
            source_pixels.iter().any(|pixel| *pixel != 0),
            "fixed character-select strip {} lost its source ink",
            spec.primary_source_ui_id
        );
        ensure!(
            allocations
                .iter()
                .all(|existing: &CharacterSelectFixedStripAllocation| {
                    !cells_overlap(existing.cell, spec.cell)
                }),
            "fixed character-select strip {} overlaps another owned strip",
            spec.primary_source_ui_id
        );
        let clear_index = if spec.preserve_source_background {
            dominant_index(&source_pixels)
        } else {
            0
        };
        allocations.push(CharacterSelectFixedStripAllocation {
            physical_text_region_id: spec.primary_source_ui_id.to_string(),
            source_ui_ids: entries
                .into_iter()
                .map(|entry| entry.source_ui_id.clone())
                .collect(),
            translation_id: translation_id.to_string(),
            text_selection: super::super::model::CharacterSelectTextSelection::Entire,
            text_flow: super::super::model::CharacterSelectTextFlow::Horizontal,
            write_mode: super::super::model::CharacterSelectFixedStripWriteMode::ReplaceRegion,
            font_role: spec.font_role,
            surface: spec.surface,
            tim_offset: SHARED_ATLAS_OFFSET,
            cell: spec.cell,
            text_cell: spec.cell,
            clear_index,
        });
        occurrences.push(fixed_strip_occurrence(
            spec,
            &allocations.last().unwrap().source_ui_ids,
        )?);
    }
    Ok(FixedTextureStripPlan {
        allocations,
        occurrences,
    })
}

fn fixed_strip_occurrence(
    spec: &FixedStripSpec,
    source_ui_ids: &[String],
) -> Result<CharacterSelectRouteOccurrence> {
    let (producer_family, producer_records, consumer_records): (&str, &[&str], &[&str]) =
        match spec.surface {
            CharacterSelectTextureSurface::SharedFixedStripAtlas => (
                "selp_shared_fixed_strip",
                &[
                    "DAT2/SELP1.BIZ",
                    "DAT2/SELP2.BIZ",
                    "DAT2/SELP3.BIZ",
                    "DAT2/SELP4.BIZ",
                    "DAT2/SELP5.BIZ",
                ],
                &[
                    "DAT1/PLSEL1.BIN",
                    "DAT1/PLSEL2.BIN",
                    "DAT1/PLSEL3.BIN",
                    "DAT1/PLSEL4.BIN",
                    "DAT1/PLSEL5.BIN",
                ],
            ),
            CharacterSelectTextureSurface::LeagueTournamentPromptAtlas => (
                "selp3_4_shared_prompt_strip",
                &["DAT2/SELP3.BIZ", "DAT2/SELP4.BIZ"],
                &["DAT1/PLSEL3.BIN", "DAT1/PLSEL4.BIN"],
            ),
            CharacterSelectTextureSurface::TournamentPromptAtlas => (
                "selp4_tournament_prompt_strip",
                &["DAT2/SELP4.BIZ"],
                &["DAT1/PLSEL4.BIN"],
            ),
            _ => unreachable!("general fixed-strip planner owns only SELP strip surfaces"),
        };
    ensure!(
        producer_records.len() == consumer_records.len(),
        "fixed character-select strip {} has mismatched producer and consumer records",
        spec.primary_source_ui_id
    );
    let consumer_targets = producer_records
        .iter()
        .zip(consumer_records)
        .map(|(producer_record, consumer_record)| {
            let byte_offsets = resource_load_byte_offsets(producer_record, consumer_record)
                .with_context(|| {
                    format!(
                        "fixed character-select strip {} lost its {} to {} resource-load relation",
                        spec.primary_source_ui_id, producer_record, consumer_record
                    )
                })?;
            Ok(consumer_target(
                *producer_record,
                *consumer_record,
                CharacterSelectConsumerReferenceKind::ResourceLoad,
                byte_offsets.iter().copied(),
                [spec.primary_source_ui_id],
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(bound_occurrence(
        format!("fixed:{}", spec.primary_source_ui_id),
        producer_family,
        source_ui_ids.iter().cloned(),
        producer_records
            .iter()
            .map(|record| {
                producer_target(
                    *record,
                    Some(SHARED_ATLAS_OFFSET),
                    Some(spec.surface),
                    [spec.primary_source_ui_id],
                )
            })
            .collect(),
        consumer_targets,
    ))
}

pub(super) fn validate_fixed_texture_strip_conflicts(
    fixed_strips: &[CharacterSelectFixedStripAllocation],
    glyph_allocations: &[CharacterSelectGlyphAllocation],
) -> Result<()> {
    for (index, strip) in fixed_strips.iter().enumerate() {
        ensure!(
            fixed_strips[index + 1..].iter().all(|other| {
                strip.tim_offset != other.tim_offset
                    || !strip.surface.shares_physical_texture(other.surface)
                    || !cells_overlap(strip.cell, other.cell)
            }),
            "fixed character-select physical text region {} overlaps another fixed text region",
            strip.physical_text_region_id
        );
        ensure!(
            glyph_allocations.iter().all(|glyph| {
                !glyph.surface.shares_physical_texture(strip.surface)
                    || !cells_overlap(glyph.cell, strip.cell)
            }),
            "fixed character-select strip {} overlaps a dynamic glyph allocation",
            strip.source_ui_ids.join(", ")
        );
    }
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
        .expect("a fixed strip has pixels")
}
