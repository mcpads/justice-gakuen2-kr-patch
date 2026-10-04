//! Source-owned SOLO story-introduction lines in OP01.BIZ.

use anyhow::{Context, Result, ensure};

use crate::character_select_graphics::model::{
    CharacterSelectConsumerReferenceKind, CharacterSelectFixedStripWriteMode,
    CharacterSelectFontRole, CharacterSelectLocalizedSource, CharacterSelectRouteOccurrence,
    CharacterSelectSegmentedFixedStripAllocation, CharacterSelectTextFlow,
    CharacterSelectTextSelection, CharacterSelectTextureSurface,
};
use crate::character_select_graphics::resource_loads::resource_load_byte_offsets;
use crate::character_select_graphics::route_census::{
    bound_occurrence, consumer_target, producer_target,
};
use crate::character_select_graphics::texture_targets::{OP01_PATH, SOLO_STORY_INTRO_TIM_OFFSET};
use crate::pipeline::sha256_bytes;
use crate::tim::read_indexed_cell_in_prefix;

#[path = "solo_story_intro/source_layout.rs"]
mod source_layout;

use source_layout::{
    STORY_INTRO_ASSETS, StoryIntroAssetSpec, StoryIntroLineSpec, TOTAL_STORY_INTRO_LINES,
};

pub(super) struct SoloStoryIntroPlan {
    pub(super) segmented_fixed_strips: Vec<CharacterSelectSegmentedFixedStripAllocation>,
    pub(super) occurrences: Vec<CharacterSelectRouteOccurrence>,
}

pub(super) fn plan_solo_story_intro(
    source_decoded: Option<&[u8]>,
    localized_sources: &[CharacterSelectLocalizedSource],
) -> Result<SoloStoryIntroPlan> {
    let selected_assets = STORY_INTRO_ASSETS
        .iter()
        .filter_map(|spec| {
            localized_sources
                .iter()
                .find(|entry| entry.source_ui_id == spec.source_ui_id)
                .map(|entry| (spec, entry))
        })
        .collect::<Vec<_>>();
    let selected_source_ui_ids = selected_assets
        .iter()
        .map(|(spec, _)| spec.source_ui_id)
        .collect::<Vec<_>>();
    validate_complete_story_intro_translation(&selected_source_ui_ids)?;
    if selected_assets.is_empty() {
        return Ok(SoloStoryIntroPlan {
            segmented_fixed_strips: Vec::new(),
            occurrences: Vec::new(),
        });
    }

    let source_decoded = source_decoded.context("OP01.BIZ is required by solo story intro")?;
    validate_source_tim(source_decoded)?;
    validate_all_source_segments(source_decoded)?;

    let mut segmented_fixed_strips = Vec::new();
    for (spec, entry) in &selected_assets {
        ensure!(
            entry.source_text == spec.source_text,
            "solo story-intro source identity changed for {}",
            spec.source_ui_id
        );
        ensure!(
            entry.korean_text.lines().count() == spec.lines.len()
                && entry
                    .korean_text
                    .lines()
                    .all(|line| !line.trim().is_empty()),
            "solo story-intro translation {} must contain exactly {} non-empty timed lines",
            spec.source_ui_id,
            spec.lines.len()
        );
        segmented_fixed_strips.extend(spec.lines.iter().enumerate().map(|(line_index, line)| {
            CharacterSelectSegmentedFixedStripAllocation {
                logical_text_region_id: line.logical_text_region_id.to_string(),
                source_ui_ids: vec![spec.source_ui_id.to_string()],
                translation_id: entry.translation_id.clone(),
                text_selection: CharacterSelectTextSelection::Line { index: line_index },
                text_flow: CharacterSelectTextFlow::HorizontalLeft,
                write_mode: CharacterSelectFixedStripWriteMode::ReplaceRegion,
                font_role: CharacterSelectFontRole::SoloStoryIntro,
                surface: CharacterSelectTextureSurface::SoloStoryIntroAtlas,
                tim_offset: SOLO_STORY_INTRO_TIM_OFFSET,
                logical_width: line.logical_width,
                logical_height: 24,
                tracking_px: 0.0,
                horizontal_scale: 1,
                segments: line
                    .segments
                    .iter()
                    .map(|segment| segment.segment)
                    .collect(),
                clear_index: 1,
            }
        }));
    }

    Ok(SoloStoryIntroPlan {
        segmented_fixed_strips,
        occurrences: vec![story_intro_occurrence(&selected_assets)?],
    })
}

fn validate_complete_story_intro_translation(selected_source_ui_ids: &[&str]) -> Result<()> {
    if selected_source_ui_ids.is_empty() {
        return Ok(());
    }
    let missing = STORY_INTRO_ASSETS
        .iter()
        .filter(|asset| !selected_source_ui_ids.contains(&asset.source_ui_id))
        .map(|asset| asset.source_ui_id)
        .collect::<Vec<_>>();
    ensure!(
        missing.is_empty(),
        "solo story-intro translation is missing timed source units: {}",
        missing.join(", ")
    );
    Ok(())
}

fn validate_all_source_segments(source_decoded: &[u8]) -> Result<()> {
    for asset in &STORY_INTRO_ASSETS {
        for line in asset.lines {
            validate_line_segments(source_decoded, *line)?;
        }
    }
    Ok(())
}

fn validate_line_segments(source_decoded: &[u8], line: StoryIntroLineSpec) -> Result<()> {
    let mut next_logical_x = 0usize;
    for spec in line.segments {
        ensure!(
            spec.segment.logical_origin == [next_logical_x, 0] && spec.segment.cell.height == 24,
            "solo story-intro logical line {} has a discontinuous segment layout",
            line.logical_text_region_id
        );
        let indexed = read_indexed_cell_in_prefix(
            source_decoded,
            SOLO_STORY_INTRO_TIM_OFFSET,
            spec.segment.cell,
        )?;
        let source_sha256 = sha256_bytes(&indexed);
        ensure!(
            source_sha256 == spec.source_indexed_sha256,
            "solo story-intro source region {} changed: found {source_sha256}",
            spec.segment.physical_text_region_id
        );
        next_logical_x += spec.segment.cell.width;
    }
    ensure!(
        next_logical_x == line.logical_width,
        "solo story-intro logical line {} owns {}px but consumes {}px",
        line.logical_text_region_id,
        line.logical_width,
        next_logical_x
    );
    Ok(())
}

fn story_intro_occurrence(
    selected_assets: &[(&StoryIntroAssetSpec, &CharacterSelectLocalizedSource)],
) -> Result<CharacterSelectRouteOccurrence> {
    let resource_load_offsets = resource_load_byte_offsets(OP01_PATH, "DAT1/ODEMO.BIN")
        .context("solo story intro lost its OP01 resource-load relation")?;
    let mut line_indices = selected_assets
        .iter()
        .flat_map(|(asset, _)| asset.lines.iter().map(|line| line.layout_index))
        .collect::<Vec<_>>();
    line_indices.sort_unstable();
    line_indices.dedup();

    let mut primitive_layout_offsets = vec![0x0010];
    primitive_layout_offsets.extend(line_indices.iter().map(|index| 0x0012 + index * 8));
    primitive_layout_offsets.extend([
        0x03a8, 0x0450, 0x048c, 0x0528, 0x0554, 0x05b8, 0x0660, 0x0664,
    ]);

    Ok(bound_occurrence(
        "solo-progressive-story-intro",
        "op01_progressive_story_text",
        selected_assets.iter().map(|(asset, _)| asset.source_ui_id),
        vec![producer_target(
            OP01_PATH,
            Some(SOLO_STORY_INTRO_TIM_OFFSET),
            Some(CharacterSelectTextureSurface::SoloStoryIntroAtlas),
            selected_assets.iter().flat_map(|(asset, _)| {
                asset.lines.iter().flat_map(|line| {
                    line.segments
                        .iter()
                        .map(|segment| segment.segment.physical_text_region_id)
                })
            }),
        )],
        vec![
            consumer_target(
                OP01_PATH,
                "DAT1/ODEMO.BIN",
                CharacterSelectConsumerReferenceKind::ResourceLoad,
                resource_load_offsets.iter().copied(),
                ["solo-story-intro/op01-resource-load"],
            ),
            consumer_target(
                OP01_PATH,
                "DAT1/ODEMO.BIN",
                CharacterSelectConsumerReferenceKind::PrimitiveLayout,
                primitive_layout_offsets,
                [layout_runtime_state(&line_indices)?],
            ),
        ],
    ))
}

fn layout_runtime_state(line_indices: &[usize]) -> Result<String> {
    let first = *line_indices
        .first()
        .context("solo story intro has no selected timed line")?;
    let last = *line_indices.last().unwrap();
    ensure!(
        line_indices
            .iter()
            .all(|index| *index < TOTAL_STORY_INTRO_LINES),
        "solo story-intro timed-line index exceeds the source layout"
    );
    let selected_lines = if line_indices.windows(2).all(|pair| pair[1] == pair[0] + 1) {
        format!("{first}-{last}")
    } else {
        line_indices
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join("_")
    };
    Ok(format!(
        "solo-story-intro/layout-0/lines-{selected_lines}-of-{TOTAL_STORY_INTRO_LINES}"
    ))
}

fn validate_source_tim(source_decoded: &[u8]) -> Result<()> {
    let tim = crate::tim::parse_4bpp_prefix(
        source_decoded
            .get(SOLO_STORY_INTRO_TIM_OFFSET..)
            .context("OP01 story-intro TIM offset is outside the decoded source")?,
    )?;
    ensure!(
        tim.pixel_width() == 512
            && tim.image_height == 256
            && tim.image_x == 768
            && tim.image_y == 256
            && tim.clut_x == 0
            && tim.clut_y == 500
            && tim.clut_width == 16
            && tim.clut_height == 1
            && tim.total_size == 0x10040,
        "OP01 story-intro TIM geometry changed"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{STORY_INTRO_ASSETS, validate_complete_story_intro_translation};

    #[test]
    fn complete_story_intro_translation_cannot_omit_later_timed_lines() {
        let error = validate_complete_story_intro_translation(&["solo_story_intro"]).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("solo_story_intro_target_justice_academy")
        );

        let all_source_ui_ids = STORY_INTRO_ASSETS
            .iter()
            .map(|asset| asset.source_ui_id)
            .collect::<Vec<_>>();
        validate_complete_story_intro_translation(&all_source_ui_ids).unwrap();
    }
}
