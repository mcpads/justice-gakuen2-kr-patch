use std::path::{Path, PathBuf};

use anyhow::Result;

use super::selector_translation_model::{
    DialogueSelectorTranslationContextEntry, DialogueSelectorTranslationContextShard,
    DialogueSelectorTranslationDecision, DialogueSelectorTranslationReviewShard,
    DialogueSelectorTranslationRoleManifest, DialogueSelectorTranslationRoleManifestFileRef,
    DialogueSelectorTranslationRoleRef, DialogueSelectorTranslationShard,
    DialogueSelectorTranslationShardRef, DialogueSelectorTranslationSourceGroup,
    DialogueSelectorTranslationSourceShard,
};
use super::selector_translation_source::SelectorTranslationSource;
use super::translation_workspace_io::{relative_path, write_bounded_json};
use super::translation_workspace_model::{
    DialogueTranslationDecisionStatus, DialogueTranslationReviewDecision,
    DialogueTranslationReviewStatus, DialogueTranslationWorkspaceRole,
};
use super::translation_workspace_writer::{partition, path_string};

pub(super) struct SelectorRoleRefs {
    pub(super) source: DialogueSelectorTranslationRoleRef,
    pub(super) context: DialogueSelectorTranslationRoleRef,
    pub(super) translation: DialogueSelectorTranslationRoleRef,
    pub(super) review: DialogueSelectorTranslationRoleRef,
}

pub(super) fn write_selector_units(
    root: &Path,
    source: &SelectorTranslationSource,
    canonical_selector: usize,
    groups: &[DialogueSelectorTranslationSourceGroup],
    contexts: &[DialogueSelectorTranslationContextEntry],
) -> Result<SelectorRoleRefs> {
    let group_chunks = partition(groups, |entries| DialogueSelectorTranslationSourceShard {
        kind: "Justice Gakuen 2 protected selector translation source shard".to_string(),
        shard_id: probe_shard_id(),
        source_bin_sha256: source.source_bin_sha256.clone(),
        codebook_sha256: source.codebook_sha256.clone(),
        source_corpus_sha256: source.source_corpus_sha256.clone(),
        selector_consumer_audit_sha256: source.selector_consumer_audit_sha256.clone(),
        canonical_selector,
        entries: entries.to_vec(),
    })?;
    let mut source_refs = Vec::new();
    let mut translation_refs = Vec::new();
    let mut review_refs = Vec::new();
    for (unit_index, entries) in group_chunks.into_iter().enumerate() {
        write_group_unit(
            root,
            source,
            canonical_selector,
            unit_index,
            entries,
            &mut source_refs,
            &mut translation_refs,
            &mut review_refs,
        )?;
    }

    let context_chunks = partition(contexts, |entries| {
        DialogueSelectorTranslationContextShard {
            kind: "Justice Gakuen 2 selector translation context shard".to_string(),
            shard_id: probe_shard_id(),
            source_corpus_sha256: source.source_corpus_sha256.clone(),
            selector_consumer_audit_sha256: source.selector_consumer_audit_sha256.clone(),
            canonical_selector,
            entries: entries.to_vec(),
        }
    })?;
    let mut context_refs = Vec::new();
    for (unit_index, entries) in context_chunks.into_iter().enumerate() {
        let shard_id = shard_id(canonical_selector, "context", unit_index);
        let shard = DialogueSelectorTranslationContextShard {
            kind: "Justice Gakuen 2 selector translation context shard".to_string(),
            shard_id: shard_id.clone(),
            source_corpus_sha256: source.source_corpus_sha256.clone(),
            selector_consumer_audit_sha256: source.selector_consumer_audit_sha256.clone(),
            canonical_selector,
            entries,
        };
        let path = unit_path("context", canonical_selector, unit_index)?;
        let (content_sha256, _) = write_bounded_json(root, &path, &shard)?;
        context_refs.push(DialogueSelectorTranslationShardRef {
            shard_id,
            path: path_string(&path),
            entry_count: shard.entries.len(),
            content_sha256: Some(content_sha256),
            source_shard_sha256: None,
        });
    }

    Ok(SelectorRoleRefs {
        source: write_role_manifests(
            root,
            canonical_selector,
            DialogueTranslationWorkspaceRole::Source,
            source_refs,
        )?,
        context: write_role_manifests(
            root,
            canonical_selector,
            DialogueTranslationWorkspaceRole::Context,
            context_refs,
        )?,
        translation: write_role_manifests(
            root,
            canonical_selector,
            DialogueTranslationWorkspaceRole::Translation,
            translation_refs,
        )?,
        review: write_role_manifests(
            root,
            canonical_selector,
            DialogueTranslationWorkspaceRole::Review,
            review_refs,
        )?,
    })
}

#[allow(clippy::too_many_arguments)]
fn write_group_unit(
    root: &Path,
    source: &SelectorTranslationSource,
    canonical_selector: usize,
    unit_index: usize,
    entries: Vec<DialogueSelectorTranslationSourceGroup>,
    source_refs: &mut Vec<DialogueSelectorTranslationShardRef>,
    translation_refs: &mut Vec<DialogueSelectorTranslationShardRef>,
    review_refs: &mut Vec<DialogueSelectorTranslationShardRef>,
) -> Result<()> {
    let source_id = shard_id(canonical_selector, "source", unit_index);
    let source_shard = DialogueSelectorTranslationSourceShard {
        kind: "Justice Gakuen 2 protected selector translation source shard".to_string(),
        shard_id: source_id.clone(),
        source_bin_sha256: source.source_bin_sha256.clone(),
        codebook_sha256: source.codebook_sha256.clone(),
        source_corpus_sha256: source.source_corpus_sha256.clone(),
        selector_consumer_audit_sha256: source.selector_consumer_audit_sha256.clone(),
        canonical_selector,
        entries,
    };
    let source_path = unit_path("source", canonical_selector, unit_index)?;
    let (source_sha256, _) = write_bounded_json(root, &source_path, &source_shard)?;
    source_refs.push(DialogueSelectorTranslationShardRef {
        shard_id: source_id,
        path: path_string(&source_path),
        entry_count: source_shard.entries.len(),
        content_sha256: Some(source_sha256.clone()),
        source_shard_sha256: None,
    });

    let translation_id = shard_id(canonical_selector, "translation", unit_index);
    let translation_path = unit_path("translations/in-progress", canonical_selector, unit_index)?;
    let translation = DialogueSelectorTranslationShard {
        kind: "Justice Gakuen 2 Korean selector translation shard".to_string(),
        shard_id: translation_id.clone(),
        source_shard_sha256: source_sha256.clone(),
        entries: source_shard.entries.iter().map(initial_decision).collect(),
    };
    write_bounded_json(root, &translation_path, &translation)?;
    translation_refs.push(DialogueSelectorTranslationShardRef {
        shard_id: translation_id,
        path: path_string(&translation_path),
        entry_count: translation.entries.len(),
        content_sha256: None,
        source_shard_sha256: Some(source_sha256.clone()),
    });

    let review_id = shard_id(canonical_selector, "review", unit_index);
    let review_path = unit_path("reviews", canonical_selector, unit_index)?;
    let review = DialogueSelectorTranslationReviewShard {
        kind: "Justice Gakuen 2 independent selector translation review shard".to_string(),
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
    review_refs.push(DialogueSelectorTranslationShardRef {
        shard_id: review_id,
        path: path_string(&review_path),
        entry_count: review.entries.len(),
        content_sha256: None,
        source_shard_sha256: Some(source_sha256),
    });
    Ok(())
}

fn initial_decision(
    group: &DialogueSelectorTranslationSourceGroup,
) -> DialogueSelectorTranslationDecision {
    let runtime_segments = group.development_korean_segments.as_ref();
    DialogueSelectorTranslationDecision {
        semantic_source_sha256: group.semantic_source_sha256.clone(),
        korean_segments: runtime_segments.map_or_else(
            || group.source_segments.iter().map(|_| None).collect(),
            |segments| segments.iter().cloned().map(Some).collect(),
        ),
        status: if runtime_segments.is_some() {
            DialogueTranslationDecisionStatus::Draft
        } else {
            DialogueTranslationDecisionStatus::Untranslated
        },
        translator: runtime_segments
            .is_some()
            .then(|| "source-bound-runtime-insertion".to_string()),
        notes: String::new(),
        development_resolution: group.development_resolution,
    }
}

fn write_role_manifests(
    root: &Path,
    canonical_selector: usize,
    role: DialogueTranslationWorkspaceRole,
    shards: Vec<DialogueSelectorTranslationShardRef>,
) -> Result<DialogueSelectorTranslationRoleRef> {
    let shard_count = shards.len();
    let entry_count = shards.iter().map(|shard| shard.entry_count).sum();
    let chunks = partition(&shards, |probe_shards| {
        DialogueSelectorTranslationRoleManifest {
            kind: "Justice Gakuen 2 selector translation role manifest".to_string(),
            canonical_selector,
            role,
            shard_count: probe_shards.len(),
            entry_count: probe_shards.iter().map(|shard| shard.entry_count).sum(),
            shards: probe_shards.to_vec(),
        }
    })?;
    let mut manifests = Vec::new();
    for (manifest_index, shards) in chunks.into_iter().enumerate() {
        let manifest = DialogueSelectorTranslationRoleManifest {
            kind: "Justice Gakuen 2 selector translation role manifest".to_string(),
            canonical_selector,
            role,
            shard_count: shards.len(),
            entry_count: shards.iter().map(|shard| shard.entry_count).sum(),
            shards,
        };
        let role_name = match role {
            DialogueTranslationWorkspaceRole::Source => "source",
            DialogueTranslationWorkspaceRole::Context => "context",
            DialogueTranslationWorkspaceRole::Translation => "translation",
            DialogueTranslationWorkspaceRole::Review => "review",
        };
        let path = relative_path(format!(
            "manifests/selector-{canonical_selector}/{role_name}-{manifest_index:03}.json"
        ))?;
        let (manifest_sha256, _) = write_bounded_json(root, &path, &manifest)?;
        manifests.push(DialogueSelectorTranslationRoleManifestFileRef {
            manifest_path: path_string(&path),
            manifest_sha256,
            shard_count: manifest.shard_count,
            entry_count: manifest.entry_count,
        });
    }
    Ok(DialogueSelectorTranslationRoleRef {
        role,
        manifest_count: manifests.len(),
        manifests,
        shard_count,
        entry_count,
    })
}

fn unit_path(role: &str, canonical_selector: usize, unit_index: usize) -> Result<PathBuf> {
    relative_path(format!(
        "{role}/selector-{canonical_selector}/unit-{unit_index:03}.json"
    ))
}

fn shard_id(canonical_selector: usize, role: &str, unit_index: usize) -> String {
    format!("selector-{canonical_selector}-{role}-unit-{unit_index:03}")
}

fn probe_shard_id() -> String {
    "x".repeat(128)
}
