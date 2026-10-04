use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};

use crate::pipeline::sha256_bytes;

use super::super::dialogue_translation_assets_model::{
    DialogueTranslationAssetFamily, TrackedPrimaryDialogueTranslationEntry,
    TrackedPrimaryDialogueTranslationShard, TrackedSelectorDialogueTranslationEntry,
    TrackedSelectorDialogueTranslationShard,
};
use super::super::selector_translation_model::{
    DialogueSelectorDevelopmentResolution, DialogueSelectorTranslationRoleManifest,
    DialogueSelectorTranslationRoleRef, DialogueSelectorTranslationSelectorManifest,
    DialogueSelectorTranslationShard, DialogueSelectorTranslationShardRef,
    DialogueSelectorTranslationSourceShard, DialogueSelectorTranslationWorkspaceManifest,
};
use super::super::translation_model::DialogueTranslationScope;
use super::super::translation_workspace_asset_index::read_asset_index;
use super::super::translation_workspace_io::read_bounded_json;
use super::super::translation_workspace_model::{
    DialogueTranslationAssetManifest, DialogueTranslationRoleManifest,
    DialogueTranslationRoleManifestRef, DialogueTranslationShard, DialogueTranslationShardRef,
    DialogueTranslationSourceShard, DialogueTranslationWorkspaceManifest,
    DialogueTranslationWorkspaceRole,
};
use super::super::translation_workspace_source::asset_stem;
use super::super::translation_workspace_validation::{MAX_SHARD_ENTRIES, validate_asset_ref};
use super::super::translation_workspace_writer::partition;
use super::{DecisionCounts, LoadedFamily, LoadedOwner, LoadedShard, validation};

const PRIMARY_SHARD_KIND: &str =
    "Justice Gakuen 2 tracked primary dialogue translation asset shard";
const SELECTOR_SHARD_KIND: &str =
    "Justice Gakuen 2 tracked selector dialogue translation asset shard";

pub(super) fn load_primary_translation_assets(root: &Path) -> Result<LoadedFamily> {
    let (manifest, _): (DialogueTranslationWorkspaceManifest, _) =
        read_bounded_json(root, Path::new("manifest.json"))?;
    ensure!(
        DialogueTranslationScope::from_target_scope(&manifest.target_scope)
            == Some(DialogueTranslationScope::ResolvedPrimary),
        "tracked primary assets require the resolved-primary translation workspace"
    );
    ensure!(
        manifest.unresolved_primary_script_assets.is_empty(),
        "tracked primary assets cannot omit unresolved source assets"
    );
    let (asset_refs, _) = read_asset_index(root, &manifest)?;
    let mut owners = Vec::new();
    let mut family_counts = DecisionCounts::default();
    let mut observed_source_paths = BTreeSet::new();

    for asset_ref in asset_refs {
        ensure!(
            observed_source_paths.insert(asset_ref.source_path.clone()),
            "primary translation owner occurs more than once: {}",
            asset_ref.source_path
        );
        let asset_path = PathBuf::from(&asset_ref.manifest_path);
        let (asset, asset_bytes): (DialogueTranslationAssetManifest, _) =
            read_bounded_json(root, &asset_path)?;
        ensure!(
            sha256_bytes(&asset_bytes) == asset_ref.manifest_sha256,
            "primary asset manifest binding changed at {}",
            asset_path.display()
        );
        validate_asset_ref(&asset_ref, &asset)?;
        let source_refs =
            load_primary_role_shards(root, &asset, DialogueTranslationWorkspaceRole::Source)?;
        let translation_refs =
            load_primary_role_shards(root, &asset, DialogueTranslationWorkspaceRole::Translation)?;
        let (shards, counts) =
            merge_primary_shards(root, &asset.source_path, &source_refs, &translation_refs)?;
        ensure!(
            counts.decision_count() == asset.semantic_group_count,
            "primary decision aggregate changed for {}",
            asset.source_path
        );
        family_counts.merge(counts);
        owners.push(LoadedOwner {
            owner_id: asset_stem(&asset.source_path)?,
            source_path: Some(asset.source_path),
            canonical_selector: None,
            shards,
        });
    }
    owners.sort_by(|left, right| left.owner_id.cmp(&right.owner_id));
    ensure!(
        owners.len() == manifest.asset_count
            && family_counts.decision_count() == manifest.semantic_group_count,
        "primary tracked translation coverage changed"
    );
    Ok(LoadedFamily {
        family: DialogueTranslationAssetFamily::Primary,
        source_bin_sha256: manifest.source_bin_sha256,
        codebook_sha256: manifest.codebook_sha256,
        source_corpus_sha256: manifest.source_corpus_sha256,
        source_binding_sha256: manifest.script_inventory_sha256,
        target_scope: manifest.target_scope,
        source_asset_count: manifest.source_asset_count,
        owners,
        counts: family_counts,
    })
}

pub(super) fn load_selector_translation_assets(root: &Path) -> Result<LoadedFamily> {
    let (manifest, _): (DialogueSelectorTranslationWorkspaceManifest, _) =
        read_bounded_json(root, Path::new("manifest.json"))?;
    ensure!(
        manifest.target_scope == "all_runtime_image_selector_banks_2_through_6",
        "tracked selector assets require the all-runtime-image selector workspace"
    );
    let mut owners = Vec::new();
    let mut family_counts = DecisionCounts::default();
    let mut observed_selectors = BTreeSet::new();
    for selector_ref in &manifest.selectors {
        ensure!(
            observed_selectors.insert(selector_ref.canonical_selector),
            "selector translation owner occurs more than once: {}",
            selector_ref.canonical_selector
        );
        let selector_path = PathBuf::from(&selector_ref.manifest_path);
        let (selector, bytes): (DialogueSelectorTranslationSelectorManifest, _) =
            read_bounded_json(root, &selector_path)?;
        ensure!(
            sha256_bytes(&bytes) == selector_ref.manifest_sha256
                && selector.canonical_selector == selector_ref.canonical_selector
                && selector.semantic_group_count == selector_ref.semantic_group_count
                && selector.coordinate_count == selector_ref.coordinate_count,
            "selector translation manifest binding changed at {}",
            selector_path.display()
        );
        let source_refs =
            load_selector_role_shards(root, &selector, DialogueTranslationWorkspaceRole::Source)?;
        let translation_refs = load_selector_role_shards(
            root,
            &selector,
            DialogueTranslationWorkspaceRole::Translation,
        )?;
        let (shards, counts) = merge_selector_shards(
            root,
            selector.canonical_selector,
            &source_refs,
            &translation_refs,
        )?;
        ensure!(
            counts.decision_count() == selector.semantic_group_count,
            "selector decision aggregate changed for selector {}",
            selector.canonical_selector
        );
        family_counts.merge(counts);
        owners.push(LoadedOwner {
            owner_id: format!("selector-{}", selector.canonical_selector),
            source_path: None,
            canonical_selector: Some(selector.canonical_selector),
            shards,
        });
    }
    owners.sort_by_key(|owner| owner.canonical_selector);
    ensure!(
        owners.len() == manifest.selector_count
            && family_counts.decision_count() == manifest.semantic_group_count,
        "selector tracked translation coverage changed"
    );
    Ok(LoadedFamily {
        family: DialogueTranslationAssetFamily::Selector,
        source_bin_sha256: manifest.source_bin_sha256,
        codebook_sha256: manifest.codebook_sha256,
        source_corpus_sha256: manifest.source_corpus_sha256,
        source_binding_sha256: manifest.selector_consumer_audit_sha256,
        target_scope: manifest.target_scope,
        source_asset_count: manifest.source_asset_count,
        owners,
        counts: family_counts,
    })
}

pub(super) fn preserve_tracked_translation_decisions(
    root: &Path,
    primary: &mut LoadedFamily,
    selector: &mut LoadedFamily,
) -> Result<()> {
    let mut decisions = BTreeMap::new();
    collect_tracked_decisions(root, &mut decisions)?;
    let expected_count = primary.counts.decision_count() + selector.counts.decision_count();
    ensure!(
        decisions.len() == expected_count,
        "tracked translation decision population differs from current evidence: expected {expected_count}, found {}",
        decisions.len()
    );
    overlay_tracked_decisions(primary, &mut decisions)?;
    overlay_tracked_decisions(selector, &mut decisions)?;
    ensure!(
        decisions.is_empty(),
        "tracked translation assets contain decisions absent from current evidence"
    );
    primary.counts = counts_for_family(primary);
    selector.counts = counts_for_family(selector);
    ensure_unique_family_semantics(primary)?;
    ensure_unique_family_semantics(selector)?;
    Ok(())
}

fn collect_tracked_decisions(
    directory: &Path,
    decisions: &mut BTreeMap<String, TrackedDecision>,
) -> Result<()> {
    for entry in std::fs::read_dir(directory)
        .with_context(|| format!("failed to list {}", directory.display()))?
    {
        let path = entry?.path();
        if path.is_dir() {
            collect_tracked_decisions(&path, decisions)?;
        } else if path.extension().and_then(|value| value.to_str()) == Some("json") {
            let bytes = std::fs::read(&path)
                .with_context(|| format!("failed to read {}", path.display()))?;
            let value: serde_json::Value = serde_json::from_slice(&bytes)
                .with_context(|| format!("failed to parse {}", path.display()))?;
            match value.get("kind").and_then(serde_json::Value::as_str) {
                Some(PRIMARY_SHARD_KIND) => {
                    let shard: TrackedPrimaryDialogueTranslationShard =
                        serde_json::from_value(value).with_context(|| {
                            format!("failed to parse tracked primary shard {}", path.display())
                        })?;
                    for entry in shard.entries {
                        insert_tracked_decision(decisions, TrackedDecision::Primary(entry), &path)?;
                    }
                }
                Some(SELECTOR_SHARD_KIND) => {
                    let shard: TrackedSelectorDialogueTranslationShard =
                        serde_json::from_value(value).with_context(|| {
                            format!("failed to parse tracked selector shard {}", path.display())
                        })?;
                    for entry in shard.entries {
                        insert_tracked_decision(
                            decisions,
                            TrackedDecision::Selector(entry),
                            &path,
                        )?;
                    }
                }
                _ => {}
            }
        }
    }
    Ok(())
}

fn insert_tracked_decision(
    decisions: &mut BTreeMap<String, TrackedDecision>,
    decision: TrackedDecision,
    path: &Path,
) -> Result<()> {
    let semantic_id = decision.semantic_id().to_string();
    ensure!(
        decisions.insert(semantic_id.clone(), decision).is_none(),
        "tracked semantic decision is duplicated at {}: {semantic_id}",
        path.display()
    );
    Ok(())
}

fn overlay_tracked_decisions(
    family: &mut LoadedFamily,
    decisions: &mut BTreeMap<String, TrackedDecision>,
) -> Result<()> {
    for owner in &mut family.owners {
        for shard in &mut owner.shards {
            match shard {
                LoadedShard::Primary { shard, counts, .. } => {
                    for entry in &mut shard.entries {
                        let tracked = decisions
                            .remove(&entry.semantic_source_sha256)
                            .with_context(|| {
                                format!(
                                    "tracked primary decision is missing: {}",
                                    entry.semantic_source_sha256
                                )
                            })?;
                        let TrackedDecision::Primary(tracked) = tracked else {
                            anyhow::bail!(
                                "tracked decision changed consumer family: {}",
                                entry.semantic_source_sha256
                            );
                        };
                        ensure!(
                            tracked.source_segments == entry.source_segments
                                && tracked.controls == entry.controls,
                            "tracked primary source/control changed for {}",
                            entry.semantic_source_sha256
                        );
                        copy_primary_decision(entry, tracked);
                    }
                    *counts = counts_for_primary_entries(&shard.entries);
                }
                LoadedShard::Selector { shard, counts, .. } => {
                    for entry in &mut shard.entries {
                        let tracked = decisions
                            .remove(&entry.semantic_source_sha256)
                            .with_context(|| {
                                format!(
                                    "tracked selector decision is missing: {}",
                                    entry.semantic_source_sha256
                                )
                            })?;
                        let TrackedDecision::Selector(tracked) = tracked else {
                            anyhow::bail!(
                                "tracked decision changed consumer family: {}",
                                entry.semantic_source_sha256
                            );
                        };
                        ensure!(
                            tracked.source_segments == entry.source_segments
                                && tracked.controls == entry.controls
                                && tracked.target_selectors == entry.target_selectors
                                && tracked.development_resolution == entry.development_resolution,
                            "tracked selector source/control changed for {}",
                            entry.semantic_source_sha256
                        );
                        copy_selector_decision(entry, tracked);
                    }
                    *counts = counts_for_selector_entries(&shard.entries);
                }
            }
        }
    }
    Ok(())
}

fn copy_primary_decision(
    target: &mut TrackedPrimaryDialogueTranslationEntry,
    source: TrackedPrimaryDialogueTranslationEntry,
) {
    target.korean_segments = source.korean_segments;
    target.status = source.status;
    target.translator = source.translator;
    target.notes = source.notes;
}

fn copy_selector_decision(
    target: &mut TrackedSelectorDialogueTranslationEntry,
    source: TrackedSelectorDialogueTranslationEntry,
) {
    target.korean_segments = source.korean_segments;
    target.status = source.status;
    target.translator = source.translator;
    target.notes = source.notes;
}

fn counts_for_family(family: &LoadedFamily) -> DecisionCounts {
    let mut counts = DecisionCounts::default();
    for owner in &family.owners {
        for shard in &owner.shards {
            counts.merge(shard.counts());
        }
    }
    counts
}

fn ensure_unique_family_semantics(family: &LoadedFamily) -> Result<()> {
    let mut semantic_ids = BTreeSet::new();
    for owner in &family.owners {
        for shard in &owner.shards {
            for semantic_id in shard.semantic_ids() {
                ensure!(
                    semantic_ids.insert(semantic_id.to_string()),
                    "tracked semantic decision is duplicated after preservation: {semantic_id}"
                );
            }
        }
    }
    ensure!(
        semantic_ids.len() == family.counts.decision_count(),
        "tracked decision counts changed after preservation"
    );
    Ok(())
}

enum TrackedDecision {
    Primary(TrackedPrimaryDialogueTranslationEntry),
    Selector(TrackedSelectorDialogueTranslationEntry),
}

impl TrackedDecision {
    fn semantic_id(&self) -> &str {
        match self {
            Self::Primary(entry) => &entry.semantic_source_sha256,
            Self::Selector(entry) => &entry.semantic_source_sha256,
        }
    }
}

fn load_primary_role_shards(
    root: &Path,
    asset: &DialogueTranslationAssetManifest,
    role: DialogueTranslationWorkspaceRole,
) -> Result<Vec<DialogueTranslationShardRef>> {
    let role_ref = exactly_one_primary_role(&asset.roles, role)?;
    let mut shards = Vec::new();
    let mut entry_count = 0usize;
    for manifest_ref in &role_ref.manifests {
        let path = PathBuf::from(&manifest_ref.manifest_path);
        let (manifest, bytes): (DialogueTranslationRoleManifest, _) =
            read_bounded_json(root, &path)?;
        ensure!(
            sha256_bytes(&bytes) == manifest_ref.manifest_sha256
                && manifest.role == role
                && manifest.source_path == asset.source_path
                && manifest.shard_count == manifest_ref.shard_count
                && manifest.entry_count == manifest_ref.entry_count
                && manifest.shard_count == manifest.shards.len(),
            "primary role manifest binding changed at {}",
            path.display()
        );
        entry_count += manifest.entry_count;
        shards.extend(manifest.shards);
    }
    ensure!(
        role_ref.manifest_count == role_ref.manifests.len()
            && role_ref.shard_count == shards.len()
            && role_ref.entry_count == entry_count,
        "primary role aggregate changed for {}",
        asset.source_path
    );
    Ok(shards)
}

fn exactly_one_primary_role(
    roles: &[DialogueTranslationRoleManifestRef],
    role: DialogueTranslationWorkspaceRole,
) -> Result<&DialogueTranslationRoleManifestRef> {
    let matches = roles
        .iter()
        .filter(|candidate| candidate.role == role)
        .collect::<Vec<_>>();
    ensure!(
        matches.len() == 1,
        "primary asset role is missing or duplicated"
    );
    Ok(matches[0])
}

fn load_selector_role_shards(
    root: &Path,
    selector: &DialogueSelectorTranslationSelectorManifest,
    role: DialogueTranslationWorkspaceRole,
) -> Result<Vec<DialogueSelectorTranslationShardRef>> {
    let role_ref = exactly_one_selector_role(&selector.roles, role)?;
    let mut shards = Vec::new();
    let mut entry_count = 0usize;
    for manifest_ref in &role_ref.manifests {
        let path = PathBuf::from(&manifest_ref.manifest_path);
        let (manifest, bytes): (DialogueSelectorTranslationRoleManifest, _) =
            read_bounded_json(root, &path)?;
        ensure!(
            sha256_bytes(&bytes) == manifest_ref.manifest_sha256
                && manifest.canonical_selector == selector.canonical_selector
                && manifest.role == role
                && manifest.shard_count == manifest_ref.shard_count
                && manifest.entry_count == manifest_ref.entry_count
                && manifest.shard_count == manifest.shards.len(),
            "selector role manifest binding changed at {}",
            path.display()
        );
        entry_count += manifest.entry_count;
        shards.extend(manifest.shards);
    }
    ensure!(
        role_ref.manifest_count == role_ref.manifests.len()
            && role_ref.shard_count == shards.len()
            && role_ref.entry_count == entry_count,
        "selector role aggregate changed for selector {}",
        selector.canonical_selector
    );
    Ok(shards)
}

fn exactly_one_selector_role(
    roles: &[DialogueSelectorTranslationRoleRef],
    role: DialogueTranslationWorkspaceRole,
) -> Result<&DialogueSelectorTranslationRoleRef> {
    let matches = roles
        .iter()
        .filter(|candidate| candidate.role == role)
        .collect::<Vec<_>>();
    ensure!(matches.len() == 1, "selector role is missing or duplicated");
    Ok(matches[0])
}

fn merge_primary_shards(
    root: &Path,
    source_path: &str,
    source_refs: &[DialogueTranslationShardRef],
    translation_refs: &[DialogueTranslationShardRef],
) -> Result<(Vec<LoadedShard>, DecisionCounts)> {
    ensure!(
        source_refs.len() == translation_refs.len(),
        "primary source and translation shard counts differ for {source_path}"
    );
    let translations_by_source = primary_translations_by_source(translation_refs)?;
    let mut shards = Vec::new();
    let mut owner_counts = DecisionCounts::default();
    for source_ref in source_refs {
        let source_path_on_disk = PathBuf::from(&source_ref.path);
        let (source, source_bytes): (DialogueTranslationSourceShard, _) =
            read_bounded_json(root, &source_path_on_disk)?;
        let source_sha256 = sha256_bytes(&source_bytes);
        ensure!(
            source_ref.content_sha256.as_deref() == Some(source_sha256.as_str())
                && source_ref.source_shard_sha256.is_none()
                && source.shard_id == source_ref.shard_id
                && source.owner.source_path == source_path
                && source.entries.len() == source_ref.entry_count
                && source.entries.len() <= MAX_SHARD_ENTRIES,
            "primary source shard binding changed at {}",
            source_ref.path
        );
        let translation_ref = translations_by_source
            .get(&source_sha256)
            .with_context(|| {
                format!(
                    "primary source shard has no translation: {}",
                    source_ref.path
                )
            })?;
        let translation_path = PathBuf::from(&translation_ref.path);
        let (translation, _): (DialogueTranslationShard, _) =
            read_bounded_json(root, &translation_path)?;
        ensure!(
            translation_ref.content_sha256.is_none()
                && translation_ref.source_shard_sha256.as_deref() == Some(source_sha256.as_str())
                && translation.shard_id == translation_ref.shard_id
                && translation.source_shard_sha256 == source_sha256
                && translation.entries.len() == translation_ref.entry_count
                && translation.entries.len() == source.entries.len(),
            "primary translation shard binding changed at {}",
            translation_ref.path
        );
        let mut entries = Vec::new();
        for (source_entry, decision) in source.entries.iter().zip(&translation.entries) {
            ensure!(
                source_entry.semantic_source_sha256 == decision.semantic_source_sha256,
                "primary source and translation order changed at {}",
                translation_ref.path
            );
            let counts = validation::validate_translation_entry(
                &decision.semantic_source_sha256,
                &source_entry.source_segments,
                &decision.korean_segments,
                decision.status,
                decision.translator.as_deref(),
            )?;
            owner_counts.merge(counts);
            entries.push(TrackedPrimaryDialogueTranslationEntry {
                semantic_source_sha256: decision.semantic_source_sha256.clone(),
                source_segments: source_entry.source_segments.clone(),
                controls: source_entry.controls.clone(),
                korean_segments: decision.korean_segments.clone(),
                status: decision.status,
                translator: decision.translator.clone(),
                notes: decision.notes.clone(),
            });
        }
        let chunks = partition(&entries, |probe_entries| {
            TrackedPrimaryDialogueTranslationShard {
                kind: PRIMARY_SHARD_KIND.to_string(),
                shard_id: translation.shard_id.clone(),
                source_shard_sha256: source_sha256.clone(),
                owner: source.owner.clone(),
                entries: probe_entries.to_vec(),
            }
        })?;
        let chunk_count = chunks.len();
        for (part_index, entries) in chunks.into_iter().enumerate() {
            let shard_id = split_shard_id(&translation.shard_id, part_index, chunk_count);
            let path = tracked_shard_path(
                DialogueTranslationAssetFamily::Primary,
                &translation_ref.path,
                part_index,
                chunk_count,
            )?;
            let counts = counts_for_primary_entries(&entries);
            shards.push(LoadedShard::Primary {
                path,
                shard: TrackedPrimaryDialogueTranslationShard {
                    kind: PRIMARY_SHARD_KIND.to_string(),
                    shard_id,
                    source_shard_sha256: source_sha256.clone(),
                    owner: source.owner.clone(),
                    entries,
                },
                counts,
            });
        }
    }
    ensure!(
        translations_by_source.len() == source_refs.len(),
        "primary workspace has an unpaired translation shard for {source_path}"
    );
    Ok((shards, owner_counts))
}

fn primary_translations_by_source(
    refs: &[DialogueTranslationShardRef],
) -> Result<BTreeMap<String, &DialogueTranslationShardRef>> {
    let mut result = BTreeMap::new();
    for shard in refs {
        let source_sha256 = shard
            .source_shard_sha256
            .clone()
            .context("primary translation shard lacks source binding")?;
        ensure!(
            result.insert(source_sha256, shard).is_none(),
            "primary source shard has multiple translations"
        );
    }
    Ok(result)
}

fn merge_selector_shards(
    root: &Path,
    canonical_selector: usize,
    source_refs: &[DialogueSelectorTranslationShardRef],
    translation_refs: &[DialogueSelectorTranslationShardRef],
) -> Result<(Vec<LoadedShard>, DecisionCounts)> {
    ensure!(
        source_refs.len() == translation_refs.len(),
        "selector source and translation shard counts differ for selector {canonical_selector}"
    );
    let translations_by_source = selector_translations_by_source(translation_refs)?;
    let mut shards = Vec::new();
    let mut owner_counts = DecisionCounts::default();
    for source_ref in source_refs {
        let source_path_on_disk = PathBuf::from(&source_ref.path);
        let (source, source_bytes): (DialogueSelectorTranslationSourceShard, _) =
            read_bounded_json(root, &source_path_on_disk)?;
        let source_sha256 = sha256_bytes(&source_bytes);
        ensure!(
            source_ref.content_sha256.as_deref() == Some(source_sha256.as_str())
                && source_ref.source_shard_sha256.is_none()
                && source.shard_id == source_ref.shard_id
                && source.canonical_selector == canonical_selector
                && source.entries.len() == source_ref.entry_count
                && source.entries.len() <= MAX_SHARD_ENTRIES,
            "selector source shard binding changed at {}",
            source_ref.path
        );
        let translation_ref = translations_by_source
            .get(&source_sha256)
            .with_context(|| {
                format!(
                    "selector source shard has no translation: {}",
                    source_ref.path
                )
            })?;
        let translation_path = PathBuf::from(&translation_ref.path);
        let (translation, _): (DialogueSelectorTranslationShard, _) =
            read_bounded_json(root, &translation_path)?;
        ensure!(
            translation_ref.content_sha256.is_none()
                && translation_ref.source_shard_sha256.as_deref() == Some(source_sha256.as_str())
                && translation.shard_id == translation_ref.shard_id
                && translation.source_shard_sha256 == source_sha256
                && translation.entries.len() == translation_ref.entry_count
                && translation.entries.len() == source.entries.len(),
            "selector translation shard binding changed at {}",
            translation_ref.path
        );
        let mut entries = Vec::new();
        for (source_entry, decision) in source.entries.iter().zip(&translation.entries) {
            ensure!(
                source_entry.semantic_source_sha256 == decision.semantic_source_sha256
                    && source_entry.development_resolution == decision.development_resolution,
                "selector source and translation shape changed at {}",
                translation_ref.path
            );
            validate_selector_resolution(source_entry, decision)?;
            let counts = validation::validate_translation_entry(
                &decision.semantic_source_sha256,
                &source_entry.source_segments,
                &decision.korean_segments,
                decision.status,
                decision.translator.as_deref(),
            )?;
            owner_counts.merge(counts);
            entries.push(TrackedSelectorDialogueTranslationEntry {
                semantic_source_sha256: decision.semantic_source_sha256.clone(),
                source_segments: source_entry.source_segments.clone(),
                controls: source_entry.controls.clone(),
                korean_segments: decision.korean_segments.clone(),
                status: decision.status,
                translator: decision.translator.clone(),
                notes: decision.notes.clone(),
                target_selectors: source_entry.target_selectors.clone(),
                development_resolution: decision.development_resolution,
            });
        }
        let chunks = partition(&entries, |probe_entries| {
            TrackedSelectorDialogueTranslationShard {
                kind: SELECTOR_SHARD_KIND.to_string(),
                shard_id: translation.shard_id.clone(),
                source_shard_sha256: source_sha256.clone(),
                canonical_selector,
                entries: probe_entries.to_vec(),
            }
        })?;
        let chunk_count = chunks.len();
        for (part_index, entries) in chunks.into_iter().enumerate() {
            let shard_id = split_shard_id(&translation.shard_id, part_index, chunk_count);
            let path = tracked_shard_path(
                DialogueTranslationAssetFamily::Selector,
                &translation_ref.path,
                part_index,
                chunk_count,
            )?;
            let counts = counts_for_selector_entries(&entries);
            shards.push(LoadedShard::Selector {
                path,
                shard: TrackedSelectorDialogueTranslationShard {
                    kind: SELECTOR_SHARD_KIND.to_string(),
                    shard_id,
                    source_shard_sha256: source_sha256.clone(),
                    canonical_selector,
                    entries,
                },
                counts,
            });
        }
    }
    ensure!(
        translations_by_source.len() == source_refs.len(),
        "selector workspace has an unpaired translation shard for selector {canonical_selector}"
    );
    Ok((shards, owner_counts))
}

fn selector_translations_by_source(
    refs: &[DialogueSelectorTranslationShardRef],
) -> Result<BTreeMap<String, &DialogueSelectorTranslationShardRef>> {
    let mut result = BTreeMap::new();
    for shard in refs {
        let source_sha256 = shard
            .source_shard_sha256
            .clone()
            .context("selector translation shard lacks source binding")?;
        ensure!(
            result.insert(source_sha256, shard).is_none(),
            "selector source shard has multiple translations"
        );
    }
    Ok(result)
}

fn validate_selector_resolution(
    source: &super::super::selector_translation_model::DialogueSelectorTranslationSourceGroup,
    decision: &super::super::selector_translation_model::DialogueSelectorTranslationDecision,
) -> Result<()> {
    if source.development_resolution == DialogueSelectorDevelopmentResolution::RuntimeInsertionRewrite
    {
        let expected = source
            .development_korean_segments
            .as_ref()
            .context("selector runtime insertion source lacks Korean development bytes")?;
        ensure!(
            decision.development_resolution
                == DialogueSelectorDevelopmentResolution::RuntimeInsertionRewrite
                && decision.status
                    != super::super::translation_workspace_model::DialogueTranslationDecisionStatus::Untranslated
                && decision.korean_segments
                    == expected.iter().cloned().map(Some).collect::<Vec<_>>()
                && decision.translator.as_deref() == Some("source-bound-runtime-insertion"),
            "selector runtime insertion decision changed for {}",
            source.semantic_source_sha256
        );
    } else if decision.status
        == super::super::translation_workspace_model::DialogueTranslationDecisionStatus::Untranslated
    {
        ensure!(
            source.development_resolution
                == DialogueSelectorDevelopmentResolution::AuthoredTranslation,
            "non-authored selector decision cannot be untranslated"
        );
    }
    Ok(())
}

fn tracked_shard_path(
    family: DialogueTranslationAssetFamily,
    workspace_path: &str,
    part_index: usize,
    part_count: usize,
) -> Result<PathBuf> {
    let relative = Path::new(workspace_path)
        .strip_prefix("translations/in-progress")
        .with_context(|| {
            format!("translation shard is outside the editable tree: {workspace_path}")
        })?;
    let family_root = match family {
        DialogueTranslationAssetFamily::Primary => "primary",
        DialogueTranslationAssetFamily::Selector => "selector",
    };
    let mut output = PathBuf::from(family_root).join(relative);
    if part_count > 1 {
        let stem = output
            .file_stem()
            .and_then(|value| value.to_str())
            .context("translation shard path has no UTF-8 file stem")?;
        output.set_file_name(format!("{stem}-part-{part_index:03}.json"));
    }
    Ok(output)
}

fn split_shard_id(shard_id: &str, part_index: usize, part_count: usize) -> String {
    if part_count == 1 {
        shard_id.to_string()
    } else {
        format!("{shard_id}-part-{part_index:03}")
    }
}

fn counts_for_primary_entries(
    entries: &[TrackedPrimaryDialogueTranslationEntry],
) -> DecisionCounts {
    let mut counts = DecisionCounts::default();
    for entry in entries {
        counts.observe(entry.status);
    }
    counts
}

fn counts_for_selector_entries(
    entries: &[TrackedSelectorDialogueTranslationEntry],
) -> DecisionCounts {
    let mut counts = DecisionCounts::default();
    for entry in entries {
        counts.observe(entry.status);
    }
    counts
}
