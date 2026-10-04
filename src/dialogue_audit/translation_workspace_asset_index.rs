use std::path::{Path, PathBuf};

use anyhow::{Result, ensure};

use crate::pipeline::sha256_bytes;

use super::translation_workspace_io::{read_bounded_json, relative_path, write_bounded_json};
use super::translation_workspace_model::{
    DialogueTranslationAssetIndexManifest, DialogueTranslationAssetIndexManifestRef,
    DialogueTranslationWorkspaceAssetRef, DialogueTranslationWorkspaceManifest,
};
use super::translation_workspace_writer::{partition, path_string};

pub(super) fn write_asset_index(
    root: &Path,
    assets: &[DialogueTranslationWorkspaceAssetRef],
) -> Result<Vec<DialogueTranslationAssetIndexManifestRef>> {
    let chunks = partition(assets, |probe_assets| {
        DialogueTranslationAssetIndexManifest {
            kind: "Justice Gakuen 2 dialogue translation asset index".to_string(),
            asset_count: probe_assets.len(),
            assets: probe_assets.to_vec(),
        }
    })?;
    chunks
        .into_iter()
        .enumerate()
        .map(|(index, assets)| {
            let manifest = DialogueTranslationAssetIndexManifest {
                kind: "Justice Gakuen 2 dialogue translation asset index".to_string(),
                asset_count: assets.len(),
                assets,
            };
            let path = relative_path(format!("manifests/assets-{index:03}.json"))?;
            let (manifest_sha256, _) = write_bounded_json(root, &path, &manifest)?;
            Ok(DialogueTranslationAssetIndexManifestRef {
                manifest_path: path_string(&path),
                manifest_sha256,
                asset_count: manifest.asset_count,
            })
        })
        .collect()
}

pub(super) fn read_asset_index(
    root: &Path,
    manifest: &DialogueTranslationWorkspaceManifest,
) -> Result<(Vec<DialogueTranslationWorkspaceAssetRef>, Vec<PathBuf>)> {
    if !manifest.assets.is_empty() {
        ensure!(
            manifest.asset_manifest_count == 0 && manifest.asset_manifests.is_empty(),
            "workspace mixes direct and sharded asset indexes"
        );
        return Ok((manifest.assets.clone(), Vec::new()));
    }
    ensure!(
        manifest.asset_manifest_count == manifest.asset_manifests.len()
            && manifest.asset_manifest_count > 0,
        "workspace asset index manifest population changed"
    );
    let mut assets = Vec::new();
    let mut paths = Vec::new();
    for manifest_ref in &manifest.asset_manifests {
        let path = PathBuf::from(&manifest_ref.manifest_path);
        let (index, bytes): (DialogueTranslationAssetIndexManifest, _) =
            read_bounded_json(root, &path)?;
        ensure!(
            sha256_bytes(&bytes) == manifest_ref.manifest_sha256
                && index.kind == "Justice Gakuen 2 dialogue translation asset index"
                && index.asset_count == index.assets.len()
                && index.asset_count == manifest_ref.asset_count,
            "workspace asset index binding changed at {}",
            path.display()
        );
        ensure!(!index.assets.is_empty(), "workspace asset index is empty");
        assets.extend(index.assets);
        paths.push(path);
    }
    ensure!(
        assets.len() == manifest.asset_count,
        "workspace asset index aggregate changed"
    );
    Ok((assets, paths))
}
