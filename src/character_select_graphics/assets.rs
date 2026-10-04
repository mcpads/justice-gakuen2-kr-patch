use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path};

use crate::pipeline::sha256_bytes;
use anyhow::{Context, Result, ensure};

use super::model::{
    CharacterSelectFixedStripSet, CharacterSelectLocalizedSource,
    CharacterSelectSourceInventoryEntry, CharacterSelectSourceInventoryManifest,
    CharacterSelectSourceInventoryUnit, CharacterSelectSourceTreatment,
    CharacterSelectTranslationEntry, CharacterSelectTranslationManifest,
    CharacterSelectTranslationUnit,
};

const MANIFEST_KIND: &str = "justice_gakuen2_character_select_translation_manifest";
const UNIT_KIND: &str = "justice_gakuen2_character_select_translation_unit";
const SOURCE_INVENTORY_MANIFEST_KIND: &str = "justice_gakuen2_character_select_source_inventory";
const SOURCE_INVENTORY_UNIT_KIND: &str = "justice_gakuen2_character_select_source_inventory_unit";

#[derive(Debug)]
pub(super) struct CharacterSelectTranslationAssets {
    pub(super) assets_sha256: String,
    pub(super) unit_count: usize,
    pub(super) source_inventory_complete: bool,
    pub(super) source_inventory_unit_count: usize,
    pub(super) source_inventory_entries: Vec<CharacterSelectSourceInventoryEntry>,
    pub(super) required_fixed_strip_sets: BTreeSet<CharacterSelectFixedStripSet>,
    pub(super) entries: Vec<CharacterSelectTranslationEntry>,
    pub(super) localized_sources: Vec<CharacterSelectLocalizedSource>,
}

pub(super) struct CharacterSelectSourceTranslationCoverage {
    pub(super) translated_source_ui_count: usize,
    pub(super) untranslated_source_ui_ids: Vec<String>,
}

impl CharacterSelectTranslationAssets {
    pub(super) fn source_translation_coverage(&self) -> CharacterSelectSourceTranslationCoverage {
        let translation_ids = self
            .entries
            .iter()
            .map(|entry| entry.id.as_str())
            .collect::<BTreeSet<_>>();
        let untranslated_source_ui_ids = self
            .source_inventory_entries
            .iter()
            .filter(|entry| entry.treatment == CharacterSelectSourceTreatment::Translate)
            .filter(|entry| !translation_ids.contains(entry.translation_id()))
            .map(|entry| entry.id.clone())
            .collect::<Vec<_>>();
        let translated_source_ui_count = self
            .source_inventory_entries
            .iter()
            .filter(|entry| entry.treatment == CharacterSelectSourceTreatment::Translate)
            .count()
            - untranslated_source_ui_ids.len();
        CharacterSelectSourceTranslationCoverage {
            translated_source_ui_count,
            untranslated_source_ui_ids,
        }
    }
}

pub(super) fn load_character_select_translation_assets(
    directory: &Path,
) -> Result<CharacterSelectTranslationAssets> {
    let manifest_path = directory.join("manifest.json");
    let manifest_bytes = std::fs::read(&manifest_path)
        .with_context(|| format!("failed to read {}", manifest_path.display()))?;
    let manifest: CharacterSelectTranslationManifest = serde_json::from_slice(&manifest_bytes)
        .with_context(|| format!("failed to parse {}", manifest_path.display()))?;
    ensure!(
        manifest.kind == MANIFEST_KIND,
        "unsupported character-select manifest kind {:?}",
        manifest.kind
    );
    ensure!(
        !manifest.units.is_empty(),
        "character-select manifest has no units"
    );
    let required_fixed_strip_sets = manifest
        .required_fixed_strip_sets
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    ensure!(
        required_fixed_strip_sets.len() == manifest.required_fixed_strip_sets.len(),
        "character-select manifest repeats a required fixed-strip set"
    );

    let mut unit_paths = BTreeSet::new();
    let mut entries = Vec::new();
    let mut digest_input = Vec::new();
    append_digest_component(&mut digest_input, b"manifest.json", &manifest_bytes);
    for relative_path in &manifest.units {
        validate_relative_path(relative_path)?;
        ensure!(
            unit_paths.insert(relative_path.clone()),
            "duplicate character-select unit {}",
            relative_path.display()
        );
        let unit_path = directory.join(relative_path);
        let unit_bytes = std::fs::read(&unit_path)
            .with_context(|| format!("failed to read {}", unit_path.display()))?;
        let unit: CharacterSelectTranslationUnit = serde_json::from_slice(&unit_bytes)
            .with_context(|| format!("failed to parse {}", unit_path.display()))?;
        ensure!(
            unit.kind == UNIT_KIND,
            "unsupported character-select unit kind in {}",
            relative_path.display()
        );
        ensure!(
            !unit.entries.is_empty(),
            "character-select unit {} is empty",
            relative_path.display()
        );
        append_digest_component(
            &mut digest_input,
            relative_path
                .to_str()
                .context("character-select asset path is not valid UTF-8")?
                .as_bytes(),
            &unit_bytes,
        );
        entries.extend(unit.entries);
    }
    validate_entries(&entries)?;
    let (source_inventory_complete, source_inventory_unit_count, source_inventory_entries) =
        load_source_inventory(
            directory,
            &manifest.source_inventory,
            &entries,
            &mut digest_input,
        )?;
    let localized_sources = resolve_localized_sources(&source_inventory_entries, &entries);
    Ok(CharacterSelectTranslationAssets {
        assets_sha256: sha256_bytes(&digest_input),
        unit_count: unit_paths.len(),
        source_inventory_complete,
        source_inventory_unit_count,
        source_inventory_entries,
        required_fixed_strip_sets,
        entries,
        localized_sources,
    })
}

fn resolve_localized_sources(
    inventory: &[CharacterSelectSourceInventoryEntry],
    translations: &[CharacterSelectTranslationEntry],
) -> Vec<CharacterSelectLocalizedSource> {
    let translations_by_id = translations
        .iter()
        .map(|entry| (entry.id.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    inventory
        .iter()
        .filter(|source| source.treatment == CharacterSelectSourceTreatment::Translate)
        .filter_map(|source| {
            translations_by_id
                .get(source.translation_id())
                .map(|translation| CharacterSelectLocalizedSource {
                    source_ui_id: source.id.clone(),
                    translation_id: translation.id.clone(),
                    source_text: source.source_text.clone(),
                    korean_text: translation.korean_text.clone(),
                })
        })
        .collect()
}

fn load_source_inventory(
    assets_directory: &Path,
    manifest_relative_path: &Path,
    translations: &[CharacterSelectTranslationEntry],
    digest_input: &mut Vec<u8>,
) -> Result<(bool, usize, Vec<CharacterSelectSourceInventoryEntry>)> {
    validate_relative_path(manifest_relative_path)?;
    let manifest_path = assets_directory.join(manifest_relative_path);
    let manifest_bytes = std::fs::read(&manifest_path)
        .with_context(|| format!("failed to read {}", manifest_path.display()))?;
    let manifest: CharacterSelectSourceInventoryManifest = serde_json::from_slice(&manifest_bytes)
        .with_context(|| format!("failed to parse {}", manifest_path.display()))?;
    ensure!(
        manifest.kind == SOURCE_INVENTORY_MANIFEST_KIND,
        "unsupported character-select source-inventory manifest kind {:?}",
        manifest.kind
    );
    ensure!(
        !manifest.units.is_empty(),
        "character-select source inventory has no units"
    );
    append_digest_component(
        digest_input,
        manifest_relative_path
            .to_str()
            .context("character-select source-inventory path is not valid UTF-8")?
            .as_bytes(),
        &manifest_bytes,
    );

    let inventory_directory = manifest_path
        .parent()
        .context("character-select source-inventory manifest has no parent")?;
    let inventory_manifest_directory = manifest_relative_path
        .parent()
        .unwrap_or_else(|| Path::new(""));
    let mut unit_paths = BTreeSet::new();
    let mut entries = Vec::new();
    for relative_path in &manifest.units {
        validate_relative_path(relative_path)?;
        ensure!(
            unit_paths.insert(relative_path.clone()),
            "duplicate character-select source-inventory unit {}",
            relative_path.display()
        );
        let unit_path = inventory_directory.join(relative_path);
        let unit_bytes = std::fs::read(&unit_path)
            .with_context(|| format!("failed to read {}", unit_path.display()))?;
        let unit: CharacterSelectSourceInventoryUnit = serde_json::from_slice(&unit_bytes)
            .with_context(|| format!("failed to parse {}", unit_path.display()))?;
        ensure!(
            unit.kind == SOURCE_INVENTORY_UNIT_KIND,
            "unsupported character-select source-inventory unit kind in {}",
            relative_path.display()
        );
        ensure!(
            !unit.entries.is_empty(),
            "character-select source-inventory unit {} is empty",
            relative_path.display()
        );
        let digest_path = inventory_manifest_directory.join(relative_path);
        append_digest_component(
            digest_input,
            digest_path
                .to_str()
                .context("character-select source-inventory unit path is not valid UTF-8")?
                .as_bytes(),
            &unit_bytes,
        );
        entries.extend(unit.entries);
    }
    validate_source_inventory(&entries, translations)?;
    Ok((manifest.complete, unit_paths.len(), entries))
}

fn append_digest_component(output: &mut Vec<u8>, path: &[u8], contents: &[u8]) {
    output.extend_from_slice(&(path.len() as u64).to_le_bytes());
    output.extend_from_slice(path);
    output.extend_from_slice(&(contents.len() as u64).to_le_bytes());
    output.extend_from_slice(contents);
}

fn validate_entries(entries: &[CharacterSelectTranslationEntry]) -> Result<()> {
    ensure!(
        !entries.is_empty(),
        "character-select translation assets have no entries"
    );
    let mut ids = BTreeSet::new();
    for entry in entries {
        ensure!(
            ids.insert(entry.id.as_str()),
            "duplicate character-select translation id {}",
            entry.id
        );
        ensure!(
            !entry.korean_text.trim().is_empty(),
            "character-select entry {} has no Korean text",
            entry.id
        );
        ensure!(
            entry
                .korean_text
                .chars()
                .all(|character| !character.is_control() || character == '\n'),
            "character-select entry {} contains a control character",
            entry.id
        );
        ensure!(
            entry
                .korean_text
                .lines()
                .all(|line| !line.trim().is_empty()),
            "character-select entry {} contains an empty semantic line",
            entry.id
        );
    }
    Ok(())
}

fn validate_source_inventory(
    inventory: &[CharacterSelectSourceInventoryEntry],
    translations: &[CharacterSelectTranslationEntry],
) -> Result<()> {
    ensure!(
        !inventory.is_empty(),
        "character-select source inventory has no entries"
    );
    let mut inventory_by_id = BTreeMap::new();
    for entry in inventory {
        ensure!(
            inventory_by_id.insert(entry.id.as_str(), entry).is_none(),
            "duplicate character-select source-inventory id {}",
            entry.id
        );
        ensure!(
            !entry.source_text.trim().is_empty(),
            "character-select source-inventory entry {} has no source text",
            entry.id
        );
        if entry.treatment == CharacterSelectSourceTreatment::Exclude {
            ensure!(
                entry
                    .reason
                    .as_deref()
                    .is_some_and(|reason| !reason.trim().is_empty()),
                "excluded character-select source-inventory entry {} has no reason",
                entry.id
            );
        }
        ensure!(
            entry.treatment == CharacterSelectSourceTreatment::Translate
                || entry.translation_id.is_none(),
            "non-translated character-select source-inventory entry {} names a translation",
            entry.id
        );
        ensure!(
            entry
                .translation_id
                .as_deref()
                .is_none_or(|id| !id.trim().is_empty()),
            "character-select source-inventory entry {} has an empty translation id",
            entry.id
        );
    }

    let inventory_translation_ids = inventory
        .iter()
        .filter(|entry| entry.treatment == CharacterSelectSourceTreatment::Translate)
        .map(CharacterSelectSourceInventoryEntry::translation_id)
        .collect::<BTreeSet<_>>();
    for translation in translations {
        ensure!(
            inventory_translation_ids.contains(translation.id.as_str()),
            "character-select translation {} is absent from the source inventory",
            translation.id
        );
        let matching_sources = inventory.iter().filter(|entry| {
            entry.treatment == CharacterSelectSourceTreatment::Translate
                && entry.translation_id() == translation.id
        });
        let source_texts = matching_sources
            .map(|source| source.source_text.as_str())
            .collect::<BTreeSet<_>>();
        ensure!(
            source_texts.len() == 1,
            "character-select translation {} is shared by different source texts: {source_texts:?}",
            translation.id
        );
    }
    Ok(())
}

fn validate_relative_path(path: &Path) -> Result<()> {
    ensure!(
        !path.as_os_str().is_empty()
            && !path.is_absolute()
            && path
                .components()
                .all(|component| matches!(component, Component::Normal(_))),
        "character-select asset path must be a normalized relative path: {}",
        path.display()
    );
    Ok(())
}
