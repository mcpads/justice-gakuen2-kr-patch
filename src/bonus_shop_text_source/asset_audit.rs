use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};

use super::asset_sync::load_existing_assets;
use super::collect_bonus_shop_text_source;
use super::model::{
    BonusShopDevelopmentStatus, BonusShopReleaseStatus, BonusShopTextAssetAuditConfig,
    BonusShopTextAssetAuditReport, BonusShopTextDelegatedRecord, BonusShopTextRoleAudit,
    BonusShopTextSourceUnit, BonusShopTranslationUnit, ShopTextRole,
};
use crate::bonus_shop_source::OVERLAY_SOURCE_SHA256;
use crate::pipeline::sha256_file;

pub fn audit_bonus_shop_text_assets(
    config: &BonusShopTextAssetAuditConfig,
) -> Result<BonusShopTextAssetAuditReport> {
    let collected = collect_bonus_shop_text_source(&config.cue, &config.dialogue_codebook)?;
    let expected_by_id = collected
        .tables
        .iter()
        .flat_map(|table| table.records.iter().map(|record| record.unit.clone()))
        .map(|unit| (unit.unit_id.clone(), unit))
        .collect::<BTreeMap<String, BonusShopTextSourceUnit>>();
    ensure!(
        expected_by_id.len() == 186,
        "KOUBAI source population changed while auditing translation assets"
    );
    let assets = load_existing_assets(&config.assets, &expected_by_id)?;
    let manifest_path = config.assets.join("manifest.json");
    ensure!(
        manifest_path.is_file(),
        "bonus-shop translation asset manifest is missing"
    );
    let report = summarize_assets(
        collected.source.source_bin_sha256,
        collected.dialogue_codebook_sha256,
        sha256_file(&manifest_path)?,
        &expected_by_id,
        &assets.units,
        &assets.delegated_records,
    );
    if let Some(parent) = config.output.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    std::fs::write(
        &config.output,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )
    .with_context(|| format!("failed to write {}", config.output.display()))?;
    Ok(report)
}

pub(super) fn summarize_assets(
    source_bin_sha256: String,
    dialogue_codebook_sha256: String,
    manifest_sha256: String,
    expected_by_id: &BTreeMap<String, BonusShopTextSourceUnit>,
    units: &BTreeMap<String, BonusShopTranslationUnit>,
    delegated_records: &BTreeMap<String, BonusShopTextDelegatedRecord>,
) -> BonusShopTextAssetAuditReport {
    let untranslated_unit_count = units
        .values()
        .filter(|unit| unit.development_status == BonusShopDevelopmentStatus::Untranslated)
        .count();
    let authored_unit_count = units
        .values()
        .filter(|unit| unit.development_status == BonusShopDevelopmentStatus::Authored)
        .count();
    let release_approved_unit_count = units
        .values()
        .filter(|unit| unit.release_status == BonusShopReleaseStatus::Approved)
        .count();
    let required_korean_characters = units
        .values()
        .filter_map(|unit| unit.korean_text.as_deref())
        .flat_map(str::chars)
        .filter(|character| !character.is_whitespace())
        .collect::<BTreeSet<_>>();
    let required_korean_character_count = required_korean_characters.len();
    let required_korean_characters = required_korean_characters.iter().collect::<String>();
    let role_counts = ShopTextRole::ALL
        .into_iter()
        .map(|role| {
            let source_record_count = expected_by_id
                .values()
                .filter(|unit| unit.role == role)
                .count();
            let role_units = units
                .values()
                .filter(|unit| unit.source.role == role)
                .collect::<Vec<_>>();
            let delegated_record_count = delegated_records
                .values()
                .filter(|record| record.source.role == role)
                .count();
            BonusShopTextRoleAudit {
                role,
                source_record_count,
                delegated_record_count,
                untranslated_unit_count: role_units
                    .iter()
                    .filter(|unit| {
                        unit.development_status == BonusShopDevelopmentStatus::Untranslated
                    })
                    .count(),
                authored_unit_count: role_units
                    .iter()
                    .filter(|unit| unit.development_status == BonusShopDevelopmentStatus::Authored)
                    .count(),
                release_approved_unit_count: role_units
                    .iter()
                    .filter(|unit| unit.release_status == BonusShopReleaseStatus::Approved)
                    .count(),
            }
        })
        .collect::<Vec<_>>();
    let complete_source_population = units.len() + delegated_records.len() == expected_by_id.len()
        && expected_by_id
            .keys()
            .all(|id| units.contains_key(id) || delegated_records.contains_key(id))
        && units.keys().all(|id| !delegated_records.contains_key(id));
    let development_can_continue = complete_source_population;
    let development_translation_input_available = authored_unit_count > 0;
    let release_candidate_input_eligible = complete_source_population
        && authored_unit_count == units.len()
        && release_approved_unit_count == units.len()
        && delegated_records.is_empty();
    BonusShopTextAssetAuditReport {
        kind: "Justice Gakuen 2 bonus-shop translation asset audit".to_string(),
        source_bin_sha256,
        source_overlay_sha256: OVERLAY_SOURCE_SHA256.to_string(),
        dialogue_codebook_sha256,
        manifest_sha256,
        source_record_count: expected_by_id.len(),
        tracked_unit_count: units.len(),
        delegated_record_count: delegated_records.len(),
        untranslated_unit_count,
        authored_unit_count,
        release_approved_unit_count,
        role_counts,
        source_records_match: true,
        complete_source_population,
        development_can_continue,
        development_translation_input_available,
        release_candidate_input_eligible,
        required_korean_characters,
        required_korean_character_count,
    }
}
