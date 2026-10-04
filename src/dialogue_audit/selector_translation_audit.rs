use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};

use crate::pipeline::sha256_bytes;
use crate::source_disc::SupportedSourceDisc;

use super::selector_translation_model::{
    DialogueSelectorTranslationAuditConfig, DialogueSelectorTranslationAuditReport,
    DialogueSelectorTranslationContextShard, DialogueSelectorTranslationReviewShard,
    DialogueSelectorTranslationRoleManifest, DialogueSelectorTranslationRoleRef,
    DialogueSelectorTranslationScope, DialogueSelectorTranslationShard,
    DialogueSelectorTranslationShardRef, DialogueSelectorTranslationSourceShard,
    DialogueSelectorTranslationWorkspaceManifest,
};
use super::selector_translation_source::extract_selector_translation_source_from_source;
use super::selector_translation_validation::{
    SelectorTranslationAuditCounts, expected_selector_contexts, expected_selector_groups,
    validate_selector_context_entries, validate_selector_manifest,
    validate_selector_review_entries, validate_selector_source_entries,
    validate_selector_translation_entries,
};
use super::translation_workspace_io::{json_bytes, read_bounded_json};
use super::translation_workspace_model::DialogueTranslationWorkspaceRole;
use super::translation_workspace_validation::MAX_SHARD_ENTRIES;

pub fn audit_dialogue_selector_translation(
    config: &DialogueSelectorTranslationAuditConfig,
) -> Result<DialogueSelectorTranslationAuditReport> {
    Ok(audit_selector_translation_with_source(config)?.0)
}

pub(super) fn audit_selector_translation_with_source(
    config: &DialogueSelectorTranslationAuditConfig,
) -> Result<(
    DialogueSelectorTranslationAuditReport,
    super::selector_translation_source::SelectorTranslationSource,
)> {
    let source = SupportedSourceDisc::open(&config.cue)?;
    audit_selector_translation_with_verified_source(config, &source)
}

pub(super) fn audit_selector_translation_with_verified_source(
    config: &DialogueSelectorTranslationAuditConfig,
    source_disc: &SupportedSourceDisc,
) -> Result<(
    DialogueSelectorTranslationAuditReport,
    super::selector_translation_source::SelectorTranslationSource,
)> {
    let (manifest, manifest_bytes): (DialogueSelectorTranslationWorkspaceManifest, _) =
        read_bounded_json(&config.translation, Path::new("manifest.json"))?;
    let scope = DialogueSelectorTranslationScope::from_target_scope(&manifest.target_scope)
        .context("selector translation target scope is unknown")?;
    let source =
        extract_selector_translation_source_from_source(source_disc, &config.codebook, scope)?;
    validate_selector_manifest(&manifest, &source)?;

    let expected_groups = expected_selector_groups(&source);
    let expected_contexts = expected_selector_contexts(&source);
    let mut observed_groups = BTreeSet::new();
    let mut observed_contexts = BTreeSet::new();
    let mut registered_paths = BTreeSet::from([PathBuf::from("manifest.json")]);
    let mut counts = SelectorTranslationAuditCounts::default();
    counts.observe_json(manifest_bytes.len());

    for selector_ref in &manifest.selectors {
        let selector_path = PathBuf::from(&selector_ref.manifest_path);
        ensure!(
            registered_paths.insert(selector_path.clone()),
            "selector manifest path is duplicated"
        );
        let (selector, bytes): (
            super::selector_translation_model::DialogueSelectorTranslationSelectorManifest,
            _,
        ) = read_bounded_json(&config.translation, &selector_path)?;
        counts.observe_json(bytes.len());
        ensure!(
            sha256_bytes(&bytes) == selector_ref.manifest_sha256
                && selector.canonical_selector == selector_ref.canonical_selector
                && selector.semantic_group_count == selector_ref.semantic_group_count
                && selector.coordinate_count == selector_ref.coordinate_count,
            "selector manifest binding changed for {}",
            selector_ref.canonical_selector
        );
        let roles = load_role_shards(
            &config.translation,
            selector.canonical_selector,
            &selector.roles,
            &mut registered_paths,
            &mut counts,
        )?;
        audit_selector_shards(
            &config.translation,
            selector.canonical_selector,
            selector.semantic_group_count,
            selector.coordinate_count,
            &roles,
            &expected_groups,
            &expected_contexts,
            &mut observed_groups,
            &mut observed_contexts,
            &mut registered_paths,
            &mut counts,
        )?;
    }

    ensure!(
        observed_groups.len() == expected_groups.len()
            && observed_contexts.len() == expected_contexts.len()
            && counts.source_group_count == manifest.semantic_group_count
            && counts.context_count == manifest.coordinate_count,
        "selector translation workspace coverage changed"
    );
    ensure!(
        registered_paths == json_files(&config.translation)?,
        "selector translation workspace contains unregistered or missing JSON"
    );

    let development_authored_group_count = counts.authored_group_count();
    let development_full_selector_input_available =
        counts.untranslated_group_count == 0 && counts.source_group_count != 0;
    let release_candidate_selector_input_eligible = development_full_selector_input_available
        && counts.ready_for_review_group_count == counts.source_group_count
        && counts.approved_group_count == counts.source_group_count;
    let report = DialogueSelectorTranslationAuditReport {
        kind: "Justice Gakuen 2 selector translation workspace audit".to_string(),
        source_bin_sha256: manifest.source_bin_sha256,
        codebook_sha256: manifest.codebook_sha256,
        source_corpus_sha256: manifest.source_corpus_sha256,
        selector_consumer_audit_sha256: manifest.selector_consumer_audit_sha256,
        target_scope: scope.target_scope().to_string(),
        source_asset_count: source.source_asset_count,
        selector_count: manifest.selector_count,
        semantic_group_count: manifest.semantic_group_count,
        coordinate_count: manifest.coordinate_count,
        authored_translation_target_group_count: manifest.authored_translation_target_group_count,
        runtime_insertion_rewrite_group_count: manifest.runtime_insertion_rewrite_group_count,
        untranslated_group_count: counts.untranslated_group_count,
        draft_group_count: counts.draft_group_count,
        ready_for_review_group_count: counts.ready_for_review_group_count,
        pending_review_group_count: counts.pending_review_group_count,
        changes_requested_group_count: counts.changes_requested_group_count,
        approved_group_count: counts.approved_group_count,
        development_authored_group_count,
        development_full_selector_input_available,
        release_candidate_selector_input_eligible,
        largest_json_byte_count: counts.largest_json_byte_count,
    };
    write_report(&config.output, &report)?;
    Ok((report, source))
}

#[derive(Default)]
struct RoleShards {
    source: Vec<DialogueSelectorTranslationShardRef>,
    context: Vec<DialogueSelectorTranslationShardRef>,
    translation: Vec<DialogueSelectorTranslationShardRef>,
    review: Vec<DialogueSelectorTranslationShardRef>,
}

fn load_role_shards(
    root: &Path,
    canonical_selector: usize,
    roles: &[DialogueSelectorTranslationRoleRef],
    registered_paths: &mut BTreeSet<PathBuf>,
    counts: &mut SelectorTranslationAuditCounts,
) -> Result<RoleShards> {
    ensure!(roles.len() == 4, "selector manifest role count changed");
    let mut result = RoleShards::default();
    let mut observed_roles = BTreeSet::new();
    for role_ref in roles {
        ensure!(
            observed_roles.insert(role_ref.role),
            "selector manifest role is duplicated"
        );
        let mut shards = Vec::new();
        let mut manifest_entry_count = 0usize;
        for manifest_ref in &role_ref.manifests {
            let path = PathBuf::from(&manifest_ref.manifest_path);
            ensure!(
                registered_paths.insert(path.clone()),
                "selector role manifest path is duplicated"
            );
            let (manifest, bytes): (DialogueSelectorTranslationRoleManifest, _) =
                read_bounded_json(root, &path)?;
            counts.observe_json(bytes.len());
            ensure!(
                sha256_bytes(&bytes) == manifest_ref.manifest_sha256
                    && manifest.canonical_selector == canonical_selector
                    && manifest.role == role_ref.role
                    && manifest.shard_count == manifest_ref.shard_count
                    && manifest.entry_count == manifest_ref.entry_count
                    && manifest.shard_count == manifest.shards.len(),
                "selector role manifest binding changed at {}",
                path.display()
            );
            manifest_entry_count += manifest.entry_count;
            shards.extend(manifest.shards);
        }
        ensure!(
            role_ref.manifest_count == role_ref.manifests.len()
                && role_ref.shard_count == shards.len()
                && role_ref.entry_count == manifest_entry_count,
            "selector role aggregate changed"
        );
        match role_ref.role {
            DialogueTranslationWorkspaceRole::Source => result.source = shards,
            DialogueTranslationWorkspaceRole::Context => result.context = shards,
            DialogueTranslationWorkspaceRole::Translation => result.translation = shards,
            DialogueTranslationWorkspaceRole::Review => result.review = shards,
        }
    }
    Ok(result)
}

#[allow(clippy::too_many_arguments)]
fn audit_selector_shards(
    root: &Path,
    canonical_selector: usize,
    semantic_group_count: usize,
    coordinate_count: usize,
    roles: &RoleShards,
    expected_groups: &super::selector_translation_validation::ExpectedSelectorGroups,
    expected_contexts: &super::selector_translation_validation::ExpectedSelectorContexts,
    observed_groups: &mut BTreeSet<String>,
    observed_contexts: &mut BTreeSet<String>,
    registered_paths: &mut BTreeSet<PathBuf>,
    counts: &mut SelectorTranslationAuditCounts,
) -> Result<()> {
    ensure!(
        roles.source.len() == roles.translation.len()
            && roles.source.len() == roles.review.len()
            && roles
                .source
                .iter()
                .map(|shard| shard.entry_count)
                .sum::<usize>()
                == semantic_group_count
            && roles
                .context
                .iter()
                .map(|shard| shard.entry_count)
                .sum::<usize>()
                == coordinate_count,
        "selector role shard aggregates changed"
    );
    for ((source_ref, translation_ref), review_ref) in roles
        .source
        .iter()
        .zip(&roles.translation)
        .zip(&roles.review)
    {
        let source_path = register_shard(source_ref, registered_paths)?;
        let (source, source_bytes): (DialogueSelectorTranslationSourceShard, _) =
            read_bounded_json(root, &source_path)?;
        counts.observe_json(source_bytes.len());
        let source_sha256 = sha256_bytes(&source_bytes);
        ensure!(
            source_ref.content_sha256.as_deref() == Some(source_sha256.as_str())
                && source_ref.source_shard_sha256.is_none()
                && source.canonical_selector == canonical_selector
                && source.entries.len() == source_ref.entry_count
                && source.entries.len() <= MAX_SHARD_ENTRIES,
            "selector source shard binding changed at {}",
            source_path.display()
        );
        validate_selector_source_entries(
            canonical_selector,
            &source.entries,
            expected_groups,
            observed_groups,
        )?;
        counts.source_group_count += source.entries.len();

        let translation_path = register_shard(translation_ref, registered_paths)?;
        let (translation, translation_bytes): (DialogueSelectorTranslationShard, _) =
            read_bounded_json(root, &translation_path)?;
        counts.observe_json(translation_bytes.len());
        ensure!(
            translation_ref.content_sha256.is_none()
                && translation_ref.source_shard_sha256.as_deref() == Some(source_sha256.as_str())
                && translation.source_shard_sha256 == source_sha256
                && translation.entries.len() == translation_ref.entry_count,
            "selector translation shard binding changed at {}",
            translation_path.display()
        );
        validate_selector_translation_entries(&translation.entries, &source.entries, counts)?;

        let review_path = register_shard(review_ref, registered_paths)?;
        let (review, review_bytes): (DialogueSelectorTranslationReviewShard, _) =
            read_bounded_json(root, &review_path)?;
        counts.observe_json(review_bytes.len());
        ensure!(
            review_ref.content_sha256.is_none()
                && review_ref.source_shard_sha256.as_deref() == Some(source_sha256.as_str())
                && review.source_shard_sha256 == source_sha256
                && review.entries.len() == review_ref.entry_count,
            "selector review shard binding changed at {}",
            review_path.display()
        );
        validate_selector_review_entries(
            &review,
            &translation_ref.path,
            &source.entries,
            &translation.entries,
            counts,
        )?;
    }

    for context_ref in &roles.context {
        let path = register_shard(context_ref, registered_paths)?;
        let (context, bytes): (DialogueSelectorTranslationContextShard, _) =
            read_bounded_json(root, &path)?;
        counts.observe_json(bytes.len());
        ensure!(
            context_ref.content_sha256.as_deref() == Some(sha256_bytes(&bytes).as_str())
                && context_ref.source_shard_sha256.is_none()
                && context.canonical_selector == canonical_selector
                && context.entries.len() == context_ref.entry_count
                && context.entries.len() <= MAX_SHARD_ENTRIES,
            "selector context shard binding changed at {}",
            path.display()
        );
        validate_selector_context_entries(
            canonical_selector,
            &context.entries,
            expected_contexts,
            observed_contexts,
        )?;
        counts.context_count += context.entries.len();
    }
    Ok(())
}

fn register_shard(
    shard: &DialogueSelectorTranslationShardRef,
    registered_paths: &mut BTreeSet<PathBuf>,
) -> Result<PathBuf> {
    ensure!(
        shard.entry_count <= MAX_SHARD_ENTRIES,
        "selector shard entry limit changed"
    );
    let path = PathBuf::from(&shard.path);
    ensure!(
        registered_paths.insert(path.clone()),
        "selector shard path is duplicated"
    );
    Ok(path)
}

pub(super) fn load_validated_authored_selector_segments(
    root: &Path,
    expected_authored_group_count: usize,
) -> Result<BTreeMap<String, Vec<String>>> {
    let mut result = BTreeMap::new();
    for path in json_files(root)? {
        let (value, _): (serde_json::Value, _) = read_bounded_json(root, &path)?;
        if value.get("kind").and_then(serde_json::Value::as_str)
            != Some("Justice Gakuen 2 Korean selector translation shard")
        {
            continue;
        }
        let shard: DialogueSelectorTranslationShard = serde_json::from_value(value)
            .with_context(|| format!("failed to parse selector translation {}", path.display()))?;
        for decision in shard.entries {
            if decision.development_resolution
                != super::selector_translation_model::DialogueSelectorDevelopmentResolution::AuthoredTranslation
                || decision.status
                    == super::translation_workspace_model::DialogueTranslationDecisionStatus::Untranslated
            {
                continue;
            }
            let segments = decision
                .korean_segments
                .into_iter()
                .map(|segment| segment.context("validated selector segment disappeared"))
                .collect::<Result<Vec<_>>>()?;
            ensure!(
                result
                    .insert(decision.semantic_source_sha256, segments)
                    .is_none(),
                "validated authored selector semantic hash is duplicated"
            );
        }
    }
    ensure!(
        result.len() == expected_authored_group_count,
        "validated authored selector group count changed"
    );
    Ok(result)
}

fn json_files(root: &Path) -> Result<BTreeSet<PathBuf>> {
    let mut files = BTreeSet::new();
    collect_json_files(root, root, &mut files)?;
    Ok(files)
}

fn collect_json_files(root: &Path, directory: &Path, files: &mut BTreeSet<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(directory)
        .with_context(|| format!("failed to list {}", directory.display()))?
    {
        let path = entry?.path();
        if path.is_dir() {
            collect_json_files(root, &path, files)?;
        } else if path.extension().and_then(|value| value.to_str()) == Some("json") {
            files.insert(path.strip_prefix(root)?.to_path_buf());
        }
    }
    Ok(())
}

fn write_report(path: &Path, report: &DialogueSelectorTranslationAuditReport) -> Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    std::fs::write(path, json_bytes(report)?)
        .with_context(|| format!("failed to write {}", path.display()))
}

#[cfg(test)]
pub(super) fn audit_test_readiness(
    source_group_count: usize,
    untranslated_group_count: usize,
    ready_for_review_group_count: usize,
    approved_group_count: usize,
) -> (bool, bool) {
    let development = source_group_count != 0 && untranslated_group_count == 0;
    let release = development
        && ready_for_review_group_count == source_group_count
        && approved_group_count == source_group_count;
    (development, release)
}
