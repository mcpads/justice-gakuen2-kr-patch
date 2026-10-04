use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, ensure};
use serde::{Serialize, de::DeserializeOwned};

use crate::pipeline::sha256_bytes;

use super::translation_workspace_validation::MAX_SHARD_BYTES;

pub(super) fn json_bytes(value: &impl Serialize) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

pub(super) fn write_bounded_json(
    root: &Path,
    relative_path: &Path,
    value: &impl Serialize,
) -> Result<(String, usize)> {
    validate_relative_path(relative_path)?;
    let bytes = json_bytes(value)?;
    ensure!(
        bytes.len() <= MAX_SHARD_BYTES,
        "workspace JSON {} exceeds {} bytes",
        relative_path.display(),
        MAX_SHARD_BYTES
    );
    let output = root.join(relative_path);
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    std::fs::write(&output, &bytes)
        .with_context(|| format!("failed to write {}", output.display()))?;
    Ok((sha256_bytes(&bytes), bytes.len()))
}

pub(super) fn read_bounded_json<T: DeserializeOwned>(
    root: &Path,
    relative_path: &Path,
) -> Result<(T, Vec<u8>)> {
    validate_relative_path(relative_path)?;
    let path = root.join(relative_path);
    let bytes =
        std::fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
    ensure!(
        bytes.len() <= MAX_SHARD_BYTES,
        "workspace JSON {} exceeds {} bytes",
        relative_path.display(),
        MAX_SHARD_BYTES
    );
    let value = serde_json::from_slice(&bytes)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    Ok((value, bytes))
}

pub(super) fn relative_path(path: impl Into<PathBuf>) -> Result<PathBuf> {
    let path = path.into();
    validate_relative_path(&path)?;
    Ok(path)
}

fn validate_relative_path(path: &Path) -> Result<()> {
    ensure!(!path.is_absolute(), "workspace path must be relative");
    ensure!(
        path.components()
            .all(|component| matches!(component, Component::Normal(_) | Component::CurDir)),
        "workspace path may not escape its root: {}",
        path.display()
    );
    Ok(())
}
