use std::collections::BTreeSet;
use std::path::{Component, Path};

use anyhow::{Context, Result, ensure};

use crate::pipeline::sha256_bytes;
use crate::tim::{Cell, read_8bpp_indexed_cell_in_prefix, read_indexed_cell_in_prefix};

use super::model::{BonusMenuManifest, BonusMenuUnit, DevelopmentStatus, ReleaseStatus};
use super::source::{
    BONUS_MENU_PATH, BonusMenuSource, ENTRY_TIM_OFFSET, SOURCE_DECODED_SHA256, SOURCE_STORED_SHA256,
};

const MANIFEST_KIND: &str = "justice_gakuen2_bonus_main_manifest";
const UNIT_KIND: &str = "justice_gakuen2_bonus_main_unit";
const EXPECTED_UNITS: [(&str, &str, u8, usize, Cell); 5] = [
    (
        "heading",
        "おまけ",
        8,
        0,
        Cell {
            x: 160,
            y: 48,
            width: 192,
            height: 96,
        },
    ),
    (
        "go_to_shop",
        "購買部へ行く",
        4,
        ENTRY_TIM_OFFSET,
        Cell {
            x: 0,
            y: 0,
            width: 256,
            height: 32,
        },
    ),
    (
        "purchased_bonus_list",
        "購入したおまけの一覧",
        4,
        ENTRY_TIM_OFFSET,
        Cell {
            x: 0,
            y: 32,
            width: 256,
            height: 32,
        },
    ),
    (
        "return_to_mode_menu",
        "モードメニューに戻る",
        4,
        ENTRY_TIM_OFFSET,
        Cell {
            x: 0,
            y: 64,
            width: 256,
            height: 32,
        },
    ),
    (
        "return_to_mode_menu_compact",
        "モードメニューに戻る",
        4,
        ENTRY_TIM_OFFSET,
        Cell {
            x: 0,
            y: 96,
            width: 242,
            height: 18,
        },
    ),
];

pub(super) struct LoadedBonusMenuAssets {
    pub(super) manifest_sha256: String,
    pub(super) units: Vec<BonusMenuUnit>,
}

pub(super) fn load_assets(root: &Path, source: &BonusMenuSource) -> Result<LoadedBonusMenuAssets> {
    let manifest_path = root.join("manifest.json");
    let manifest_bytes = std::fs::read(&manifest_path)
        .with_context(|| format!("failed to read {}", manifest_path.display()))?;
    let manifest: BonusMenuManifest = serde_json::from_slice(&manifest_bytes)
        .with_context(|| format!("failed to parse {}", manifest_path.display()))?;
    ensure!(
        manifest.kind == MANIFEST_KIND,
        "unsupported bonus-main manifest kind"
    );
    ensure!(
        manifest.source.path == BONUS_MENU_PATH
            && manifest.source.stored_sha256 == SOURCE_STORED_SHA256
            && manifest.source.decoded_sha256 == SOURCE_DECODED_SHA256,
        "bonus-main manifest source binding changed"
    );
    ensure!(
        manifest.units.len() == EXPECTED_UNITS.len(),
        "bonus-main manifest unit count changed"
    );

    let mut paths = BTreeSet::new();
    let mut ids = BTreeSet::new();
    let mut units = Vec::with_capacity(EXPECTED_UNITS.len());
    for (index, relative) in manifest.units.iter().enumerate() {
        ensure_relative_path(relative)?;
        ensure!(
            paths.insert(relative.clone()),
            "duplicate bonus-main unit path {}",
            relative.display()
        );
        let path = root.join(relative);
        let bytes =
            std::fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
        let unit: BonusMenuUnit = serde_json::from_slice(&bytes)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        ensure!(
            unit.kind == UNIT_KIND && ids.insert(unit.id.clone()),
            "bonus-main unit identity changed"
        );
        validate_unit(&unit, EXPECTED_UNITS[index], source)?;
        units.push(unit);
    }
    Ok(LoadedBonusMenuAssets {
        manifest_sha256: sha256_bytes(&manifest_bytes),
        units,
    })
}

fn validate_unit(
    unit: &BonusMenuUnit,
    expected: (&str, &str, u8, usize, Cell),
    source: &BonusMenuSource,
) -> Result<()> {
    let (expected_id, expected_text, expected_bpp, expected_offset, expected_cell) = expected;
    ensure!(
        unit.id == expected_id
            && unit.source_text == expected_text
            && unit.bits_per_pixel == expected_bpp
            && parse_hex_offset(&unit.tim_offset)? == expected_offset
            && unit.cell == expected_cell,
        "bonus-main source consumer binding changed for {}",
        unit.id
    );
    validate_translation_state(unit)?;
    let pixels = match unit.bits_per_pixel {
        8 => read_8bpp_indexed_cell_in_prefix(&source.decoded, expected_offset, unit.cell)?,
        4 => read_indexed_cell_in_prefix(&source.decoded, expected_offset, unit.cell)?,
        bits => anyhow::bail!("unsupported bonus-main TIM depth {bits}"),
    };
    ensure!(
        sha256_bytes(&pixels) == unit.source_region_sha256,
        "bonus-main source region {} changed",
        unit.id
    );
    Ok(())
}

pub(super) fn validate_translation_state(unit: &BonusMenuUnit) -> Result<()> {
    ensure!(
        !unit.source_text.trim().is_empty(),
        "bonus-main source text is empty"
    );
    match unit.development_status {
        DevelopmentStatus::Untranslated => {
            ensure!(
                unit.korean_text.is_none() && unit.release_status == ReleaseStatus::Untranslated,
                "untranslated bonus-main unit {} contains translated state",
                unit.id
            );
        }
        DevelopmentStatus::Authored => {
            ensure!(
                unit.korean_text
                    .as_deref()
                    .is_some_and(|text| !text.trim().is_empty())
                    && unit.release_status != ReleaseStatus::Untranslated,
                "authored bonus-main unit {} lacks Korean text",
                unit.id
            );
        }
        DevelopmentStatus::SuppressedDuplicate => {
            ensure!(
                unit.korean_text.is_none() && unit.release_status == ReleaseStatus::NotApplicable,
                "suppressed bonus-main duplicate {} contains translated state",
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
            .context("bonus-main TIM offset is not hexadecimal")?,
        16,
    )
    .context("invalid bonus-main TIM offset")
}

fn ensure_relative_path(path: &Path) -> Result<()> {
    ensure!(
        !path.as_os_str().is_empty()
            && !path.is_absolute()
            && path
                .components()
                .all(|component| matches!(component, Component::Normal(_))),
        "bonus-main unit path must stay inside its asset directory"
    );
    Ok(())
}
