use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};

use crate::pipeline::sha256_bytes;

use super::translation_model::{DialogueTranslationInitConfig, DialogueTranslationRefreshConfig};
use super::translation_workspace::initialize_translation_workspace;
use super::translation_workspace_asset_index::read_asset_index;
use super::translation_workspace_io::{read_bounded_json, write_bounded_json};
use super::translation_workspace_model::{
    DialogueTranslationAssetManifest, DialogueTranslationDecision,
    DialogueTranslationDecisionStatus, DialogueTranslationRoleManifest, DialogueTranslationShard,
    DialogueTranslationShardRef, DialogueTranslationSourceShard,
    DialogueTranslationWorkspaceManifest, DialogueTranslationWorkspaceRole,
};
use super::translation_workspace_validation::{AuditCounts, validate_translation_entries};

#[derive(Clone)]
struct PreviousDecision {
    source_segments: Vec<String>,
    controls: Vec<super::translation_model::DialogueTranslationControl>,
    decision: DialogueTranslationDecision,
}

fn context_coordinates(root: &Path) -> Result<BTreeMap<String, BTreeSet<String>>> {
    let mut coordinates = BTreeMap::<String, BTreeSet<String>>::new();
    for (role, reference) in registered_shards(root)? {
        if role != DialogueTranslationWorkspaceRole::Context {
            continue;
        }
        let (shard, bytes): (
            super::translation_workspace_model::DialogueTranslationContextShard,
            _,
        ) = read_bounded_json(root, Path::new(&reference.path))?;
        ensure!(
            reference.content_sha256.as_deref() == Some(sha256_bytes(&bytes).as_str()),
            "migration context hash changed at {}",
            reference.path
        );
        for entry in shard.entries {
            coordinates
                .entry(entry.semantic_source_sha256)
                .or_default()
                .insert(entry.coordinate_id);
        }
    }
    Ok(coordinates)
}

fn source_coordinates(
    group: &super::translation_workspace_model::DialogueTranslationSourceGroup,
    contexts: &BTreeMap<String, BTreeSet<String>>,
) -> Result<BTreeSet<String>> {
    let coordinates = if group.referenced_coordinate_count.is_some() {
        contexts
            .get(&group.semantic_source_sha256)
            .cloned()
            .context("source group lacks its context membership")?
    } else {
        group.referenced_coordinate_ids.iter().cloned().collect()
    };
    ensure!(
        coordinates.len() == group.coordinate_count(),
        "source/context membership count differs"
    );
    Ok(coordinates)
}

pub fn refresh_dialogue_translation(
    config: &DialogueTranslationRefreshConfig,
) -> Result<DialogueTranslationWorkspaceManifest> {
    ensure!(
        config.previous != config.output,
        "refresh output must differ from the previous workspace"
    );
    let previous = load_previous_decisions(&config.previous)?;
    let manifest = initialize_translation_workspace(&DialogueTranslationInitConfig {
        cue: config.cue.clone(),
        codebook: config.codebook.clone(),
        output: config.output.clone(),
        force: config.force,
        scope: config.scope,
    })?;
    migrate_compatible_decisions(&config.output, &previous)?;
    Ok(manifest)
}

fn load_previous_decisions(root: &Path) -> Result<BTreeMap<String, PreviousDecision>> {
    let coordinates = context_coordinates(root)?;
    let shards = registered_shards(root)?;
    let mut sources = BTreeMap::new();
    let mut translations = BTreeMap::new();
    for (role, shard_ref) in shards {
        match role {
            DialogueTranslationWorkspaceRole::Source => {
                let (shard, bytes): (DialogueTranslationSourceShard, _) =
                    read_bounded_json(root, Path::new(&shard_ref.path))?;
                let hash = sha256_bytes(&bytes);
                ensure!(
                    shard_ref.content_sha256.as_deref() == Some(hash.as_str()),
                    "previous protected source hash changed at {}",
                    shard_ref.path
                );
                ensure!(
                    sources.insert(hash, shard).is_none(),
                    "previous source shard hash is duplicated"
                );
            }
            DialogueTranslationWorkspaceRole::Translation => {
                let (shard, _): (DialogueTranslationShard, _) =
                    read_bounded_json(root, Path::new(&shard_ref.path))?;
                let source_hash = shard_ref
                    .source_shard_sha256
                    .context("previous translation lacks source hash")?;
                ensure!(
                    shard.source_shard_sha256 == source_hash,
                    "previous translation source binding changed"
                );
                ensure!(
                    translations.insert(source_hash, shard).is_none(),
                    "previous source shard has multiple translations"
                );
            }
            DialogueTranslationWorkspaceRole::Context
            | DialogueTranslationWorkspaceRole::Review => {}
        }
    }
    ensure!(
        sources.len() == translations.len(),
        "previous workspace source/translation coverage differs"
    );

    let mut by_coordinate = BTreeMap::new();
    for (source_hash, source) in sources {
        let translation = translations
            .get(&source_hash)
            .context("previous source shard has no translation")?;
        validate_translation_entries(translation, &source, &mut AuditCounts::default())?;
        ensure!(
            source.entries.len() == translation.entries.len(),
            "previous source/translation entry count differs"
        );
        for (group, decision) in source.entries.iter().zip(&translation.entries) {
            ensure!(
                group.semantic_source_sha256 == decision.semantic_source_sha256
                    && group.source_segments.len() == decision.korean_segments.len(),
                "previous translation shape changed"
            );
            let previous = PreviousDecision {
                source_segments: group.source_segments.clone(),
                controls: group.controls.clone(),
                decision: decision.clone(),
            };
            for coordinate in source_coordinates(group, &coordinates)? {
                ensure!(
                    by_coordinate
                        .insert(coordinate.clone(), previous.clone())
                        .is_none(),
                    "previous coordinate has multiple translation decisions"
                );
            }
        }
    }
    Ok(by_coordinate)
}

fn migrate_compatible_decisions(
    root: &Path,
    previous: &BTreeMap<String, PreviousDecision>,
) -> Result<()> {
    let coordinates = context_coordinates(root)?;
    let shards = registered_shards(root)?;
    let mut source_refs = BTreeMap::new();
    let mut translation_refs = BTreeMap::new();
    for (role, shard_ref) in shards {
        match role {
            DialogueTranslationWorkspaceRole::Source => {
                let hash = shard_ref
                    .content_sha256
                    .clone()
                    .context("refreshed source lacks content hash")?;
                ensure!(
                    source_refs.insert(hash, shard_ref).is_none(),
                    "refreshed source hash is registered more than once"
                );
            }
            DialogueTranslationWorkspaceRole::Translation => {
                let hash = shard_ref
                    .source_shard_sha256
                    .clone()
                    .context("refreshed translation lacks source hash")?;
                ensure!(
                    translation_refs.insert(hash, shard_ref).is_none(),
                    "refreshed source has multiple translation refs"
                );
            }
            DialogueTranslationWorkspaceRole::Context
            | DialogueTranslationWorkspaceRole::Review => {}
        }
    }
    ensure!(
        source_refs.len() == translation_refs.len(),
        "refreshed source/translation coverage differs"
    );

    let mut migrated_coordinates = BTreeSet::new();
    for (source_hash, source_ref) in source_refs {
        let (source, bytes): (DialogueTranslationSourceShard, _) =
            read_bounded_json(root, Path::new(&source_ref.path))?;
        ensure!(
            sha256_bytes(&bytes) == source_hash,
            "refreshed protected source hash changed"
        );
        let translation_ref = translation_refs
            .get(&source_hash)
            .context("refreshed source has no translation shard")?;
        let (mut translation, _): (DialogueTranslationShard, _) =
            read_bounded_json(root, Path::new(&translation_ref.path))?;
        ensure!(
            source.entries.len() == translation.entries.len(),
            "refreshed source/translation entry count differs"
        );
        for (group, output_decision) in source.entries.iter().zip(&mut translation.entries) {
            let group_coordinates = source_coordinates(group, &coordinates)?;
            let old = group_coordinates
                .iter()
                .filter_map(|coordinate| {
                    previous.get(coordinate).inspect(|_| {
                        migrated_coordinates.insert(coordinate.clone());
                    })
                })
                .collect::<Vec<_>>();
            let Some((first, rest)) = old.split_first() else {
                continue;
            };
            ensure!(
                rest.iter().all(|candidate| decision_payload_matches(
                    &candidate.decision,
                    &first.decision
                )),
                "corrected source merges conflicting translation decisions"
            );
            ensure!(
                rest.iter()
                    .all(|candidate| candidate.controls == first.controls)
                    && group.controls == first.controls
                    && group.source_segments.len() == first.decision.korean_segments.len(),
                "corrected source changes protected control or segment shape"
            );
            let source_changed = rest
                .iter()
                .any(|candidate| candidate.source_segments != group.source_segments)
                || first.source_segments != group.source_segments;
            *output_decision = first.decision.clone();
            output_decision.semantic_source_sha256 = group.semantic_source_sha256.clone();
            if source_changed
                && output_decision.status == DialogueTranslationDecisionStatus::ReadyForReview
            {
                output_decision.status = DialogueTranslationDecisionStatus::Draft;
                append_note(
                    &mut output_decision.notes,
                    "원문 코드북 교정으로 독립 재검수가 필요함",
                );
            }
        }
        write_bounded_json(root, Path::new(&translation_ref.path), &translation)?;
    }
    ensure!(
        migrated_coordinates.len() == previous.len(),
        "previous execution-referenced coordinate population changed"
    );
    Ok(())
}

fn registered_shards(
    root: &Path,
) -> Result<
    Vec<(
        DialogueTranslationWorkspaceRole,
        DialogueTranslationShardRef,
    )>,
> {
    let (manifest, _): (DialogueTranslationWorkspaceManifest, _) =
        read_bounded_json(root, Path::new("manifest.json"))?;
    let (asset_refs, _) = read_asset_index(root, &manifest)?;
    let mut shards = Vec::new();
    let mut paths = BTreeSet::new();
    for asset_ref in asset_refs {
        let asset_path = PathBuf::from(&asset_ref.manifest_path);
        ensure!(paths.insert(asset_path.clone()), "duplicate asset manifest");
        let (asset, bytes): (DialogueTranslationAssetManifest, _) =
            read_bounded_json(root, &asset_path)?;
        ensure!(
            sha256_bytes(&bytes) == asset_ref.manifest_sha256
                && asset.source_path == asset_ref.source_path,
            "workspace asset manifest binding changed"
        );
        for role_ref in asset.roles {
            ensure!(
                role_ref.manifest_count == role_ref.manifests.len(),
                "workspace role manifest count changed"
            );
            for role_manifest_ref in role_ref.manifests {
                let path = PathBuf::from(&role_manifest_ref.manifest_path);
                ensure!(paths.insert(path.clone()), "duplicate role manifest");
                let (role_manifest, bytes): (DialogueTranslationRoleManifest, _) =
                    read_bounded_json(root, &path)?;
                ensure!(
                    sha256_bytes(&bytes) == role_manifest_ref.manifest_sha256
                        && role_manifest.role == role_ref.role
                        && role_manifest.source_path == asset.source_path
                        && role_manifest.shard_count == role_manifest.shards.len(),
                    "workspace role manifest binding changed"
                );
                shards.extend(
                    role_manifest
                        .shards
                        .into_iter()
                        .map(|shard| (role_ref.role, shard)),
                );
            }
        }
    }
    Ok(shards)
}

fn decision_payload_matches(
    left: &DialogueTranslationDecision,
    right: &DialogueTranslationDecision,
) -> bool {
    left.korean_segments == right.korean_segments
        && left.status == right.status
        && left.translator == right.translator
        && left.notes == right.notes
}

fn append_note(notes: &mut String, note: &str) {
    if !notes.is_empty() {
        notes.push_str("; ");
    }
    notes.push_str(note);
}

#[cfg(test)]
mod membership_tests {
    use super::super::translation_workspace_model::DialogueTranslationSourceGroup;
    use super::*;

    #[test]
    fn compact_source_membership_requires_complete_contexts() {
        let mut group = DialogueTranslationSourceGroup {
            semantic_source_sha256: "sound".into(),
            source_segments: vec!["ドン！".into(), String::new()],
            controls: Vec::new(),
            referenced_coordinate_ids: Vec::new(),
            referenced_coordinate_count: Some(2),
            unreferenced_duplicate_coordinate_ids: Vec::new(),
            unreferenced_duplicate_coordinate_count: 0,
            context_occurrence_count: 3,
        };
        let contexts = BTreeMap::from([("sound".into(), BTreeSet::from(["a".into(), "b".into()]))]);
        assert_eq!(source_coordinates(&group, &contexts).unwrap().len(), 2);
        assert!(source_coordinates(&group, &BTreeMap::new()).is_err());
        group.referenced_coordinate_count = Some(3);
        assert!(source_coordinates(&group, &contexts).is_err());
        group.referenced_coordinate_count = None;
        group.referenced_coordinate_ids = vec!["a".into(), "b".into()];
        assert_eq!(
            source_coordinates(&group, &BTreeMap::new()).unwrap(),
            contexts["sound"]
        );
    }
}
