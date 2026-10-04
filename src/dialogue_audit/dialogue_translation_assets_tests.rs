use std::path::PathBuf;

use super::super::dialogue_translation_assets_model::DialogueTranslationAssetAuditConfig;
use super::super::translation_workspace_model::DialogueTranslationDecisionStatus;
use super::audit_dialogue_translation_assets;

#[test]
#[ignore = "requires assets/"]
fn tracked_translation_assets_are_fully_registered_and_semantically_unique() {
    let report = audit_dialogue_translation_assets(&DialogueTranslationAssetAuditConfig {
        input: PathBuf::from("assets/dialogue/translations"),
    })
    .unwrap();
    assert!(report.primary_owner_count > 0);
    assert!(report.selector_owner_count > 0);
    assert_eq!(
        report.primary_decision_count + report.selector_decision_count,
        report.semantic_decision_count
    );
    assert_eq!(
        report.untranslated_decision_count
            + report.draft_decision_count
            + report.ready_for_review_decision_count,
        report.semantic_decision_count
    );
}

#[test]
fn incomplete_authored_decision_is_rejected() {
    let result = super::validation::validate_translation_entry(
        &"a".repeat(64),
        &["原文".to_string()],
        &[None],
        DialogueTranslationDecisionStatus::Draft,
        Some("translator"),
    );
    assert!(result.is_err());
}

#[test]
fn untranslated_decision_retains_an_empty_korean_slot() {
    let counts = super::validation::validate_translation_entry(
        &"b".repeat(64),
        &["原文".to_string()],
        &[None],
        DialogueTranslationDecisionStatus::Untranslated,
        None,
    )
    .unwrap();
    assert_eq!(counts.untranslated, 1);
    assert_eq!(counts.decision_count(), 1);
}
