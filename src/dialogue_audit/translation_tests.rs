use super::translation_model::{
    DialogueTranslationControl, DialogueTranslationGroup, DialogueTranslationStatus,
};
use super::translation_validation::{valid_date, validate_group_state};

fn untranslated_group() -> DialogueTranslationGroup {
    DialogueTranslationGroup {
        semantic_source_sha256: "hash".to_string(),
        source_segments: vec!["前".to_string(), "後".to_string()],
        controls: vec![DialogueTranslationControl {
            semantic_name: "line_break".to_string(),
            arguments: Vec::new(),
        }],
        coordinate_ids: vec!["asset#bank-0-entry-0000".to_string()],
        korean_segments: vec![None, None],
        status: DialogueTranslationStatus::Untranslated,
        translator: None,
        reviewer: None,
        reviewed_at: None,
        notes: String::new(),
    }
}

#[test]
fn untranslated_group_is_valid_only_without_partial_segments() {
    let expected = untranslated_group();
    let mut actual = expected.clone();
    validate_group_state(&actual, &expected).unwrap();

    actual.korean_segments[0] = Some("앞".to_string());
    assert!(validate_group_state(&actual, &expected).is_err());
}

#[test]
fn approved_group_requires_project_owner_and_date() {
    let expected = untranslated_group();
    let mut actual = expected.clone();
    actual.korean_segments = vec![Some("앞".to_string()), Some("뒤".to_string())];
    actual.status = DialogueTranslationStatus::Approved;
    actual.translator = Some("translator".to_string());
    actual.reviewer = Some("project_owner".to_string());
    actual.reviewed_at = Some("2026-08-09".to_string());

    validate_group_state(&actual, &expected).unwrap();
    actual.reviewer = Some("automation".to_string());
    assert!(validate_group_state(&actual, &expected).is_err());
}

#[test]
fn linguistic_draft_cannot_delete_all_source_text() {
    let expected = untranslated_group();
    let mut actual = expected.clone();
    actual.korean_segments = vec![Some(String::new()), Some(String::new())];
    actual.status = DialogueTranslationStatus::Draft;
    actual.translator = Some("translator".to_string());

    assert!(validate_group_state(&actual, &expected).is_err());
}

#[test]
fn review_date_uses_fixed_calendar_shape() {
    assert!(valid_date(Some("2026-08-09")));
    assert!(!valid_date(Some("2026/08/09")));
    assert!(!valid_date(Some("2026-8-9")));
}
