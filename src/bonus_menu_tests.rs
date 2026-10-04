use super::assets::validate_translation_state;
use super::model::{BonusMenuFontRole, BonusMenuUnit, DevelopmentStatus, ReleaseStatus};
use super::raster::fit_indexed_ink_rows;
use crate::tim::Cell;

fn unit(
    korean_text: Option<&str>,
    development_status: DevelopmentStatus,
    release_status: ReleaseStatus,
) -> BonusMenuUnit {
    BonusMenuUnit {
        kind: "justice_gakuen2_bonus_main_unit".to_string(),
        id: "entry".to_string(),
        source_text: "原文".to_string(),
        korean_text: korean_text.map(str::to_string),
        font_role: BonusMenuFontRole::Entry,
        bits_per_pixel: 4,
        tim_offset: "0x3c800".to_string(),
        cell: Cell {
            x: 0,
            y: 0,
            width: 256,
            height: 32,
        },
        source_region_sha256: "hash".to_string(),
        development_status,
        release_status,
    }
}

#[test]
fn authored_bonus_entry_requires_korean_text() {
    let error = validate_translation_state(&unit(
        None,
        DevelopmentStatus::Authored,
        ReleaseStatus::NeedsHumanReview,
    ))
    .unwrap_err();
    assert!(error.to_string().contains("lacks Korean text"));
}

#[test]
fn untranslated_bonus_surface_cannot_claim_review_state() {
    let error = validate_translation_state(&unit(
        None,
        DevelopmentStatus::Untranslated,
        ReleaseStatus::NeedsHumanReview,
    ))
    .unwrap_err();
    assert!(error.to_string().contains("contains translated state"));
}

#[test]
fn compact_variant_preserves_horizontal_pixels_while_resampling_rows() {
    let source = [0, 0, 1, 2, 3, 4, 0, 0];

    assert_eq!(
        fit_indexed_ink_rows(&source, 2, 4, 2, [0, 1, 2, 3]),
        [1, 2, 3, 4]
    );
}

#[test]
fn suppressed_bonus_duplicate_must_be_non_translated() {
    let mut duplicate = unit(
        None,
        DevelopmentStatus::SuppressedDuplicate,
        ReleaseStatus::NotApplicable,
    );
    duplicate.korean_text = Some("중복".to_string());

    let error = validate_translation_state(&duplicate).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("suppressed bonus-main duplicate")
    );
}
