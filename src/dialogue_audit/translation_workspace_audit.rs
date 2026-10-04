use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};

use crate::pipeline::sha256_bytes;
use crate::source_disc::SupportedSourceDisc;

use super::translation_model::{
    DialogueTranslationAuditConfig, DialogueTranslationProjectStatus, DialogueTranslationScope,
};
use super::translation_workspace_asset_index::read_asset_index;
use super::translation_workspace_io::{json_bytes, read_bounded_json};
use super::translation_workspace_model::{
    DialogueTranslationAssetManifest, DialogueTranslationAssetRepertoire,
    DialogueTranslationContextShard, DialogueTranslationDecisionStatus,
    DialogueTranslationReviewShard, DialogueTranslationRoleManifest, DialogueTranslationShard,
    DialogueTranslationSourceShard, DialogueTranslationWorkspaceAuditReport,
    DialogueTranslationWorkspaceManifest, DialogueTranslationWorkspaceRole,
};
use super::translation_workspace_source::{
    TranslationWorkspaceSource, extract_translation_workspace_source_from_source,
};
use super::translation_workspace_validation::{
    AuditCounts, MAX_SHARD_BYTES, expected_context_entries, expected_source_groups,
    validate_asset_ref, validate_manifest, validate_review_entries, validate_shard_ref,
    validate_source_entries, validate_translation_entries, validate_workspace_project_review,
};

pub(super) fn audit_translation_workspace(
    config: &DialogueTranslationAuditConfig,
) -> Result<DialogueTranslationWorkspaceAuditReport> {
    let (report, _) = audit_translation_workspace_with_source(config)?;
    Ok(report)
}

pub(super) fn audit_translation_workspace_with_source(
    config: &DialogueTranslationAuditConfig,
) -> Result<(
    DialogueTranslationWorkspaceAuditReport,
    TranslationWorkspaceSource,
)> {
    let source = SupportedSourceDisc::open(&config.cue)?;
    audit_translation_workspace_with_verified_source(config, &source)
}

pub(super) fn audit_translation_workspace_with_verified_source(
    config: &DialogueTranslationAuditConfig,
    source_disc: &SupportedSourceDisc,
) -> Result<(
    DialogueTranslationWorkspaceAuditReport,
    TranslationWorkspaceSource,
)> {
    let root = &config.translation;
    ensure!(root.is_dir(), "translation workspace must be a directory");
    let (manifest, manifest_bytes): (DialogueTranslationWorkspaceManifest, _) =
        read_bounded_json(root, Path::new("manifest.json"))?;
    let scope = DialogueTranslationScope::from_target_scope(&manifest.target_scope)
        .context("translation workspace has an unknown target scope")?;
    let source =
        extract_translation_workspace_source_from_source(source_disc, &config.codebook, scope)?;
    validate_manifest(&manifest, &source)?;

    let mut registered_paths = BTreeSet::from([PathBuf::from("manifest.json")]);
    let (asset_refs, asset_index_paths) = read_asset_index(root, &manifest)?;
    for path in asset_index_paths {
        ensure!(
            registered_paths.insert(path),
            "duplicate asset index manifest path"
        );
    }
    let source_paths = asset_refs
        .iter()
        .map(|asset| asset.source_path.as_str())
        .collect::<BTreeSet<_>>();
    ensure!(
        source_paths.len() == asset_refs.len(),
        "duplicate asset refs"
    );
    let mut role_shards = Vec::new();
    for asset_ref in &asset_refs {
        let asset_path = PathBuf::from(&asset_ref.manifest_path);
        ensure!(
            registered_paths.insert(asset_path.clone()),
            "duplicate asset manifest path"
        );
        let (asset, bytes): (DialogueTranslationAssetManifest, _) =
            read_bounded_json(root, &asset_path)?;
        ensure!(
            sha256_bytes(&bytes) == asset_ref.manifest_sha256,
            "asset manifest hash changed for {}",
            asset_ref.source_path
        );
        validate_asset_ref(asset_ref, &asset)?;
        let roles: BTreeSet<_> = asset.roles.iter().map(|role| role.role).collect();
        ensure!(
            roles
                == BTreeSet::from([
                    DialogueTranslationWorkspaceRole::Source,
                    DialogueTranslationWorkspaceRole::Context,
                    DialogueTranslationWorkspaceRole::Translation,
                    DialogueTranslationWorkspaceRole::Review,
                ]),
            "{} asset role population changed",
            asset.source_path
        );
        for role_ref in &asset.roles {
            ensure!(
                role_ref.manifest_count == role_ref.manifests.len(),
                "role manifest count changed"
            );
            let mut role_shard_count = 0usize;
            let mut role_entry_count = 0usize;
            for manifest_ref in &role_ref.manifests {
                let path = PathBuf::from(&manifest_ref.manifest_path);
                ensure!(
                    registered_paths.insert(path.clone()),
                    "duplicate role manifest path"
                );
                let (role_manifest, bytes): (DialogueTranslationRoleManifest, _) =
                    read_bounded_json(root, &path)?;
                ensure!(
                    sha256_bytes(&bytes) == manifest_ref.manifest_sha256,
                    "role manifest hash changed at {}",
                    path.display()
                );
                ensure!(
                    role_manifest.role == role_ref.role
                        && role_manifest.source_path == asset.source_path
                        && role_manifest.shard_count == role_manifest.shards.len()
                        && role_manifest.shard_count == manifest_ref.shard_count
                        && role_manifest.entry_count == manifest_ref.entry_count
                        && role_manifest.entry_count
                            == role_manifest
                                .shards
                                .iter()
                                .map(|shard| shard.entry_count)
                                .sum::<usize>(),
                    "role manifest metadata changed at {}",
                    path.display()
                );
                role_shard_count += role_manifest.shard_count;
                role_entry_count += role_manifest.entry_count;
                for shard in role_manifest.shards {
                    let shard_path = PathBuf::from(&shard.path);
                    ensure!(
                        registered_paths.insert(shard_path.clone()),
                        "workspace shard path is registered more than once: {}",
                        shard_path.display()
                    );
                    role_shards.push((role_ref.role, asset.source_path.clone(), shard));
                }
            }
            ensure!(
                role_shard_count == role_ref.shard_count
                    && role_entry_count == role_ref.entry_count,
                "role manifest aggregate changed for {}",
                asset.source_path
            );
        }
    }

    let expected_sources = expected_source_groups(&source);
    let expected_contexts = expected_context_entries(&source);
    let mut observed_sources = BTreeSet::new();
    let mut observed_contexts = BTreeSet::new();
    let mut source_by_hash = BTreeMap::new();
    let mut translation_refs = Vec::new();
    let mut review_refs = Vec::new();
    let mut counts = AuditCounts::default();

    for (role, source_path, shard_ref) in role_shards {
        match role {
            DialogueTranslationWorkspaceRole::Source => {
                let (shard, bytes): (DialogueTranslationSourceShard, _) =
                    read_bounded_json(root, Path::new(&shard_ref.path))?;
                validate_shard_ref(&shard_ref, &shard.shard_id, shard.entries.len())?;
                let hash = sha256_bytes(&bytes);
                ensure!(
                    shard_ref.content_sha256.as_deref() == Some(hash.as_str())
                        && shard_ref.source_shard_sha256.is_none(),
                    "protected source shard hash changed at {}",
                    shard_ref.path
                );
                ensure!(
                    shard.source_bin_sha256 == source.source_bin_sha256
                        && shard.codebook_sha256 == source.codebook_sha256
                        && shard.source_corpus_sha256 == source.source_corpus_sha256
                        && shard.script_inventory_sha256 == source.script_inventory_sha256
                        && shard.owner.source_path == source_path,
                    "protected source binding changed at {}",
                    shard_ref.path
                );
                validate_source_entries(
                    &shard.owner,
                    &shard.entries,
                    &expected_sources,
                    &mut observed_sources,
                )?;
                counts.observe_shard(shard.entries.len(), bytes.len());
                counts.source_shard_count += 1;
                counts.referenced_coordinate_count += shard
                    .entries
                    .iter()
                    .map(|entry| entry.coordinate_count())
                    .sum::<usize>();
                ensure!(
                    source_by_hash.insert(hash, shard).is_none(),
                    "source shard hash is duplicated"
                );
            }
            DialogueTranslationWorkspaceRole::Context => {
                let (shard, bytes): (DialogueTranslationContextShard, _) =
                    read_bounded_json(root, Path::new(&shard_ref.path))?;
                validate_shard_ref(&shard_ref, &shard.shard_id, shard.entries.len())?;
                let hash = sha256_bytes(&bytes);
                ensure!(
                    shard_ref.content_sha256.as_deref() == Some(hash.as_str())
                        && shard_ref.source_shard_sha256.is_none()
                        && shard.script_inventory_sha256 == source.script_inventory_sha256
                        && shard.owner.source_path == source_path,
                    "context shard binding changed at {}",
                    shard_ref.path
                );
                for entry in &shard.entries {
                    let expected = expected_contexts
                        .get(&entry.occurrence_id)
                        .context("context shard contains an unregistered occurrence")?;
                    ensure!(
                        expected.0 == shard.owner && expected.1 == *entry,
                        "route context changed for {}",
                        entry.occurrence_id
                    );
                    ensure!(
                        observed_contexts.insert(entry.occurrence_id.clone()),
                        "route context occurs more than once"
                    );
                }
                counts.observe_shard(shard.entries.len(), bytes.len());
                counts.context_shard_count += 1;
                counts.context_occurrence_count += shard.entries.len();
            }
            DialogueTranslationWorkspaceRole::Translation => {
                translation_refs.push((source_path, shard_ref))
            }
            DialogueTranslationWorkspaceRole::Review => review_refs.push(shard_ref),
        }
    }
    ensure!(
        observed_sources.len() == expected_sources.len()
            && observed_contexts.len() == expected_contexts.len(),
        "workspace immutable source/context coverage is incomplete"
    );

    let mut translations_by_source = BTreeMap::new();
    let mut repertoire_by_asset = BTreeMap::<String, BTreeSet<char>>::new();
    for (source_path, shard_ref) in translation_refs {
        let (shard, bytes): (DialogueTranslationShard, _) =
            read_bounded_json(root, Path::new(&shard_ref.path))?;
        validate_shard_ref(&shard_ref, &shard.shard_id, shard.entries.len())?;
        ensure!(
            shard_ref.content_sha256.is_none(),
            "translation manifest pins editable content"
        );
        let source_hash = shard_ref
            .source_shard_sha256
            .as_ref()
            .context("translation shard lacks source hash")?;
        ensure!(
            &shard.source_shard_sha256 == source_hash,
            "translation source hash changed"
        );
        let source_shard = source_by_hash
            .get(source_hash)
            .context("translation references an unknown source shard")?;
        validate_translation_entries(&shard, source_shard, &mut counts)?;
        repertoire_by_asset
            .entry(source_path)
            .or_default()
            .extend(authored_repertoire(&shard));
        counts.observe_shard(shard.entries.len(), bytes.len());
        counts.translation_shard_count += 1;
        ensure!(
            translations_by_source
                .insert(source_hash.clone(), (shard_ref.path, shard))
                .is_none(),
            "source shard has multiple active translation shards"
        );
    }
    ensure!(
        translations_by_source.len() == source_by_hash.len(),
        "not every source shard has one active translation shard"
    );

    let mut reviewed_sources = BTreeSet::new();
    for shard_ref in review_refs {
        let (shard, bytes): (DialogueTranslationReviewShard, _) =
            read_bounded_json(root, Path::new(&shard_ref.path))?;
        validate_shard_ref(&shard_ref, &shard.shard_id, shard.entries.len())?;
        ensure!(
            shard_ref.content_sha256.is_none(),
            "review manifest pins editable content"
        );
        let source_hash = shard_ref
            .source_shard_sha256
            .as_ref()
            .context("review shard lacks source hash")?;
        ensure!(
            &shard.source_shard_sha256 == source_hash,
            "review source hash changed"
        );
        let source_shard = source_by_hash
            .get(source_hash)
            .context("review references an unknown source shard")?;
        let (translation_path, translation) = translations_by_source
            .get(source_hash)
            .context("review has no active translation shard")?;
        ensure!(
            &shard.translation_path == translation_path,
            "review points to the wrong translation shard"
        );
        validate_review_entries(&shard, source_shard, translation, &mut counts)?;
        ensure!(
            reviewed_sources.insert(source_hash.clone()),
            "source shard has multiple review shards"
        );
        counts.observe_shard(shard.entries.len(), bytes.len());
        counts.review_shard_count += 1;
    }
    ensure!(
        reviewed_sources.len() == source_by_hash.len(),
        "not every source shard has one review shard"
    );

    ensure!(
        counts.semantic_group_count() == manifest.semantic_group_count
            && counts.referenced_coordinate_count == manifest.referenced_coordinate_count
            && counts.context_occurrence_count == manifest.context_occurrence_count,
        "workspace manifest totals changed"
    );
    let disk_paths = json_files(root)?;
    ensure!(
        disk_paths == registered_paths,
        "workspace contains missing or unregistered JSON files"
    );

    let ready_for_font_repertoire = counts.untranslated_group_count == 0;
    let project_review_valid = validate_workspace_project_review(&manifest).is_ok();
    let readiness = classify_translation_readiness(
        manifest.project_status,
        manifest.semantic_group_count,
        counts.untranslated_group_count,
        counts.ready_for_review_group_count,
        counts.approved_group_count,
        project_review_valid,
    );
    if manifest.project_status == DialogueTranslationProjectStatus::Complete {
        ensure!(
            readiness.translation_project_complete,
            "complete workspace lacks release admission evidence"
        );
    } else {
        ensure!(
            manifest.project_review.is_none(),
            "in-progress workspace must not claim project approval"
        );
    }

    let korean_repertoire: String = counts.korean_repertoire.into_iter().collect();
    let asset_repertoires = repertoire_by_asset
        .into_iter()
        .map(|(source_path, characters)| {
            let korean_repertoire: String = characters.into_iter().collect();
            DialogueTranslationAssetRepertoire {
                source_path,
                korean_character_count: korean_repertoire.chars().count(),
                korean_repertoire,
            }
        })
        .collect();
    let report = DialogueTranslationWorkspaceAuditReport {
        kind: "Justice Gakuen 2 sharded Korean dialogue translation workspace audit".to_string(),
        source_bin_sha256: manifest.source_bin_sha256,
        codebook_sha256: manifest.codebook_sha256,
        source_corpus_sha256: manifest.source_corpus_sha256,
        script_inventory_sha256: manifest.script_inventory_sha256,
        asset_count: manifest.asset_count,
        source_shard_count: counts.source_shard_count,
        context_shard_count: counts.context_shard_count,
        translation_shard_count: counts.translation_shard_count,
        review_shard_count: counts.review_shard_count,
        semantic_group_count: manifest.semantic_group_count,
        referenced_coordinate_count: counts.referenced_coordinate_count,
        context_occurrence_count: counts.context_occurrence_count,
        untranslated_group_count: counts.untranslated_group_count,
        draft_group_count: counts.draft_group_count,
        ready_for_review_group_count: counts.ready_for_review_group_count,
        pending_review_group_count: counts.pending_review_group_count,
        changes_requested_group_count: counts.changes_requested_group_count,
        approved_group_count: counts.approved_group_count,
        korean_character_count: korean_repertoire.chars().count(),
        korean_repertoire,
        asset_repertoires,
        largest_shard_entry_count: counts.largest_shard_entry_count,
        largest_shard_byte_count: counts.largest_shard_byte_count,
        ready_for_font_repertoire,
        development_translation_input_available: readiness.development_translation_input_available,
        release_candidate_translation_input_eligible: readiness
            .release_candidate_translation_input_eligible,
        translation_project_complete: readiness.translation_project_complete,
    };
    write_report(&config.output, &report)?;
    ensure!(
        manifest_bytes.len() <= MAX_SHARD_BYTES,
        "root manifest exceeds byte limit"
    );
    Ok((report, source))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct DialogueTranslationReadiness {
    pub(super) development_translation_input_available: bool,
    pub(super) release_candidate_translation_input_eligible: bool,
    pub(super) translation_project_complete: bool,
}

pub(super) fn classify_translation_readiness(
    project_status: DialogueTranslationProjectStatus,
    semantic_group_count: usize,
    untranslated_group_count: usize,
    ready_for_review_group_count: usize,
    approved_group_count: usize,
    project_review_valid: bool,
) -> DialogueTranslationReadiness {
    let development_translation_input_available =
        semantic_group_count != 0 && untranslated_group_count == 0;
    let release_candidate_translation_input_eligible = development_translation_input_available
        && ready_for_review_group_count == semantic_group_count
        && approved_group_count == semantic_group_count;
    let translation_project_complete = project_status == DialogueTranslationProjectStatus::Complete
        && release_candidate_translation_input_eligible
        && project_review_valid;

    DialogueTranslationReadiness {
        development_translation_input_available,
        release_candidate_translation_input_eligible,
        translation_project_complete,
    }
}

pub(super) fn authored_repertoire(shard: &DialogueTranslationShard) -> BTreeSet<char> {
    shard
        .entries
        .iter()
        .flat_map(|decision| decision.korean_segments.iter().flatten())
        .flat_map(|segment| segment.chars())
        .filter(|character| !character.is_whitespace())
        .collect()
}

pub(super) fn load_validated_authored_translation_segments(
    root: &Path,
    expected_group_count: usize,
) -> Result<BTreeMap<String, Vec<String>>> {
    let mut segments_by_semantic_hash = BTreeMap::new();
    for path in json_files(root)? {
        let (value, _): (serde_json::Value, _) = read_bounded_json(root, &path)?;
        if value.get("kind").and_then(serde_json::Value::as_str)
            != Some("Justice Gakuen 2 Korean dialogue translation shard")
        {
            continue;
        }
        let shard: DialogueTranslationShard = serde_json::from_value(value)
            .with_context(|| format!("failed to parse validated translation {}", path.display()))?;
        for decision in shard.entries {
            if decision.status == DialogueTranslationDecisionStatus::Untranslated {
                continue;
            }
            let segments = decision
                .korean_segments
                .into_iter()
                .map(|segment| segment.context("validated authored segment disappeared"))
                .collect::<Result<Vec<_>>>()?;
            ensure!(
                segments_by_semantic_hash
                    .insert(decision.semantic_source_sha256, segments)
                    .is_none(),
                "validated translation semantic hash is duplicated"
            );
        }
    }
    ensure!(
        segments_by_semantic_hash.len() == expected_group_count,
        "validated authored translation group count changed"
    );
    Ok(segments_by_semantic_hash)
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
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_json_files(root, &path, files)?;
        } else if path.extension().and_then(|value| value.to_str()) == Some("json") {
            files.insert(path.strip_prefix(root)?.to_path_buf());
        }
    }
    Ok(())
}

fn write_report(path: &Path, report: &DialogueTranslationWorkspaceAuditReport) -> Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    std::fs::write(path, json_bytes(report)?)
        .with_context(|| format!("failed to write {}", path.display()))
}
