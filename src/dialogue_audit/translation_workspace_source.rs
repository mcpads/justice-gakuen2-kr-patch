use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use anyhow::{Context, Result, ensure};

use crate::compression::decompress;
use crate::pipeline::sha256_bytes;
use crate::source_disc::SupportedSourceDisc;

use super::corpus::{
    extract_all_dialogue_source_corpus_from_source, extract_dialogue_source_corpus_from_source,
};
use super::corpus_digest::source_corpus_sha256;
use super::corpus_model::DialogueSourceCorpus;
use super::script_model::DialogueScriptAssetAudit;
use super::script_source::load_mgame_from_source;
use super::script_topology::{audit_script_asset, serialized_asset_bytes};
use super::sources::load_mgk_dialogue_sources;
use super::translation::expected_translation_input_with_source_corpus_sha256;
use super::translation_model::DialogueTranslationScope;
use super::translation_workspace_model::{
    DialogueTranslationContextEntry, DialogueTranslationRouteOwner, DialogueTranslationSourceGroup,
};
use super::translation_workspace_primary_scripts::extract_translation_primary_scripts;

pub(super) struct TranslationWorkspaceSource {
    pub(super) corpus: Arc<DialogueSourceCorpus>,
    pub(super) source_bin_sha256: String,
    pub(super) codebook_sha256: String,
    pub(super) source_corpus_sha256: String,
    pub(super) script_inventory_sha256: String,
    pub(super) target_scope: String,
    pub(super) source_asset_count: usize,
    pub(super) unresolved_primary_script_assets: Vec<String>,
    pub(super) groups_by_owner:
        BTreeMap<DialogueTranslationRouteOwner, Vec<DialogueTranslationSourceGroup>>,
    pub(super) contexts_by_owner:
        BTreeMap<DialogueTranslationRouteOwner, Vec<DialogueTranslationContextEntry>>,
    pub(super) referenced_coordinates: BTreeSet<String>,
}

pub(super) fn extract_translation_workspace_source(
    cue_path: &std::path::Path,
    codebook_path: &std::path::Path,
    scope: DialogueTranslationScope,
) -> Result<TranslationWorkspaceSource> {
    let source = SupportedSourceDisc::open(cue_path)?;
    extract_translation_workspace_source_from_source(&source, codebook_path, scope)
}

pub(super) fn extract_translation_workspace_source_from_source(
    source: &SupportedSourceDisc,
    codebook_path: &std::path::Path,
    scope: DialogueTranslationScope,
) -> Result<TranslationWorkspaceSource> {
    extract_translation_workspace_source_from_source_with_bound_corpus_sha256(
        source,
        codebook_path,
        scope,
        None,
    )
}

pub(super) fn extract_translation_workspace_source_from_source_with_bound_corpus_sha256(
    source: &SupportedSourceDisc,
    codebook_path: &std::path::Path,
    scope: DialogueTranslationScope,
    bound_source_corpus_sha256: Option<&str>,
) -> Result<TranslationWorkspaceSource> {
    let corpus = match scope {
        DialogueTranslationScope::MgkDevelopment => {
            extract_dialogue_source_corpus_from_source(source, codebook_path)?
        }
        DialogueTranslationScope::ResolvedPrimary => {
            extract_all_dialogue_source_corpus_from_source(source, codebook_path)?
        }
    };
    let source_corpus_sha256 = match bound_source_corpus_sha256 {
        Some(bound) => bound.to_string(),
        None => source_corpus_sha256(&corpus)?,
    };
    let codebook_sha256 = corpus.codebook_sha256.clone();
    let mgame = load_mgame_from_source(source)?;
    super::script_message_contract::validate_compact_message_opcode_contract(&mgame)?;

    let (source_asset_count, audits, unresolved_primary_script_assets) = match scope {
        DialogueTranslationScope::MgkDevelopment => {
            let mut audits = Vec::new();
            for dialogue_source in load_mgk_dialogue_sources(source.image_path())? {
                let decoded = decompress(&dialogue_source.data, true)
                    .with_context(|| format!("failed to decode {}", dialogue_source.path))?;
                let corpus_asset = corpus
                    .assets
                    .iter()
                    .find(|asset| asset.source_path == dialogue_source.path)
                    .with_context(|| {
                        format!("{} corpus asset disappeared", dialogue_source.path)
                    })?;
                audits.push(audit_script_asset(&decoded, corpus_asset, &mgame)?);
            }
            (10, audits, Vec::new())
        }
        DialogueTranslationScope::ResolvedPrimary => {
            let primary_scripts =
                extract_translation_primary_scripts(source.image_path(), &corpus, &mgame)?;
            (
                primary_scripts.source_asset_count,
                primary_scripts.audits,
                primary_scripts.unresolved_asset_paths,
            )
        }
    };
    let mut inventory_bytes = Vec::new();
    for audit in &audits {
        inventory_bytes.extend(serialized_asset_bytes(audit)?);
    }
    let script_inventory_sha256 = sha256_bytes(&inventory_bytes);
    let (contexts_by_owner, owners, referenced_coordinates, context_counts) =
        collect_contexts(&audits)?;
    let expected = expected_translation_input_with_source_corpus_sha256(
        &corpus,
        source_corpus_sha256.clone(),
    )?;
    let mut groups_by_owner: BTreeMap<_, Vec<_>> = BTreeMap::new();
    for group in expected.groups {
        let Some(owner) = owners.get(&group.semantic_source_sha256) else {
            continue;
        };
        let mut referenced_coordinate_ids = Vec::new();
        let mut unreferenced_duplicate_coordinate_ids = Vec::new();
        for coordinate_id in group.coordinate_ids {
            if referenced_coordinates.contains(&coordinate_id) {
                referenced_coordinate_ids.push(coordinate_id);
            } else {
                unreferenced_duplicate_coordinate_ids.push(coordinate_id);
            }
        }
        ensure!(
            !referenced_coordinate_ids.is_empty(),
            "owned semantic group has no referenced coordinates"
        );
        let context_occurrence_count = context_counts
            .get(&group.semantic_source_sha256)
            .copied()
            .context("owned semantic group has no route context")?;
        groups_by_owner
            .entry(owner.clone())
            .or_default()
            .push(DialogueTranslationSourceGroup {
                semantic_source_sha256: group.semantic_source_sha256,
                source_segments: group.source_segments,
                controls: group.controls,
                referenced_coordinate_ids,
                referenced_coordinate_count: None,
                unreferenced_duplicate_coordinate_count: unreferenced_duplicate_coordinate_ids
                    .len(),
                unreferenced_duplicate_coordinate_ids: Vec::new(),
                context_occurrence_count,
            });
    }
    for groups in groups_by_owner.values_mut() {
        groups.sort_by(|left, right| {
            left.semantic_source_sha256
                .cmp(&right.semantic_source_sha256)
        });
    }
    let semantic_group_count: usize = groups_by_owner.values().map(Vec::len).sum();
    let expected_denominator = match scope {
        DialogueTranslationScope::MgkDevelopment => (10, 4_660, 7_049),
        DialogueTranslationScope::ResolvedPrimary => (78, 18_221, 40_026),
    };
    ensure!(
        (
            audits.len(),
            semantic_group_count,
            referenced_coordinates.len()
        ) == expected_denominator,
        "translation workspace source denominator changed: assets={}, groups={semantic_group_count}, coordinates={}",
        audits.len(),
        referenced_coordinates.len()
    );

    Ok(TranslationWorkspaceSource {
        source_bin_sha256: corpus.source_bin_sha256.clone(),
        codebook_sha256,
        source_corpus_sha256,
        script_inventory_sha256,
        target_scope: scope.target_scope().to_string(),
        source_asset_count,
        unresolved_primary_script_assets,
        groups_by_owner,
        contexts_by_owner,
        referenced_coordinates,
        corpus,
    })
}

type ContextCollection = (
    BTreeMap<DialogueTranslationRouteOwner, Vec<DialogueTranslationContextEntry>>,
    BTreeMap<String, DialogueTranslationRouteOwner>,
    BTreeSet<String>,
    BTreeMap<String, usize>,
);

fn collect_contexts(audits: &[DialogueScriptAssetAudit]) -> Result<ContextCollection> {
    let mut contexts_by_owner: BTreeMap<_, Vec<_>> = BTreeMap::new();
    let mut owners = BTreeMap::new();
    let mut referenced_coordinates = BTreeSet::new();
    let mut context_counts = BTreeMap::new();
    for audit in audits {
        for bank in &audit.bank_tables {
            for route in &bank.route_bindings {
                let owner = DialogueTranslationRouteOwner {
                    source_path: audit.source_path.clone(),
                    bank_selector: bank.bank_selector,
                    variant_selector: route.variant_selector,
                    route_table_offset: route.decoded_offset.clone(),
                };
                let entries = contexts_by_owner.entry(owner.clone()).or_default();
                for segment in &route.segments {
                    let Some(entrypoint) = segment.entrypoint.as_ref() else {
                        continue;
                    };
                    for (message_order, message) in segment.messages.iter().enumerate() {
                        let occurrence_id = format!(
                            "{}#bank-{}-variant-{:03}-segment-{:03}-message-{:03}-{}",
                            asset_stem(&audit.source_path)?,
                            bank.bank_selector,
                            route.variant_selector,
                            segment.slot_index,
                            message_order,
                            message.command_offset.trim_start_matches("0x")
                        );
                        referenced_coordinates.insert(message.coordinate_id.clone());
                        owners
                            .entry(message.semantic_source_sha256.clone())
                            .or_insert_with(|| owner.clone());
                        *context_counts
                            .entry(message.semantic_source_sha256.clone())
                            .or_insert(0usize) += 1;
                        entries.push(DialogueTranslationContextEntry {
                            occurrence_id,
                            segment_index: segment.slot_index,
                            message_order,
                            entrypoint: entrypoint.clone(),
                            command_offset: message.command_offset.clone(),
                            opcode: message.opcode.clone(),
                            entry_index: message.entry_index,
                            coordinate_id: message.coordinate_id.clone(),
                            semantic_source_sha256: message.semantic_source_sha256.clone(),
                            scene_identity: "unresolved".to_string(),
                            speaker_identity: "unresolved".to_string(),
                            listener_identity: "unresolved".to_string(),
                            semantic_evidence: "structural_route_context_only".to_string(),
                        });
                    }
                }
            }
        }
    }
    let occurrence_ids: BTreeSet<_> = contexts_by_owner
        .values()
        .flatten()
        .map(|entry| entry.occurrence_id.as_str())
        .collect();
    let occurrence_count: usize = contexts_by_owner.values().map(Vec::len).sum();
    ensure!(
        occurrence_ids.len() == occurrence_count,
        "route context occurrence IDs are not unique"
    );
    Ok((
        contexts_by_owner,
        owners,
        referenced_coordinates,
        context_counts,
    ))
}

pub(super) fn asset_stem(source_path: &str) -> Result<String> {
    Ok(std::path::Path::new(source_path)
        .file_stem()
        .context("dialogue source path has no stem")?
        .to_string_lossy()
        .to_ascii_lowercase())
}
