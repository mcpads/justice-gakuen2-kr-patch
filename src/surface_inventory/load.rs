use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path};

use anyhow::{Context, Result, ensure};
use sha2::{Digest, Sha256};

use super::model::{
    SurfaceInventory, SurfaceInventoryManifestDocument, SurfaceInventoryShardDocument,
};
use super::validation::validate_surface_graph;

const MANIFEST_KIND: &str = "justice_gakuen2_surface_inventory_manifest";
const SHARD_KIND: &str = "justice_gakuen2_surface_inventory_shard";

pub(crate) fn load_surface_inventory(path: &Path) -> Result<SurfaceInventory> {
    let manifest_bytes = std::fs::read(path)
        .with_context(|| format!("failed to read surface inventory {}", path.display()))?;
    let manifest: SurfaceInventoryManifestDocument = serde_json::from_slice(&manifest_bytes)
        .with_context(|| format!("failed to parse surface inventory {}", path.display()))?;
    ensure!(
        manifest.kind == MANIFEST_KIND,
        "unsupported surface inventory manifest kind {:?}",
        manifest.kind
    );
    ensure!(
        !manifest.shards.is_empty(),
        "surface inventory manifest has no shards"
    );
    ensure!(
        !manifest.bindings.is_empty(),
        "surface inventory manifest has no source bindings"
    );

    let base = path.parent().unwrap_or_else(|| Path::new("."));
    let mut inventory_paths = BTreeSet::new();
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut identity_files = vec![("manifest.json".to_string(), manifest_bytes)];
    for shard_path in &manifest.shards {
        validate_relative_path(shard_path)?;
        let shard_name = shard_path.to_string_lossy().into_owned();
        ensure!(
            inventory_paths.insert(shard_name.clone()),
            "duplicate surface inventory file {shard_name}"
        );
        let absolute_path = base.join(shard_path);
        let bytes = std::fs::read(&absolute_path).with_context(|| {
            format!(
                "failed to read surface inventory shard {}",
                absolute_path.display()
            )
        })?;
        let shard: SurfaceInventoryShardDocument =
            serde_json::from_slice(&bytes).with_context(|| {
                format!(
                    "failed to parse surface inventory shard {}",
                    absolute_path.display()
                )
            })?;
        ensure!(
            shard.kind == SHARD_KIND,
            "unsupported surface inventory shard kind in {shard_name}"
        );
        ensure!(
            shard.family_id == manifest.family_id,
            "surface inventory shard {shard_name} belongs to a different family"
        );
        nodes.extend(shard.nodes);
        edges.extend(shard.edges);
        identity_files.push((shard_name, bytes));
    }
    let shard_count = manifest.shards.len();

    let mut binding_files = BTreeMap::new();
    for binding_path in &manifest.bindings {
        validate_relative_path(binding_path)?;
        let binding_name = binding_path.to_string_lossy().into_owned();
        ensure!(
            inventory_paths.insert(binding_name.clone()),
            "duplicate surface inventory file {binding_name}"
        );
        let absolute_path = base.join(binding_path);
        let bytes = std::fs::read(&absolute_path).with_context(|| {
            format!(
                "failed to read surface inventory binding {}",
                absolute_path.display()
            )
        })?;
        identity_files.push((binding_name.clone(), bytes.clone()));
        binding_files.insert(binding_name, bytes);
    }

    validate_surface_graph(
        &manifest.family_id,
        &manifest.source_bin_sha256,
        &manifest.root_surface_id,
        &manifest.traversal_boundary,
        &nodes,
        &edges,
    )?;

    Ok(SurfaceInventory {
        family_id: manifest.family_id,
        source_bin_sha256: manifest.source_bin_sha256,
        root_surface_id: manifest.root_surface_id,
        traversal_boundary: manifest.traversal_boundary,
        nodes,
        edges,
        shard_count,
        binding_files,
        identity_sha256: hash_inventory_files(&identity_files),
    })
}

fn validate_relative_path(path: &Path) -> Result<()> {
    ensure!(
        !path.as_os_str().is_empty()
            && !path.is_absolute()
            && path
                .components()
                .all(|component| matches!(component, Component::Normal(_))),
        "surface inventory contains an unsafe shard path {}",
        path.display()
    );
    Ok(())
}

fn hash_inventory_files(files: &[(String, Vec<u8>)]) -> String {
    let mut digest = Sha256::new();
    digest.update(b"justice-gakuen2-surface-inventory\0");
    for (path, bytes) in files {
        digest.update((path.len() as u64).to_le_bytes());
        digest.update(path.as_bytes());
        digest.update((bytes.len() as u64).to_le_bytes());
        digest.update(bytes);
    }
    format!("{:x}", digest.finalize())
}
