//! Source reconstruction, code allocation and message preparation, before output compression.
use std::collections::BTreeMap;

use anyhow::{Context, Result, bail, ensure};

use crate::compression::decompress;
use crate::paged_compression::source_paged_compression_profile;
use crate::pipeline::{map_ordered_parallel, sha256_bytes};
use crate::source_disc::SupportedSourceDisc;

use super::dialogue_code_allocation::build_dialogue_code_allocation_from_translation_inputs;
use super::dialogue_code_allocation_model::{
    DialogueCodeAllocationAsset, DialogueCodeAllocationConfig, DialogueCodeAllocationReport,
};
use super::dialogue_message_build_model::DialogueMessageBuildConfig;
use super::dialogue_message_layout::validate_primary_message_layout;
use super::dialogue_message_plan_model::{DialogueMessageBuildPlan, DialogueMessageBuildPlanAsset};
use super::dialogue_message_rebuild::rebuild_dialogue_message_arena;
use super::dialogue_translation_input::load_dialogue_translation_inputs_from_source;
use super::selector_translation_model::DialogueSelectorTranslationAuditConfig;
use super::sources::{load_dialogue_runtime_image_sources, load_mgk_dialogue_sources};
use super::translation_model::DialogueTranslationAuditConfig;
use super::translation_workspace_source::TranslationWorkspaceSource;

pub(super) fn plan_dialogue_messages(
    config: &DialogueMessageBuildConfig,
    source: &SupportedSourceDisc,
) -> Result<(DialogueMessageBuildPlan, DialogueCodeAllocationReport)> {
    let primary_config = DialogueTranslationAuditConfig {
        cue: config.cue.clone(),
        codebook: config.codebook.clone(),
        translation: config.translation.clone(),
        output: config.translation_audit_output.clone(),
    };
    let selector_config = DialogueSelectorTranslationAuditConfig {
        cue: config.cue.clone(),
        codebook: config.codebook.clone(),
        translation: config.selector_translation.clone(),
        output: config.selector_translation_audit_output.clone(),
    };
    let (primary_input, selector_input) =
        load_dialogue_translation_inputs_from_source(&primary_config, &selector_config, source)?;
    let primary_layout = validate_primary_message_layout(&primary_input)?;
    let allocation_config = DialogueCodeAllocationConfig {
        cue: config.cue.clone(),
        codebook: config.codebook.clone(),
        translation: config.translation.clone(),
        selector_translation: config.selector_translation.clone(),
        name_input_keyboard: config.name_input_keyboard.clone(),
        translation_audit_output: config.translation_audit_output.clone(),
        selector_translation_audit_output: config.selector_translation_audit_output.clone(),
        output: config.code_allocation_output.clone(),
        input_policy: config.input_policy,
    };
    let allocation = build_dialogue_code_allocation_from_translation_inputs(
        &allocation_config,
        &primary_input,
        &selector_input,
        source,
    )?;
    ensure!(
        allocation.static_allocation_complete,
        "static dialogue code allocation is incomplete"
    );

    let translation = primary_input.report;
    let development_authored_group_count = translation
        .semantic_group_count
        .checked_sub(translation.untranslated_group_count)
        .context("dialogue authored group count underflow")?;
    ensure!(
        development_authored_group_count != 0
            && (!config.input_policy.requires_complete_scope()
                || translation.development_translation_input_available),
        "dialogue message build input policy requires unavailable primary text"
    );
    ensure!(
        primary_input.authored_segments.len() == development_authored_group_count,
        "dialogue authored input count changed"
    );
    let mut translated_segments = primary_input.authored_segments;
    let selector_translation = selector_input.report;
    let authored_selector_group_count = selector_translation
        .development_authored_group_count
        .checked_sub(selector_translation.runtime_insertion_rewrite_group_count)
        .context("selector authored group count underflow")?;
    ensure!(
        !config.input_policy.requires_complete_scope()
            || selector_translation.development_full_selector_input_available,
        "dialogue message build input policy requires unavailable selector text"
    );
    ensure!(
        selector_input.authored_segments.len() == authored_selector_group_count,
        "selector authored input count changed"
    );
    for (semantic_hash, segments) in selector_input.authored_segments {
        ensure!(
            translated_segments
                .insert(semantic_hash, segments)
                .is_none(),
            "primary and selector translation semantic hashes overlap"
        );
    }
    let stat_result_layout = super::stat_result_layout::prepare(&translated_segments)?;
    let backup_slot_text = super::backup_slot_text::backup_slot_text_counts(&translated_segments)?;
    let TranslationWorkspaceSource {
        corpus,
        groups_by_owner,
        referenced_coordinates,
        source_asset_count,
        ..
    } = primary_input.source;
    ensure!(
        source_asset_count == selector_translation.source_asset_count
            && allocation.assets.len() == source_asset_count,
        "dialogue message inputs cover different source assets"
    );
    let mut controls_by_semantic_hash = groups_by_owner
        .into_values()
        .flatten()
        .filter(|group| translated_segments.contains_key(&group.semantic_source_sha256))
        .map(|group| (group.semantic_source_sha256, group.controls))
        .collect::<BTreeMap<_, _>>();
    for group in selector_input
        .source
        .groups_by_selector
        .into_values()
        .flatten()
    {
        if translated_segments.contains_key(&group.semantic_source_sha256) {
            ensure!(
                controls_by_semantic_hash
                    .insert(group.semantic_source_sha256, group.controls)
                    .is_none(),
                "primary and selector source controls overlap"
            );
        }
    }
    ensure!(
        controls_by_semantic_hash.len() == translated_segments.len(),
        "dialogue message build source group count changed"
    );

    let sources = match source_asset_count {
        10 => load_mgk_dialogue_sources(source.image_path())?,
        78 => load_dialogue_runtime_image_sources(source.image_path())?,
        _ => bail!("unsupported dialogue message source asset count"),
    };
    let assets = map_ordered_parallel(&sources, |source| {
        let corpus_asset = corpus
            .assets
            .iter()
            .find(|asset| asset.source_path == source.path)
            .with_context(|| format!("{} disappeared from dialogue corpus", source.path))?;
        let allocation_asset = allocation
            .assets
            .iter()
            .find(|asset| asset.source_path == source.path)
            .with_context(|| format!("{} disappeared from code allocation", source.path))?;
        let character_codes = character_code_map(allocation_asset)?;
        let original_decoded = decompress(&source.data, true)
            .with_context(|| format!("failed to decode {}", source.path))?;
        let rebuilt = rebuild_dialogue_message_arena(
            &original_decoded,
            corpus_asset,
            &translated_segments,
            &controls_by_semantic_hash,
            &referenced_coordinates,
            &character_codes,
            config.input_policy,
        )?;
        let source_paged_compression_profile = source_paged_compression_profile(&source.data)
            .with_context(|| format!("failed to profile source compression for {}", source.path))?;
        let source_catalog_prefix = source
            .data
            .get(..4)
            .context("dialogue source lacks its catalog prefix")?
            .try_into()?;
        Ok(DialogueMessageBuildPlanAsset {
            source_path: source.path.clone(),
            original_decoded_sha256: sha256_bytes(&original_decoded),
            rebuilt_decoded_sha256: sha256_bytes(&rebuilt.decoded),
            original_stored_sha256: sha256_bytes(&source.data),
            original_stored_byte_count: source.data.len(),
            source_catalog_prefix,
            source_compression_profile: source_paged_compression_profile,
            original_decoded,
            rebuilt_decoded: rebuilt.decoded,
            bank_count: rebuilt.bank_count,
            message_count: rebuilt.message_count,
            translated_message_count: rebuilt.translated_message_count,
            rewritten_runtime_insertion_message_count: rebuilt.runtime_insertion_message_count,
            preserved_untranslated_message_count: rebuilt.preserved_untranslated_message_count,
            preserved_unreferenced_message_count: rebuilt.preserved_unreferenced_message_count,
            message_arena_used_byte_count: rebuilt.used_byte_count,
            message_arena_spare_byte_count: rebuilt.spare_byte_count,
            parse_back_verified: true,
        })
    })?;

    let plan = DialogueMessageBuildPlan {
        backup_slot_text,
        stat_result_layout,
        primary_layout,
        source_bin_sha256: translation.source_bin_sha256,
        codebook_sha256: translation.codebook_sha256,
        input_policy: config.input_policy.label().to_string(),
        semantic_group_count: translation.semantic_group_count,
        development_authored_group_count,
        selector_semantic_group_count: selector_translation.semantic_group_count,
        selector_development_authored_group_count: selector_translation
            .development_authored_group_count,
        selector_development_full_input_available: selector_translation
            .development_full_selector_input_available,
        release_candidate_selector_input_eligible: selector_translation
            .release_candidate_selector_input_eligible,
        development_build_input_available: true,
        development_translation_input_available: translation
            .development_translation_input_available,
        release_candidate_translation_input_eligible: translation
            .release_candidate_translation_input_eligible,
        assets,
    };
    Ok((plan, allocation))
}

fn character_code_map(asset: &DialogueCodeAllocationAsset) -> Result<BTreeMap<char, u16>> {
    let mut result = BTreeMap::new();
    for assignment in &asset.assignments {
        let mut characters = assignment.character.chars();
        let character = characters.next().context("empty character assignment")?;
        ensure!(
            characters.next().is_none(),
            "dialogue code assignment contains more than one character"
        );
        let digits = assignment
            .code
            .strip_prefix("0x")
            .context("dialogue code assignment lacks 0x prefix")?;
        let code = u16::from_str_radix(digits, 16)?;
        ensure!(
            result.insert(character, code).is_none(),
            "dialogue character is assigned more than once"
        );
    }
    Ok(result)
}
