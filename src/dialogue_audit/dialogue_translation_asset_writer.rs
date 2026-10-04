use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};

use super::super::dialogue_translation_assets_model::{
    DialogueTranslationAssetFamily, DialogueTranslationAssetManifest,
    DialogueTranslationAssetShardRef, DialogueTranslationOwnerIndexManifest,
    DialogueTranslationOwnerIndexRef, DialogueTranslationOwnerManifest,
    DialogueTranslationOwnerManifestRef, DialogueTranslationOwnerRef,
    TrackedPrimaryDialogueTranslationShard, TrackedSelectorDialogueTranslationShard,
};
use super::super::translation_workspace_io::{relative_path, write_bounded_json};
use super::super::translation_workspace_writer::{partition, path_string};
use super::{DecisionCounts, LoadedFamily, LoadedOwner, LoadedShard};

const ROOT_KIND: &str = "Justice Gakuen 2 tracked dialogue translation assets";
const OWNER_MANIFEST_KIND: &str = "Justice Gakuen 2 tracked dialogue translation owner manifest";
const OWNER_INDEX_KIND: &str = "Justice Gakuen 2 tracked dialogue translation owner index";

pub(super) fn write_translation_assets(
    root: &Path,
    primary: &LoadedFamily,
    selector: &LoadedFamily,
) -> Result<()> {
    ensure!(
        primary.family == DialogueTranslationAssetFamily::Primary
            && selector.family == DialogueTranslationAssetFamily::Selector,
        "tracked translation asset families are reversed"
    );
    let primary_owners = write_owners(root, primary)?;
    let selector_owners = write_owners(root, selector)?;
    let mut owner_index_manifests = write_owner_indexes(
        root,
        DialogueTranslationAssetFamily::Primary,
        &primary_owners,
    )?;
    owner_index_manifests.extend(write_owner_indexes(
        root,
        DialogueTranslationAssetFamily::Selector,
        &selector_owners,
    )?);
    let primary_shard_count = primary_owners.iter().map(|owner| owner.shard_count).sum();
    let selector_shard_count = selector_owners.iter().map(|owner| owner.shard_count).sum();
    let combined_counts = merged_counts(primary.counts, selector.counts);
    let manifest = DialogueTranslationAssetManifest {
        kind: ROOT_KIND.to_string(),
        source_bin_sha256: primary.source_bin_sha256.clone(),
        codebook_sha256: primary.codebook_sha256.clone(),
        source_corpus_sha256: primary.source_corpus_sha256.clone(),
        primary_script_inventory_sha256: primary.source_binding_sha256.clone(),
        selector_consumer_audit_sha256: selector.source_binding_sha256.clone(),
        primary_target_scope: primary.target_scope.clone(),
        selector_target_scope: selector.target_scope.clone(),
        primary_source_asset_count: primary.source_asset_count,
        primary_owner_count: primary.owners.len(),
        selector_owner_count: selector.owners.len(),
        primary_shard_count,
        selector_shard_count,
        primary_decision_count: primary.counts.decision_count(),
        selector_decision_count: selector.counts.decision_count(),
        semantic_decision_count: combined_counts.decision_count(),
        untranslated_decision_count: combined_counts.untranslated,
        draft_decision_count: combined_counts.draft,
        ready_for_review_decision_count: combined_counts.ready_for_review,
        owner_index_manifest_count: owner_index_manifests.len(),
        owner_index_manifests,
    };
    write_bounded_json(root, Path::new("manifest.json"), &manifest)?;
    Ok(())
}

fn write_owners(root: &Path, family: &LoadedFamily) -> Result<Vec<DialogueTranslationOwnerRef>> {
    family
        .owners
        .iter()
        .map(|owner| write_owner(root, family.family, owner))
        .collect()
}

fn write_owner(
    root: &Path,
    family: DialogueTranslationAssetFamily,
    owner: &LoadedOwner,
) -> Result<DialogueTranslationOwnerRef> {
    let mut shard_refs = Vec::new();
    let mut owner_counts = DecisionCounts::default();
    let mut final_shards = Vec::new();
    for shard in &owner.shards {
        final_shards.extend(partition_final_shard(shard)?);
    }
    for shard in &final_shards {
        let (content_sha256, _) = match shard {
            LoadedShard::Primary { path, shard, .. } => write_bounded_json(root, path, shard)?,
            LoadedShard::Selector { path, shard, .. } => write_bounded_json(root, path, shard)?,
        };
        let counts = shard.counts();
        owner_counts.merge(counts);
        shard_refs.push(DialogueTranslationAssetShardRef {
            shard_id: shard.shard_id().to_string(),
            path: path_string(shard.path()),
            content_sha256,
            source_shard_sha256: shard.source_shard_sha256().to_string(),
            decision_count: counts.decision_count(),
            untranslated_decision_count: counts.untranslated,
            draft_decision_count: counts.draft,
            ready_for_review_decision_count: counts.ready_for_review,
        });
    }
    let chunks = partition(&shard_refs, |probe_shards| {
        DialogueTranslationOwnerManifest {
            kind: OWNER_MANIFEST_KIND.to_string(),
            family,
            owner_id: owner.owner_id.clone(),
            source_path: owner.source_path.clone(),
            canonical_selector: owner.canonical_selector,
            shard_count: probe_shards.len(),
            decision_count: probe_shards.iter().map(|shard| shard.decision_count).sum(),
            shards: probe_shards.to_vec(),
        }
    })?;
    let mut manifests = Vec::new();
    for (index, shards) in chunks.into_iter().enumerate() {
        let manifest = DialogueTranslationOwnerManifest {
            kind: OWNER_MANIFEST_KIND.to_string(),
            family,
            owner_id: owner.owner_id.clone(),
            source_path: owner.source_path.clone(),
            canonical_selector: owner.canonical_selector,
            shard_count: shards.len(),
            decision_count: shards.iter().map(|shard| shard.decision_count).sum(),
            shards,
        };
        let path = relative_path(owner_manifest_path(family, &owner.owner_id, index))?;
        let (manifest_sha256, _) = write_bounded_json(root, &path, &manifest)?;
        manifests.push(DialogueTranslationOwnerManifestRef {
            manifest_path: path_string(&path),
            manifest_sha256,
            shard_count: manifest.shard_count,
            decision_count: manifest.decision_count,
        });
    }
    Ok(DialogueTranslationOwnerRef {
        family,
        owner_id: owner.owner_id.clone(),
        source_path: owner.source_path.clone(),
        canonical_selector: owner.canonical_selector,
        manifest_count: manifests.len(),
        manifests,
        shard_count: shard_refs.len(),
        decision_count: owner_counts.decision_count(),
    })
}

fn partition_final_shard(shard: &LoadedShard) -> Result<Vec<LoadedShard>> {
    match shard {
        LoadedShard::Primary { path, shard, .. } => {
            let chunks = partition(&shard.entries, |entries| {
                TrackedPrimaryDialogueTranslationShard {
                    kind: shard.kind.clone(),
                    shard_id: shard.shard_id.clone(),
                    source_shard_sha256: shard.source_shard_sha256.clone(),
                    owner: shard.owner.clone(),
                    entries: entries.to_vec(),
                }
            })?;
            let part_count = chunks.len();
            chunks
                .into_iter()
                .enumerate()
                .map(|(part_index, entries)| {
                    let counts = counts_for_statuses(entries.iter().map(|entry| entry.status));
                    Ok(LoadedShard::Primary {
                        path: split_shard_path(path, part_index, part_count)?,
                        shard: TrackedPrimaryDialogueTranslationShard {
                            kind: shard.kind.clone(),
                            shard_id: split_shard_id(&shard.shard_id, part_index, part_count),
                            source_shard_sha256: shard.source_shard_sha256.clone(),
                            owner: shard.owner.clone(),
                            entries,
                        },
                        counts,
                    })
                })
                .collect()
        }
        LoadedShard::Selector { path, shard, .. } => {
            let chunks = partition(&shard.entries, |entries| {
                TrackedSelectorDialogueTranslationShard {
                    kind: shard.kind.clone(),
                    shard_id: shard.shard_id.clone(),
                    source_shard_sha256: shard.source_shard_sha256.clone(),
                    canonical_selector: shard.canonical_selector,
                    entries: entries.to_vec(),
                }
            })?;
            let part_count = chunks.len();
            chunks
                .into_iter()
                .enumerate()
                .map(|(part_index, entries)| {
                    let counts = counts_for_statuses(entries.iter().map(|entry| entry.status));
                    Ok(LoadedShard::Selector {
                        path: split_shard_path(path, part_index, part_count)?,
                        shard: TrackedSelectorDialogueTranslationShard {
                            kind: shard.kind.clone(),
                            shard_id: split_shard_id(&shard.shard_id, part_index, part_count),
                            source_shard_sha256: shard.source_shard_sha256.clone(),
                            canonical_selector: shard.canonical_selector,
                            entries,
                        },
                        counts,
                    })
                })
                .collect()
        }
    }
}

fn counts_for_statuses(
    statuses: impl Iterator<
        Item = super::super::translation_workspace_model::DialogueTranslationDecisionStatus,
    >,
) -> DecisionCounts {
    let mut counts = DecisionCounts::default();
    for status in statuses {
        counts.observe(status);
    }
    counts
}

fn split_shard_path(path: &Path, part_index: usize, part_count: usize) -> Result<PathBuf> {
    if part_count == 1 {
        return Ok(path.to_path_buf());
    }
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .context("tracked translation shard path has no UTF-8 file stem")?;
    let mut result = path.to_path_buf();
    result.set_file_name(format!("{stem}-part-{part_index:03}.json"));
    Ok(result)
}

fn split_shard_id(shard_id: &str, part_index: usize, part_count: usize) -> String {
    if part_count == 1 {
        shard_id.to_string()
    } else {
        format!("{shard_id}-part-{part_index:03}")
    }
}

fn write_owner_indexes(
    root: &Path,
    family: DialogueTranslationAssetFamily,
    owners: &[DialogueTranslationOwnerRef],
) -> Result<Vec<DialogueTranslationOwnerIndexRef>> {
    let chunks = partition(owners, |probe_owners| {
        DialogueTranslationOwnerIndexManifest {
            kind: OWNER_INDEX_KIND.to_string(),
            family,
            owner_count: probe_owners.len(),
            shard_count: probe_owners.iter().map(|owner| owner.shard_count).sum(),
            decision_count: probe_owners.iter().map(|owner| owner.decision_count).sum(),
            owners: probe_owners.to_vec(),
        }
    })?;
    let mut refs = Vec::new();
    for (index, owners) in chunks.into_iter().enumerate() {
        let manifest = DialogueTranslationOwnerIndexManifest {
            kind: OWNER_INDEX_KIND.to_string(),
            family,
            owner_count: owners.len(),
            shard_count: owners.iter().map(|owner| owner.shard_count).sum(),
            decision_count: owners.iter().map(|owner| owner.decision_count).sum(),
            owners,
        };
        let path = relative_path(owner_index_path(family, index))?;
        let (manifest_sha256, _) = write_bounded_json(root, &path, &manifest)?;
        refs.push(DialogueTranslationOwnerIndexRef {
            family,
            manifest_path: path_string(&path),
            manifest_sha256,
            owner_count: manifest.owner_count,
            shard_count: manifest.shard_count,
            decision_count: manifest.decision_count,
        });
    }
    Ok(refs)
}

fn owner_manifest_path(
    family: DialogueTranslationAssetFamily,
    owner_id: &str,
    index: usize,
) -> PathBuf {
    PathBuf::from("manifests")
        .join(family_name(family))
        .join(owner_id)
        .join(format!("decisions-{index:03}.json"))
}

fn owner_index_path(family: DialogueTranslationAssetFamily, index: usize) -> PathBuf {
    PathBuf::from("manifests")
        .join(family_name(family))
        .join(format!("owners-{index:03}.json"))
}

fn family_name(family: DialogueTranslationAssetFamily) -> &'static str {
    match family {
        DialogueTranslationAssetFamily::Primary => "primary",
        DialogueTranslationAssetFamily::Selector => "selector",
    }
}

fn merged_counts(left: DecisionCounts, right: DecisionCounts) -> DecisionCounts {
    let mut result = left;
    result.merge(right);
    result
}

#[cfg(test)]
mod tests {
    use super::super::super::dialogue_translation_assets_model::{
        TrackedPrimaryDialogueTranslationEntry, TrackedPrimaryDialogueTranslationShard,
    };
    use super::super::super::translation_workspace_io::json_bytes;
    use super::super::super::translation_workspace_model::{
        DialogueTranslationDecisionStatus, DialogueTranslationRouteOwner,
    };
    use super::super::super::translation_workspace_validation::MAX_SHARD_BYTES;
    use super::*;

    #[test]
    fn preserved_korean_growth_is_repartitioned_before_tracked_assets_are_written() {
        let entries: Vec<_> = (0..3)
            .map(|index| TrackedPrimaryDialogueTranslationEntry {
                semantic_source_sha256: format!("{index:064x}"),
                source_segments: vec!["原文".to_string()],
                controls: Vec::new(),
                korean_segments: vec![Some("가".repeat(4_000))],
                status: DialogueTranslationDecisionStatus::Draft,
                translator: Some("source-first-writer".to_string()),
                notes: String::new(),
            })
            .collect();
        let original_ids: Vec<_> = entries
            .iter()
            .map(|entry| entry.semantic_source_sha256.clone())
            .collect();
        let loaded = LoadedShard::Primary {
            path: PathBuf::from("primary/example/unit-000.json"),
            shard: TrackedPrimaryDialogueTranslationShard {
                kind: "tracked primary".to_string(),
                shard_id: "example-unit-000".to_string(),
                source_shard_sha256: "a".repeat(64),
                owner: DialogueTranslationRouteOwner {
                    source_path: "DAT2/EXAMPLE.BIZ".to_string(),
                    bank_selector: 0,
                    variant_selector: 0,
                    route_table_offset: "0x0".to_string(),
                },
                entries,
            },
            counts: DecisionCounts {
                untranslated: 0,
                draft: 3,
                ready_for_review: 0,
            },
        };

        let parts = partition_final_shard(&loaded).unwrap();
        assert!(parts.len() > 1);
        let mut observed_ids = Vec::new();
        for part in &parts {
            let LoadedShard::Primary {
                path,
                shard,
                counts,
            } = part
            else {
                panic!("primary input changed family while partitioning");
            };
            assert!(path.to_string_lossy().contains("-part-"));
            assert!(json_bytes(shard).unwrap().len() <= MAX_SHARD_BYTES);
            assert_eq!(counts.decision_count(), shard.entries.len());
            observed_ids.extend(
                shard
                    .entries
                    .iter()
                    .map(|entry| entry.semantic_source_sha256.clone()),
            );
        }
        assert_eq!(observed_ids, original_ids);
    }
}
