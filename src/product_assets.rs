//! Shared asset manifests that are read from the supplied asset root.
//!
//! The development build spec is located at `<asset-root>/build/development.json`.
//! Loading it binds `<asset-root>`, and product steps read the few shared
//! manifests without a dedicated spec entry from that root. Unit tests read the
//! same relative paths under this crate's `assets/` directory; tests that need
//! them are marked `#[ignore = "requires ..."]`.

#[cfg(not(test))]
use anyhow::ensure;
use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

#[cfg(not(test))]
static ROOT: OnceLock<PathBuf> = OnceLock::new();
static CACHE: OnceLock<Mutex<BTreeMap<PathBuf, &'static [u8]>>> = OnceLock::new();

/// Binds the asset root that contains the loaded development build spec.
#[cfg(not(test))]
pub(crate) fn bind_root_from_spec(spec: &Path) -> Result<()> {
    let base = spec.parent().unwrap_or_else(|| Path::new("."));
    let root = base
        .join("..")
        .canonicalize()
        .with_context(|| format!("asset root of {} is unavailable", spec.display()))?;
    let bound = ROOT.get_or_init(|| root.clone());
    ensure!(
        *bound == root,
        "development build specs from different asset roots: {} and {}",
        bound.display(),
        root.display()
    );
    Ok(())
}

/// Tests always read the crate-local asset root.
#[cfg(test)]
pub(crate) fn bind_root_from_spec(_spec: &Path) -> Result<()> {
    Ok(())
}

#[cfg(not(test))]
fn root() -> Result<PathBuf> {
    ROOT.get()
        .cloned()
        .context("asset root is unavailable; pass --spec <asset-root>/build/development.json")
}

#[cfg(test)]
fn root() -> Result<PathBuf> {
    Ok(Path::new(env!("CARGO_MANIFEST_DIR")).join("assets"))
}

/// Reads `relative` under the bound asset root once per process.
pub(crate) fn read(relative: &str) -> Result<&'static [u8]> {
    let path = root()?.join(relative);
    let mut cache = CACHE
        .get_or_init(|| Mutex::new(BTreeMap::new()))
        .lock()
        .map_err(|_| anyhow::anyhow!("asset cache poisoned"))?;
    if let Some(bytes) = cache.get(&path) {
        return Ok(bytes);
    }
    let bytes: &'static [u8] = std::fs::read(&path)
        .with_context(|| format!("required asset {} is unavailable", path.display()))?
        .leak();
    cache.insert(path, bytes);
    Ok(bytes)
}
