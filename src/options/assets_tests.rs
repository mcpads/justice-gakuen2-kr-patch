use super::assets::{validate_source_binding, validate_translation_state};
use super::model::{
    OptionsDevelopmentStatus, OptionsFontRole, OptionsReleaseStatus, OptionsTranslationUnit,
};

const RUNTIME_BASE: u32 = 0x800a_2000;

fn bound_unit() -> (Vec<u8>, OptionsTranslationUnit) {
    let source_offset = 0x20usize;
    let pointer_offset = 0x08usize;
    let source_codes = [0x0218u16, 0x0219, 0x0195];
    let mut overlay = vec![0u8; 0x40];
    overlay[pointer_offset..pointer_offset + 4]
        .copy_from_slice(&(RUNTIME_BASE + source_offset as u32).to_le_bytes());
    overlay[source_offset..source_offset + 2]
        .copy_from_slice(&(source_codes.len() as u16).to_le_bytes());
    for (index, code) in source_codes.into_iter().enumerate() {
        let offset = source_offset + 2 + index * 2;
        overlay[offset..offset + 2].copy_from_slice(&code.to_le_bytes());
    }
    let unit = OptionsTranslationUnit {
        kind: "Justice Gakuen 2 options-screen translation unit".to_string(),
        id: "attack_power".to_string(),
        source_offset: "0x0020".to_string(),
        pointer_offsets: vec!["0x0008".to_string()],
        source_codes: vec![
            "0x0218".to_string(),
            "0x0219".to_string(),
            "0x0195".to_string(),
        ],
        source_text: Some("攻撃力".to_string()),
        korean_text: Some("공격력".to_string()),
        font_role: Some(OptionsFontRole::Label),
        development_status: OptionsDevelopmentStatus::Authored,
        release_status: OptionsReleaseStatus::NeedsHumanReview,
    };
    (overlay, unit)
}

#[test]
fn accepts_codes_and_pointer_bound_to_the_same_source_string() {
    let (overlay, unit) = bound_unit();

    validate_source_binding(&overlay, 0x20, &unit).unwrap();
}

#[test]
fn rejects_changed_source_codes_before_building_from_translation_assets() {
    let (mut overlay, unit) = bound_unit();
    overlay[0x22..0x24].copy_from_slice(&0x0029u16.to_le_bytes());

    let error = validate_source_binding(&overlay, 0x20, &unit).unwrap_err();

    assert!(error.to_string().contains("source glyph codes changed"));
}

#[test]
fn rejects_a_pointer_that_no_longer_selects_the_bound_string() {
    let (mut overlay, unit) = bound_unit();
    overlay[0x08..0x0c].copy_from_slice(&(RUNTIME_BASE + 0x24).to_le_bytes());

    let error = validate_source_binding(&overlay, 0x20, &unit).unwrap_err();

    assert!(error.to_string().contains("source pointer changed"));
}

#[test]
fn untranslated_source_asset_preserves_codes_without_inventing_text_or_font_role() {
    let (_, mut unit) = bound_unit();
    unit.source_text = None;
    unit.korean_text = None;
    unit.font_role = None;
    unit.development_status = OptionsDevelopmentStatus::Untranslated;
    unit.release_status = OptionsReleaseStatus::Untranslated;

    validate_translation_state(&unit).unwrap();
}

#[test]
fn authored_asset_rejects_an_absent_korean_translation() {
    let (_, mut unit) = bound_unit();
    unit.korean_text = None;

    let error = validate_translation_state(&unit).unwrap_err();

    assert!(error.to_string().contains("lacks Korean text"));
}
