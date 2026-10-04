use std::path::Path;

use super::asset_sync::{validate_source_binding, validate_translation_decision};
use super::delegated_records::{
    DELEGATED_RECORD_SPECS, delegated_record, validate_delegated_record,
};
use super::model::{
    BonusShopDevelopmentStatus, BonusShopFontRole, BonusShopReleaseStatus,
    BonusShopTextAssetSyncConfig, BonusShopTextSourceToken, BonusShopTextSourceTokenKind,
    BonusShopTextSourceUnit, BonusShopTranslationUnit, ShopTextRole,
};
use super::sync_bonus_shop_text_assets;

#[test]
fn purchase_choices_stay_inside_native_cursor_windows() {
    use super::asset_sync::validate_purchase_choices;
    for prompt in ["이걸 사시겠어요?", "이걸 살 텐가?"] {
        validate_purchase_choices(&format!("{prompt}\n\n     예       아니요")).unwrap();
        assert!(validate_purchase_choices(&format!("{prompt}\n\n   예      아니요")).is_err());
        assert!(validate_purchase_choices(&format!("{prompt}\n     예       아니요")).is_err());
        assert!(validate_purchase_choices(&format!("{prompt}\n\n     예       아닙니다")).is_err());
    }
}

#[test]
fn untranslated_and_authored_states_keep_role_scoped_font_decisions_separate() {
    let mut unit = untranslated_unit();
    validate_translation_decision(&unit).unwrap();

    unit.development_status = BonusShopDevelopmentStatus::Authored;
    unit.release_status = BonusShopReleaseStatus::NeedsHumanReview;
    unit.korean_text = Some("열혈 카드".to_string());
    unit.font_role = Some(BonusShopFontRole::ProductLabel);
    validate_translation_decision(&unit).unwrap();

    unit.font_role = Some(BonusShopFontRole::ClerkDialogue);
    assert!(
        validate_translation_decision(&unit)
            .unwrap_err()
            .to_string()
            .contains("wrong role-scoped font")
    );
}

#[test]
fn codebook_text_can_refresh_but_raw_source_drift_is_rejected() {
    let expected = source_unit();
    let mut stale_codebook_projection = expected.clone();
    stale_codebook_projection.resolved_glyph_count = 0;
    stale_codebook_projection.unresolved_glyph_count = 1;
    stale_codebook_projection.exact_source_text = None;
    stale_codebook_projection.tokens[0].exact_source_text = None;
    stale_codebook_projection
        .source_pointer_interval_end_offset
        .clear();
    stale_codebook_projection.source_pointer_interval_byte_count = 0;
    stale_codebook_projection.trailing_interval_byte_count = 0;
    stale_codebook_projection.trailing_interval_sha256.clear();
    stale_codebook_projection.trailing_interval_all_zero = false;
    validate_source_binding(&stale_codebook_projection, &expected).unwrap();

    stale_codebook_projection.source_record_hex = "0081".to_string();
    assert!(
        validate_source_binding(&stale_codebook_projection, &expected)
            .unwrap_err()
            .to_string()
            .contains("source binding changed")
    );
}

#[test]
fn exit_confirmation_records_delegate_to_the_existing_six_semantic_units() {
    let expected = [
        (
            "clerk-dialogue-004",
            vec!["woman_prompt", "woman_yes", "woman_no"],
        ),
        (
            "clerk-dialogue-014",
            vec!["elder_prompt", "elder_yes", "elder_no"],
        ),
    ];

    for (spec, (id, unit_ids)) in DELEGATED_RECORD_SPECS.into_iter().zip(expected) {
        let mut source = source_unit();
        source.unit_id = id.to_string();
        source.role = ShopTextRole::ClerkDialogue;
        let record = delegated_record(source, spec);

        validate_delegated_record(&record).unwrap();
        assert_eq!(record.id, id);
        assert_eq!(record.writer.id, "bonus_shop_exit_confirmation");
        assert_eq!(record.writer.unit_ids, unit_ids);
    }
}

#[test]
#[ignore = "requires the user-supplied supported source disc and verified codebook"]
fn source_asset_sync_creates_all_units_once_and_is_idempotent() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let assets = std::env::temp_dir().join(format!(
        "justice-bonus-shop-text-assets-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&assets);
    let config = BonusShopTextAssetSyncConfig {
        cue: root.join("roms/Shiritsu Justice Gakuen - Nekketsu Seishun Nikki 2 (Japan).cue"),
        dialogue_codebook: root.join("assets/dialogue/codebook.json"),
        assets: assets.clone(),
    };

    let first = sync_bonus_shop_text_assets(&config).unwrap();
    assert_eq!(first.existing_unit_count, 0);
    assert_eq!(first.existing_delegated_record_count, 0);
    assert_eq!(first.created_untranslated_unit_count, 184);
    assert_eq!(first.created_delegated_record_count, 2);
    assert_eq!(first.final_unit_count, 184);
    assert_eq!(first.final_delegated_record_count, 2);
    assert!(first.complete_source_population);

    let second = sync_bonus_shop_text_assets(&config).unwrap();
    assert_eq!(second.existing_unit_count, 184);
    assert_eq!(second.existing_delegated_record_count, 2);
    assert_eq!(second.created_untranslated_unit_count, 0);
    assert_eq!(second.created_delegated_record_count, 0);
    assert_eq!(second.refreshed_source_unit_count, 0);
    assert_eq!(second.final_unit_count, 184);
    assert_eq!(second.final_delegated_record_count, 2);
    assert!(second.complete_source_population);
    std::fs::remove_dir_all(assets).unwrap();
}

pub(super) fn untranslated_unit() -> BonusShopTranslationUnit {
    BonusShopTranslationUnit {
        kind: "Justice Gakuen 2 bonus-shop translation unit".to_string(),
        id: "product-label-000".to_string(),
        source: source_unit(),
        korean_text: None,
        font_role: None,
        development_status: BonusShopDevelopmentStatus::Untranslated,
        release_status: BonusShopReleaseStatus::Untranslated,
    }
}

pub(super) fn source_unit() -> BonusShopTextSourceUnit {
    BonusShopTextSourceUnit {
        kind: "Justice Gakuen 2 source-bound bonus-shop text unit".to_string(),
        unit_id: "product-label-000".to_string(),
        role: ShopTextRole::ProductLabel,
        record_index: 0,
        pointer_storage_offset: "0x2cbc".to_string(),
        source_offset: "0x25a4".to_string(),
        source_runtime_address: "0x800a45a4".to_string(),
        source_record_end_offset: "0x25a8".to_string(),
        source_record_byte_count: 4,
        source_record_sha256: "ad9f1073e20a9f85b88ba8383e5b5135f7881e43ba7be9c274f0c7502cb52a0a"
            .to_string(),
        source_record_hex: "01000a81".to_string(),
        source_pointer_interval_end_offset: "0x25a9".to_string(),
        source_pointer_interval_byte_count: 5,
        trailing_interval_byte_count: 1,
        trailing_interval_sha256:
            "6e340b9cffb37a989ca544e6bb780a2c78901d3fb33738768511a30617afa01d".to_string(),
        trailing_interval_all_zero: true,
        line_count: 1,
        glyph_count: 1,
        resolved_glyph_count: 1,
        unresolved_glyph_count: 0,
        exact_source_text: Some("熱".to_string()),
        tokens: vec![
            BonusShopTextSourceToken {
                source_offset: "0x25a4".to_string(),
                raw_hex: "01000a".to_string(),
                kind: BonusShopTextSourceTokenKind::Glyph,
                code: Some("0x01a0".to_string()),
                page: Some(1),
                column: Some(0),
                row: Some(10),
                source_pixel_sha256: Some(
                    "c75feab499082c11e7db3b7f95b6630565bf002751837bc992b5786c599aa2ab".to_string(),
                ),
                exact_source_text: Some("熱".to_string()),
            },
            BonusShopTextSourceToken {
                source_offset: "0x25a7".to_string(),
                raw_hex: "81".to_string(),
                kind: BonusShopTextSourceTokenKind::Terminator,
                code: None,
                page: None,
                column: None,
                row: None,
                source_pixel_sha256: None,
                exact_source_text: None,
            },
        ],
    }
}
