use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};

use crate::pipeline::sha256_bytes;

use super::assets::{load_assets, validate_complete_source_population};
use super::model::{
    OptionsAssetReference, OptionsDevelopmentStatus, OptionsManifest, OptionsReleaseStatus,
    OptionsSourceAssetSyncConfig, OptionsSourceAssetSyncReport, OptionsTranslationUnit,
};
use super::source::{OVERLAY_PATH, load_options_source};
use super::source_catalog::catalog_newopt_pointer_records;

const TRANSLATION_KIND: &str = "Justice Gakuen 2 options-screen translation unit";

pub fn sync_options_source_assets(
    config: &OptionsSourceAssetSyncConfig,
) -> Result<OptionsSourceAssetSyncReport> {
    let source = load_options_source(&config.cue)?;
    let loaded = load_assets(
        &config.assets,
        &source.source_bin_sha256,
        &source.overlay,
        &source.menu_stored,
        &source.menu_decoded,
    )?;
    let catalog = catalog_newopt_pointer_records(&source.overlay)?;
    ensure!(
        catalog.pointer_slot_count == 148 && catalog.records.len() == 135,
        "NEWOPT pointer-backed source population changed"
    );

    let mut tracked_offsets = loaded
        .units
        .iter()
        .map(|unit| super::assets::parse_hex_usize(&unit.source_offset, "source offset"))
        .collect::<Result<BTreeSet<_>>>()?;
    ensure!(
        tracked_offsets.iter().all(|offset| catalog
            .records
            .iter()
            .any(|record| record.source_offset == *offset)),
        "existing options asset is outside the NEWOPT source population"
    );

    let mut references = loaded.references;
    let mut additions = Vec::new();
    for record in &catalog.records {
        if !tracked_offsets.insert(record.source_offset) {
            continue;
        }
        let id = format!("newopt_{:04x}", record.source_offset);
        let relative_file = format!("text/untranslated/newopt-{:04x}.json", record.source_offset);
        let path = config.assets.join(&relative_file);
        ensure!(
            !path.exists(),
            "untracked options asset path already exists: {}",
            path.display()
        );
        additions.push((
            path,
            OptionsAssetReference {
                id: id.clone(),
                file: relative_file,
            },
            OptionsTranslationUnit {
                kind: TRANSLATION_KIND.to_string(),
                id,
                source_offset: format!("0x{:04x}", record.source_offset),
                pointer_offsets: record
                    .pointer_offsets
                    .iter()
                    .map(|offset| format!("0x{offset:04x}"))
                    .collect(),
                source_codes: record
                    .source_codes
                    .iter()
                    .map(|code| format!("0x{code:04x}"))
                    .collect(),
                source_text: None,
                korean_text: None,
                font_role: None,
                development_status: OptionsDevelopmentStatus::Untranslated,
                release_status: OptionsReleaseStatus::Untranslated,
            },
        ));
    }

    let untranslated_dir = config.assets.join("text/untranslated");
    std::fs::create_dir_all(&untranslated_dir)
        .with_context(|| format!("failed to create {}", untranslated_dir.display()))?;
    for (path, reference, unit) in &additions {
        std::fs::write(path, format!("{}\n", serde_json::to_string_pretty(unit)?))
            .with_context(|| format!("failed to write {}", path.display()))?;
        references.push(reference.clone());
    }
    let manifest = OptionsManifest {
        kind: "Justice Gakuen 2 options-screen translation manifest".to_string(),
        source_bin_sha256: source.source_bin_sha256.clone(),
        overlay_path: OVERLAY_PATH.to_string(),
        overlay_sha256: sha256_bytes(&source.overlay),
        menu_path: "DAT2/MENU.BIZ".to_string(),
        menu_stored_sha256: sha256_bytes(&source.menu_stored),
        menu_decoded_sha256: sha256_bytes(&source.menu_decoded),
        units: references,
    };
    let manifest_path = config.assets.join("manifest.json");
    std::fs::write(
        &manifest_path,
        format!("{}\n", serde_json::to_string_pretty(&manifest)?),
    )
    .with_context(|| format!("failed to write {}", manifest_path.display()))?;

    let final_assets = load_assets(
        &config.assets,
        &source.source_bin_sha256,
        &source.overlay,
        &source.menu_stored,
        &source.menu_decoded,
    )?;
    validate_complete_source_population(&final_assets.units, &catalog)?;
    Ok(OptionsSourceAssetSyncReport {
        kind: "Justice Gakuen 2 NEWOPT unique source asset sync".to_string(),
        source_bin_sha256: source.source_bin_sha256,
        overlay_path: OVERLAY_PATH.to_string(),
        overlay_sha256: sha256_bytes(&source.overlay),
        pointer_slot_count: catalog.pointer_slot_count,
        unique_source_record_count: catalog.records.len(),
        existing_unit_count: loaded.units.len(),
        created_untranslated_unit_count: additions.len(),
        final_unit_count: final_assets.units.len(),
    })
}
