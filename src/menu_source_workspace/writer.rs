use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};

use super::model::{MenuSourceCandidate, MenuSourceShard, MenuSourceShardRef};
use crate::pipeline::sha256_bytes;

pub(super) const MAX_MENU_SOURCE_SHARD_ENTRIES: usize = 32;
pub(super) const MAX_MENU_SOURCE_SHARD_BYTES: usize = 24 * 1024;

pub(super) struct MenuSourceShardIdentity<'a> {
    pub(super) source_bin_sha256: &'a str,
    pub(super) dialogue_codebook_sha256: &'a str,
    pub(super) menu_code_analysis_sha256: &'a str,
    pub(super) menu_glyph_analysis_sha256: &'a str,
}

pub(super) fn prepare_workspace_root(root: &Path, force: bool) -> Result<()> {
    ensure!(
        force || !root.exists(),
        "refusing to replace existing menu source workspace {}; preserve authored work before using --force",
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

pub(super) fn write_overlay_shards(
    root: &Path,
    overlay_path: &str,
    entries: Vec<MenuSourceCandidate>,
    identity: &MenuSourceShardIdentity<'_>,
) -> Result<Vec<MenuSourceShardRef>> {
    let stem = overlay_stem(overlay_path)?;
    let chunks = partition_entries(overlay_path, entries, identity)?;
    let mut refs = Vec::with_capacity(chunks.len());
    for (unit_index, entries) in chunks.into_iter().enumerate() {
        let shard_id = format!("{stem}-source-{unit_index:03}");
        let shard = build_shard(&shard_id, overlay_path, entries, identity);
        let relative_path = PathBuf::from(format!("source/{stem}/unit-{unit_index:03}.json"));
        let (content_sha256, byte_count) = write_json(root, &relative_path, &shard)?;
        ensure!(
            byte_count <= MAX_MENU_SOURCE_SHARD_BYTES,
            "menu source shard exceeds byte limit after writing"
        );
        refs.push(MenuSourceShardRef {
            overlay_path: overlay_path.to_string(),
            shard_id,
            path: path_string(&relative_path),
            entry_count: shard.entries.len(),
            content_sha256,
        });
    }
    Ok(refs)
}

pub(super) fn write_json<T: serde::Serialize>(
    root: &Path,
    relative_path: &Path,
    value: &T,
) -> Result<(String, usize)> {
    ensure!(
        relative_path.is_relative()
            && relative_path
                .components()
                .all(|component| matches!(component, std::path::Component::Normal(_))),
        "workspace path must stay inside its root"
    );
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    let output = root.join(relative_path);
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    std::fs::write(&output, &bytes)
        .with_context(|| format!("failed to write {}", output.display()))?;
    Ok((sha256_bytes(&bytes), bytes.len()))
}

pub(super) fn partition_entries(
    overlay_path: &str,
    entries: Vec<MenuSourceCandidate>,
    identity: &MenuSourceShardIdentity<'_>,
) -> Result<Vec<Vec<MenuSourceCandidate>>> {
    let stem = overlay_stem(overlay_path)?;
    let mut chunks = Vec::new();
    let mut current = Vec::new();
    for entry in entries {
        current.push(entry);
        let shard_id = format!("{stem}-source-{:03}", chunks.len());
        let probe = build_shard(&shard_id, overlay_path, current, identity);
        let byte_count = serde_json::to_vec_pretty(&probe)?.len() + 1;
        current = probe.entries;
        if current.len() > MAX_MENU_SOURCE_SHARD_ENTRIES || byte_count > MAX_MENU_SOURCE_SHARD_BYTES
        {
            let overflow = current.pop().expect("candidate was just inserted");
            ensure!(
                !current.is_empty(),
                "one menu source entry exceeds shard limits"
            );
            chunks.push(current);
            current = vec![overflow];
            let shard_id = format!("{stem}-source-{:03}", chunks.len());
            let probe = build_shard(&shard_id, overlay_path, current, identity);
            let candidate_id = probe.entries[0].candidate_id.clone();
            let byte_count = serde_json::to_vec_pretty(&probe)?.len() + 1;
            ensure!(
                byte_count <= MAX_MENU_SOURCE_SHARD_BYTES,
                "menu source entry {candidate_id} serializes to {byte_count} bytes, exceeding the {MAX_MENU_SOURCE_SHARD_BYTES}-byte shard limit"
            );
            current = probe.entries;
        }
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    Ok(chunks)
}

fn build_shard(
    shard_id: &str,
    overlay_path: &str,
    entries: Vec<MenuSourceCandidate>,
    identity: &MenuSourceShardIdentity<'_>,
) -> MenuSourceShard {
    MenuSourceShard {
        kind: "Justice Gakuen 2 protected menu source candidate shard".to_string(),
        shard_id: shard_id.to_string(),
        source_bin_sha256: identity.source_bin_sha256.to_string(),
        dialogue_codebook_sha256: identity.dialogue_codebook_sha256.to_string(),
        menu_code_analysis_sha256: identity.menu_code_analysis_sha256.to_string(),
        menu_glyph_analysis_sha256: identity.menu_glyph_analysis_sha256.to_string(),
        overlay_path: overlay_path.to_string(),
        entries,
    }
}

fn overlay_stem(overlay_path: &str) -> Result<String> {
    let name = Path::new(overlay_path)
        .file_stem()
        .and_then(|name| name.to_str())
        .context("overlay path has no UTF-8 file stem")?;
    let stem = name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    ensure!(!stem.is_empty(), "overlay path has an empty file stem");
    Ok(stem)
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
