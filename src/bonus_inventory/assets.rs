use std::collections::BTreeSet;
use std::path::{Component, Path};

use anyhow::{Context, Result, ensure};

use crate::pipeline::sha256_bytes;
use crate::tim::{cells_overlap, read_indexed_cell_in_prefix};

use super::catalog::{EXPECTED_UNITS, expected_occurrences};
use super::model::{BonusInventoryManifest, BonusInventoryUnit, DevelopmentStatus, ReleaseStatus};
use super::source::{
    BONUS_INVENTORY_PATH, BonusInventorySource, FIXED_UI_TIM_OFFSET, SOURCE_DECODED_SHA256,
    SOURCE_STORED_SHA256,
};

const MANIFEST_KIND: &str = "justice_gakuen2_bonus_inventory_manifest";
const UNIT_KIND: &str = "justice_gakuen2_bonus_inventory_unit";

pub(super) struct LoadedBonusInventoryAssets {
    pub(super) manifest_sha256: String,
    pub(super) units: Vec<BonusInventoryUnit>,
}

pub(super) fn load_assets(
    root: &Path,
    source: &BonusInventorySource,
) -> Result<LoadedBonusInventoryAssets> {
    let manifest_path = root.join("manifest.json");
    let manifest_bytes = std::fs::read(&manifest_path)
        .with_context(|| format!("failed to read {}", manifest_path.display()))?;
    let manifest: BonusInventoryManifest = serde_json::from_slice(&manifest_bytes)
        .with_context(|| format!("failed to parse {}", manifest_path.display()))?;
    ensure!(
        manifest.kind == MANIFEST_KIND,
        "unsupported bonus-inventory manifest kind"
    );
    ensure!(
        manifest.source.path == BONUS_INVENTORY_PATH
            && manifest.source.stored_sha256 == SOURCE_STORED_SHA256
            && manifest.source.decoded_sha256 == SOURCE_DECODED_SHA256
            && parse_hex_offset(&manifest.source.fixed_ui_tim_offset)? == FIXED_UI_TIM_OFFSET,
        "bonus-inventory manifest source binding changed"
    );
    ensure!(
        manifest.runtime_consumers == ["bonus_inventory_main", "bonus_inventory_card_viewer"],
        "bonus-inventory runtime consumer set changed"
    );
    ensure!(
        manifest.units.len() == EXPECTED_UNITS.len(),
        "bonus-inventory fixed unit count changed"
    );

    let mut paths = BTreeSet::new();
    let mut ids = BTreeSet::new();
    let mut cells = Vec::new();
    let mut units = Vec::with_capacity(EXPECTED_UNITS.len());
    for (index, relative) in manifest.units.iter().enumerate() {
        ensure_relative_path(relative)?;
        ensure!(
            paths.insert(relative.clone()),
            "duplicate bonus-inventory unit path"
        );
        let path = root.join(relative);
        let bytes =
            std::fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
        let unit: BonusInventoryUnit = serde_json::from_slice(&bytes)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        ensure!(
            unit.kind == UNIT_KIND && ids.insert(unit.id.clone()),
            "bonus-inventory unit identity changed"
        );
        validate_unit(&unit, EXPECTED_UNITS[index], source, &mut cells)?;
        units.push(unit);
    }
    Ok(LoadedBonusInventoryAssets {
        manifest_sha256: sha256_bytes(&manifest_bytes),
        units,
    })
}

fn validate_unit(
    unit: &BonusInventoryUnit,
    expected: (&str, &str),
    source: &BonusInventorySource,
    cells: &mut Vec<crate::tim::Cell>,
) -> Result<()> {
    ensure!(
        unit.id == expected.0 && unit.source_text == expected.1,
        "bonus-inventory source meaning changed for {}",
        unit.id
    );
    validate_translation_state(unit)?;
    let expected_occurrences = expected_occurrences(&unit.id);
    ensure!(
        unit.occurrences.len() == expected_occurrences.len(),
        "bonus-inventory occurrence count changed for {}",
        unit.id
    );
    for (occurrence, expected_occurrence) in unit.occurrences.iter().zip(expected_occurrences) {
        ensure!(
            occurrence.id == expected_occurrence.id,
            "bonus-inventory occurrence id changed for {}:{}",
            unit.id,
            occurrence.id
        );
        ensure!(
            occurrence.font_role == expected_occurrence.font_role,
            "bonus-inventory font role changed for {}:{}",
            unit.id,
            occurrence.id
        );
        ensure!(
            occurrence.cell == expected_occurrence.cell,
            "bonus-inventory cell changed for {}:{}: expected {:?}, found {:?}",
            unit.id,
            occurrence.id,
            expected_occurrence.cell,
            occurrence.cell
        );
        ensure!(
            cells
                .iter()
                .all(|cell| !cells_overlap(*cell, occurrence.cell)),
            "bonus-inventory editable cells overlap"
        );
        let pixels = read_indexed_cell_in_prefix(
            &source.inventory_decoded,
            FIXED_UI_TIM_OFFSET,
            occurrence.cell,
        )?;
        ensure!(
            sha256_bytes(&pixels) == occurrence.source_indexed_pixel_sha256,
            "bonus-inventory source region changed for {}:{}",
            unit.id,
            occurrence.id
        );
        cells.push(occurrence.cell);
    }
    Ok(())
}

pub(super) fn validate_translation_state(unit: &BonusInventoryUnit) -> Result<()> {
    ensure!(
        !unit.source_text.trim().is_empty() && !unit.occurrences.is_empty(),
        "bonus-inventory unit {} has incomplete source fields",
        unit.id
    );
    match unit.development_status {
        DevelopmentStatus::Authored => {
            ensure!(
                unit.korean_text
                    .as_deref()
                    .is_some_and(|text| !text.trim().is_empty())
                    && unit.release_status != ReleaseStatus::Untranslated
                    && unit
                        .occurrences
                        .iter()
                        .all(|occurrence| occurrence.font_role.is_some()),
                "authored bonus-inventory unit {} lacks build input",
                unit.id
            );
        }
        DevelopmentStatus::Untranslated => {
            ensure!(
                unit.korean_text.is_none()
                    && unit.release_status == ReleaseStatus::Untranslated
                    && unit
                        .occurrences
                        .iter()
                        .all(|occurrence| occurrence.font_role.is_none()),
                "untranslated bonus-inventory unit {} contains translated state",
                unit.id
            );
        }
    }
    Ok(())
}

pub(super) fn parse_hex_offset(value: &str) -> Result<usize> {
    usize::from_str_radix(
        value
            .strip_prefix("0x")
            .context("bonus-inventory TIM offset is not hexadecimal")?,
        16,
    )
    .context("invalid bonus-inventory TIM offset")
}

fn ensure_relative_path(path: &Path) -> Result<()> {
    ensure!(
        !path.as_os_str().is_empty()
            && !path.is_absolute()
            && path
                .components()
                .all(|component| matches!(component, Component::Normal(_))),
        "bonus-inventory unit path must stay inside its asset directory"
    );
    Ok(())
}
