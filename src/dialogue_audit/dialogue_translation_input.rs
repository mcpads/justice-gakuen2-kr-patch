use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Context, Result, ensure};

use crate::pipeline::sha256_bytes;
use crate::source_disc::SupportedSourceDisc;

use super::dialogue_translation_assets::{
    ValidatedDialogueTranslationAssets, load_validated_dialogue_translation_assets,
};
use super::dialogue_translation_assets_model::{
    DialogueTranslationAssetManifest, TrackedPrimaryDialogueTranslationShard,
    TrackedSelectorDialogueTranslationShard,
};
use super::selector_translation_audit::{
    audit_selector_translation_with_verified_source, load_validated_authored_selector_segments,
};
use super::selector_translation_model::{
    DialogueSelectorDevelopmentResolution, DialogueSelectorTranslationAuditConfig,
    DialogueSelectorTranslationAuditReport, DialogueSelectorTranslationScope,
};
use super::selector_translation_source::{
    SelectorTranslationSource, extract_selector_translation_source_from_corpus_and_source,
    extract_selector_translation_source_from_source_with_bound_corpus_sha256,
};
use super::translation_model::{DialogueTranslationAuditConfig, DialogueTranslationScope};
use super::translation_workspace_audit::{
    audit_translation_workspace_with_verified_source, load_validated_authored_translation_segments,
};
use super::translation_workspace_io::{json_bytes, read_bounded_json};
use super::translation_workspace_model::{
    DialogueTranslationAssetRepertoire, DialogueTranslationDecisionStatus,
    DialogueTranslationWorkspaceAuditReport,
};
use super::translation_workspace_source::{
    TranslationWorkspaceSource,
    extract_translation_workspace_source_from_source_with_bound_corpus_sha256,
};

const TRACKED_ASSET_ROOT_KIND: &str = "Justice Gakuen 2 tracked dialogue translation assets";

pub(super) struct PrimaryDialogueTranslationInput {
    pub(super) report: DialogueTranslationWorkspaceAuditReport,
    pub(super) source: TranslationWorkspaceSource,
    pub(super) authored_segments: BTreeMap<String, Vec<String>>,
    pub(super) decision_paths: BTreeMap<String, String>,
}

pub(super) struct SelectorDialogueTranslationInput {
    pub(super) report: DialogueSelectorTranslationAuditReport,
    pub(super) source: SelectorTranslationSource,
    pub(super) authored_segments: BTreeMap<String, Vec<String>>,
}

pub(super) fn load_dialogue_translation_inputs_from_source(
    primary_config: &DialogueTranslationAuditConfig,
    selector_config: &DialogueSelectorTranslationAuditConfig,
    source: &SupportedSourceDisc,
) -> Result<(
    PrimaryDialogueTranslationInput,
    SelectorDialogueTranslationInput,
)> {
    if same_existing_directory(&primary_config.translation, &selector_config.translation)?
        && is_tracked_asset_root(&primary_config.translation)?
    {
        let assets = load_validated_dialogue_translation_assets(&primary_config.translation)?;
        let primary_input =
            load_tracked_primary_input_from_validated_assets(primary_config, source, &assets)?;
        let selector_source = extract_selector_translation_source_from_corpus_and_source(
            source,
            &primary_input.source.corpus,
            assets.manifest.source_corpus_sha256.clone(),
            DialogueSelectorTranslationScope::AllRuntimeImages,
        )?;
        let selector_input = load_tracked_selector_input_from_validated_assets(
            selector_config,
            selector_source,
            &assets,
        )?;
        return Ok((primary_input, selector_input));
    }

    let primary_input =
        load_primary_dialogue_translation_input_from_source(primary_config, source)?;
    let selector_input = load_selector_dialogue_translation_input_with_sources(
        selector_config,
        &primary_input.source,
        source,
    )?;
    Ok((primary_input, selector_input))
}

pub(super) fn load_primary_dialogue_translation_input(
    config: &DialogueTranslationAuditConfig,
) -> Result<PrimaryDialogueTranslationInput> {
    let source = SupportedSourceDisc::open(&config.cue)?;
    load_primary_dialogue_translation_input_from_source(config, &source)
}

pub(super) fn load_primary_dialogue_translation_input_from_source(
    config: &DialogueTranslationAuditConfig,
    source: &SupportedSourceDisc,
) -> Result<PrimaryDialogueTranslationInput> {
    if !is_tracked_asset_root(&config.translation)? {
        let (report, source) = audit_translation_workspace_with_verified_source(config, source)?;
        let authored_count = report
            .semantic_group_count
            .checked_sub(report.untranslated_group_count)
            .context("dialogue authored group count underflow")?;
        let authored_segments =
            load_validated_authored_translation_segments(&config.translation, authored_count)?;
        let decision_paths = authored_segments
            .keys()
            .map(|semantic_id| {
                (
                    semantic_id.clone(),
                    format!("{}#{semantic_id}", config.translation.display()),
                )
            })
            .collect();
        return Ok(PrimaryDialogueTranslationInput {
            report,
            source,
            authored_segments,
            decision_paths,
        });
    }

    let assets = load_validated_dialogue_translation_assets(&config.translation)?;
    load_tracked_primary_input_from_validated_assets(config, source, &assets)
}

pub(super) fn load_selector_dialogue_translation_input(
    config: &DialogueSelectorTranslationAuditConfig,
) -> Result<SelectorDialogueTranslationInput> {
    let source = SupportedSourceDisc::open(&config.cue)?;
    load_selector_dialogue_translation_input_from_source(config, &source)
}

pub(super) fn load_selector_dialogue_translation_input_from_source(
    config: &DialogueSelectorTranslationAuditConfig,
    source: &SupportedSourceDisc,
) -> Result<SelectorDialogueTranslationInput> {
    if !is_tracked_asset_root(&config.translation)? {
        let (report, source) = audit_selector_translation_with_verified_source(config, source)?;
        let authored_count = report
            .development_authored_group_count
            .checked_sub(report.runtime_insertion_rewrite_group_count)
            .context("selector authored group count underflow")?;
        let authored_segments =
            load_validated_authored_selector_segments(&config.translation, authored_count)?;
        return Ok(SelectorDialogueTranslationInput {
            report,
            source,
            authored_segments,
        });
    }

    load_tracked_selector_input_from_source(config, source)
}

pub(super) fn load_selector_dialogue_translation_input_with_sources(
    config: &DialogueSelectorTranslationAuditConfig,
    primary_source: &TranslationWorkspaceSource,
    source: &SupportedSourceDisc,
) -> Result<SelectorDialogueTranslationInput> {
    if !is_tracked_asset_root(&config.translation)? {
        return load_selector_dialogue_translation_input_from_source(config, source);
    }
    let assets = load_validated_dialogue_translation_assets(&config.translation)?;
    let selector_source = extract_selector_translation_source_from_corpus_and_source(
        source,
        &primary_source.corpus,
        assets.manifest.source_corpus_sha256.clone(),
        DialogueSelectorTranslationScope::AllRuntimeImages,
    )?;
    load_tracked_selector_input_from_validated_assets(config, selector_source, &assets)
}

fn is_tracked_asset_root(root: &Path) -> Result<bool> {
    let (manifest, _): (serde_json::Value, _) =
        read_bounded_json(root, Path::new("manifest.json"))?;
    Ok(manifest.get("kind").and_then(serde_json::Value::as_str) == Some(TRACKED_ASSET_ROOT_KIND))
}

fn same_existing_directory(left: &Path, right: &Path) -> Result<bool> {
    let left = std::fs::canonicalize(left)
        .with_context(|| format!("failed to resolve translation root {}", left.display()))?;
    let right = std::fs::canonicalize(right)
        .with_context(|| format!("failed to resolve translation root {}", right.display()))?;
    Ok(left == right)
}

fn load_tracked_primary_input_from_validated_assets(
    config: &DialogueTranslationAuditConfig,
    source_disc: &SupportedSourceDisc,
    assets: &ValidatedDialogueTranslationAssets,
) -> Result<PrimaryDialogueTranslationInput> {
    let asset_report = &assets.report;
    let manifest = &assets.manifest;
    let source = extract_translation_workspace_source_from_source_with_bound_corpus_sha256(
        source_disc,
        &config.codebook,
        DialogueTranslationScope::ResolvedPrimary,
        Some(&manifest.source_corpus_sha256),
    )?;
    validate_primary_source_binding(manifest, &source, asset_report.primary_decision_count)?;

    let mut expected = source
        .groups_by_owner
        .iter()
        .flat_map(|(owner, groups)| {
            groups.iter().map(move |group| {
                (
                    group.semantic_source_sha256.clone(),
                    (owner.clone(), group.clone()),
                )
            })
        })
        .collect::<BTreeMap<_, _>>();
    ensure!(
        expected.len() == asset_report.primary_decision_count,
        "tracked primary semantic decision denominator changed"
    );

    let mut counts = StatusCounts::default();
    let mut authored_segments = BTreeMap::new();
    let mut decision_paths = BTreeMap::new();
    let mut repertoire_by_asset = BTreeMap::<String, BTreeSet<char>>::new();
    let mut primary_shard_count = 0usize;
    let mut largest_shard_byte_count = 0usize;
    let mut largest_shard_entry_count = 0usize;
    for validated_shard in &assets.primary_shards {
        let path = &validated_shard.path;
        let (shard, bytes): (TrackedPrimaryDialogueTranslationShard, _) =
            read_bounded_json(&config.translation, path)?;
        ensure!(
            bytes.len() == validated_shard.json_byte_count
                && sha256_bytes(&bytes) == validated_shard.content_sha256,
            "tracked primary translation shard changed after validation at {}",
            path.display()
        );
        primary_shard_count += 1;
        largest_shard_byte_count = largest_shard_byte_count.max(validated_shard.json_byte_count);
        largest_shard_entry_count = largest_shard_entry_count.max(shard.entries.len());
        for entry in shard.entries {
            let (owner, group) = expected
                .remove(&entry.semantic_source_sha256)
                .with_context(|| {
                    format!(
                        "tracked primary decision is absent from current source: {}",
                        entry.semantic_source_sha256
                    )
                })?;
            ensure!(
                shard.owner == owner
                    && entry.source_segments == group.source_segments
                    && entry.controls == group.controls,
                "tracked primary source/control binding changed for {}",
                entry.semantic_source_sha256
            );
            counts.observe(entry.status);
            if let Some(segments) = authored_segments_for(
                &entry.semantic_source_sha256,
                entry.status,
                &entry.korean_segments,
            )? {
                repertoire_by_asset
                    .entry(owner.source_path)
                    .or_default()
                    .extend(non_whitespace_characters(&segments));
                ensure!(
                    authored_segments
                        .insert(entry.semantic_source_sha256.clone(), segments)
                        .is_none(),
                    "tracked primary authored decision is duplicated"
                );
                decision_paths.insert(
                    entry.semantic_source_sha256.clone(),
                    path.to_string_lossy().into_owned(),
                );
            }
        }
    }
    ensure!(
        expected.is_empty()
            && primary_shard_count == asset_report.primary_shard_count
            && counts.total() == asset_report.primary_decision_count,
        "tracked primary translation coverage is incomplete"
    );

    let korean_repertoire = authored_segments
        .values()
        .flat_map(|segments| non_whitespace_characters(segments))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<String>();
    let asset_repertoires = repertoire_by_asset
        .into_iter()
        .map(|(source_path, repertoire)| {
            let korean_repertoire = repertoire.into_iter().collect::<String>();
            DialogueTranslationAssetRepertoire {
                source_path,
                korean_character_count: korean_repertoire.chars().count(),
                korean_repertoire,
            }
        })
        .collect();
    let semantic_group_count = counts.total();
    let report = DialogueTranslationWorkspaceAuditReport {
        kind: "Justice Gakuen 2 tracked primary dialogue development input audit".to_string(),
        source_bin_sha256: manifest.source_bin_sha256.clone(),
        codebook_sha256: manifest.codebook_sha256.clone(),
        source_corpus_sha256: manifest.source_corpus_sha256.clone(),
        script_inventory_sha256: manifest.primary_script_inventory_sha256.clone(),
        asset_count: source.source_asset_count,
        source_shard_count: primary_shard_count,
        context_shard_count: 0,
        translation_shard_count: primary_shard_count,
        review_shard_count: 0,
        semantic_group_count,
        referenced_coordinate_count: source.referenced_coordinates.len(),
        context_occurrence_count: source.contexts_by_owner.values().map(Vec::len).sum(),
        untranslated_group_count: counts.untranslated,
        draft_group_count: counts.draft,
        ready_for_review_group_count: counts.ready_for_review,
        pending_review_group_count: semantic_group_count,
        changes_requested_group_count: 0,
        approved_group_count: 0,
        korean_character_count: korean_repertoire.chars().count(),
        korean_repertoire,
        asset_repertoires,
        largest_shard_entry_count,
        largest_shard_byte_count,
        ready_for_font_repertoire: counts.untranslated == 0,
        development_translation_input_available: counts.untranslated == 0,
        release_candidate_translation_input_eligible: false,
        translation_project_complete: false,
    };
    write_report(&config.output, &report)?;
    Ok(PrimaryDialogueTranslationInput {
        report,
        source,
        authored_segments,
        decision_paths,
    })
}

fn load_tracked_selector_input_from_source(
    config: &DialogueSelectorTranslationAuditConfig,
    source_disc: &SupportedSourceDisc,
) -> Result<SelectorDialogueTranslationInput> {
    let assets = load_validated_dialogue_translation_assets(&config.translation)?;
    let source = extract_selector_translation_source_from_source_with_bound_corpus_sha256(
        source_disc,
        &config.codebook,
        DialogueSelectorTranslationScope::AllRuntimeImages,
        &assets.manifest.source_corpus_sha256,
    )?;
    load_tracked_selector_input_from_validated_assets(config, source, &assets)
}

fn load_tracked_selector_input_from_validated_assets(
    config: &DialogueSelectorTranslationAuditConfig,
    source: SelectorTranslationSource,
    assets: &ValidatedDialogueTranslationAssets,
) -> Result<SelectorDialogueTranslationInput> {
    let asset_report = &assets.report;
    let manifest = &assets.manifest;
    validate_selector_source_binding(manifest, &source, asset_report.selector_decision_count)?;

    let mut expected = source
        .groups_by_selector
        .iter()
        .flat_map(|(selector, groups)| {
            groups.iter().map(move |group| {
                (
                    group.semantic_source_sha256.clone(),
                    (*selector, group.clone()),
                )
            })
        })
        .collect::<BTreeMap<_, _>>();
    ensure!(
        expected.len() == asset_report.selector_decision_count,
        "tracked selector semantic decision denominator changed"
    );

    let mut counts = StatusCounts::default();
    let mut authored_segments = BTreeMap::new();
    let mut selector_shard_count = 0usize;
    let mut largest_json_byte_count = 0usize;
    for validated_shard in &assets.selector_shards {
        let path = &validated_shard.path;
        let (shard, bytes): (TrackedSelectorDialogueTranslationShard, _) =
            read_bounded_json(&config.translation, path)?;
        ensure!(
            bytes.len() == validated_shard.json_byte_count
                && sha256_bytes(&bytes) == validated_shard.content_sha256,
            "tracked selector translation shard changed after validation at {}",
            path.display()
        );
        selector_shard_count += 1;
        largest_json_byte_count = largest_json_byte_count.max(validated_shard.json_byte_count);
        for entry in shard.entries {
            let (canonical_selector, group) = expected
                .remove(&entry.semantic_source_sha256)
                .with_context(|| {
                    format!(
                        "tracked selector decision is absent from current source: {}",
                        entry.semantic_source_sha256
                    )
                })?;
            ensure!(
                shard.canonical_selector == canonical_selector
                    && entry.source_segments == group.source_segments
                    && entry.controls == group.controls
                    && entry.target_selectors == group.target_selectors
                    && entry.development_resolution == group.development_resolution,
                "tracked selector source/control binding changed for {}",
                entry.semantic_source_sha256
            );
            counts.observe(entry.status);
            let segments = authored_segments_for(
                &entry.semantic_source_sha256,
                entry.status,
                &entry.korean_segments,
            )?;
            match group.development_resolution {
                DialogueSelectorDevelopmentResolution::AuthoredTranslation => {
                    if let Some(segments) = segments {
                        ensure!(
                            authored_segments
                                .insert(entry.semantic_source_sha256.clone(), segments)
                                .is_none(),
                            "tracked selector authored decision is duplicated"
                        );
                    }
                }
                DialogueSelectorDevelopmentResolution::RuntimeInsertionRewrite => {
                    let segments = segments.context(
                        "tracked runtime-insertion selector decision is not development-ready",
                    )?;
                    ensure!(
                        group.development_korean_segments.as_ref() == Some(&segments),
                        "tracked runtime-insertion selector rewrite changed for {}",
                        entry.semantic_source_sha256
                    );
                }
            }
        }
    }
    ensure!(
        expected.is_empty()
            && selector_shard_count == asset_report.selector_shard_count
            && counts.total() == asset_report.selector_decision_count,
        "tracked selector translation coverage is incomplete"
    );

    let groups = source
        .groups_by_selector
        .values()
        .flatten()
        .collect::<Vec<_>>();
    let count_resolution = |resolution| {
        groups
            .iter()
            .filter(|group| group.development_resolution == resolution)
            .count()
    };
    let runtime_insertion_rewrite_group_count =
        count_resolution(DialogueSelectorDevelopmentResolution::RuntimeInsertionRewrite);
    let authored_translation_target_group_count =
        count_resolution(DialogueSelectorDevelopmentResolution::AuthoredTranslation);
    let semantic_group_count = counts.total();
    ensure!(
        authored_segments.len() + runtime_insertion_rewrite_group_count
            == semantic_group_count - counts.untranslated,
        "tracked selector development input aggregate changed"
    );
    let report = DialogueSelectorTranslationAuditReport {
        kind: "Justice Gakuen 2 tracked selector dialogue development input audit".to_string(),
        source_bin_sha256: manifest.source_bin_sha256.clone(),
        codebook_sha256: manifest.codebook_sha256.clone(),
        source_corpus_sha256: manifest.source_corpus_sha256.clone(),
        selector_consumer_audit_sha256: manifest.selector_consumer_audit_sha256.clone(),
        target_scope: source.scope.target_scope().to_string(),
        source_asset_count: source.source_asset_count,
        selector_count: source.groups_by_selector.len(),
        semantic_group_count,
        coordinate_count: source.contexts_by_selector.values().map(Vec::len).sum(),
        authored_translation_target_group_count,
        runtime_insertion_rewrite_group_count,
        untranslated_group_count: counts.untranslated,
        draft_group_count: counts.draft,
        ready_for_review_group_count: counts.ready_for_review,
        pending_review_group_count: semantic_group_count,
        changes_requested_group_count: 0,
        approved_group_count: 0,
        development_authored_group_count: semantic_group_count - counts.untranslated,
        development_full_selector_input_available: counts.untranslated == 0,
        release_candidate_selector_input_eligible: false,
        largest_json_byte_count,
    };
    write_report(&config.output, &report)?;
    Ok(SelectorDialogueTranslationInput {
        report,
        source,
        authored_segments,
    })
}

fn validate_primary_source_binding(
    manifest: &DialogueTranslationAssetManifest,
    source: &TranslationWorkspaceSource,
    primary_decision_count: usize,
) -> Result<()> {
    ensure!(
        manifest.source_bin_sha256 == source.source_bin_sha256
            && manifest.codebook_sha256 == source.codebook_sha256
            && manifest.source_corpus_sha256 == source.source_corpus_sha256
            && manifest.primary_script_inventory_sha256 == source.script_inventory_sha256
            && manifest.primary_target_scope == source.target_scope
            && manifest.primary_source_asset_count == source.source_asset_count
            && manifest.primary_decision_count == primary_decision_count
            && source.unresolved_primary_script_assets.is_empty(),
        "tracked primary translations do not match the exact source CUE and codebook"
    );
    Ok(())
}

fn validate_selector_source_binding(
    manifest: &DialogueTranslationAssetManifest,
    source: &SelectorTranslationSource,
    selector_decision_count: usize,
) -> Result<()> {
    ensure!(
        manifest.source_bin_sha256 == source.source_bin_sha256
            && manifest.codebook_sha256 == source.codebook_sha256
            && manifest.source_corpus_sha256 == source.source_corpus_sha256
            && manifest.selector_consumer_audit_sha256 == source.selector_consumer_audit_sha256
            && manifest.selector_target_scope == source.scope.target_scope()
            && manifest.primary_source_asset_count == source.source_asset_count
            && manifest.selector_owner_count == source.groups_by_selector.len()
            && manifest.selector_decision_count == selector_decision_count,
        "tracked selector translations do not match the exact source CUE and codebook"
    );
    Ok(())
}

fn authored_segments_for(
    semantic_id: &str,
    status: DialogueTranslationDecisionStatus,
    segments: &[Option<String>],
) -> Result<Option<Vec<String>>> {
    if status == DialogueTranslationDecisionStatus::Untranslated {
        ensure!(
            segments.iter().all(Option::is_none),
            "untranslated tracked decision contains authored text for {semantic_id}"
        );
        return Ok(None);
    }
    segments
        .iter()
        .map(|segment| {
            segment
                .clone()
                .context("tracked authored segment disappeared")
        })
        .collect::<Result<Vec<_>>>()
        .map(Some)
}

fn non_whitespace_characters(segments: &[String]) -> impl Iterator<Item = char> + '_ {
    segments
        .iter()
        .flat_map(|segment| segment.chars())
        .filter(|character| !character.is_whitespace())
}

fn write_report(path: &Path, report: &impl serde::Serialize) -> Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    std::fs::write(path, json_bytes(report)?)
        .with_context(|| format!("failed to write {}", path.display()))
}

#[derive(Debug, Default, Clone, Copy)]
struct StatusCounts {
    untranslated: usize,
    draft: usize,
    ready_for_review: usize,
}

impl StatusCounts {
    fn observe(&mut self, status: DialogueTranslationDecisionStatus) {
        match status {
            DialogueTranslationDecisionStatus::Untranslated => self.untranslated += 1,
            DialogueTranslationDecisionStatus::Draft => self.draft += 1,
            DialogueTranslationDecisionStatus::ReadyForReview => self.ready_for_review += 1,
        }
    }

    fn total(self) -> usize {
        self.untranslated + self.draft + self.ready_for_review
    }
}
