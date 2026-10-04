use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};
use serde::Serialize;

use crate::compression::decompress;
use crate::cue::CueSheet;
use crate::pipeline::{BASELINE_BIN_SHA256, sha256_file};

use super::atlas::parse_dialogue_atlas;
use super::codebook::{load_resolved_dialogue_glyphs, mgk_source_glyph_inventory};
use super::dialogue_development_input::{
    classify_runtime_character_demand, collect_asset_dialogue_demand,
};
use super::dialogue_translation_input::load_primary_dialogue_translation_input;
use super::name_entry::inspect_dialogue_name_entry;
use super::sources::load_mgk_dialogue_sources;
use super::translation_model::DialogueTranslationAuditConfig;
use super::translation_workspace_source::TranslationWorkspaceSource;

#[derive(Debug, Clone)]
pub struct DialogueFontAllocationAuditConfig {
    pub cue: PathBuf,
    pub codebook: PathBuf,
    pub translation: PathBuf,
    pub translation_audit_output: PathBuf,
    pub output: PathBuf,
}

#[derive(Debug, Serialize)]
pub struct DialogueFontAllocationAuditReport {
    pub kind: String,
    pub source_bin_sha256: String,
    pub codebook_sha256: String,
    pub authored_group_count: usize,
    pub development_translation_input_available: bool,
    pub release_candidate_translation_input_eligible: bool,
    pub name_entry_selectable_position_count: usize,
    pub worst_case_name_entry_glyphs_fit_rewritten_atlas_in_every_asset: bool,
    pub assets: Vec<DialogueFontAssetAllocationAudit>,
}

#[derive(Debug, Serialize)]
pub struct DialogueFontAssetAllocationAudit {
    pub source_path: String,
    pub fixed_cell_count: usize,
    pub addressable_slot_count: usize,
    pub extension_slot_count: usize,
    pub authored_character_count: usize,
    pub known_runtime_character_count: usize,
    pub reusable_source_character_count: usize,
    pub required_new_character_count: usize,
    pub extension_shortfall_count: usize,
    pub authored_repertoire: String,
    pub known_runtime_repertoire: String,
    pub reusable_source_repertoire: String,
    pub required_new_repertoire: String,
    pub unresolved_runtime_sources: Vec<String>,
    pub known_repertoire_fits_extension_slots: bool,
    pub runtime_repertoire_complete: bool,
    pub rewritten_atlas_spare_slot_count: usize,
    pub worst_case_name_entry_glyphs_fit_rewritten_atlas: bool,
    pub rewritten_atlas_spare_slot_count_after_worst_case_name_entry_glyphs: usize,
}

pub fn audit_dialogue_font_allocation(
    config: &DialogueFontAllocationAuditConfig,
) -> Result<DialogueFontAllocationAuditReport> {
    let primary_input = load_primary_dialogue_translation_input(&DialogueTranslationAuditConfig {
        cue: config.cue.clone(),
        codebook: config.codebook.clone(),
        translation: config.translation.clone(),
        output: config.translation_audit_output.clone(),
    })?;
    let translation = primary_input.report;
    ensure!(
        translation.ready_for_font_repertoire,
        "font allocation requires a complete authored development repertoire"
    );

    let cue = CueSheet::parse(&config.cue)?;
    let source_bin_sha256 = sha256_file(&cue.image_path)?;
    ensure!(
        source_bin_sha256 == BASELINE_BIN_SHA256,
        "unsupported source BIN SHA-256: {source_bin_sha256}"
    );
    let name_entry = inspect_dialogue_name_entry(&config.cue)?;
    let name_entry_selectable_position_count = name_entry.source_selectable_cell_count;
    let (source_pixel_hashes, usage) = mgk_source_glyph_inventory(&cue.image_path)?;
    let (codebook_sha256, resolved_glyphs) = load_resolved_dialogue_glyphs(
        &config.codebook,
        &source_bin_sha256,
        &source_pixel_hashes,
        &usage,
    )?;
    ensure!(
        primary_input.authored_segments.len() == translation.semantic_group_count,
        "dialogue font allocation authored input count changed"
    );
    let translated_segments = primary_input.authored_segments;
    let TranslationWorkspaceSource {
        corpus,
        groups_by_owner,
        ..
    } = primary_input.source;
    let controls_by_semantic_hash = groups_by_owner
        .into_values()
        .flatten()
        .map(|group| (group.semantic_source_sha256, group.controls))
        .collect();
    let mut demand_by_asset =
        collect_asset_dialogue_demand(&corpus, &translated_segments, &controls_by_semantic_hash)?;

    let mut assets = Vec::new();
    for source in load_mgk_dialogue_sources(&cue.image_path)? {
        let decoded = decompress(&source.data, true)
            .with_context(|| format!("failed to decode {}", source.path))?;
        let atlas = parse_dialogue_atlas(&decoded)
            .with_context(|| format!("failed to parse {} dialogue atlas", source.path))?;
        let demand = demand_by_asset
            .remove(&source.path)
            .with_context(|| format!("{} lacks translated font demand", source.path))?;
        let authored = demand.static_characters;
        let runtime = classify_runtime_character_demand(&demand.controls);
        let available: BTreeSet<_> = atlas
            .fixed_cell_sha256
            .iter()
            .filter_map(|hash| resolved_glyphs.get(hash))
            .filter_map(|glyph| one_character(&glyph.text))
            .collect();
        let required: BTreeSet<_> = authored.union(&runtime.characters).copied().collect();
        let reusable: BTreeSet<_> = required.intersection(&available).copied().collect();
        let required_new: BTreeSet<_> = required.difference(&available).copied().collect();
        let extension_slot_count = atlas.addressable_slot_count - atlas.fixed_cell_count;
        let extension_shortfall_count = required_new.len().saturating_sub(extension_slot_count);
        let runtime_repertoire_complete = runtime.is_complete();
        let rewritten_capacity = assess_rewritten_atlas_capacity(
            atlas.addressable_slot_count,
            required.len(),
            name_entry_selectable_position_count,
        );
        assets.push(DialogueFontAssetAllocationAudit {
            source_path: source.path,
            fixed_cell_count: atlas.fixed_cell_count,
            addressable_slot_count: atlas.addressable_slot_count,
            extension_slot_count,
            authored_character_count: authored.len(),
            known_runtime_character_count: runtime.characters.len(),
            reusable_source_character_count: reusable.len(),
            required_new_character_count: required_new.len(),
            extension_shortfall_count,
            authored_repertoire: characters(&authored),
            known_runtime_repertoire: characters(&runtime.characters),
            reusable_source_repertoire: characters(&reusable),
            required_new_repertoire: characters(&required_new),
            unresolved_runtime_sources: runtime.unresolved_sources.into_iter().collect(),
            known_repertoire_fits_extension_slots: extension_shortfall_count == 0,
            runtime_repertoire_complete,
            rewritten_atlas_spare_slot_count: rewritten_capacity.spare_slot_count,
            worst_case_name_entry_glyphs_fit_rewritten_atlas: rewritten_capacity
                .candidate_positions_fit,
            rewritten_atlas_spare_slot_count_after_worst_case_name_entry_glyphs: rewritten_capacity
                .spare_slot_count_after_candidates,
        });
    }
    ensure!(
        demand_by_asset.is_empty(),
        "font demand contains an unknown dialogue asset"
    );

    let worst_case_name_entry_glyphs_fit_rewritten_atlas_in_every_asset = assets
        .iter()
        .all(|asset| asset.worst_case_name_entry_glyphs_fit_rewritten_atlas);

    let report = DialogueFontAllocationAuditReport {
        kind: "Justice Gakuen 2 development dialogue font allocation audit".to_string(),
        source_bin_sha256,
        codebook_sha256,
        authored_group_count: translation.semantic_group_count,
        development_translation_input_available: translation
            .development_translation_input_available,
        release_candidate_translation_input_eligible: translation
            .release_candidate_translation_input_eligible,
        name_entry_selectable_position_count,
        worst_case_name_entry_glyphs_fit_rewritten_atlas_in_every_asset,
        assets,
    };
    write_report(&config.output, &report)?;
    Ok(report)
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct RewrittenAtlasCapacity {
    pub(super) spare_slot_count: usize,
    pub(super) candidate_positions_fit: bool,
    pub(super) spare_slot_count_after_candidates: usize,
}

pub(super) fn assess_rewritten_atlas_capacity(
    addressable_slot_count: usize,
    known_repertoire_character_count: usize,
    candidate_position_count: usize,
) -> RewrittenAtlasCapacity {
    let spare_slot_count = addressable_slot_count.saturating_sub(known_repertoire_character_count);
    RewrittenAtlasCapacity {
        spare_slot_count,
        candidate_positions_fit: candidate_position_count <= spare_slot_count,
        spare_slot_count_after_candidates: spare_slot_count
            .saturating_sub(candidate_position_count),
    }
}

fn one_character(text: &str) -> Option<char> {
    let mut characters = text.chars();
    let character = characters.next()?;
    characters.next().is_none().then_some(character)
}

fn characters(characters: &BTreeSet<char>) -> String {
    characters.iter().collect()
}

fn write_report(path: &Path, report: &DialogueFontAllocationAuditReport) -> Result<()> {
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
