//! Source-owned SOLO episode cards in the indexed TITLE.BIN streams.

use anyhow::{Context, Result, ensure};

use crate::character_select_graphics::episode_card_consumer::{
    CONSUMER_RECORD, PRIMITIVE_LAYOUT_EVIDENCE_OFFSETS, TITLE_MEMBER_SELECTOR_RAM_OFFSET,
};
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
use crate::character_select_graphics::texture_targets::TITLE_PATH;
use crate::pipeline::sha256_bytes;
use crate::tim::read_indexed_cell_in_prefix;

#[path = "solo_episode_card/source_layout.rs"]
mod source_layout;

use source_layout::{EPISODE_CARDS, EpisodeCardLineSpec, EpisodeCardSpec};

pub(super) struct SoloEpisodeCardsPlan {
    pub(super) segmented_fixed_strips: Vec<CharacterSelectSegmentedFixedStripAllocation>,
    pub(super) occurrences: Vec<CharacterSelectRouteOccurrence>,
}

pub(super) fn plan_solo_episode_cards(
    source_decoded: Option<&[u8]>,
    localized_sources: &[CharacterSelectLocalizedSource],
) -> Result<SoloEpisodeCardsPlan> {
    let selected_cards = EPISODE_CARDS
        .iter()
        .filter_map(|spec| {
            localized_sources
                .iter()
                .find(|entry| entry.source_ui_id == spec.source_ui_id)
                .map(|entry| (spec, entry))
        })
        .collect::<Vec<_>>();
    let selected_source_ui_ids = selected_cards
        .iter()
        .map(|(spec, _)| spec.source_ui_id)
        .collect::<Vec<_>>();
    validate_complete_episode_card_translation(&selected_source_ui_ids)?;
    if selected_cards.is_empty() {
        return Ok(SoloEpisodeCardsPlan {
            segmented_fixed_strips: Vec::new(),
            occurrences: Vec::new(),
        });
    }

    let source_decoded = source_decoded.context("TITLE.BIN is required by solo episode cards")?;
    validate_source_layout(source_decoded)?;

    let mut segmented_fixed_strips = Vec::new();
    let mut occurrences = Vec::with_capacity(selected_cards.len());
    for (spec, entry) in selected_cards {
        ensure!(
            entry.source_text == spec.source_text,
            "solo episode-card source identity changed for {}",
            spec.source_ui_id
        );
        validated_korean_lines(spec.source_ui_id, &entry.korean_text)?;
        segmented_fixed_strips.extend(spec.lines.iter().enumerate().map(|(line_index, line)| {
            CharacterSelectSegmentedFixedStripAllocation {
                logical_text_region_id: line.logical_text_region_id.to_string(),
                source_ui_ids: vec![spec.source_ui_id.to_string()],
                translation_id: entry.translation_id.clone(),
                text_selection: CharacterSelectTextSelection::Line { index: line_index },
                text_flow: CharacterSelectTextFlow::Horizontal,
                write_mode: CharacterSelectFixedStripWriteMode::ReplaceRegion,
                font_role: CharacterSelectFontRole::SoloEpisodeCard,
                surface: CharacterSelectTextureSurface::SoloEpisodeCardAtlas,
                tim_offset: spec.tim_offset,
                logical_width: line.logical_width,
                logical_height: line.logical_height,
                tracking_px: 0.0,
                horizontal_scale: 1,
                segments: line.iter_segments().collect(),
                clear_index: 15,
            }
        }));
        occurrences.push(episode_card_occurrence(spec)?);
    }

    Ok(SoloEpisodeCardsPlan {
        segmented_fixed_strips,
        occurrences,
    })
}

fn validate_complete_episode_card_translation(selected_source_ui_ids: &[&str]) -> Result<()> {
    if selected_source_ui_ids.is_empty() {
        return Ok(());
    }
    let missing = EPISODE_CARDS
        .iter()
        .filter(|card| !selected_source_ui_ids.contains(&card.source_ui_id))
        .map(|card| card.source_ui_id)
        .collect::<Vec<_>>();
    ensure!(
        missing.is_empty(),
        "solo episode-card translation is missing indexed source members: {}",
        missing.join(", ")
    );
    Ok(())
}

fn validated_korean_lines<'a>(source_ui_id: &str, korean_text: &'a str) -> Result<[&'a str; 3]> {
    let lines = korean_text.lines().collect::<Vec<_>>();
    ensure!(
        lines.len() == 3 && lines.iter().all(|line| !line.trim().is_empty()),
        "solo episode-card translation {source_ui_id} must contain exactly three non-empty semantic lines"
    );
    Ok(lines
        .try_into()
        .expect("validated three episode-card lines"))
}

fn validate_source_layout(source_decoded: &[u8]) -> Result<()> {
    let mut expected_offset = 0usize;
    for spec in &EPISODE_CARDS {
        ensure!(
            spec.tim_offset == expected_offset,
            "TITLE.BIN episode-card decoded members are not contiguous at member {}",
            spec.member_index
        );
        let expected_tim_size = 0x40 + 256 * spec.image_height / 2;
        let tim = crate::tim::parse_4bpp_prefix(
            source_decoded
                .get(spec.tim_offset..)
                .context("TITLE.BIN episode-card TIM offset exceeds the decoded source")?,
        )?;
        ensure!(
            tim.pixel_width() == 256
                && tim.image_height == spec.image_height
                && tim.image_x == 512
                && tim.image_y == 0
                && tim.clut_x == 0
                && tim.clut_y == 480
                && tim.clut_width == 16
                && tim.clut_height == 1
                && tim.total_size == expected_tim_size,
            "TITLE.BIN episode-card member {} TIM geometry changed",
            spec.member_index
        );
        for line in spec.lines {
            validate_line_segments(source_decoded, spec, line)?;
        }
        expected_offset += expected_tim_size;
    }
    ensure!(
        expected_offset == source_decoded.len(),
        "TITLE.BIN episode-card decoded members do not cover the declared source"
    );
    Ok(())
}

fn validate_line_segments(
    source_decoded: &[u8],
    card: &EpisodeCardSpec,
    line: &EpisodeCardLineSpec,
) -> Result<()> {
    let mut next_logical_x = 0usize;
    for spec in line.segments {
        ensure!(
            spec.segment.logical_origin == [next_logical_x, 0]
                && spec.segment.cell.height == line.logical_height,
            "solo episode-card region {} has a discontinuous segment layout",
            line.logical_text_region_id
        );
        let indexed =
            read_indexed_cell_in_prefix(source_decoded, card.tim_offset, spec.segment.cell)?;
        let source_sha256 = sha256_bytes(&indexed);
        ensure!(
            source_sha256 == spec.source_indexed_sha256,
            "solo episode-card source region {} changed: found {source_sha256}",
            spec.segment.physical_text_region_id
        );
        next_logical_x += spec.segment.cell.width;
    }
    ensure!(
        next_logical_x == line.logical_width,
        "solo episode-card region {} owns {}px but consumes {}px",
        line.logical_text_region_id,
        line.logical_width,
        next_logical_x
    );
    Ok(())
}

fn episode_card_occurrence(spec: &EpisodeCardSpec) -> Result<CharacterSelectRouteOccurrence> {
    let resource_load_offsets = resource_load_byte_offsets(TITLE_PATH, CONSUMER_RECORD)
        .context("solo episode cards lost their TITLE.BIN resource-load relation")?;
    let member_state = format!("solo-story/episode-card/title-member-{}", spec.member_index);
    let selector_state =
        format!("{member_state}/loader-selector-ram-0x{TITLE_MEMBER_SELECTOR_RAM_OFFSET:06x}");
    Ok(bound_occurrence(
        format!("cdemo-solo-episode-card-{}", spec.member_index + 1),
        format!("title_member_{}_solo_episode_card", spec.member_index),
        [spec.source_ui_id],
        vec![producer_target(
            TITLE_PATH,
            Some(spec.tim_offset),
            Some(CharacterSelectTextureSurface::SoloEpisodeCardAtlas),
            spec.lines.iter().flat_map(|line| {
                line.segments
                    .iter()
                    .map(|segment| segment.segment.physical_text_region_id)
            }),
        )],
        vec![
            consumer_target(
                TITLE_PATH,
                CONSUMER_RECORD,
                CharacterSelectConsumerReferenceKind::ResourceLoad,
                resource_load_offsets.iter().copied(),
                [selector_state],
            ),
            consumer_target(
                TITLE_PATH,
                CONSUMER_RECORD,
                CharacterSelectConsumerReferenceKind::PrimitiveLayout,
                PRIMITIVE_LAYOUT_EVIDENCE_OFFSETS,
                [format!(
                    "{member_state}/source-layout-reference-not-presentation-proof"
                )],
            ),
        ],
    ))
}

impl EpisodeCardLineSpec {
    fn iter_segments(
        &self,
    ) -> impl Iterator<
        Item = crate::character_select_graphics::model::CharacterSelectFixedStripSegment,
    > + '_ {
        self.segments.iter().map(|segment| segment.segment)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        EPISODE_CARDS, validate_complete_episode_card_translation, validated_korean_lines,
    };

    #[test]
    fn episode_card_translation_keeps_its_three_semantic_lines() {
        assert!(
            validated_korean_lines(
                "solo_episode_card_1",
                "제1화\n단서를 가진 자\n태양학원 고등부 옥상에서"
            )
            .is_ok()
        );
        assert!(validated_korean_lines("solo_episode_card_1", "제1화\n단서를 가진 자").is_err());
        assert!(
            validated_korean_lines("solo_episode_card_1", "제1화\n\n태양학원 고등부 옥상에서")
                .is_err()
        );
    }

    #[test]
    fn indexed_episode_card_family_cannot_be_partially_translated() {
        let all_ids = EPISODE_CARDS
            .iter()
            .map(|card| card.source_ui_id)
            .collect::<Vec<_>>();
        assert!(validate_complete_episode_card_translation(&[]).is_ok());
        assert!(validate_complete_episode_card_translation(&all_ids).is_ok());
        assert!(validate_complete_episode_card_translation(&all_ids[..7]).is_err());
    }
}
