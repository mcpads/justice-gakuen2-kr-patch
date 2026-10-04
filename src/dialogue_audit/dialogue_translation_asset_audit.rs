use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};

use crate::pipeline::sha256_bytes;

use super::super::dialogue_translation_assets_model::{
    DialogueTranslationAssetFamily, DialogueTranslationAssetManifest,
    DialogueTranslationAssetReport, DialogueTranslationAssetShardRef,
    DialogueTranslationOwnerIndexManifest, DialogueTranslationOwnerManifest,
    DialogueTranslationOwnerRef, TrackedPrimaryDialogueTranslationShard,
    TrackedSelectorDialogueTranslationShard,
};
use super::super::selector_translation_model::DialogueSelectorDevelopmentResolution;
use super::super::translation_model::DialogueTranslationScope;
use super::super::translation_workspace_io::read_bounded_json;
use super::super::translation_workspace_validation::MAX_SHARD_ENTRIES;
use super::{
    DecisionCounts, ValidatedDialogueTranslationAssets, ValidatedDialogueTranslationShardFile,
    validation,
};

const ROOT_KIND: &str = "Justice Gakuen 2 tracked dialogue translation assets";
const OWNER_MANIFEST_KIND: &str = "Justice Gakuen 2 tracked dialogue translation owner manifest";
const OWNER_INDEX_KIND: &str = "Justice Gakuen 2 tracked dialogue translation owner index";
const PRIMARY_SHARD_KIND: &str =
    "Justice Gakuen 2 tracked primary dialogue translation asset shard";
const SELECTOR_SHARD_KIND: &str =
    "Justice Gakuen 2 tracked selector dialogue translation asset shard";

pub(super) fn load_validated_translation_assets(
    root: &Path,
) -> Result<ValidatedDialogueTranslationAssets> {
    let (manifest, manifest_bytes): (DialogueTranslationAssetManifest, _) =
        read_bounded_json(root, Path::new("manifest.json"))?;
    validate_root_manifest(&manifest)?;
    let mut registered_paths = BTreeSet::from([PathBuf::from("manifest.json")]);
    let mut semantic_ids = BTreeSet::new();
    let mut shard_ids = BTreeSet::new();
    let mut owner_ids = BTreeSet::new();
    let mut observed = ObservedAssets {
        largest_json_byte_count: manifest_bytes.len(),
        ..ObservedAssets::default()
    };

    ensure!(
        manifest.owner_index_manifest_count == manifest.owner_index_manifests.len(),
        "tracked translation owner index count changed"
    );
    for index_ref in &manifest.owner_index_manifests {
        let path = register_path(&mut registered_paths, &index_ref.manifest_path)?;
        let (index, bytes): (DialogueTranslationOwnerIndexManifest, _) =
            read_bounded_json(root, &path)?;
        observed.observe_json(bytes.len());
        ensure!(
            sha256_bytes(&bytes) == index_ref.manifest_sha256
                && index.kind == OWNER_INDEX_KIND
                && index.family == index_ref.family
                && index.owner_count == index_ref.owner_count
                && index.shard_count == index_ref.shard_count
                && index.decision_count == index_ref.decision_count
                && index.owner_count == index.owners.len()
                && index.shard_count
                    == index
                        .owners
                        .iter()
                        .map(|owner| owner.shard_count)
                        .sum::<usize>()
                && index.decision_count
                    == index
                        .owners
                        .iter()
                        .map(|owner| owner.decision_count)
                        .sum::<usize>(),
            "tracked translation owner index binding changed at {}",
            path.display()
        );
        for owner in &index.owners {
            ensure!(
                owner.family == index.family
                    && owner_ids.insert((owner.family, owner.owner_id.clone())),
                "tracked translation owner is duplicated: {}",
                owner.owner_id
            );
            audit_owner(
                root,
                owner,
                &mut registered_paths,
                &mut semantic_ids,
                &mut shard_ids,
                &mut observed,
            )?;
        }
    }

    ensure!(
        observed.primary_owner_count == manifest.primary_owner_count
            && observed.selector_owner_count == manifest.selector_owner_count
            && observed.primary_shard_count == manifest.primary_shard_count
            && observed.selector_shard_count == manifest.selector_shard_count
            && observed.primary_counts.decision_count() == manifest.primary_decision_count
            && observed.selector_counts.decision_count() == manifest.selector_decision_count,
        "tracked translation family aggregates changed"
    );
    let mut combined_counts = observed.primary_counts;
    combined_counts.merge(observed.selector_counts);
    ensure!(
        semantic_ids.len() == manifest.semantic_decision_count
            && semantic_ids.len() == combined_counts.decision_count()
            && combined_counts.untranslated == manifest.untranslated_decision_count
            && combined_counts.draft == manifest.draft_decision_count
            && combined_counts.ready_for_review == manifest.ready_for_review_decision_count,
        "tracked translation decision aggregates changed"
    );
    ensure!(
        registered_paths == json_files(root)?,
        "tracked dialogue translation assets contain unregistered or missing JSON"
    );
    let report = DialogueTranslationAssetReport {
        kind: "Justice Gakuen 2 tracked dialogue translation asset audit".to_string(),
        primary_owner_count: observed.primary_owner_count,
        selector_owner_count: observed.selector_owner_count,
        primary_shard_count: observed.primary_shard_count,
        selector_shard_count: observed.selector_shard_count,
        primary_decision_count: observed.primary_counts.decision_count(),
        selector_decision_count: observed.selector_counts.decision_count(),
        semantic_decision_count: semantic_ids.len(),
        untranslated_decision_count: combined_counts.untranslated,
        draft_decision_count: combined_counts.draft,
        ready_for_review_decision_count: combined_counts.ready_for_review,
        largest_json_byte_count: observed.largest_json_byte_count,
    };
    Ok(ValidatedDialogueTranslationAssets {
        manifest,
        report,
        primary_shards: observed.primary_shards,
        selector_shards: observed.selector_shards,
    })
}

fn validate_root_manifest(manifest: &DialogueTranslationAssetManifest) -> Result<()> {
    ensure!(
        manifest.kind == ROOT_KIND
            && validation::is_sha256(&manifest.source_bin_sha256)
            && validation::is_sha256(&manifest.codebook_sha256)
            && validation::is_sha256(&manifest.source_corpus_sha256)
            && validation::is_sha256(&manifest.primary_script_inventory_sha256)
            && validation::is_sha256(&manifest.selector_consumer_audit_sha256)
            && DialogueTranslationScope::from_target_scope(&manifest.primary_target_scope)
                == Some(DialogueTranslationScope::ResolvedPrimary)
            && manifest.selector_target_scope == "all_runtime_image_selector_banks_2_through_6"
            && manifest.primary_source_asset_count == manifest.primary_owner_count
            && manifest.primary_owner_count != 0
            && manifest.selector_owner_count != 0
            && manifest.primary_decision_count + manifest.selector_decision_count
                == manifest.semantic_decision_count
            && manifest.untranslated_decision_count
                + manifest.draft_decision_count
                + manifest.ready_for_review_decision_count
                == manifest.semantic_decision_count,
        "tracked dialogue translation root manifest changed"
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn audit_owner(
    root: &Path,
    owner: &DialogueTranslationOwnerRef,
    registered_paths: &mut BTreeSet<PathBuf>,
    semantic_ids: &mut BTreeSet<String>,
    shard_ids: &mut BTreeSet<String>,
    observed: &mut ObservedAssets,
) -> Result<()> {
    validate_owner_identity(owner)?;
    ensure!(
        owner.manifest_count == owner.manifests.len() && owner.manifest_count != 0,
        "tracked translation owner manifest count changed for {}",
        owner.owner_id
    );
    let mut shard_count = 0usize;
    let mut decision_count = 0usize;
    for manifest_ref in &owner.manifests {
        let path = register_path(registered_paths, &manifest_ref.manifest_path)?;
        let (manifest, bytes): (DialogueTranslationOwnerManifest, _) =
            read_bounded_json(root, &path)?;
        observed.observe_json(bytes.len());
        ensure!(
            sha256_bytes(&bytes) == manifest_ref.manifest_sha256
                && manifest.kind == OWNER_MANIFEST_KIND
                && manifest.family == owner.family
                && manifest.owner_id == owner.owner_id
                && manifest.source_path == owner.source_path
                && manifest.canonical_selector == owner.canonical_selector
                && manifest.shard_count == manifest_ref.shard_count
                && manifest.decision_count == manifest_ref.decision_count
                && manifest.shard_count == manifest.shards.len()
                && manifest.decision_count
                    == manifest
                        .shards
                        .iter()
                        .map(|shard| shard.decision_count)
                        .sum::<usize>(),
            "tracked translation owner manifest binding changed at {}",
            path.display()
        );
        for shard in &manifest.shards {
            audit_shard(
                root,
                owner,
                shard,
                registered_paths,
                semantic_ids,
                shard_ids,
                observed,
            )?;
        }
        shard_count += manifest.shard_count;
        decision_count += manifest.decision_count;
    }
    ensure!(
        shard_count == owner.shard_count && decision_count == owner.decision_count,
        "tracked translation owner aggregate changed for {}",
        owner.owner_id
    );
    match owner.family {
        DialogueTranslationAssetFamily::Primary => observed.primary_owner_count += 1,
        DialogueTranslationAssetFamily::Selector => observed.selector_owner_count += 1,
    }
    Ok(())
}

fn validate_owner_identity(owner: &DialogueTranslationOwnerRef) -> Result<()> {
    ensure!(
        !owner.owner_id.is_empty()
            && match owner.family {
                DialogueTranslationAssetFamily::Primary =>
                    owner.source_path.is_some() && owner.canonical_selector.is_none(),
                DialogueTranslationAssetFamily::Selector =>
                    owner.source_path.is_none()
                        && owner.canonical_selector.is_some()
                        && owner.owner_id
                            == format!("selector-{}", owner.canonical_selector.unwrap_or_default()),
            },
        "tracked translation owner identity changed for {}",
        owner.owner_id
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn audit_shard(
    root: &Path,
    owner: &DialogueTranslationOwnerRef,
    shard_ref: &DialogueTranslationAssetShardRef,
    registered_paths: &mut BTreeSet<PathBuf>,
    semantic_ids: &mut BTreeSet<String>,
    shard_ids: &mut BTreeSet<String>,
    observed: &mut ObservedAssets,
) -> Result<()> {
    ensure!(
        validation::is_sha256(&shard_ref.content_sha256)
            && validation::is_sha256(&shard_ref.source_shard_sha256)
            && shard_ref.decision_count <= MAX_SHARD_ENTRIES
            && shard_ref.untranslated_decision_count
                + shard_ref.draft_decision_count
                + shard_ref.ready_for_review_decision_count
                == shard_ref.decision_count
            && shard_ids.insert(shard_ref.shard_id.clone()),
        "tracked translation shard metadata changed for {}",
        shard_ref.shard_id
    );
    let path = register_path(registered_paths, &shard_ref.path)?;
    validate_shard_path(owner, &path)?;
    let counts = match owner.family {
        DialogueTranslationAssetFamily::Primary => {
            let (shard, bytes): (TrackedPrimaryDialogueTranslationShard, _) =
                read_bounded_json(root, &path)?;
            observed.observe_json(bytes.len());
            ensure!(
                sha256_bytes(&bytes) == shard_ref.content_sha256
                    && shard.kind == PRIMARY_SHARD_KIND
                    && shard.shard_id == shard_ref.shard_id
                    && shard.source_shard_sha256 == shard_ref.source_shard_sha256
                    && shard.owner.source_path == owner.source_path.as_deref().unwrap_or_default()
                    && shard.entries.len() == shard_ref.decision_count,
                "tracked primary translation shard binding changed at {}",
                path.display()
            );
            let mut counts = DecisionCounts::default();
            for entry in &shard.entries {
                ensure!(
                    semantic_ids.insert(entry.semantic_source_sha256.clone()),
                    "semantic translation decision is duplicated: {}",
                    entry.semantic_source_sha256
                );
                counts.merge(validation::validate_translation_entry(
                    &entry.semantic_source_sha256,
                    &entry.source_segments,
                    &entry.korean_segments,
                    entry.status,
                    entry.translator.as_deref(),
                )?);
            }
            observed.primary_shard_count += 1;
            observed.primary_counts.merge(counts);
            observed
                .primary_shards
                .push(ValidatedDialogueTranslationShardFile {
                    path: path.clone(),
                    content_sha256: shard_ref.content_sha256.clone(),
                    json_byte_count: bytes.len(),
                });
            counts
        }
        DialogueTranslationAssetFamily::Selector => {
            let (shard, bytes): (TrackedSelectorDialogueTranslationShard, _) =
                read_bounded_json(root, &path)?;
            observed.observe_json(bytes.len());
            ensure!(
                sha256_bytes(&bytes) == shard_ref.content_sha256
                    && shard.kind == SELECTOR_SHARD_KIND
                    && shard.shard_id == shard_ref.shard_id
                    && shard.source_shard_sha256 == shard_ref.source_shard_sha256
                    && Some(shard.canonical_selector) == owner.canonical_selector
                    && shard.entries.len() == shard_ref.decision_count,
                "tracked selector translation shard binding changed at {}",
                path.display()
            );
            let mut counts = DecisionCounts::default();
            for entry in &shard.entries {
                ensure!(
                    semantic_ids.insert(entry.semantic_source_sha256.clone())
                        && entry.target_selectors.contains(&shard.canonical_selector)
                        && (entry.development_resolution
                            != DialogueSelectorDevelopmentResolution::RuntimeInsertionRewrite
                            || entry.status
                                != super::super::translation_workspace_model::DialogueTranslationDecisionStatus::Untranslated),
                    "tracked selector decision metadata changed for {}",
                    entry.semantic_source_sha256
                );
                counts.merge(validation::validate_translation_entry(
                    &entry.semantic_source_sha256,
                    &entry.source_segments,
                    &entry.korean_segments,
                    entry.status,
                    entry.translator.as_deref(),
                )?);
            }
            observed.selector_shard_count += 1;
            observed.selector_counts.merge(counts);
            observed
                .selector_shards
                .push(ValidatedDialogueTranslationShardFile {
                    path: path.clone(),
                    content_sha256: shard_ref.content_sha256.clone(),
                    json_byte_count: bytes.len(),
                });
            counts
        }
    };
    ensure!(
        counts.decision_count() == shard_ref.decision_count
            && counts.untranslated == shard_ref.untranslated_decision_count
            && counts.draft == shard_ref.draft_decision_count
            && counts.ready_for_review == shard_ref.ready_for_review_decision_count,
        "tracked translation shard status aggregate changed at {}",
        path.display()
    );
    Ok(())
}

fn validate_shard_path(owner: &DialogueTranslationOwnerRef, path: &Path) -> Result<()> {
    let family = match owner.family {
        DialogueTranslationAssetFamily::Primary => "primary",
        DialogueTranslationAssetFamily::Selector => "selector",
    };
    ensure!(
        path.starts_with(Path::new(family).join(&owner.owner_id)),
        "tracked translation shard escaped owner directory: {}",
        path.display()
    );
    Ok(())
}

fn register_path(paths: &mut BTreeSet<PathBuf>, value: &str) -> Result<PathBuf> {
    let path = PathBuf::from(value);
    ensure!(
        paths.insert(path.clone()),
        "tracked translation path is registered more than once: {}",
        path.display()
    );
    Ok(path)
}

#[derive(Debug, Default)]
struct ObservedAssets {
    primary_owner_count: usize,
    selector_owner_count: usize,
    primary_shard_count: usize,
    selector_shard_count: usize,
    primary_counts: DecisionCounts,
    selector_counts: DecisionCounts,
    largest_json_byte_count: usize,
    primary_shards: Vec<ValidatedDialogueTranslationShardFile>,
    selector_shards: Vec<ValidatedDialogueTranslationShardFile>,
}

impl ObservedAssets {
    fn observe_json(&mut self, byte_count: usize) {
        self.largest_json_byte_count = self.largest_json_byte_count.max(byte_count);
    }
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
