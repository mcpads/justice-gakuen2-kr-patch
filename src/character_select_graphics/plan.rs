use std::path::Path;

use anyhow::{Context, Result, bail, ensure};

use super::allocation::{SHARED_ATLAS_OFFSET, plan_character_select_atlas_from_assets};
use super::assets::load_character_select_translation_assets;
use super::model::{CharacterSelectAtlasPlanConfig, CharacterSelectAtlasPlanReport};
use super::planning_sources::CharacterSelectPlanningSources;
use super::source::{
    load_character_select_auxiliary_source_from_disc, load_character_select_source_from_disc,
};
use crate::source_disc::SupportedSourceDisc;

pub fn write_character_select_atlas_plan(
    config: &CharacterSelectAtlasPlanConfig,
) -> Result<CharacterSelectAtlasPlanReport> {
    prepare_output(&config.output, config.force)?;
    let source = SupportedSourceDisc::open(&config.cue)?;
    let (source_bin_sha256, records) = load_character_select_source_from_disc(&source)?;
    let auxiliary = load_character_select_auxiliary_source_from_disc(&source)?;
    let planning_sources =
        CharacterSelectPlanningSources::from_loaded_records(&records, &auxiliary)?;
    let assets = load_character_select_translation_assets(&config.assets)?;
    let atlas = plan_character_select_atlas_from_assets(&planning_sources, &assets)?;
    let shared_atlas_identical_across_records = records.iter().all(|record| {
        shared_atlas_bytes(&record.decoded)
            .map(|bytes| crate::pipeline::sha256_bytes(bytes) == atlas.source_shared_atlas_sha256)
            .unwrap_or(false)
    });
    ensure!(
        shared_atlas_identical_across_records,
        "SELP1 through SELP5 no longer share the planned character-select atlas"
    );
    let report = CharacterSelectAtlasPlanReport {
        kind: "justice_gakuen2_character_select_atlas_source_plan".to_string(),
        source_bin_sha256,
        build_spec_sha256: config.build_spec_sha256.clone(),
        source_record_count: records.len(),
        shared_atlas_identical_across_records,
        select_heading_font: config.fonts.select_heading.path.clone(),
        select_heading_font_px: config.fonts.select_heading.font_px,
        select_heading_vertical_shift_px: config.fonts.select_heading.vertical_shift_px,
        mode_menu_heading_font: config.fonts.mode_menu_heading.path.clone(),
        mode_menu_heading_font_px: config.fonts.mode_menu_heading.font_px,
        mode_menu_heading_vertical_shift_px: config.fonts.mode_menu_heading.vertical_shift_px,
        mode_menu_label_font: config.fonts.mode_menu_label.path.clone(),
        mode_menu_label_font_px: config.fonts.mode_menu_label.font_px,
        mode_menu_label_vertical_shift_px: config.fonts.mode_menu_label.vertical_shift_px,
        cooperative_emblem_character_font: config.fonts.cooperative_emblem_character.path.clone(),
        cooperative_emblem_character_font_px: config.fonts.cooperative_emblem_character.font_px,
        cooperative_emblem_character_vertical_shift_px: config
            .fonts
            .cooperative_emblem_character
            .vertical_shift_px,
        tournament_bracket_label_font: config.fonts.tournament_bracket_label.path.clone(),
        tournament_bracket_label_font_px: config.fonts.tournament_bracket_label.font_px,
        tournament_bracket_label_vertical_shift_px: config
            .fonts
            .tournament_bracket_label
            .vertical_shift_px,
        tournament_certificate_title_font: config.fonts.tournament_certificate_title.path.clone(),
        tournament_certificate_title_font_px: config.fonts.tournament_certificate_title.font_px,
        tournament_certificate_title_vertical_shift_px: config
            .fonts
            .tournament_certificate_title
            .vertical_shift_px,
        tournament_certificate_label_font: config.fonts.tournament_certificate_label.path.clone(),
        tournament_certificate_label_font_px: config.fonts.tournament_certificate_label.font_px,
        tournament_certificate_label_vertical_shift_px: config
            .fonts
            .tournament_certificate_label
            .vertical_shift_px,
        tournament_certificate_body_font: config.fonts.tournament_certificate_body.path.clone(),
        tournament_certificate_body_font_px: config.fonts.tournament_certificate_body.font_px,
        tournament_certificate_body_vertical_shift_px: config
            .fonts
            .tournament_certificate_body
            .vertical_shift_px,
        label_font: config.fonts.label.path.clone(),
        label_font_px: config.fonts.label.font_px,
        label_vertical_shift_px: config.fonts.label.vertical_shift_px,
        roster_name_font: config.fonts.roster_name.path.clone(),
        roster_name_font_px: config.fonts.roster_name.font_px,
        roster_name_vertical_shift_px: config.fonts.roster_name.vertical_shift_px,
        fixed_prompt_font: config.fonts.fixed_prompt.path.clone(),
        fixed_prompt_font_px: config.fonts.fixed_prompt.font_px,
        fixed_prompt_vertical_shift_px: config.fonts.fixed_prompt.vertical_shift_px,
        compact_prompt_font: config.fonts.compact_prompt.path.clone(),
        compact_prompt_font_px: config.fonts.compact_prompt.font_px,
        compact_prompt_vertical_shift_px: config.fonts.compact_prompt.vertical_shift_px,
        solo_state_prompt_font: config.fonts.solo_state_prompt.path.clone(),
        solo_state_prompt_font_px: config.fonts.solo_state_prompt.font_px,
        solo_state_prompt_vertical_shift_px: config.fonts.solo_state_prompt.vertical_shift_px,
        common_pause_menu_font: config.fonts.common_pause_menu.path.clone(),
        common_pause_menu_font_px: config.fonts.common_pause_menu.font_px,
        common_pause_menu_vertical_shift_px: config.fonts.common_pause_menu.vertical_shift_px,
        solo_story_intro_font: config.fonts.solo_story_intro.path.clone(),
        solo_story_intro_font_px: config.fonts.solo_story_intro.font_px,
        solo_story_intro_vertical_shift_px: config.fonts.solo_story_intro.vertical_shift_px,
        solo_episode_card_font: config.fonts.solo_episode_card.path.clone(),
        solo_episode_card_font_px: config.fonts.solo_episode_card.font_px,
        solo_episode_card_vertical_shift_px: config.fonts.solo_episode_card.vertical_shift_px,
        practical_selection_label_font: config.fonts.practical_selection_label.path.clone(),
        practical_selection_label_font_px: config.fonts.practical_selection_label.font_px,
        practical_selection_label_vertical_shift_px: config
            .fonts
            .practical_selection_label
            .vertical_shift_px,
        stage_label_font: config.fonts.stage_label.path.clone(),
        stage_label_font_px: config.fonts.stage_label.font_px,
        stage_label_vertical_shift_px: config.fonts.stage_label.vertical_shift_px,
        league_standing_label_font: config.fonts.league_standing_label.path.clone(),
        league_standing_label_font_px: config.fonts.league_standing_label.font_px,
        league_standing_label_vertical_shift_px: config
            .fonts
            .league_standing_label
            .vertical_shift_px,
        selection_help_font: config.fonts.selection_help.path.clone(),
        selection_help_font_px: config.fonts.selection_help.font_px,
        selection_help_vertical_shift_px: config.fonts.selection_help.vertical_shift_px,
        atlas,
    };
    let mut bytes = serde_json::to_vec_pretty(&report)?;
    bytes.push(b'\n');
    if let Some(parent) = config.output.parent() {
        std::fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create character-select plan directory {}",
                parent.display()
            )
        })?;
    }
    std::fs::write(&config.output, bytes)
        .with_context(|| format!("failed to write {}", config.output.display()))?;
    Ok(report)
}

pub(super) fn shared_atlas_bytes(source_decoded: &[u8]) -> Result<&[u8]> {
    let source = source_decoded
        .get(SHARED_ATLAS_OFFSET..)
        .context("SELP shared atlas offset is outside the decoded source")?;
    let tim = crate::tim::parse_4bpp_prefix(source)?;
    ensure!(
        tim.pixel_width() == 1024 && tim.image_height == 256,
        "SELP shared atlas geometry changed"
    );
    source
        .get(..tim.total_size)
        .context("SELP shared atlas is truncated")
}

fn prepare_output(output: &Path, force: bool) -> Result<()> {
    if output.exists() && !force {
        bail!(
            "character-select atlas plan output exists; pass --force to replace {}",
            output.display()
        );
    }
    if output.is_dir() {
        bail!(
            "character-select atlas plan output is a directory: {}",
            output.display()
        );
    }
    Ok(())
}
