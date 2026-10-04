use std::collections::BTreeSet;
use std::path::{Component, Path};

use anyhow::{Context, Result, ensure};

use super::model::{DiaryHeaderEntry, DiaryHeaderManifest, DiaryHeaderUnit};

const MANIFEST_KIND: &str = "justice_gakuen2_diary_header_manifest";
const UNIT_KIND: &str = "justice_gakuen2_diary_header_unit";

pub(super) fn load_diary_header_assets(
    directory: &Path,
) -> Result<(DiaryHeaderManifest, Vec<DiaryHeaderEntry>)> {
    let manifest_path = directory.join("manifest.json");
    let manifest_bytes = std::fs::read(&manifest_path)
        .with_context(|| format!("failed to read {}", manifest_path.display()))?;
    let manifest: DiaryHeaderManifest = serde_json::from_slice(&manifest_bytes)
        .with_context(|| format!("failed to parse {}", manifest_path.display()))?;
    ensure!(
        manifest.kind == MANIFEST_KIND,
        "unsupported diary header manifest kind {:?}",
        manifest.kind
    );
    ensure!(
        !manifest.units.is_empty(),
        "diary header manifest has no units"
    );

    let mut unit_paths = BTreeSet::new();
    let mut entries = Vec::new();
    for relative_path in &manifest.units {
        ensure!(
            relative_path
                .components()
                .all(|component| matches!(component, Component::Normal(_))),
            "diary header unit path must stay inside its asset directory: {}",
            relative_path.display()
        );
        ensure!(
            unit_paths.insert(relative_path.clone()),
            "duplicate diary header unit {}",
            relative_path.display()
        );
        let path = directory.join(relative_path);
        let bytes =
            std::fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
        let unit: DiaryHeaderUnit = serde_json::from_slice(&bytes)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        ensure!(
            unit.kind == UNIT_KIND,
            "unsupported diary header unit kind in {}",
            path.display()
        );
        ensure!(
            !unit.entries.is_empty(),
            "diary header unit is empty: {}",
            path.display()
        );
        entries.extend(unit.entries);
    }
    Ok((manifest, entries))
}
