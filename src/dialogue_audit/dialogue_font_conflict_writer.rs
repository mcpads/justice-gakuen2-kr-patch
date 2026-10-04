use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};

use super::dialogue_font_conflict_attribution::AttributedConflictCode;
use super::dialogue_font_conflicts_model::{
    DialogueFontConflictAssetManifest, DialogueFontConflictAssetRef, DialogueFontConflictCode,
    DialogueFontConflictCodeShard, DialogueFontConflictGroup, DialogueFontConflictGroupIndex,
    DialogueFontConflictGroupIndexRef, DialogueFontConflictGroupShard,
    DialogueFontConflictRequirementShard, DialogueFontConflictShardRef,
};
use super::format::hex_code;
use super::translation_workspace_io::write_bounded_json;
use super::translation_workspace_source::asset_stem;
use super::translation_workspace_writer::{partition, path_string};

pub(super) fn write_asset_reports(
    root: &Path,
    assets: Vec<(String, Vec<AttributedConflictCode>)>,
) -> Result<Vec<DialogueFontConflictAssetRef>> {
    let mut refs = Vec::new();
    for (source_path, conflicts) in assets {
        let stem = asset_stem(&source_path)?;
        let serialized_conflicts = write_requirement_shards(root, &stem, &source_path, &conflicts)?;
        let chunks = partition(&serialized_conflicts, |entries| {
            DialogueFontConflictCodeShard {
                kind: "Justice Gakuen 2 dialogue font conflict code shard".to_string(),
                source_path: source_path.clone(),
                entries: entries.to_vec(),
            }
        })?;
        let mut conflict_shards = Vec::new();
        for (index, entries) in chunks.into_iter().enumerate() {
            let path = PathBuf::from(format!("assets/{stem}/conflicts/unit-{index:03}.json"));
            let shard = DialogueFontConflictCodeShard {
                kind: "Justice Gakuen 2 dialogue font conflict code shard".to_string(),
                source_path: source_path.clone(),
                entries,
            };
            let entry_count = shard.entries.len();
            let (content_sha256, _) = write_bounded_json(root, &path, &shard)?;
            conflict_shards.push(DialogueFontConflictShardRef {
                path: path_string(&path),
                content_sha256,
                entry_count,
            });
        }
        let required_groups = conflicts
            .iter()
            .flat_map(|conflict| conflict.required_semantic_hashes.iter())
            .collect::<BTreeSet<_>>();
        let single_group_owned_code_count = conflicts
            .iter()
            .filter(|conflict| conflict.required_semantic_hashes.len() == 1)
            .count();
        let manifest = DialogueFontConflictAssetManifest {
            kind: "Justice Gakuen 2 dialogue font conflict asset manifest".to_string(),
            source_path: source_path.clone(),
            conflicting_code_count: conflicts.len(),
            required_untranslated_group_count: required_groups.len(),
            single_group_owned_code_count,
            conflict_shards,
        };
        let manifest_path = PathBuf::from(format!("assets/{stem}/manifest.json"));
        let (manifest_sha256, _) = write_bounded_json(root, &manifest_path, &manifest)?;
        refs.push(DialogueFontConflictAssetRef {
            source_path,
            manifest_path: path_string(&manifest_path),
            manifest_sha256,
            conflicting_code_count: manifest.conflicting_code_count,
            required_untranslated_group_count: manifest.required_untranslated_group_count,
            single_group_owned_code_count,
        });
    }
    Ok(refs)
}

fn write_requirement_shards(
    root: &Path,
    stem: &str,
    source_path: &str,
    conflicts: &[AttributedConflictCode],
) -> Result<Vec<DialogueFontConflictCode>> {
    let mut result = Vec::new();
    for conflict in conflicts {
        let code = hex_code(conflict.code);
        let chunks = partition(&conflict.required_semantic_hashes, |entries| {
            DialogueFontConflictRequirementShard {
                kind: "Justice Gakuen 2 dialogue font conflict requirement shard".to_string(),
                source_path: source_path.to_string(),
                code: code.clone(),
                semantic_source_sha256: entries.to_vec(),
            }
        })?;
        let mut requirement_shards = Vec::new();
        for (index, entries) in chunks.into_iter().enumerate() {
            let path = PathBuf::from(format!(
                "assets/{stem}/requirements/code-{:04x}/unit-{index:03}.json",
                conflict.code
            ));
            let shard = DialogueFontConflictRequirementShard {
                kind: "Justice Gakuen 2 dialogue font conflict requirement shard".to_string(),
                source_path: source_path.to_string(),
                code: code.clone(),
                semantic_source_sha256: entries,
            };
            let entry_count = shard.semantic_source_sha256.len();
            let (content_sha256, _) = write_bounded_json(root, &path, &shard)?;
            requirement_shards.push(DialogueFontConflictShardRef {
                path: path_string(&path),
                content_sha256,
                entry_count,
            });
        }
        result.push(DialogueFontConflictCode {
            code,
            replacement_character: conflict.replacement_character.clone(),
            preserved_source_character: conflict.preserved_source_character.clone(),
            preserved_coordinate_count: conflict.preserved_coordinate_count,
            preserved_glyph_occurrence_count: conflict.preserved_glyph_occurrence_count,
            required_group_count: conflict.required_semantic_hashes.len(),
            requirement_shards,
            single_group_owned: conflict.required_semantic_hashes.len() == 1,
        });
    }
    Ok(result)
}

pub(super) fn write_group_reports(
    root: &Path,
    groups: &[DialogueFontConflictGroup],
) -> Result<Vec<DialogueFontConflictGroupIndexRef>> {
    let chunks = partition(groups, |entries| DialogueFontConflictGroupShard {
        kind: "Justice Gakuen 2 dialogue font conflict translation group shard".to_string(),
        entries: entries.to_vec(),
    })?;
    let mut refs = Vec::new();
    for (index, entries) in chunks.into_iter().enumerate() {
        let path = PathBuf::from(format!("groups/unit-{index:03}.json"));
        let shard = DialogueFontConflictGroupShard {
            kind: "Justice Gakuen 2 dialogue font conflict translation group shard".to_string(),
            entries,
        };
        let entry_count = shard.entries.len();
        let (content_sha256, _) = write_bounded_json(root, &path, &shard)?;
        refs.push(DialogueFontConflictShardRef {
            path: path_string(&path),
            content_sha256,
            entry_count,
        });
    }
    write_group_indexes(root, &refs)
}

pub(super) fn write_group_indexes(
    root: &Path,
    shard_refs: &[DialogueFontConflictShardRef],
) -> Result<Vec<DialogueFontConflictGroupIndexRef>> {
    let chunks = partition(shard_refs, |group_shards| DialogueFontConflictGroupIndex {
        kind: "Justice Gakuen 2 dialogue font conflict group index".to_string(),
        group_shards: group_shards.to_vec(),
    })?;
    let mut indexes = Vec::new();
    for (index, group_shards) in chunks.into_iter().enumerate() {
        let path = PathBuf::from(format!("group-indexes/unit-{index:03}.json"));
        let shard_count = group_shards.len();
        let entry_count = group_shards.iter().map(|shard| shard.entry_count).sum();
        let manifest = DialogueFontConflictGroupIndex {
            kind: "Justice Gakuen 2 dialogue font conflict group index".to_string(),
            group_shards,
        };
        let (content_sha256, _) = write_bounded_json(root, &path, &manifest)?;
        indexes.push(DialogueFontConflictGroupIndexRef {
            path: path_string(&path),
            content_sha256,
            shard_count,
            entry_count,
        });
    }
    Ok(indexes)
}

pub(super) fn ensure_output_available(path: &Path, force: bool) -> Result<()> {
    ensure!(
        !path.exists() || force,
        "{} already exists; pass --force",
        path.display()
    );
    Ok(())
}

pub(super) fn prepare_output_directory(path: &Path) -> Result<()> {
    if path.exists() {
        std::fs::remove_dir_all(path)
            .with_context(|| format!("failed to replace {}", path.display()))?;
    }
    std::fs::create_dir_all(path).with_context(|| format!("failed to create {}", path.display()))
}
