use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};

use super::model::BonusShopTextSourceUnitRef;
use super::parser::ParsedShopTextTable;
use crate::pipeline::sha256_bytes;

pub(super) fn prepare_output(root: &Path, force: bool) -> Result<()> {
    ensure!(
        force || !root.exists(),
        "refusing to replace existing bonus-shop source workspace {}; preserve it or pass --force",
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

pub(super) fn write_source_units(
    root: &Path,
    table: &ParsedShopTextTable,
) -> Result<Vec<BonusShopTextSourceUnitRef>> {
    let mut refs = Vec::with_capacity(table.records.len());
    for record in &table.records {
        let relative_path = PathBuf::from(format!(
            "{}/units/{}.json",
            table.spec.role.slug(),
            record.unit.unit_id
        ));
        let (content_sha256, _) = write_json(root, &relative_path, &record.unit)?;
        refs.push(BonusShopTextSourceUnitRef {
            unit_id: record.unit.unit_id.clone(),
            path: path_string(&relative_path),
            source_offset: record.unit.source_offset.clone(),
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
        "bonus-shop source path must stay inside its workspace root"
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

pub(super) fn path_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
