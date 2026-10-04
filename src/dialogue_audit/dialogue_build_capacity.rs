use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result, ensure};

use crate::compression::decompress;

use super::dialogue_build_capacity_model::{
    DialogueBuildCapacityAssetAudit, DialogueBuildCapacityAuditConfig,
    DialogueBuildCapacityAuditReport, DialogueBuildCapacityBankAudit,
};
use super::dialogue_message_encoding::encoded_message_byte_count;
use super::dialogue_translation_input::load_primary_dialogue_translation_input;
use super::format::hex_offset;
use super::parser::parse_dialogue_banks;
use super::script_topology::PRIMARY_SCRIPT_OFFSET;
use super::sources::load_mgk_dialogue_sources;
use super::translation_model::DialogueTranslationAuditConfig;
use super::translation_workspace_source::TranslationWorkspaceSource;

pub fn audit_dialogue_build_capacity(
    config: &DialogueBuildCapacityAuditConfig,
) -> Result<DialogueBuildCapacityAuditReport> {
    let primary_input = load_primary_dialogue_translation_input(&DialogueTranslationAuditConfig {
        cue: config.cue.clone(),
        codebook: config.codebook.clone(),
        translation: config.translation.clone(),
        output: config.translation_audit_output.clone(),
    })?;
    let translation = primary_input.report;
    ensure!(
        translation.development_translation_input_available,
        "dialogue build capacity requires complete authored development input"
    );
    ensure!(
        primary_input.authored_segments.len() == translation.semantic_group_count,
        "dialogue capacity authored input count changed"
    );
    let translated_segments = primary_input.authored_segments;
    let TranslationWorkspaceSource {
        corpus,
        groups_by_owner,
        referenced_coordinates,
        ..
    } = primary_input.source;
    let graph_referenced_coordinate_count = referenced_coordinates.len();
    let mut source_groups = BTreeMap::new();
    for group in groups_by_owner.into_values().flatten() {
        ensure!(
            source_groups
                .insert(group.semantic_source_sha256.clone(), group)
                .is_none(),
            "translation source semantic hash is duplicated"
        );
    }
    ensure!(
        source_groups.len() == translation.semantic_group_count,
        "translation build source group count changed"
    );
    let sources = load_mgk_dialogue_sources(&crate::cue::CueSheet::parse(&config.cue)?.image_path)?;
    let mut assets = Vec::new();
    let mut coordinate_count = 0usize;
    let mut translated_coordinate_count = 0usize;
    let mut preserved_unreferenced_coordinate_count = 0usize;
    for source in sources {
        let decoded = decompress(&source.data, true)
            .with_context(|| format!("failed to decode {}", source.path))?;
        let parsed_banks = parse_dialogue_banks(&decoded)?;
        let message_arena_start = parsed_banks
            .first()
            .context("dialogue asset has no message arena")?
            .message_data_start;
        let source_bank_region_end = parsed_banks
            .last()
            .context("dialogue asset has no final bank")?
            .pointer_table_end;
        ensure!(
            source_bank_region_end <= PRIMARY_SCRIPT_OFFSET,
            "dialogue bank regions overlap the primary script"
        );
        let source_zero_tail = &decoded[source_bank_region_end..PRIMARY_SCRIPT_OFFSET];
        let source_zero_tail_nonzero_byte_count =
            source_zero_tail.iter().filter(|byte| **byte != 0).count();
        ensure!(
            source_zero_tail_nonzero_byte_count == 0,
            "dialogue message arena tail contains source data"
        );
        let corpus_asset = corpus
            .assets
            .iter()
            .find(|asset| asset.source_path == source.path)
            .with_context(|| format!("{} disappeared from dialogue corpus", source.path))?;
        ensure!(
            parsed_banks.len() == corpus_asset.banks.len(),
            "{} bank count changed during capacity audit",
            source.path
        );

        let mut bank_reports = Vec::new();
        for bank in parsed_banks {
            let corpus_bank = corpus_asset
                .banks
                .iter()
                .find(|candidate| candidate.selector_index == bank.selector_index)
                .context("dialogue corpus bank disappeared")?;
            ensure!(
                corpus_bank.entries.len() == bank.messages.len(),
                "dialogue bank message count changed"
            );
            let mut rebuilt_message_byte_count = 0usize;
            let mut translated_message_count = 0usize;
            let mut preserved_unreferenced_message_count = 0usize;
            for (entry, original_message) in corpus_bank.entries.iter().zip(&bank.messages) {
                ensure!(
                    entry.raw_words.len() * 2 == original_message.data.len()
                        && original_message.data.len().is_multiple_of(4),
                    "source dialogue message alignment changed"
                );
                let message_byte_count = if let Some(segments) =
                    translated_segments.get(&entry.semantic_source_sha256)
                {
                    let source_group = source_groups
                        .get(&entry.semantic_source_sha256)
                        .context("authored dialogue lacks protected source controls")?;
                    translated_message_count += 1;
                    translated_coordinate_count += 1;
                    let (segments, controls) = super::choice_columns::prepare(
                        &entry.semantic_source_sha256,
                        segments,
                        &source_group.controls,
                    )?;
                    encoded_message_byte_count(&segments, &controls)?
                } else {
                    ensure!(
                        !referenced_coordinates.contains(&entry.coordinate_id),
                        "execution-referenced dialogue coordinate lacks authored Korean segments"
                    );
                    preserved_unreferenced_message_count += 1;
                    preserved_unreferenced_coordinate_count += 1;
                    original_message.data.len()
                };
                rebuilt_message_byte_count = rebuilt_message_byte_count
                    .checked_add(message_byte_count)
                    .context("rebuilt dialogue byte count overflow")?;
                coordinate_count += 1;
            }
            let pointer_table_byte_count = bank
                .messages
                .len()
                .checked_add(1)
                .and_then(|words| words.checked_mul(4))
                .context("dialogue pointer table byte count overflow")?;
            ensure!(
                pointer_table_byte_count == bank.pointer_table_end - bank.pointer_table_offset,
                "dialogue pointer table size changed"
            );
            let fixed_region_byte_count = bank.pointer_table_end - bank.message_data_start;
            let original_message_byte_count = bank.message_data_end - bank.message_data_start;
            let rebuilt_region_byte_count = rebuilt_message_byte_count
                .checked_add(pointer_table_byte_count)
                .context("rebuilt dialogue region byte count overflow")?;
            bank_reports.push(DialogueBuildCapacityBankAudit {
                selector_index: bank.selector_index,
                message_count: bank.messages.len(),
                translated_message_count,
                preserved_unreferenced_message_count,
                fixed_region_start: hex_offset(bank.message_data_start),
                fixed_region_end: hex_offset(bank.pointer_table_end),
                fixed_region_byte_count,
                pointer_table_byte_count,
                original_message_byte_count,
                rebuilt_message_byte_count,
                rebuilt_region_byte_count,
                fixed_region_spare_byte_count: fixed_region_byte_count
                    .saturating_sub(rebuilt_region_byte_count),
                fixed_region_shortfall_byte_count: rebuilt_region_byte_count
                    .saturating_sub(fixed_region_byte_count),
                fits_fixed_region: rebuilt_region_byte_count <= fixed_region_byte_count,
            });
        }
        assets.push(finish_asset(
            source.path,
            bank_reports,
            message_arena_start,
            source_bank_region_end,
            source_zero_tail_nonzero_byte_count,
        ));
    }
    ensure!(
        coordinate_count == corpus.coordinate_count,
        "dialogue build capacity coordinate count changed"
    );
    let translated_unreferenced_duplicate_coordinate_count = translated_coordinate_count
        .checked_sub(graph_referenced_coordinate_count)
        .context("translated coordinate coverage excludes a graph reference")?;

    let report = DialogueBuildCapacityAuditReport {
        kind: "Justice Gakuen 2 development dialogue build capacity audit".to_string(),
        source_bin_sha256: translation.source_bin_sha256,
        codebook_sha256: translation.codebook_sha256,
        semantic_group_count: translation.semantic_group_count,
        coordinate_count,
        graph_referenced_coordinate_count,
        translated_coordinate_count,
        translated_unreferenced_duplicate_coordinate_count,
        preserved_unreferenced_coordinate_count,
        development_translation_input_available: translation
            .development_translation_input_available,
        release_candidate_translation_input_eligible: translation
            .release_candidate_translation_input_eligible,
        fixed_bank_regions_fit: assets.iter().all(|asset| asset.fixed_bank_regions_fit),
        relocated_bank_regions_fit: assets.iter().all(|asset| asset.relocated_bank_regions_fit),
        total_original_message_byte_count: assets
            .iter()
            .map(|asset| asset.original_message_byte_count)
            .sum(),
        total_rebuilt_message_byte_count: assets
            .iter()
            .map(|asset| asset.rebuilt_message_byte_count)
            .sum(),
        total_fixed_region_shortfall_byte_count: assets
            .iter()
            .map(|asset| asset.fixed_region_shortfall_byte_count)
            .sum(),
        assets,
    };
    write_report(&config.output, &report)?;
    Ok(report)
}

fn finish_asset(
    source_path: String,
    banks: Vec<DialogueBuildCapacityBankAudit>,
    message_arena_start: usize,
    source_bank_region_end: usize,
    source_zero_tail_nonzero_byte_count: usize,
) -> DialogueBuildCapacityAssetAudit {
    let message_arena_byte_count = PRIMARY_SCRIPT_OFFSET - message_arena_start;
    let rebuilt_region_byte_count: usize = banks
        .iter()
        .map(|bank| bank.rebuilt_region_byte_count)
        .sum();
    DialogueBuildCapacityAssetAudit {
        source_path,
        bank_count: banks.len(),
        message_count: banks.iter().map(|bank| bank.message_count).sum(),
        translated_message_count: banks.iter().map(|bank| bank.translated_message_count).sum(),
        preserved_unreferenced_message_count: banks
            .iter()
            .map(|bank| bank.preserved_unreferenced_message_count)
            .sum(),
        original_message_byte_count: banks
            .iter()
            .map(|bank| bank.original_message_byte_count)
            .sum(),
        rebuilt_message_byte_count: banks
            .iter()
            .map(|bank| bank.rebuilt_message_byte_count)
            .sum(),
        fixed_bank_regions_fit: banks.iter().all(|bank| bank.fits_fixed_region),
        fixed_region_shortfall_byte_count: banks
            .iter()
            .map(|bank| bank.fixed_region_shortfall_byte_count)
            .sum(),
        message_arena_start: hex_offset(message_arena_start),
        message_arena_end: hex_offset(PRIMARY_SCRIPT_OFFSET),
        message_arena_byte_count,
        source_zero_tail_byte_count: PRIMARY_SCRIPT_OFFSET - source_bank_region_end,
        source_zero_tail_nonzero_byte_count,
        relocated_bank_regions_fit: rebuilt_region_byte_count <= message_arena_byte_count,
        relocated_region_spare_byte_count: message_arena_byte_count
            .saturating_sub(rebuilt_region_byte_count),
        banks,
    }
}

fn write_report(path: &Path, report: &DialogueBuildCapacityAuditReport) -> Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let mut bytes = serde_json::to_vec_pretty(report)?;
    bytes.push(b'\n');
    std::fs::write(path, bytes).with_context(|| format!("failed to write {}", path.display()))
}
