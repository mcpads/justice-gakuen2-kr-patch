use std::collections::BTreeSet;
use std::path::{Component, Path};

use anyhow::{Context, Result, ensure};

use crate::bonus_shop_source::BonusShopSource;
use crate::pipeline::sha256_bytes;
use crate::tim::{Cell, read_indexed_cell_in_prefix};

use super::model::{DevelopmentStatus, ReleaseStatus, ShopUiFontRole, ShopUiManifest, ShopUiUnit};
use super::source::{SHOP_UI_PATH, SOURCE_DECODED_SHA256, SOURCE_STORED_SHA256, UI_TIM_OFFSET};

const MANIFEST_KIND: &str = "justice_gakuen2_shop_ui_manifest";
const UNIT_KIND: &str = "justice_gakuen2_shop_ui_unit";
const EXPECTED_UNITS: [(&str, &str, ShopUiFontRole, Cell); 2] = [
    (
        "current_points",
        "現在のポイント",
        ShopUiFontRole::CurrentPoints,
        Cell {
            x: 0,
            y: 32,
            width: 148,
            height: 32,
        },
    ),
    (
        "heading",
        "購買部",
        ShopUiFontRole::Heading,
        Cell {
            x: 96,
            y: 64,
            width: 136,
            height: 48,
        },
    ),
];

pub(super) struct LoadedShopUiAssets {
    pub(super) manifest_sha256: String,
    pub(super) units: Vec<ShopUiUnit>,
}

pub(super) fn load_assets(root: &Path, source: &BonusShopSource) -> Result<LoadedShopUiAssets> {
    let manifest_path = root.join("manifest.json");
    let manifest_bytes = std::fs::read(&manifest_path)
        .with_context(|| format!("failed to read {}", manifest_path.display()))?;
    let manifest: ShopUiManifest = serde_json::from_slice(&manifest_bytes)
        .with_context(|| format!("failed to parse {}", manifest_path.display()))?;
    ensure!(
        manifest.kind == MANIFEST_KIND,
        "unsupported shop UI manifest kind"
    );
    ensure!(
        manifest.source.path == SHOP_UI_PATH
            && manifest.source.stored_sha256 == SOURCE_STORED_SHA256
            && manifest.source.decoded_sha256 == SOURCE_DECODED_SHA256,
        "shop UI manifest source binding changed"
    );
    ensure!(
        manifest.units.len() == EXPECTED_UNITS.len(),
        "shop UI manifest unit count changed"
    );

    let mut paths = BTreeSet::new();
    let mut ids = BTreeSet::new();
    let mut units = Vec::with_capacity(EXPECTED_UNITS.len());
    for (index, relative) in manifest.units.iter().enumerate() {
        ensure_relative_path(relative)?;
        ensure!(
            paths.insert(relative.clone()),
            "duplicate shop UI unit path"
        );
        let path = root.join(relative);
        let bytes =
            std::fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
        let unit: ShopUiUnit = serde_json::from_slice(&bytes)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        ensure!(
            unit.kind == UNIT_KIND && ids.insert(unit.id.clone()),
            "shop UI unit identity changed"
        );
        validate_unit(&unit, EXPECTED_UNITS[index], source)?;
        units.push(unit);
    }
    Ok(LoadedShopUiAssets {
        manifest_sha256: sha256_bytes(&manifest_bytes),
        units,
    })
}

fn validate_unit(
    unit: &ShopUiUnit,
    expected: (&str, &str, ShopUiFontRole, Cell),
    source: &BonusShopSource,
) -> Result<()> {
    let (expected_id, expected_text, expected_role, expected_cell) = expected;
    ensure!(
        unit.id == expected_id
            && unit.source_text == expected_text
            && unit.font_role == expected_role
            && parse_hex_offset(&unit.tim_offset)? == UI_TIM_OFFSET
            && unit.cell == expected_cell,
        "shop UI source consumer binding changed for {}",
        unit.id
    );
    validate_translation_state(unit)?;
    let pixels = read_indexed_cell_in_prefix(&source.shop_ui_decoded, UI_TIM_OFFSET, unit.cell)?;
    ensure!(
        sha256_bytes(&pixels) == unit.source_region_sha256,
        "shop UI source region {} changed",
        unit.id
    );
    Ok(())
}

pub(super) fn validate_translation_state(unit: &ShopUiUnit) -> Result<()> {
    ensure!(
        unit.development_status == DevelopmentStatus::Authored
            && !unit.source_text.trim().is_empty()
            && !unit.korean_text.trim().is_empty(),
        "authored shop UI unit {} lacks text",
        unit.id
    );
    ensure!(
        matches!(
            unit.release_status,
            ReleaseStatus::NeedsHumanReview | ReleaseStatus::Approved
        ),
        "shop UI unit {} has invalid release state",
        unit.id
    );
    Ok(())
}

pub(super) fn parse_hex_offset(value: &str) -> Result<usize> {
    usize::from_str_radix(
        value
            .strip_prefix("0x")
            .context("shop UI TIM offset is not hexadecimal")?,
        16,
    )
    .context("invalid shop UI TIM offset")
}

fn ensure_relative_path(path: &Path) -> Result<()> {
    ensure!(
        !path.as_os_str().is_empty()
            && path
                .components()
                .all(|component| { matches!(component, Component::Normal(_)) }),
        "shop UI unit path must stay inside its asset directory"
    );
    Ok(())
}
