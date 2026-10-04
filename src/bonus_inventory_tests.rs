use super::assets::validate_translation_state;
use super::model::{
    BonusInventoryFontRole, BonusInventoryOccurrence, BonusInventoryUnit, DevelopmentStatus,
    ReleaseStatus,
};
use super::raster::overlay_text_preserving_background;
use crate::tim::Cell;

fn unit(
    korean_text: Option<&str>,
    font_role: Option<BonusInventoryFontRole>,
    development_status: DevelopmentStatus,
    release_status: ReleaseStatus,
) -> BonusInventoryUnit {
    BonusInventoryUnit {
        kind: "justice_gakuen2_bonus_inventory_unit".to_string(),
        id: "title".to_string(),
        source_text: "おまけを見る".to_string(),
        korean_text: korean_text.map(str::to_string),
        occurrences: vec![BonusInventoryOccurrence {
            id: "main".to_string(),
            font_role,
            cell: Cell {
                x: 0,
                y: 0,
                width: 256,
                height: 48,
            },
            source_indexed_pixel_sha256: "hash".to_string(),
        }],
        development_status,
        release_status,
    }
}

#[test]
fn authored_fixed_ui_requires_text_and_font_roles() {
    let error = validate_translation_state(&unit(
        None,
        Some(BonusInventoryFontRole::Title),
        DevelopmentStatus::Authored,
        ReleaseStatus::NeedsHumanReview,
    ))
    .unwrap_err();
    assert!(error.to_string().contains("lacks build input"));
}

#[test]
fn untranslated_background_bound_text_cannot_select_a_font() {
    let error = validate_translation_state(&unit(
        None,
        Some(BonusInventoryFontRole::Title),
        DevelopmentStatus::Untranslated,
        ReleaseStatus::Untranslated,
    ))
    .unwrap_err();
    assert!(error.to_string().contains("contains translated state"));
}

#[test]
fn authored_fixed_ui_does_not_require_release_approval() {
    validate_translation_state(&unit(
        Some("보너스 보기"),
        Some(BonusInventoryFontRole::Title),
        DevelopmentStatus::Authored,
        ReleaseStatus::NeedsHumanReview,
    ))
    .unwrap();
}

#[test]
#[ignore = "requires assets/"]
fn runtime_bound_help_fragments_are_independently_authored() {
    for document in [
        crate::test_input::read_str("assets/menu/bonus-inventory/fixed/select-help.json"),
        crate::test_input::read_str("assets/menu/bonus-inventory/fixed/decide-help.json"),
        crate::test_input::read_str("assets/menu/bonus-inventory/fixed/back-help.json"),
        crate::test_input::read_str("assets/menu/bonus-inventory/fixed/viewer-help.json"),
        crate::test_input::read_str("assets/menu/bonus-inventory/fixed/viewer-exit.json"),
    ] {
        let unit: BonusInventoryUnit = serde_json::from_str(document).unwrap();

        validate_translation_state(&unit).unwrap();
        assert_eq!(unit.development_status, DevelopmentStatus::Authored);
        assert!(unit.korean_text.is_some());
        assert!(
            unit.occurrences
                .iter()
                .all(|occurrence| occurrence.font_role == Some(BonusInventoryFontRole::Help))
        );
    }
}

#[test]
#[ignore = "requires assets/"]
fn playback_help_keeps_the_cancel_icon_outside_its_write_cell() {
    let unit: BonusInventoryUnit = serde_json::from_str(crate::test_input::read_str(
        "assets/menu/bonus-inventory/fixed/playback-stop-help.json",
    ))
    .unwrap();
    validate_translation_state(&unit).unwrap();
    let cell = unit.occurrences[0].cell;
    assert_eq!(
        (cell.x, cell.y, cell.width, cell.height),
        (15, 240, 241, 16)
    );
    assert_eq!(
        unit.occurrences[0].font_role,
        Some(BonusInventoryFontRole::Help)
    );
}

#[test]
#[ignore = "requires assets/"]
fn next_label_clears_the_source_n_stem_without_touching_the_arrow() {
    let unit: BonusInventoryUnit = serde_json::from_str(crate::test_input::read_str(
        "assets/menu/bonus-inventory/fixed/viewer-next.json",
    ))
    .unwrap();
    let cells = [
        unit.occurrences[0].cell,
        super::catalog::expected_occurrences("viewer_next")[0].cell,
    ];
    // Retail atlas: NEXT ink spans x=717..764, y=202..213. The adjacent
    // right arrow reaches x=712. Starting at 720 leaves a twelve-row stem.
    for cell in cells {
        assert!(cell.x > 712, "replacement must preserve the arrow");
        assert!(cell.x <= 717 && cell.x + cell.width > 764);
        assert!(cell.y <= 202 && cell.y + cell.height > 213);
    }
}

#[test]
fn card_placeholder_replaces_source_text_without_clearing_its_background() {
    let source = [7, 8, 7, 8, 15, 8, 7, 8, 7];
    let text = [0, 0, 0, 0, 0, 0, 0, 0, 15];

    let output = overlay_text_preserving_background(&source, &text, 3, 3, 8).unwrap();

    assert_eq!(output[0..4], source[0..4]);
    assert!(output[4] <= 8);
    assert_eq!(output[5..8], source[5..8]);
    assert_eq!(output[8], 15);
}

#[test]
#[ignore = "requires assets/"]
fn card_placeholder_uses_its_independent_viewer_font() {
    let unit: BonusInventoryUnit = serde_json::from_str(crate::test_input::read_str(
        "assets/menu/bonus-inventory/fixed/no-card.json",
    ))
    .unwrap();

    validate_translation_state(&unit).unwrap();
    assert_eq!(unit.development_status, DevelopmentStatus::Authored);
    assert_eq!(unit.korean_text.as_deref(), Some(unit.source_text.as_str()));
    assert!(unit.occurrences.iter().all(|occurrence| {
        occurrence.font_role == Some(BonusInventoryFontRole::ViewerCardPlaceholder)
    }));
}
