use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, ensure};

use super::collect_bonus_shop_text_source;
use super::delegated_records::{
    delegated_record, delegated_record_spec, validate_delegated_record,
};
use super::model::{
    BonusShopDevelopmentStatus, BonusShopReleaseStatus, BonusShopTextAssetManifest,
    BonusShopTextAssetReference, BonusShopTextAssetSyncConfig, BonusShopTextAssetSyncReport,
    BonusShopTextDelegatedRecord, BonusShopTextSourceUnit, BonusShopTranslationUnit,
};
use crate::bonus_shop_source::{
    GLYPH_TIM_SHA256, OVERLAY_PATH, OVERLAY_SOURCE_SHA256, SHOP_UI_PATH,
    SHOP_UI_SOURCE_DECODED_SHA256,
};
use crate::pipeline::BASELINE_BIN_SHA256;

const MANIFEST_KIND: &str = "Justice Gakuen 2 bonus-shop translation asset manifest";
const UNIT_KIND: &str = "Justice Gakuen 2 bonus-shop translation unit";

pub fn sync_bonus_shop_text_assets(
    config: &BonusShopTextAssetSyncConfig,
) -> Result<BonusShopTextAssetSyncReport> {
    let collected = collect_bonus_shop_text_source(&config.cue, &config.dialogue_codebook)?;
    let expected_units = collected
        .tables
        .iter()
        .flat_map(|table| table.records.iter().map(|record| record.unit.clone()))
        .collect::<Vec<_>>();
    ensure!(
        expected_units.len() == 186,
        "KOUBAI translation asset denominator changed"
    );
    let expected_by_id = expected_units
        .iter()
        .cloned()
        .map(|unit| (unit.unit_id.clone(), unit))
        .collect::<BTreeMap<_, _>>();
    ensure!(
        expected_by_id.len() == expected_units.len(),
        "KOUBAI source acquisition produced duplicate unit ids"
    );

    let existing = load_existing_assets(&config.assets, &expected_by_id)?;
    let existing_unit_count = existing.units.len();
    let existing_delegated_record_count = existing.delegated_records.len();
    let mut existing_units = existing.units;
    let mut existing_delegated_records = existing.delegated_records;
    let mut existing_references = existing
        .references
        .into_iter()
        .map(|reference| (reference.id.clone(), reference))
        .collect::<BTreeMap<_, _>>();
    let mut existing_delegated_references = existing
        .delegated_references
        .into_iter()
        .map(|reference| (reference.id.clone(), reference))
        .collect::<BTreeMap<_, _>>();
    let mut references = Vec::with_capacity(expected_units.len());
    let mut delegated_references = Vec::new();
    let mut units_to_write = Vec::with_capacity(expected_units.len());
    let mut delegated_records_to_write = Vec::new();
    let mut retired_unit_paths = Vec::new();
    let mut created_untranslated_unit_count = 0usize;
    let mut created_delegated_record_count = 0usize;
    let mut migrated_delegated_record_count = 0usize;
    let mut refreshed_source_unit_count = 0usize;
    for expected_source in expected_units {
        let id = expected_source.unit_id.clone();
        if let Some(spec) = delegated_record_spec(&id) {
            let (reference, mut record) = match existing_delegated_records.remove(&id) {
                Some(record) => {
                    validate_delegated_record(&record)?;
                    validate_source_binding(&record.source, &expected_source)?;
                    let reference = existing_delegated_references
                        .remove(&id)
                        .context("delegated bonus-shop record lost its manifest reference")?;
                    (reference, record)
                }
                None => {
                    let record = if let Some(unit) = existing_units.remove(&id) {
                        validate_translation_decision(&unit)?;
                        ensure!(
                            unit.development_status == BonusShopDevelopmentStatus::Untranslated,
                            "cannot delegate authored bonus-shop unit {id} without an explicit translation merge"
                        );
                        let old_reference = existing_references
                            .remove(&id)
                            .context("migrated bonus-shop unit lost its manifest reference")?;
                        retired_unit_paths.push(asset_path(&config.assets, &old_reference.file)?);
                        migrated_delegated_record_count += 1;
                        delegated_record(unit.source, spec)
                    } else {
                        created_delegated_record_count += 1;
                        delegated_record(expected_source.clone(), spec)
                    };
                    (
                        BonusShopTextAssetReference {
                            id: id.clone(),
                            file: spec.file.to_string(),
                        },
                        record,
                    )
                }
            };
            if record.source != expected_source {
                refreshed_source_unit_count += 1;
                record.source = expected_source;
            }
            delegated_records_to_write.push((asset_path(&config.assets, &reference.file)?, record));
            delegated_references.push(reference);
            continue;
        }
        let (reference, unit) = match existing_units.remove(&id) {
            Some(mut unit) => {
                validate_translation_decision(&unit)?;
                validate_source_binding(&unit.source, &expected_source)?;
                if unit.source != expected_source {
                    refreshed_source_unit_count += 1;
                    unit.source = expected_source;
                }
                let reference = existing_references
                    .remove(&id)
                    .context("bonus-shop unit lost its manifest reference")?;
                (reference, unit)
            }
            None => {
                created_untranslated_unit_count += 1;
                let relative_file = format!(
                    "untranslated/{}/{}.json",
                    expected_source.role.slug(),
                    expected_source.unit_id
                );
                let reference = BonusShopTextAssetReference {
                    id: id.clone(),
                    file: relative_file,
                };
                let unit = BonusShopTranslationUnit {
                    kind: UNIT_KIND.to_string(),
                    id,
                    source: expected_source,
                    korean_text: None,
                    font_role: None,
                    development_status: BonusShopDevelopmentStatus::Untranslated,
                    release_status: BonusShopReleaseStatus::Untranslated,
                };
                (reference, unit)
            }
        };
        units_to_write.push((asset_path(&config.assets, &reference.file)?, unit));
        references.push(reference);
    }
    ensure!(
        existing_units.is_empty()
            && existing_references.is_empty()
            && existing_delegated_records.is_empty()
            && existing_delegated_references.is_empty(),
        "tracked bonus-shop assets contain units outside the source pointer population"
    );

    std::fs::create_dir_all(&config.assets)
        .with_context(|| format!("failed to create {}", config.assets.display()))?;
    for (path, unit) in &units_to_write {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        write_json(path, unit)?;
    }
    for (path, record) in &delegated_records_to_write {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        write_json(path, record)?;
    }
    let manifest = BonusShopTextAssetManifest {
        kind: MANIFEST_KIND.to_string(),
        source_bin_sha256: collected.source.source_bin_sha256.clone(),
        source_overlay_path: OVERLAY_PATH.to_string(),
        source_overlay_sha256: OVERLAY_SOURCE_SHA256.to_string(),
        source_shop_ui_path: SHOP_UI_PATH.to_string(),
        source_shop_ui_decoded_sha256: SHOP_UI_SOURCE_DECODED_SHA256.to_string(),
        source_glyph_tim_sha256: GLYPH_TIM_SHA256.to_string(),
        dialogue_codebook_sha256: collected.dialogue_codebook_sha256.clone(),
        source_record_count: references.len() + delegated_references.len(),
        units: references,
        delegated_records: delegated_references,
    };
    write_json(&config.assets.join("manifest.json"), &manifest)?;
    for path in retired_unit_paths {
        std::fs::remove_file(&path)
            .with_context(|| format!("failed to remove migrated asset {}", path.display()))?;
    }

    let final_assets = load_existing_assets(&config.assets, &expected_by_id)?;
    let complete_source_population =
        final_assets.units.len() + final_assets.delegated_records.len() == expected_by_id.len();
    ensure!(
        complete_source_population,
        "bonus-shop translation assets do not cover all source records after synchronization"
    );
    Ok(BonusShopTextAssetSyncReport {
        kind: "Justice Gakuen 2 complete bonus-shop source asset sync".to_string(),
        source_bin_sha256: collected.source.source_bin_sha256,
        source_overlay_sha256: OVERLAY_SOURCE_SHA256.to_string(),
        dialogue_codebook_sha256: collected.dialogue_codebook_sha256,
        source_record_count: expected_by_id.len(),
        existing_unit_count,
        existing_delegated_record_count,
        created_untranslated_unit_count,
        created_delegated_record_count,
        migrated_delegated_record_count,
        refreshed_source_unit_count,
        final_unit_count: final_assets.units.len(),
        final_delegated_record_count: final_assets.delegated_records.len(),
        complete_source_population,
    })
}

pub(super) struct LoadedAssets {
    pub(super) references: Vec<BonusShopTextAssetReference>,
    pub(super) units: BTreeMap<String, BonusShopTranslationUnit>,
    pub(super) delegated_references: Vec<BonusShopTextAssetReference>,
    pub(super) delegated_records: BTreeMap<String, BonusShopTextDelegatedRecord>,
}

pub(super) fn load_existing_assets(
    root: &Path,
    expected_by_id: &BTreeMap<String, BonusShopTextSourceUnit>,
) -> Result<LoadedAssets> {
    let manifest_path = root.join("manifest.json");
    if !manifest_path.exists() {
        if root.exists() {
            let mut entries = std::fs::read_dir(root)
                .with_context(|| format!("failed to read {}", root.display()))?;
            ensure!(
                entries.next().is_none(),
                "bonus-shop asset directory exists without a manifest: {}",
                root.display()
            );
        }
        return Ok(LoadedAssets {
            references: Vec::new(),
            units: BTreeMap::new(),
            delegated_references: Vec::new(),
            delegated_records: BTreeMap::new(),
        });
    }
    let manifest: BonusShopTextAssetManifest = read_json(&manifest_path)?;
    ensure!(
        manifest.kind == MANIFEST_KIND,
        "bonus-shop manifest kind changed"
    );
    ensure!(
        manifest.source_bin_sha256 == BASELINE_BIN_SHA256
            && manifest.source_overlay_path == OVERLAY_PATH
            && manifest.source_overlay_sha256 == OVERLAY_SOURCE_SHA256
            && manifest.source_shop_ui_path == SHOP_UI_PATH
            && manifest.source_shop_ui_decoded_sha256 == SHOP_UI_SOURCE_DECODED_SHA256
            && manifest.source_glyph_tim_sha256 == GLYPH_TIM_SHA256,
        "bonus-shop manifest source identity changed"
    );
    ensure!(
        manifest.source_record_count == manifest.units.len() + manifest.delegated_records.len(),
        "bonus-shop manifest record count does not match its source-record references"
    );
    let mut ids = BTreeSet::new();
    let mut files = BTreeSet::new();
    let mut units = BTreeMap::new();
    for reference in &manifest.units {
        ensure!(
            ids.insert(reference.id.clone()) && files.insert(reference.file.clone()),
            "bonus-shop manifest repeats a unit id or file"
        );
        let path = asset_path(root, &reference.file)?;
        let unit: BonusShopTranslationUnit = read_json(&path)?;
        ensure!(
            unit.kind == UNIT_KIND && unit.id == reference.id && unit.source.unit_id == unit.id,
            "bonus-shop asset identity does not match its manifest reference"
        );
        let expected = expected_by_id
            .get(unit.id.as_str())
            .with_context(|| format!("bonus-shop asset {} is outside source scope", unit.id))?;
        validate_source_binding(&unit.source, expected)?;
        validate_translation_decision(&unit)?;
        ensure!(
            units.insert(unit.id.clone(), unit).is_none(),
            "bonus-shop assets repeat a unit id"
        );
    }
    let mut delegated_records = BTreeMap::new();
    for reference in &manifest.delegated_records {
        ensure!(
            ids.insert(reference.id.clone()) && files.insert(reference.file.clone()),
            "bonus-shop manifest repeats a unit id or file"
        );
        let path = asset_path(root, &reference.file)?;
        let record: BonusShopTextDelegatedRecord = read_json(&path)?;
        ensure!(
            record.id == reference.id && record.source.unit_id == record.id,
            "delegated bonus-shop record identity does not match its manifest reference"
        );
        let expected = expected_by_id.get(record.id.as_str()).with_context(|| {
            format!(
                "delegated bonus-shop record {} is outside source scope",
                record.id
            )
        })?;
        validate_source_binding(&record.source, expected)?;
        validate_delegated_record(&record)?;
        ensure!(
            delegated_records
                .insert(record.id.clone(), record)
                .is_none(),
            "bonus-shop assets repeat a delegated record id"
        );
    }
    Ok(LoadedAssets {
        references: manifest.units,
        units,
        delegated_references: manifest.delegated_records,
        delegated_records,
    })
}

pub(super) fn validate_source_binding(
    actual: &BonusShopTextSourceUnit,
    expected: &BonusShopTextSourceUnit,
) -> Result<()> {
    let mut normalized = actual.clone();
    normalized.resolved_glyph_count = expected.resolved_glyph_count;
    normalized.unresolved_glyph_count = expected.unresolved_glyph_count;
    normalized.exact_source_text = expected.exact_source_text.clone();
    normalized.source_pointer_interval_end_offset =
        expected.source_pointer_interval_end_offset.clone();
    normalized.source_pointer_interval_byte_count = expected.source_pointer_interval_byte_count;
    normalized.trailing_interval_byte_count = expected.trailing_interval_byte_count;
    normalized.trailing_interval_sha256 = expected.trailing_interval_sha256.clone();
    normalized.trailing_interval_all_zero = expected.trailing_interval_all_zero;
    ensure!(
        normalized.tokens.len() == expected.tokens.len(),
        "bonus-shop source token count changed for {}",
        actual.unit_id
    );
    for (token, expected_token) in normalized.tokens.iter_mut().zip(&expected.tokens) {
        token.exact_source_text = expected_token.exact_source_text.clone();
    }
    ensure!(
        normalized == *expected,
        "bonus-shop source binding changed for {}",
        actual.unit_id
    );
    Ok(())
}

pub(super) fn validate_translation_decision(unit: &BonusShopTranslationUnit) -> Result<()> {
    match unit.development_status {
        BonusShopDevelopmentStatus::Untranslated => ensure!(
            unit.korean_text.is_none()
                && unit.font_role.is_none()
                && unit.release_status == BonusShopReleaseStatus::Untranslated,
            "untranslated bonus-shop unit {} contains authored fields",
            unit.id
        ),
        BonusShopDevelopmentStatus::Authored => {
            ensure!(
                unit.korean_text
                    .as_deref()
                    .is_some_and(|text| !text.is_empty()),
                "authored bonus-shop unit {} has no Korean text",
                unit.id
            );
            ensure!(
                unit.font_role
                    == Some(super::model::BonusShopFontRole::for_source_role(
                        unit.source.role
                    )),
                "authored bonus-shop unit {} uses the wrong role-scoped font",
                unit.id
            );
            ensure!(
                unit.release_status != BonusShopReleaseStatus::Untranslated,
                "authored bonus-shop unit {} has an untranslated release status",
                unit.id
            );
            if unit.source.role == super::model::ShopTextRole::ClerkDialogue
                && matches!(unit.source.record_index, 1 | 11)
            {
                validate_purchase_choices(unit.korean_text.as_deref().unwrap())?;
            }
        }
    }
    Ok(())
}

pub(super) fn validate_purchase_choices(text: &str) -> Result<()> {
    let lines: Vec<_> = text.lines().collect();
    ensure!(
        lines.len() == 3,
        "shop purchase choices must stay on the native third row"
    );
    let mut runs = Vec::new();
    let mut start = None;
    for (column, character) in lines[2].chars().chain(std::iter::once(' ')).enumerate() {
        if character == ' ' {
            if let Some(begin) = start.take() {
                runs.push((begin, column - begin));
            }
        } else {
            start.get_or_insert(column);
        }
    }
    // Native command x=80, 20px per glyph/blank: YES x=180, NO x=340.
    // The cursor windows reserve two and three glyph cells respectively.
    ensure!(
        matches!(runs.as_slice(), [(5, 1..=2), (13, 1..=3)]),
        "shop purchase labels leave their native YES/NO cursor windows"
    );
    Ok(())
}

fn asset_path(root: &Path, relative: &str) -> Result<PathBuf> {
    let relative = Path::new(relative);
    ensure!(
        relative.is_relative()
            && relative
                .components()
                .all(|component| matches!(component, Component::Normal(_))),
        "bonus-shop asset path must stay inside its root"
    );
    Ok(root.join(relative))
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    serde_json::from_slice(
        &std::fs::read(path).with_context(|| format!("failed to read {}", path.display()))?,
    )
    .with_context(|| format!("failed to parse {}", path.display()))
}

fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> Result<()> {
    std::fs::write(path, format!("{}\n", serde_json::to_string_pretty(value)?))
        .with_context(|| format!("failed to write {}", path.display()))
}
