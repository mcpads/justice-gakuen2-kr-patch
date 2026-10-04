use std::path::{Path, PathBuf};

use anyhow::{Result, ensure};
use serde::Serialize;

use super::translation_workspace_io::{json_bytes, relative_path, write_bounded_json};
use super::translation_workspace_model::{
    DialogueTranslationContextEntry, DialogueTranslationContextShard, DialogueTranslationDecision,
    DialogueTranslationDecisionStatus, DialogueTranslationReviewDecision,
    DialogueTranslationReviewShard, DialogueTranslationReviewStatus,
    DialogueTranslationRoleManifest, DialogueTranslationRoleManifestFileRef,
    DialogueTranslationRoleManifestRef, DialogueTranslationRouteOwner, DialogueTranslationShard,
    DialogueTranslationShardRef, DialogueTranslationSourceGroup, DialogueTranslationSourceShard,
    DialogueTranslationWorkspaceRole,
};
use super::translation_workspace_source::{TranslationWorkspaceSource, asset_stem};
use super::translation_workspace_validation::{MAX_SHARD_BYTES, MAX_SHARD_ENTRIES};

pub(super) fn write_context_units(
    root: &Path,
    source: &TranslationWorkspaceSource,
    stem: &str,
    owner: &DialogueTranslationRouteOwner,
    entries: &[DialogueTranslationContextEntry],
    refs: &mut Vec<DialogueTranslationShardRef>,
) -> Result<()> {
    let chunks = partition(entries, |probe_entries| DialogueTranslationContextShard {
        kind: "Justice Gakuen 2 route dialogue context shard".to_string(),
        shard_id: probe_shard_id(),
        script_inventory_sha256: source.script_inventory_sha256.clone(),
        owner: owner.clone(),
        entries: probe_entries.to_vec(),
    })?;
    for (unit_index, entries) in chunks.into_iter().enumerate() {
        let shard_id = shard_id(stem, owner, "context", unit_index);
        let shard = DialogueTranslationContextShard {
            kind: "Justice Gakuen 2 route dialogue context shard".to_string(),
            shard_id: shard_id.clone(),
            script_inventory_sha256: source.script_inventory_sha256.clone(),
            owner: owner.clone(),
            entries,
        };
        let path = unit_path("context", stem, owner, unit_index)?;
        let (content_sha256, _) = write_bounded_json(root, &path, &shard)?;
        refs.push(DialogueTranslationShardRef {
            shard_id,
            path: path_string(&path),
            entry_count: shard.entries.len(),
            content_sha256: Some(content_sha256),
            source_shard_sha256: None,
        });
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn write_translation_units(
    root: &Path,
    source: &TranslationWorkspaceSource,
    stem: &str,
    owner: &DialogueTranslationRouteOwner,
    groups: &[DialogueTranslationSourceGroup],
    source_refs: &mut Vec<DialogueTranslationShardRef>,
    translation_refs: &mut Vec<DialogueTranslationShardRef>,
    review_refs: &mut Vec<DialogueTranslationShardRef>,
) -> Result<()> {
    // Membership belongs to the independently validated context shards. Keeping
    // every repeated coordinate here can make a single short sound effect
    // exceed the source shard bound.
    let groups = groups
        .iter()
        .map(|group| {
            let mut group = group.clone();
            group.referenced_coordinate_count = Some(group.coordinate_count());
            group.referenced_coordinate_ids.clear();
            group
        })
        .collect::<Vec<_>>();
    let chunks = partition(&groups, |probe_groups| DialogueTranslationSourceShard {
        kind: "Justice Gakuen 2 protected dialogue source shard".to_string(),
        shard_id: probe_shard_id(),
        source_bin_sha256: source.source_bin_sha256.clone(),
        codebook_sha256: source.codebook_sha256.clone(),
        source_corpus_sha256: source.source_corpus_sha256.clone(),
        script_inventory_sha256: source.script_inventory_sha256.clone(),
        owner: owner.clone(),
        entries: probe_groups.to_vec(),
    })?;
    for (unit_index, entries) in chunks.into_iter().enumerate() {
        let source_id = shard_id(stem, owner, "source", unit_index);
        let source_shard = DialogueTranslationSourceShard {
            kind: "Justice Gakuen 2 protected dialogue source shard".to_string(),
            shard_id: source_id.clone(),
            source_bin_sha256: source.source_bin_sha256.clone(),
            codebook_sha256: source.codebook_sha256.clone(),
            source_corpus_sha256: source.source_corpus_sha256.clone(),
            script_inventory_sha256: source.script_inventory_sha256.clone(),
            owner: owner.clone(),
            entries,
        };
        let source_path = unit_path("source", stem, owner, unit_index)?;
        let (source_sha256, _) = write_bounded_json(root, &source_path, &source_shard)?;
        source_refs.push(DialogueTranslationShardRef {
            shard_id: source_id,
            path: path_string(&source_path),
            entry_count: source_shard.entries.len(),
            content_sha256: Some(source_sha256.clone()),
            source_shard_sha256: None,
        });

        let translation_id = shard_id(stem, owner, "translation", unit_index);
        let translation_path = unit_path("translations/in-progress", stem, owner, unit_index)?;
        let translation = DialogueTranslationShard {
            kind: "Justice Gakuen 2 Korean dialogue translation shard".to_string(),
            shard_id: translation_id.clone(),
            source_shard_sha256: source_sha256.clone(),
            entries: source_shard
                .entries
                .iter()
                .map(|group| DialogueTranslationDecision {
                    semantic_source_sha256: group.semantic_source_sha256.clone(),
                    korean_segments: group.source_segments.iter().map(|_| None).collect(),
                    status: DialogueTranslationDecisionStatus::Untranslated,
                    translator: None,
                    notes: String::new(),
                })
                .collect(),
        };
        write_bounded_json(root, &translation_path, &translation)?;
        translation_refs.push(DialogueTranslationShardRef {
            shard_id: translation_id,
            path: path_string(&translation_path),
            entry_count: translation.entries.len(),
            content_sha256: None,
            source_shard_sha256: Some(source_sha256.clone()),
        });

        let review_id = shard_id(stem, owner, "review", unit_index);
        let review_path = unit_path("reviews", stem, owner, unit_index)?;
        let review = DialogueTranslationReviewShard {
            kind: "Justice Gakuen 2 independent semantic review shard".to_string(),
            shard_id: review_id.clone(),
            source_shard_sha256: source_sha256.clone(),
            translation_path: path_string(&translation_path),
            entries: source_shard
                .entries
                .iter()
                .map(|group| DialogueTranslationReviewDecision {
                    semantic_source_sha256: group.semantic_source_sha256.clone(),
                    status: DialogueTranslationReviewStatus::Pending,
                    reviewer: None,
                    reviewed_at: None,
                    reviewed_translation_sha256: None,
                    notes: String::new(),
                })
                .collect(),
        };
        write_bounded_json(root, &review_path, &review)?;
        review_refs.push(DialogueTranslationShardRef {
            shard_id: review_id,
            path: path_string(&review_path),
            entry_count: review.entries.len(),
            content_sha256: None,
            source_shard_sha256: Some(source_sha256),
        });
    }
    Ok(())
}

pub(super) fn write_role_manifest(
    root: &Path,
    source_path: &str,
    role: DialogueTranslationWorkspaceRole,
    shards: Vec<DialogueTranslationShardRef>,
) -> Result<DialogueTranslationRoleManifestRef> {
    let stem = asset_stem(source_path)?;
    let shard_count = shards.len();
    let entry_count = shards.iter().map(|shard| shard.entry_count).sum();
    let role_name = match role {
        DialogueTranslationWorkspaceRole::Source => "source",
        DialogueTranslationWorkspaceRole::Context => "context",
        DialogueTranslationWorkspaceRole::Translation => "translation",
        DialogueTranslationWorkspaceRole::Review => "review",
    };
    let chunks = partition(&shards, |probe_shards| DialogueTranslationRoleManifest {
        kind: "Justice Gakuen 2 dialogue translation role manifest".to_string(),
        role,
        source_path: source_path.to_string(),
        shard_count: probe_shards.len(),
        entry_count: probe_shards.iter().map(|shard| shard.entry_count).sum(),
        shards: probe_shards.to_vec(),
    })?;
    let mut manifests = Vec::new();
    for (manifest_index, shards) in chunks.into_iter().enumerate() {
        let manifest = DialogueTranslationRoleManifest {
            kind: "Justice Gakuen 2 dialogue translation role manifest".to_string(),
            role,
            source_path: source_path.to_string(),
            shard_count: shards.len(),
            entry_count: shards.iter().map(|shard| shard.entry_count).sum(),
            shards,
        };
        let path = relative_path(format!(
            "manifests/{stem}/{role_name}-{manifest_index:03}.json"
        ))?;
        let (manifest_sha256, _) = write_bounded_json(root, &path, &manifest)?;
        manifests.push(DialogueTranslationRoleManifestFileRef {
            manifest_path: path_string(&path),
            manifest_sha256,
            shard_count: manifest.shard_count,
            entry_count: manifest.entry_count,
        });
    }
    Ok(DialogueTranslationRoleManifestRef {
        role,
        manifest_count: manifests.len(),
        manifests,
        shard_count,
        entry_count,
    })
}

pub(super) fn partition<T, S>(items: &[T], build_probe: impl Fn(&[T]) -> S) -> Result<Vec<Vec<T>>>
where
    T: Clone,
    S: Serialize,
{
    let mut chunks = Vec::new();
    let mut current = Vec::new();
    for item in items {
        let mut candidate = current.clone();
        candidate.push(item.clone());
        let candidate_size = json_bytes(&build_probe(&candidate))?.len();
        if candidate.len() > MAX_SHARD_ENTRIES || candidate_size > MAX_SHARD_BYTES {
            ensure!(
                !current.is_empty(),
                "one workspace entry exceeds shard limits"
            );
            chunks.push(current);
            current = vec![item.clone()];
            ensure!(
                json_bytes(&build_probe(&current))?.len() <= MAX_SHARD_BYTES,
                "one workspace entry exceeds byte limit: {} ({} bytes)",
                std::any::type_name::<T>(),
                json_bytes(&build_probe(&current))?.len()
            );
        } else {
            current = candidate;
        }
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    Ok(chunks)
}

fn unit_path(
    role: &str,
    stem: &str,
    owner: &DialogueTranslationRouteOwner,
    unit_index: usize,
) -> Result<PathBuf> {
    relative_path(format!(
        "{role}/{stem}/bank-{}/variant-{:03}-route-{}/unit-{unit_index:03}.json",
        owner.bank_selector,
        owner.variant_selector,
        owner.route_table_offset.trim_start_matches("0x")
    ))
}

fn shard_id(
    stem: &str,
    owner: &DialogueTranslationRouteOwner,
    role: &str,
    unit_index: usize,
) -> String {
    format!(
        "{stem}-bank-{}-variant-{:03}-route-{}-{role}-unit-{unit_index:03}",
        owner.bank_selector,
        owner.variant_selector,
        owner.route_table_offset.trim_start_matches("0x")
    )
}

fn probe_shard_id() -> String {
    "x".repeat(128)
}

pub(super) fn path_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
