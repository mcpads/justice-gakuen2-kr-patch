use std::collections::BTreeMap;

use super::asset_sync_tests::untranslated_unit;
use super::glyph_allocation::select_bonus_shop_glyph_codes;
use super::model::{BonusShopDevelopmentStatus, BonusShopFontRole, BonusShopReleaseStatus};
use crate::bonus_shop_source::{DIRECT_SELECTOR_REGION, sprite_selector};

#[test]
fn role_scoped_characters_receive_distinct_consumer_safe_cells() {
    let mut label = untranslated_unit();
    label.id = "label".to_string();
    label.source.unit_id = label.id.clone();
    label.korean_text = Some("가".to_string());
    label.font_role = Some(BonusShopFontRole::ProductLabel);
    label.development_status = BonusShopDevelopmentStatus::Authored;
    label.release_status = BonusShopReleaseStatus::NeedsHumanReview;

    let mut description = label.clone();
    description.id = "description".to_string();
    description.source.unit_id = description.id.clone();
    description.source.role = super::model::ShopTextRole::ProductDescription;
    description.font_role = Some(BonusShopFontRole::ProductDescription);
    let units = BTreeMap::from([
        (label.id.clone(), label),
        (description.id.clone(), description),
    ]);
    let overlay = vec![0; DIRECT_SELECTOR_REGION[1]];

    let selection =
        select_bonus_shop_glyph_codes(&overlay, &units, &BTreeMap::new(), [], []).unwrap();

    assert_eq!(selection.assignments.len(), 2);
    assert_ne!(selection.assignments[0].1, selection.assignments[1].1);
}

#[test]
fn source_glyphs_reserved_writers_and_direct_selectors_are_not_reallocated() {
    let mut authored = untranslated_unit();
    authored.korean_text = Some("가".to_string());
    authored.font_role = Some(BonusShopFontRole::ProductLabel);
    authored.development_status = BonusShopDevelopmentStatus::Authored;
    authored.release_status = BonusShopReleaseStatus::NeedsHumanReview;
    let mut untranslated = untranslated_unit();
    untranslated.id = "untranslated".to_string();
    untranslated.source.unit_id = untranslated.id.clone();
    untranslated.source.tokens[0].code = Some("0x0002".to_string());
    let units = BTreeMap::from([
        (authored.id.clone(), authored),
        (untranslated.id.clone(), untranslated),
    ]);
    let mut overlay = vec![0; DIRECT_SELECTOR_REGION[1]];
    overlay[DIRECT_SELECTOR_REGION[0]..DIRECT_SELECTOR_REGION[0] + 3]
        .copy_from_slice(&sprite_selector(0x0001));

    let selection =
        select_bonus_shop_glyph_codes(&overlay, &units, &BTreeMap::new(), [0x0000], []).unwrap();

    assert_eq!(selection.assignments[0].1, 0x0003);
    assert_eq!(selection.direct_selector_code_count, 1);
}

#[test]
fn runtime_numeric_cells_are_not_reallocated() {
    let mut authored = untranslated_unit();
    authored.korean_text = Some("가".to_string());
    authored.font_role = Some(BonusShopFontRole::ProductLabel);
    authored.development_status = BonusShopDevelopmentStatus::Authored;
    authored.release_status = BonusShopReleaseStatus::NeedsHumanReview;
    let units = BTreeMap::from([(authored.id.clone(), authored)]);
    let overlay = vec![0; DIRECT_SELECTOR_REGION[1]];
    let runtime_glyph_codes = [
        0x0000, 0x0001, 0x0002, 0x0003, 0x0004, 0x0005, 0x0006, 0x0007, 0x0008, 0x0009, 0x0059,
    ];

    let selection =
        select_bonus_shop_glyph_codes(&overlay, &units, &BTreeMap::new(), runtime_glyph_codes, [])
            .unwrap();

    assert_eq!(selection.assignments[0].1, 0x000a);
}
