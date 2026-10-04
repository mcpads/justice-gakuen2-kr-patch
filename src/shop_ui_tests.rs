use super::assets::validate_translation_state;
use super::model::{DevelopmentStatus, ReleaseStatus, ShopUiFontRole, ShopUiUnit};
use crate::tim::Cell;

fn unit(korean_text: &str) -> ShopUiUnit {
    ShopUiUnit {
        kind: "justice_gakuen2_shop_ui_unit".to_string(),
        id: "heading".to_string(),
        source_text: "購買部".to_string(),
        korean_text: korean_text.to_string(),
        font_role: ShopUiFontRole::Heading,
        tim_offset: "0x1e000".to_string(),
        cell: Cell {
            x: 96,
            y: 64,
            width: 136,
            height: 48,
        },
        source_region_sha256: "hash".to_string(),
        development_status: DevelopmentStatus::Authored,
        release_status: ReleaseStatus::NeedsHumanReview,
    }
}

#[test]
fn authored_shop_ui_unit_requires_korean_text() {
    let error = validate_translation_state(&unit(" ")).unwrap_err();
    assert!(error.to_string().contains("lacks text"));
}

#[test]
fn authored_shop_ui_unit_does_not_require_release_approval() {
    validate_translation_state(&unit("구매부")).unwrap();
}
