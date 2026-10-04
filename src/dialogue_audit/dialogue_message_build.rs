//! Standalone compression and file output for explicitly prepared message images.
use std::path::Path;

use anyhow::{Context, Result, bail, ensure};

use crate::compression::decompress;
use crate::paged_compression::{compress_page_safe_image, profile_paged_compression};
use crate::pipeline::{map_ordered_parallel, sha256_bytes};
use crate::source_disc::SupportedSourceDisc;

use super::dialogue_message_build_model::{
    DialogueMessageBuildAsset, DialogueMessageBuildConfig, DialogueMessageBuildReport,
};
use super::dialogue_message_plan::plan_dialogue_messages;
use super::dialogue_message_plan_model::{DialogueMessageBuildPlan, DialogueMessageBuildPlanAsset};
use super::format::hex_offset;
use super::parser::{SELECTOR_SLOT_COUNT, SELECTOR_TABLE_OFFSET};
use super::script_topology::PRIMARY_SCRIPT_OFFSET;

const MANIFEST_FILE: &str = "dialogue-message-build.json";

pub fn prepare_dialogue_message_images(
    config: &DialogueMessageBuildConfig,
) -> Result<DialogueMessageBuildReport> {
    prepare_output_directory(&config.output_dir, config.force)?;
    let source = SupportedSourceDisc::open(&config.cue)?;
    let (plan, _) = plan_dialogue_messages(config, &source)?;
    materialize_dialogue_message_images(&config.output_dir, plan)
}

fn materialize_dialogue_message_images(
    output_dir: &Path,
    plan: DialogueMessageBuildPlan,
) -> Result<DialogueMessageBuildReport> {
    let assets = map_ordered_parallel(&plan.assets, |asset| {
        materialize_dialogue_message_asset(output_dir, asset)
    })?;
    let report = DialogueMessageBuildReport {
        backup_slot_text: plan.backup_slot_text,
        primary_layout: plan.primary_layout,
        kind: "Justice Gakuen 2 non-release development dialogue message build".to_string(),
        source_bin_sha256: plan.source_bin_sha256,
        codebook_sha256: plan.codebook_sha256,
        input_policy: plan.input_policy,
        semantic_group_count: plan.semantic_group_count,
        development_authored_group_count: plan.development_authored_group_count,
        selector_semantic_group_count: plan.selector_semantic_group_count,
        selector_development_authored_group_count: plan.selector_development_authored_group_count,
        selector_development_full_input_available: plan.selector_development_full_input_available,
        release_candidate_selector_input_eligible: plan.release_candidate_selector_input_eligible,
        development_build_input_available: plan.development_build_input_available,
        development_translation_input_available: plan.development_translation_input_available,
        release_candidate_translation_input_eligible: plan
            .release_candidate_translation_input_eligible,
        compression_maximum_match_words: assets
            .iter()
            .map(|asset| asset.compression_maximum_match_words)
            .max()
            .unwrap_or(0),
        compression_maximum_control_block_output_words: assets
            .iter()
            .map(|asset| asset.compression_maximum_control_block_output_words)
            .max()
            .unwrap_or(0),
        message_arena_start: hex_offset(SELECTOR_TABLE_OFFSET + SELECTOR_SLOT_COUNT * 4),
        message_arena_end: hex_offset(PRIMARY_SCRIPT_OFFSET),
        asset_count: assets.len(),
        translated_coordinate_count: assets
            .iter()
            .map(|asset| asset.translated_message_count)
            .sum(),
        rewritten_runtime_insertion_coordinate_count: assets
            .iter()
            .map(|asset| asset.rewritten_runtime_insertion_message_count)
            .sum(),
        preserved_untranslated_coordinate_count: assets
            .iter()
            .map(|asset| asset.preserved_untranslated_message_count)
            .sum(),
        preserved_unreferenced_coordinate_count: assets
            .iter()
            .map(|asset| asset.preserved_unreferenced_message_count)
            .sum(),
        all_assets_parse_back: assets.iter().all(|asset| asset.parse_back_verified),
        all_assets_compress_within_original_extents: assets
            .iter()
            .all(|asset| asset.compressed_within_original_extent),
        assets,
    };
    write_report(&output_dir.join(MANIFEST_FILE), &report)?;
    Ok(report)
}

pub(super) fn materialize_dialogue_message_asset(
    output_dir: &Path,
    plan: &DialogueMessageBuildPlanAsset,
) -> Result<DialogueMessageBuildAsset> {
    let compressed =
        compress_page_safe_image(&plan.rebuilt_decoded, plan.source_compression_profile)
            .with_context(|| format!("failed to compress rebuilt {}", plan.source_path))?;
    let rebuilt_compression_profile = profile_paged_compression(&compressed)?;
    ensure!(
        decompress(&compressed, false)? == plan.rebuilt_decoded,
        "{} compression roundtrip changed rebuilt dialogue messages",
        plan.source_path
    );
    ensure!(
        compressed.len() <= plan.original_stored_byte_count,
        "{} rebuilt compressed asset exceeds its original extent",
        plan.source_path
    );
    ensure!(
        compressed.get(..4) == Some(plan.source_catalog_prefix.as_slice()),
        "{} rebuilt compressed asset changed its catalog prefix",
        plan.source_path
    );
    let compressed_byte_count = compressed.len();
    let mut padded = compressed;
    padded.resize(plan.original_stored_byte_count, 0);

    let stem = asset_stem(&plan.source_path)?;
    let decoded_output_file = format!("{stem}.decoded.bin");
    let stored_output_file = format!("{stem}.rebuilt.biz");
    std::fs::write(output_dir.join(&decoded_output_file), &plan.rebuilt_decoded)?;
    std::fs::write(output_dir.join(&stored_output_file), &padded)?;
    Ok(DialogueMessageBuildAsset {
        source_path: plan.source_path.clone(),
        decoded_output_file,
        stored_output_file,
        original_decoded_sha256: plan.original_decoded_sha256.clone(),
        rebuilt_decoded_sha256: plan.rebuilt_decoded_sha256.clone(),
        original_stored_sha256: plan.original_stored_sha256.clone(),
        rebuilt_stored_sha256: sha256_bytes(&padded),
        original_stored_byte_count: plan.original_stored_byte_count,
        compressed_byte_count,
        compression_maximum_match_words: plan.source_compression_profile.maximum_match_words,
        compression_maximum_control_block_output_words: plan
            .source_compression_profile
            .maximum_control_block_output_words,
        source_compression_control_blocks_crossing_input_pages: plan
            .source_compression_profile
            .control_blocks_crossing_input_pages,
        rebuilt_compression_control_blocks_crossing_input_pages: rebuilt_compression_profile
            .control_blocks_crossing_input_pages,
        source_compression_stream_byte_count: plan.source_compression_profile.stream_byte_count,
        stored_padding_byte_count: plan.original_stored_byte_count - compressed_byte_count,
        bank_count: plan.bank_count,
        message_count: plan.message_count,
        translated_message_count: plan.translated_message_count,
        rewritten_runtime_insertion_message_count: plan.rewritten_runtime_insertion_message_count,
        preserved_untranslated_message_count: plan.preserved_untranslated_message_count,
        preserved_unreferenced_message_count: plan.preserved_unreferenced_message_count,
        message_arena_used_byte_count: plan.message_arena_used_byte_count,
        message_arena_spare_byte_count: plan.message_arena_spare_byte_count,
        parse_back_verified: plan.parse_back_verified,
        compression_roundtrip_verified: true,
        compressed_within_original_extent: true,
    })
}

fn prepare_output_directory(output_dir: &Path, force: bool) -> Result<()> {
    if output_dir.exists() {
        ensure!(
            output_dir.is_dir(),
            "dialogue message output is not a directory"
        );
        if !force && std::fs::read_dir(output_dir)?.next().is_some() {
            bail!(
                "dialogue message output is not empty; pass --force to replace owned files in {}",
                output_dir.display()
            );
        }
        return Ok(());
    }
    std::fs::create_dir_all(output_dir)
        .with_context(|| format!("failed to create {}", output_dir.display()))
}

fn asset_stem(source_path: &str) -> Result<String> {
    Ok(Path::new(source_path)
        .file_stem()
        .context("dialogue asset path has no stem")?
        .to_string_lossy()
        .to_ascii_lowercase())
}

fn write_report(path: &Path, report: &DialogueMessageBuildReport) -> Result<()> {
    let mut bytes = serde_json::to_vec_pretty(report)?;
    bytes.push(b'\n');
    std::fs::write(path, bytes).with_context(|| format!("failed to write {}", path.display()))
}
