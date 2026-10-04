use std::collections::{BTreeMap, BTreeSet};

use super::selector_translation_migration::{
    PreviousSelectorGroup, preserve_compatible_selector_groups,
};
use super::selector_translation_model::{
    DialogueSelectorConsumerEvidence, DialogueSelectorDevelopmentResolution,
    DialogueSelectorTranslationDecision, DialogueSelectorTranslationSourceGroup,
};
use super::translation_model::DialogueTranslationControl;
use super::translation_workspace_model::{
    DialogueTranslationDecisionStatus, DialogueTranslationReviewDecision,
    DialogueTranslationReviewStatus,
};

#[test]
fn refreshed_consumer_evidence_preserves_authored_text_and_review() {
    let previous_group = source_group(vec!["old_consumer".to_string()]);
    let previous_translation = authored_translation();
    let previous_review = approved_review();
    let previous = BTreeMap::from([(
        semantic_hash(),
        PreviousSelectorGroup {
            source_segments: previous_group.source_segments,
            controls: previous_group.controls,
            development_resolution: previous_group.development_resolution,
            translation: previous_translation.clone(),
            review: previous_review.clone(),
        },
    )]);
    let refreshed_group = source_group(vec![
        "new_consumer_a".to_string(),
        "new_consumer_b".to_string(),
    ]);
    let mut translations = vec![blank_translation()];
    let mut reviews = vec![pending_review()];
    let mut migrated = BTreeSet::new();

    preserve_compatible_selector_groups(
        &[refreshed_group],
        &mut translations,
        &mut reviews,
        &previous,
        &mut migrated,
    )
    .unwrap();

    assert_eq!(
        translations[0].korean_segments,
        previous_translation.korean_segments
    );
    assert_eq!(
        translations[0].status,
        DialogueTranslationDecisionStatus::ReadyForReview
    );
    assert_eq!(reviews[0].status, DialogueTranslationReviewStatus::Approved);
    assert_eq!(
        reviews[0].reviewed_translation_sha256,
        previous_review.reviewed_translation_sha256
    );
    assert_eq!(migrated, BTreeSet::from([semantic_hash()]));
}

#[test]
fn selector_refresh_rejects_changed_source_controls() {
    let previous_group = source_group(vec!["old_consumer".to_string()]);
    let previous = BTreeMap::from([(
        semantic_hash(),
        PreviousSelectorGroup {
            source_segments: previous_group.source_segments,
            controls: previous_group.controls,
            development_resolution: previous_group.development_resolution,
            translation: authored_translation(),
            review: approved_review(),
        },
    )]);
    let mut changed = source_group(vec!["new_consumer".to_string()]);
    changed.controls[0].semantic_name = "different_control".to_string();

    let error = preserve_compatible_selector_groups(
        &[changed],
        &mut [blank_translation()],
        &mut [pending_review()],
        &previous,
        &mut BTreeSet::new(),
    )
    .unwrap_err();

    assert!(error.to_string().contains("source meaning changed"));
}

#[test]
fn selector_scope_expansion_leaves_a_new_semantic_group_blank() {
    let mut translation = blank_translation();
    let mut review = pending_review();
    preserve_compatible_selector_groups(
        &[source_group(vec!["new_consumer".to_string()])],
        std::slice::from_mut(&mut translation),
        std::slice::from_mut(&mut review),
        &BTreeMap::new(),
        &mut BTreeSet::new(),
    )
    .unwrap();

    assert_eq!(
        translation.status,
        DialogueTranslationDecisionStatus::Untranslated
    );
    assert_eq!(review.status, DialogueTranslationReviewStatus::Pending);
}

fn semantic_hash() -> String {
    "semantic-a".to_string()
}

fn source_group(evidence_basis: Vec<String>) -> DialogueSelectorTranslationSourceGroup {
    DialogueSelectorTranslationSourceGroup {
        semantic_source_sha256: semantic_hash(),
        source_segments: vec!["前".to_string(), "後".to_string()],
        controls: vec![DialogueTranslationControl {
            semantic_name: "line_break".to_string(),
            arguments: Vec::new(),
        }],
        target_selectors: vec![2],
        coordinate_count: 1,
        coordinate_ids: vec!["coordinate-a".to_string()],
        consumer_evidence: DialogueSelectorConsumerEvidence::DirectSelectorLoad,
        evidence_basis,
        development_resolution: DialogueSelectorDevelopmentResolution::AuthoredTranslation,
        development_korean_segments: None,
    }
}

fn authored_translation() -> DialogueSelectorTranslationDecision {
    DialogueSelectorTranslationDecision {
        semantic_source_sha256: semantic_hash(),
        korean_segments: vec![Some("앞".to_string()), Some("뒤".to_string())],
        status: DialogueTranslationDecisionStatus::ReadyForReview,
        translator: Some("translator-a".to_string()),
        notes: String::new(),
        development_resolution: DialogueSelectorDevelopmentResolution::AuthoredTranslation,
    }
}

fn blank_translation() -> DialogueSelectorTranslationDecision {
    DialogueSelectorTranslationDecision {
        semantic_source_sha256: semantic_hash(),
        korean_segments: vec![None, None],
        status: DialogueTranslationDecisionStatus::Untranslated,
        translator: None,
        notes: String::new(),
        development_resolution: DialogueSelectorDevelopmentResolution::AuthoredTranslation,
    }
}

fn approved_review() -> DialogueTranslationReviewDecision {
    DialogueTranslationReviewDecision {
        semantic_source_sha256: semantic_hash(),
        status: DialogueTranslationReviewStatus::Approved,
        reviewer: Some("reviewer-b".to_string()),
        reviewed_at: Some("2026-08-10".to_string()),
        reviewed_translation_sha256: Some("translation-hash".to_string()),
        notes: String::new(),
    }
}

fn pending_review() -> DialogueTranslationReviewDecision {
    DialogueTranslationReviewDecision {
        semantic_source_sha256: semantic_hash(),
        status: DialogueTranslationReviewStatus::Pending,
        reviewer: None,
        reviewed_at: None,
        reviewed_translation_sha256: None,
        notes: String::new(),
    }
}
