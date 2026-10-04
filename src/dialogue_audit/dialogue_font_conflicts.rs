use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Context, Result, ensure};

use super::dialogue_code_allocation::build_dialogue_code_allocation;
use super::dialogue_code_allocation_model::DialogueCodeAllocationConfig;
use super::dialogue_font_conflict_attribution::{
    attribute_conflict_occurrences, collect_conflict_occurrences, conflict_targets,
};
use super::dialogue_font_conflict_writer::{
    ensure_output_available, prepare_output_directory, write_asset_reports, write_group_reports,
};
use super::dialogue_font_conflicts_model::{
    DialogueFontConflictAuditConfig, DialogueFontConflictAuditManifest, DialogueFontConflictGroup,
    DialogueFontConflictGroupAsset,
};
use super::dialogue_translation_input::{
    load_primary_dialogue_translation_input, load_selector_dialogue_translation_input,
};
use super::format::hex_code;
use super::selector_translation_model::DialogueSelectorTranslationAuditConfig;
use super::translation_model::{DialogueDevelopmentInputPolicy, DialogueTranslationAuditConfig};
use super::translation_workspace_io::write_bounded_json;
use super::translation_workspace_model::{
    DialogueTranslationRouteOwner, DialogueTranslationSourceGroup,
};
use super::translation_workspace_source::TranslationWorkspaceSource;

const MANIFEST_FILE: &str = "manifest.json";

#[derive(Debug, Clone)]
struct ConflictGroupSource {
    translation_scope: &'static str,
    primary_owner: Option<DialogueTranslationRouteOwner>,
    canonical_selector: Option<usize>,
    target_selectors: Vec<usize>,
    selector_consumer_evidence: Option<String>,
    source_segments: Vec<String>,
    context_occurrence_count: usize,
}

#[derive(Default)]
struct GroupAssetAccumulator {
    conflicting_codes: BTreeSet<u16>,
    single_group_owned_codes: BTreeSet<u16>,
    preserved_coordinate_ids: BTreeSet<String>,
}

pub fn audit_dialogue_font_conflicts(
    config: &DialogueFontConflictAuditConfig,
) -> Result<DialogueFontConflictAuditManifest> {
    ensure_output_available(&config.output_dir, config.force)?;
    let allocation = build_dialogue_code_allocation(&DialogueCodeAllocationConfig {
        cue: config.cue.clone(),
        codebook: config.codebook.clone(),
        translation: config.translation.clone(),
        selector_translation: config.selector_translation.clone(),
        name_input_keyboard: config.name_input_keyboard.clone(),
        translation_audit_output: config.translation_audit_output.clone(),
        selector_translation_audit_output: config.selector_translation_audit_output.clone(),
        output: config.code_allocation_output.clone(),
        input_policy: DialogueDevelopmentInputPolicy::AuthoredSubset,
    })?;

    let primary_input = load_primary_dialogue_translation_input(&DialogueTranslationAuditConfig {
        cue: config.cue.clone(),
        codebook: config.codebook.clone(),
        translation: config.translation.clone(),
        output: config.translation_audit_output.clone(),
    })?;
    let translation = primary_input.report;
    let authored_primary_count = translation
        .semantic_group_count
        .checked_sub(translation.untranslated_group_count)
        .context("dialogue authored group count underflow")?;
    ensure!(
        primary_input.authored_segments.len() == authored_primary_count,
        "primary authored input count changed"
    );
    let mut authored_semantic_hashes = primary_input
        .authored_segments
        .into_keys()
        .collect::<BTreeSet<_>>();

    let selector_input =
        load_selector_dialogue_translation_input(&DialogueSelectorTranslationAuditConfig {
            cue: config.cue.clone(),
            codebook: config.codebook.clone(),
            translation: config.selector_translation.clone(),
            output: config.selector_translation_audit_output.clone(),
        })?;
    let selector_translation = selector_input.report;
    let authored_selector_count = selector_translation
        .development_authored_group_count
        .checked_sub(selector_translation.runtime_insertion_rewrite_group_count)
        .context("selector authored group count underflow")?;
    ensure!(
        selector_input.authored_segments.len() == authored_selector_count,
        "selector authored input count changed"
    );
    for semantic_hash in selector_input.authored_segments.into_keys() {
        ensure!(
            authored_semantic_hashes.insert(semantic_hash),
            "primary and selector authored semantic hashes overlap"
        );
    }
    let runtime_insertion_semantic_hashes = selector_input
        .source
        .groups_by_selector
        .values()
        .flatten()
        .filter(|group| {
            group.development_resolution
                == super::selector_translation_model::DialogueSelectorDevelopmentResolution::RuntimeInsertionRewrite
        })
        .map(|group| group.semantic_source_sha256.clone())
        .collect::<BTreeSet<_>>();

    let group_sources = collect_group_sources(&primary_input.source, &selector_input.source)?;
    ensure!(
        allocation.assets.len() == primary_input.source.source_asset_count
            && selector_translation.source_asset_count == primary_input.source.source_asset_count,
        "font-conflict inputs cover different source asset populations"
    );

    let mut asset_results = Vec::new();
    let mut group_assets = BTreeMap::<String, BTreeMap<String, GroupAssetAccumulator>>::new();
    for corpus_asset in &primary_input.source.corpus.assets {
        let allocation_asset = allocation
            .assets
            .iter()
            .find(|asset| asset.source_path == corpus_asset.source_path)
            .with_context(|| {
                format!(
                    "{} disappeared from dialogue code allocation",
                    corpus_asset.source_path
                )
            })?;
        let targets = conflict_targets(allocation_asset)?;
        if targets.is_empty() {
            continue;
        }
        let occurrences = collect_conflict_occurrences(
            corpus_asset,
            &targets,
            &authored_semantic_hashes,
            &runtime_insertion_semantic_hashes,
            &primary_input.source.referenced_coordinates,
        )?;
        let conflicts = attribute_conflict_occurrences(&targets, &occurrences)?;
        for conflict in &conflicts {
            let code = conflict.code;
            for semantic_hash in &conflict.required_semantic_hashes {
                let accumulator = group_assets
                    .entry(semantic_hash.clone())
                    .or_default()
                    .entry(corpus_asset.source_path.clone())
                    .or_default();
                accumulator.conflicting_codes.insert(code);
                if conflict.required_semantic_hashes.len() == 1 {
                    accumulator.single_group_owned_codes.insert(code);
                }
            }
        }
        for occurrence in &occurrences {
            group_assets
                .entry(occurrence.semantic_source_sha256.clone())
                .or_default()
                .entry(corpus_asset.source_path.clone())
                .or_default()
                .preserved_coordinate_ids
                .insert(occurrence.coordinate_id.clone());
        }
        asset_results.push((corpus_asset.source_path.clone(), conflicts));
    }

    let attributed_conflict_count: usize = asset_results
        .iter()
        .map(|(_, conflicts)| conflicts.len())
        .sum();
    ensure!(
        attributed_conflict_count
            == allocation
                .fixed_code_consumers
                .protected_source_glyph_overwrite_count,
        "font-conflict attribution denominator changed"
    );
    for semantic_hash in group_assets.keys() {
        ensure!(
            group_sources.contains_key(semantic_hash),
            "conflicting source glyph belongs to an unknown translation group"
        );
    }

    let groups = build_group_reports(&group_sources, group_assets)?;
    prepare_output_directory(&config.output_dir)?;
    let asset_refs = write_asset_reports(&config.output_dir, asset_results)?;
    let group_indexes = write_group_reports(&config.output_dir, &groups)?;
    let single_group_owned_code_count = asset_refs
        .iter()
        .map(|asset| asset.single_group_owned_code_count)
        .sum();
    let manifest = DialogueFontConflictAuditManifest {
        kind: "Justice Gakuen 2 development dialogue font conflict audit".to_string(),
        source_bin_sha256: allocation.source_bin_sha256,
        codebook_sha256: allocation.codebook_sha256,
        input_policy: allocation.input_policy,
        source_asset_count: allocation.assets.len(),
        conflict_asset_count: asset_refs.len(),
        conflicting_code_count: attributed_conflict_count,
        required_untranslated_group_count: groups.len(),
        single_group_owned_code_count,
        all_conflicts_attributed: true,
        prose_generated: false,
        planning_order_basis: "descending single-group-owned protected conflict code count, then conflict participation count, then semantic source hash; this does not predict net conflict reduction because authored text may add glyph demand; translation remains source-and-context-first human work".to_string(),
        assets: asset_refs,
        group_indexes,
    };
    write_bounded_json(&config.output_dir, Path::new(MANIFEST_FILE), &manifest)?;
    Ok(manifest)
}

fn collect_group_sources(
    workspace: &TranslationWorkspaceSource,
    selector: &super::selector_translation_source::SelectorTranslationSource,
) -> Result<BTreeMap<String, ConflictGroupSource>> {
    let mut result = BTreeMap::new();
    for (owner, groups) in &workspace.groups_by_owner {
        for group in groups {
            insert_primary_group(&mut result, owner, group)?;
        }
    }
    for (&canonical_selector, groups) in &selector.groups_by_selector {
        for group in groups {
            let source = ConflictGroupSource {
                translation_scope: "selector",
                primary_owner: None,
                canonical_selector: Some(canonical_selector),
                target_selectors: group.target_selectors.clone(),
                selector_consumer_evidence: Some(
                    serde_json::to_value(group.consumer_evidence)?
                        .as_str()
                        .context("selector evidence did not serialize as a string")?
                        .to_string(),
                ),
                source_segments: group.source_segments.clone(),
                context_occurrence_count: group.coordinate_count,
            };
            ensure!(
                result
                    .insert(group.semantic_source_sha256.clone(), source)
                    .is_none(),
                "primary and selector source semantic hashes overlap"
            );
        }
    }
    Ok(result)
}

fn insert_primary_group(
    result: &mut BTreeMap<String, ConflictGroupSource>,
    owner: &DialogueTranslationRouteOwner,
    group: &DialogueTranslationSourceGroup,
) -> Result<()> {
    let source = ConflictGroupSource {
        translation_scope: "primary",
        primary_owner: Some(owner.clone()),
        canonical_selector: None,
        target_selectors: Vec::new(),
        selector_consumer_evidence: None,
        source_segments: group.source_segments.clone(),
        context_occurrence_count: group.context_occurrence_count,
    };
    ensure!(
        result
            .insert(group.semantic_source_sha256.clone(), source)
            .is_none(),
        "primary source semantic hash has multiple translation owners"
    );
    Ok(())
}

fn build_group_reports(
    sources: &BTreeMap<String, ConflictGroupSource>,
    accumulators: BTreeMap<String, BTreeMap<String, GroupAssetAccumulator>>,
) -> Result<Vec<DialogueFontConflictGroup>> {
    let mut groups = Vec::new();
    for (semantic_hash, assets) in accumulators {
        let source = sources
            .get(&semantic_hash)
            .context("font-conflict group source disappeared")?;
        let mut asset_reports = Vec::new();
        let mut conflicting_code_count = 0usize;
        let mut single_group_owned_code_count = 0usize;
        for (source_path, asset) in assets {
            conflicting_code_count += asset.conflicting_codes.len();
            single_group_owned_code_count += asset.single_group_owned_codes.len();
            asset_reports.push(DialogueFontConflictGroupAsset {
                source_path,
                conflicting_codes: asset.conflicting_codes.into_iter().map(hex_code).collect(),
                single_group_owned_codes: asset
                    .single_group_owned_codes
                    .into_iter()
                    .map(hex_code)
                    .collect(),
                preserved_coordinate_count: asset.preserved_coordinate_ids.len(),
            });
        }
        groups.push(DialogueFontConflictGroup {
            planning_rank: 0,
            semantic_source_sha256: semantic_hash,
            translation_scope: source.translation_scope.to_string(),
            primary_owner: source.primary_owner.clone(),
            canonical_selector: source.canonical_selector,
            target_selectors: source.target_selectors.clone(),
            selector_consumer_evidence: source.selector_consumer_evidence.clone(),
            source_segment_count: source.source_segments.len(),
            context_occurrence_count: source.context_occurrence_count,
            conflicting_asset_count: asset_reports.len(),
            conflicting_code_count,
            single_group_owned_code_count,
            assets: asset_reports,
        });
    }
    groups.sort_by(|left, right| {
        right
            .single_group_owned_code_count
            .cmp(&left.single_group_owned_code_count)
            .then_with(|| {
                right
                    .conflicting_code_count
                    .cmp(&left.conflicting_code_count)
            })
            .then_with(|| {
                left.semantic_source_sha256
                    .cmp(&right.semantic_source_sha256)
            })
    });
    for (index, group) in groups.iter_mut().enumerate() {
        group.planning_rank = index + 1;
    }
    Ok(groups)
}
