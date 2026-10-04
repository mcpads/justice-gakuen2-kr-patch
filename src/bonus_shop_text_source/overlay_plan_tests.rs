use std::collections::BTreeMap;

use super::asset_sync_tests::untranslated_unit;
use super::model::{
    BonusShopDevelopmentStatus, BonusShopFontRole, BonusShopReleaseStatus, BonusShopTranslationUnit,
};
use super::overlay_plan::{plan_bonus_shop_text_overlay, validate_message_width};
use crate::pipeline::sha256_bytes;

const INTERVAL_START: usize = 0x25a4;
const INTERVAL_END: usize = 0x25b4;

#[test]
fn message_width_counts_native_cells_including_spaces_and_resets_at_newlines() {
    for role in [
        BonusShopFontRole::ProductDescription,
        BonusShopFontRole::ClerkDialogue,
    ] {
        assert!(validate_message_width(&"가".repeat(20), role).is_ok());
        assert!(validate_message_width(&format!("{} ", "가".repeat(20)), role).is_err());
        assert!(
            validate_message_width(&format!("{}\n{}", "가".repeat(20), "나".repeat(20)), role)
                .is_ok()
        );
        assert!(validate_message_width("들어 있는 열혈 카드 10장 세트입니다", role).is_err());
    }
}

#[test]
fn untranslated_units_leave_the_overlay_byte_identical() {
    let unit = unit_with_measured_interval();
    let source = source_overlay(&unit);
    let units = BTreeMap::from([(unit.id.clone(), unit)]);

    let plan = plan_bonus_shop_text_overlay(&source, &units, &BTreeMap::new()).unwrap();

    assert_eq!(plan.bytes, source);
    assert_eq!(plan.authored_unit_count, 0);
    assert_eq!(plan.untranslated_unit_count, 1);
    assert!(plan.expected_write_ranges.is_empty());
    assert!(plan.changed_byte_ranges.is_empty());
    assert_eq!(plan.units[0].id, unit_id(&units));
    assert!(!plan.units[0].changed);
}

#[test]
fn authored_unit_changes_only_its_owned_pointer_interval() {
    let mut unit = unit_with_measured_interval();
    unit.korean_text = Some("가 나".to_string());
    unit.font_role = Some(BonusShopFontRole::ProductLabel);
    unit.development_status = BonusShopDevelopmentStatus::Authored;
    unit.release_status = BonusShopReleaseStatus::NeedsHumanReview;
    let mut source = source_overlay(&unit);
    source[INTERVAL_END] = 0x5a;
    let units = BTreeMap::from([(unit.id.clone(), unit)]);
    let glyph_codes = BTreeMap::from([(
        BonusShopFontRole::ProductLabel,
        BTreeMap::from([('가', 0x0300), ('나', 0x0301)]),
    )]);

    let plan = plan_bonus_shop_text_overlay(&source, &units, &glyph_codes).unwrap();

    assert_eq!(
        &plan.bytes[INTERVAL_START..INTERVAL_START + 10],
        &[0x03, 0x00, 0x00, 0x63, 0x63, 0x63, 0x03, 0x01, 0x00, 0x81]
    );
    assert!(
        plan.bytes[INTERVAL_START + 10..INTERVAL_END]
            .iter()
            .all(|byte| *byte == 0)
    );
    assert_eq!(plan.bytes[INTERVAL_END], 0x5a);
    assert_eq!(plan.authored_unit_count, 1);
    assert_eq!(plan.untranslated_unit_count, 0);
    assert_eq!(
        plan.expected_write_ranges,
        vec![[INTERVAL_START, INTERVAL_END]]
    );
    assert!(plan.units[0].changed);
    assert_eq!(plan.units[0].source_record_byte_count, 4);
    assert_eq!(plan.units[0].output_record_byte_count, 10);
}

#[test]
fn authored_text_must_fit_its_pointer_interval() {
    let mut unit = unit_with_measured_interval();
    unit.korean_text = Some("가가가가가가".to_string());
    unit.font_role = Some(BonusShopFontRole::ProductLabel);
    unit.development_status = BonusShopDevelopmentStatus::Authored;
    unit.release_status = BonusShopReleaseStatus::NeedsHumanReview;
    let source = source_overlay(&unit);
    let units = BTreeMap::from([(unit.id.clone(), unit)]);
    let glyph_codes = BTreeMap::from([(
        BonusShopFontRole::ProductLabel,
        BTreeMap::from([('가', 0x0300)]),
    )]);

    let error = plan_bonus_shop_text_overlay(&source, &units, &glyph_codes).unwrap_err();

    assert!(error.to_string().contains("needs 19 bytes"));
    assert!(error.to_string().contains("owns 16"));
}

#[test]
fn authored_text_rejects_an_unallocated_glyph() {
    let mut unit = unit_with_measured_interval();
    unit.korean_text = Some("가".to_string());
    unit.font_role = Some(BonusShopFontRole::ProductLabel);
    unit.development_status = BonusShopDevelopmentStatus::Authored;
    unit.release_status = BonusShopReleaseStatus::NeedsHumanReview;
    let source = source_overlay(&unit);
    let units = BTreeMap::from([(unit.id.clone(), unit)]);

    let error = plan_bonus_shop_text_overlay(&source, &units, &BTreeMap::new()).unwrap_err();

    assert!(error.to_string().contains("no allocated glyph"));
}

fn unit_with_measured_interval() -> BonusShopTranslationUnit {
    let mut unit = untranslated_unit();
    unit.source.source_record_sha256 = sha256_bytes(&[0x01, 0x00, 0x0a, 0x81]);
    unit.source.source_pointer_interval_end_offset = format!("0x{INTERVAL_END:04x}");
    unit.source.source_pointer_interval_byte_count = INTERVAL_END - INTERVAL_START;
    unit.source.trailing_interval_byte_count =
        unit.source.source_pointer_interval_byte_count - unit.source.source_record_byte_count;
    unit.source.trailing_interval_sha256 =
        sha256_bytes(&vec![0; unit.source.trailing_interval_byte_count]);
    unit.source.trailing_interval_all_zero = true;
    unit
}

fn source_overlay(unit: &BonusShopTranslationUnit) -> Vec<u8> {
    let mut source = vec![0u8; INTERVAL_END + 1];
    source[INTERVAL_START..INTERVAL_START + 4].copy_from_slice(&[0x01, 0x00, 0x0a, 0x81]);
    assert_eq!(
        sha256_bytes(&source[INTERVAL_START..INTERVAL_START + 4]),
        unit.source.source_record_sha256
    );
    source
}

fn unit_id(units: &BTreeMap<String, BonusShopTranslationUnit>) -> String {
    units.keys().next().unwrap().clone()
}
