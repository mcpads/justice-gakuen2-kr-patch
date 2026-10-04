use std::collections::BTreeMap;
use std::path::Path;

use super::asset_audit::summarize_assets;
use super::asset_sync_tests::untranslated_unit;
use super::audit_bonus_shop_text_assets;
use super::model::{
    BonusShopDevelopmentStatus, BonusShopFontRole, BonusShopReleaseStatus,
    BonusShopTextAssetAuditConfig, ShopTextRole,
};

#[test]
fn complete_untranslated_assets_allow_development_without_claiming_release() {
    let (expected, units) = three_role_assets();

    let report = summarize_assets(
        "source".to_string(),
        "codebook".to_string(),
        "manifest".to_string(),
        &expected,
        &units,
        &BTreeMap::new(),
    );

    assert_eq!(report.source_record_count, 3);
    assert_eq!(report.tracked_unit_count, 3);
    assert_eq!(report.untranslated_unit_count, 3);
    assert_eq!(report.authored_unit_count, 0);
    assert!(report.complete_source_population);
    assert!(report.development_can_continue);
    assert!(!report.development_translation_input_available);
    assert!(!report.release_candidate_input_eligible);
}

#[test]
fn authored_and_release_approved_counts_are_independent_gates() {
    let (expected, mut units) = three_role_assets();
    for unit in units.values_mut() {
        unit.korean_text = Some("가".to_string());
        unit.font_role = Some(BonusShopFontRole::for_source_role(unit.source.role));
        unit.development_status = BonusShopDevelopmentStatus::Authored;
        unit.release_status = BonusShopReleaseStatus::NeedsHumanReview;
    }

    let review_report = summarize_assets(
        "source".to_string(),
        "codebook".to_string(),
        "manifest".to_string(),
        &expected,
        &units,
        &BTreeMap::new(),
    );
    assert!(review_report.development_can_continue);
    assert!(review_report.development_translation_input_available);
    assert!(!review_report.release_candidate_input_eligible);

    for unit in units.values_mut() {
        unit.release_status = BonusShopReleaseStatus::Approved;
    }
    let approved_report = summarize_assets(
        "source".to_string(),
        "codebook".to_string(),
        "manifest".to_string(),
        &expected,
        &units,
        &BTreeMap::new(),
    );
    assert!(approved_report.release_candidate_input_eligible);
}

#[test]
#[ignore = "requires the user-supplied supported source disc and tracked shop assets"]
fn tracked_assets_cover_all_records_without_blocking_development() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let output = std::env::temp_dir().join(format!(
        "justice-bonus-shop-text-audit-{}.json",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&output);
    let report = audit_bonus_shop_text_assets(&BonusShopTextAssetAuditConfig {
        cue: root.join("roms/Shiritsu Justice Gakuen - Nekketsu Seishun Nikki 2 (Japan).cue"),
        dialogue_codebook: root.join("assets/dialogue/codebook.json"),
        assets: root.join("assets/menu/shop-text"),
        output: output.clone(),
    })
    .unwrap();

    assert_eq!(report.source_record_count, 186);
    assert_eq!(report.tracked_unit_count, 184);
    assert_eq!(report.delegated_record_count, 2);
    assert_eq!(report.untranslated_unit_count, 0);
    assert_eq!(report.authored_unit_count, report.tracked_unit_count);
    assert!(report.source_records_match);
    assert!(report.complete_source_population);
    assert!(report.development_can_continue);
    assert!(report.development_translation_input_available);
    assert!(!report.release_candidate_input_eligible);
    std::fs::remove_file(output).unwrap();
}

fn three_role_assets() -> (
    BTreeMap<String, super::model::BonusShopTextSourceUnit>,
    BTreeMap<String, super::model::BonusShopTranslationUnit>,
) {
    let mut expected = BTreeMap::new();
    let mut units = BTreeMap::new();
    for (index, role) in ShopTextRole::ALL.into_iter().enumerate() {
        let id = format!("unit-{index}");
        let mut unit = untranslated_unit();
        unit.id = id.clone();
        unit.source.unit_id = id.clone();
        unit.source.role = role;
        expected.insert(id.clone(), unit.source.clone());
        units.insert(id, unit);
    }
    (expected, units)
}
