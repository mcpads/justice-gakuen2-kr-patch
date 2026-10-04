use std::collections::BTreeSet;
use std::path::Path;

use anyhow::{Context, Result, ensure};

use super::translation_model::{DialogueTranslationInitConfig, DialogueTranslationProjectStatus};
use super::translation_workspace_asset_index::write_asset_index;
use super::translation_workspace_io::{relative_path, write_bounded_json};
use super::translation_workspace_model::{
    DialogueTranslationAssetManifest, DialogueTranslationWorkspaceAssetRef,
    DialogueTranslationWorkspaceManifest, DialogueTranslationWorkspaceRole,
};
use super::translation_workspace_source::{
    TranslationWorkspaceSource, asset_stem, extract_translation_workspace_source,
};
use super::translation_workspace_validation::{MAX_SHARD_BYTES, MAX_SHARD_ENTRIES};
use super::translation_workspace_writer::{
    path_string, write_context_units, write_role_manifest, write_translation_units,
};

const WORKSPACE_KIND: &str = "Justice Gakuen 2 sharded Korean dialogue translation workspace";

pub(super) fn initialize_translation_workspace(
    config: &DialogueTranslationInitConfig,
) -> Result<DialogueTranslationWorkspaceManifest> {
    prepare_workspace_root(&config.output, config.force)?;
    let source = extract_translation_workspace_source(&config.cue, &config.codebook, config.scope)?;
    let asset_paths: BTreeSet<_> = source
        .contexts_by_owner
        .keys()
        .chain(source.groups_by_owner.keys())
        .map(|owner| owner.source_path.clone())
        .collect();
    ensure!(
        asset_paths.len() + source.unresolved_primary_script_assets.len()
            == source.source_asset_count,
        "workspace asset population changed"
    );

    let mut assets = Vec::new();
    for source_path in asset_paths {
        assets.push(write_asset_workspace(
            &config.output,
            &source,
            &source_path,
        )?);
    }
    let semantic_group_count: usize = assets.iter().map(|asset| asset.semantic_group_count).sum();
    let context_occurrence_count: usize = assets
        .iter()
        .map(|asset| asset.context_occurrence_count)
        .sum();
    let asset_count = assets.len();
    let asset_manifests = write_asset_index(&config.output, &assets)?;
    let manifest = DialogueTranslationWorkspaceManifest {
        kind: WORKSPACE_KIND.to_string(),
        source_bin_sha256: source.source_bin_sha256,
        codebook_sha256: source.codebook_sha256,
        source_corpus_sha256: source.source_corpus_sha256,
        script_inventory_sha256: source.script_inventory_sha256,
        target_scope: source.target_scope,
        source_asset_count: source.source_asset_count,
        unresolved_primary_script_assets: source.unresolved_primary_script_assets,
        project_status: DialogueTranslationProjectStatus::InProgress,
        project_review: None,
        asset_count,
        semantic_group_count,
        referenced_coordinate_count: source.referenced_coordinates.len(),
        context_occurrence_count,
        max_entries_per_shard: MAX_SHARD_ENTRIES,
        max_bytes_per_shard: MAX_SHARD_BYTES,
        asset_manifest_count: asset_manifests.len(),
        asset_manifests,
        assets: Vec::new(),
    };
    ensure!(
        manifest.asset_count + manifest.unresolved_primary_script_assets.len()
            == manifest.source_asset_count,
        "workspace manifest denominator changed"
    );
    write_bounded_json(&config.output, Path::new("manifest.json"), &manifest)?;
    Ok(manifest)
}

fn write_asset_workspace(
    root: &Path,
    source: &TranslationWorkspaceSource,
    source_path: &str,
) -> Result<DialogueTranslationWorkspaceAssetRef> {
    let stem = asset_stem(source_path)?;
    let owners: Vec<_> = source
        .contexts_by_owner
        .keys()
        .chain(source.groups_by_owner.keys())
        .filter(|owner| owner.source_path == source_path)
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    let mut source_refs = Vec::new();
    let mut context_refs = Vec::new();
    let mut translation_refs = Vec::new();
    let mut review_refs = Vec::new();
    for owner in owners {
        if let Some(contexts) = source.contexts_by_owner.get(&owner) {
            write_context_units(root, source, &stem, &owner, contexts, &mut context_refs)?;
        }
        if let Some(groups) = source.groups_by_owner.get(&owner) {
            write_translation_units(
                root,
                source,
                &stem,
                &owner,
                groups,
                &mut source_refs,
                &mut translation_refs,
                &mut review_refs,
            )?;
        }
    }

    let role_refs = vec![
        write_role_manifest(
            root,
            source_path,
            DialogueTranslationWorkspaceRole::Source,
            source_refs,
        )?,
        write_role_manifest(
            root,
            source_path,
            DialogueTranslationWorkspaceRole::Context,
            context_refs,
        )?,
        write_role_manifest(
            root,
            source_path,
            DialogueTranslationWorkspaceRole::Translation,
            translation_refs,
        )?,
        write_role_manifest(
            root,
            source_path,
            DialogueTranslationWorkspaceRole::Review,
            review_refs,
        )?,
    ];
    let semantic_group_count = role_refs
        .iter()
        .find(|role| role.role == DialogueTranslationWorkspaceRole::Source)
        .unwrap()
        .entry_count;
    let context_occurrence_count = role_refs
        .iter()
        .find(|role| role.role == DialogueTranslationWorkspaceRole::Context)
        .unwrap()
        .entry_count;
    let referenced_coordinate_count: usize = source
        .groups_by_owner
        .iter()
        .filter(|(owner, _)| owner.source_path == source_path)
        .flat_map(|(_, groups)| groups)
        .map(|group| group.referenced_coordinate_ids.len())
        .sum();
    let asset_manifest = DialogueTranslationAssetManifest {
        kind: "Justice Gakuen 2 dialogue translation asset manifest".to_string(),
        source_path: source_path.to_string(),
        semantic_group_count,
        referenced_coordinate_count,
        context_occurrence_count,
        roles: role_refs,
    };
    let manifest_path = relative_path(format!("manifests/{stem}/asset.json"))?;
    let (manifest_sha256, _) = write_bounded_json(root, &manifest_path, &asset_manifest)?;
    Ok(DialogueTranslationWorkspaceAssetRef {
        source_path: source_path.to_string(),
        manifest_path: path_string(&manifest_path),
        manifest_sha256,
        semantic_group_count,
        referenced_coordinate_count,
        context_occurrence_count,
    })
}

pub(super) fn prepare_workspace_root(root: &Path, force: bool) -> Result<()> {
    ensure!(
        force || !root.exists(),
        "refusing to replace existing translation workspace {}; preserve authored work before using --force",
        root.display()
    );
    if force && root.exists() {
        if root.is_dir() {
            std::fs::remove_dir_all(root)
                .with_context(|| format!("failed to remove {}", root.display()))?;
        } else {
            std::fs::remove_file(root)
                .with_context(|| format!("failed to remove {}", root.display()))?;
        }
    }
    std::fs::create_dir_all(root).with_context(|| format!("failed to create {}", root.display()))
}
