use serde::Serialize;

use super::translation_decision_hash::translation_decision_sha256;
use super::translation_model::DialogueTranslationProjectStatus;
use super::translation_model::DialogueTranslationRefreshConfig;
use super::translation_model::DialogueTranslationScope;
use super::translation_workspace_audit::{authored_repertoire, classify_translation_readiness};
use super::translation_workspace_io::{json_bytes, relative_path};
use super::translation_workspace_migration::refresh_dialogue_translation;
use super::translation_workspace_model::{
    DialogueTranslationDecision, DialogueTranslationDecisionStatus,
    DialogueTranslationReviewDecision, DialogueTranslationReviewShard,
    DialogueTranslationReviewStatus, DialogueTranslationRouteOwner, DialogueTranslationShard,
    DialogueTranslationSourceGroup, DialogueTranslationSourceShard,
};
use super::translation_workspace_validation::{
    AuditCounts, MAX_SHARD_BYTES, MAX_SHARD_ENTRIES, validate_review_entries,
    validate_translation_entries,
};
use super::translation_workspace_writer::partition;

#[derive(Serialize)]
struct ProbeShard {
    shard_id: String,
    entries: Vec<String>,
}

#[test]
fn partition_enforces_entry_and_serialized_byte_limits() {
    let entries = (0..65)
        .map(|index| format!("{index:03}-{}", "가".repeat(340)))
        .collect::<Vec<_>>();
    let chunks = partition(&entries, |entries| ProbeShard {
        shard_id: "x".repeat(128),
        entries: entries.to_vec(),
    })
    .unwrap();

    assert_eq!(chunks.iter().map(Vec::len).sum::<usize>(), entries.len());
    assert!(chunks.len() > 2);
    for chunk in chunks {
        assert!(chunk.len() <= MAX_SHARD_ENTRIES);
        let probe = ProbeShard {
            shard_id: "x".repeat(128),
            entries: chunk,
        };
        assert!(json_bytes(&probe).unwrap().len() <= MAX_SHARD_BYTES);
    }
}

#[test]
fn one_oversized_workspace_entry_is_rejected() {
    let entry = vec!["가".repeat(MAX_SHARD_BYTES)];
    assert!(
        partition(&entry, |entries| ProbeShard {
            shard_id: "x".repeat(128),
            entries: entries.to_vec(),
        })
        .is_err()
    );
}

#[test]
fn workspace_paths_cannot_escape_the_root() {
    assert!(relative_path("source/mgk04/unit-000.json").is_ok());
    assert!(relative_path("../outside.json").is_err());
    assert!(relative_path("/absolute.json").is_err());
}

#[test]
fn translation_scope_keeps_legacy_mgk_and_resolved_primary_distinct() {
    assert_eq!(
        DialogueTranslationScope::from_target_scope("primary_execution_referenced_dialogue"),
        Some(DialogueTranslationScope::MgkDevelopment)
    );
    assert_eq!(
        DialogueTranslationScope::from_target_scope("mgk_primary_execution_referenced_dialogue"),
        Some(DialogueTranslationScope::MgkDevelopment)
    );
    assert_eq!(
        DialogueTranslationScope::from_target_scope(
            "resolved_primary_execution_referenced_dialogue"
        ),
        Some(DialogueTranslationScope::ResolvedPrimary)
    );
    assert_ne!(
        DialogueTranslationScope::MgkDevelopment.target_scope(),
        DialogueTranslationScope::ResolvedPrimary.target_scope()
    );
}

#[test]
fn source_refresh_never_targets_the_previous_workspace() {
    let workspace = std::path::PathBuf::from("work/dialogue-translation");
    let result = refresh_dialogue_translation(&DialogueTranslationRefreshConfig {
        cue: "unused.cue".into(),
        codebook: "unused-codebook.json".into(),
        previous: workspace.clone(),
        output: workspace,
        force: true,
        scope: DialogueTranslationScope::MgkDevelopment,
    });

    assert!(result.is_err());
}

#[test]
fn untranslated_decision_rejects_partial_authored_text() {
    let source = source_shard();
    let mut translation = translation_shard();
    translation.entries[0].korean_segments[0] = Some("앞".to_string());

    assert!(
        validate_translation_entries(&translation, &source, &mut AuditCounts::default()).is_err()
    );
}

#[test]
fn authored_decision_rejects_untranslated_japanese_text() {
    let source = source_shard();
    let mut translation = translation_shard();
    translation.entries[0].korean_segments =
        vec![Some("그대로".to_string()), Some("後".to_string())];
    translation.entries[0].status = DialogueTranslationDecisionStatus::Draft;
    translation.entries[0].translator = Some("translator-a".to_string());

    assert!(
        validate_translation_entries(&translation, &source, &mut AuditCounts::default()).is_err()
    );
}

#[test]
fn asset_repertoire_is_scoped_to_authored_non_whitespace_characters() {
    let mut translation = translation_shard();
    translation.entries[0].korean_segments =
        vec![Some("가 나".to_string()), Some("나!".to_string())];
    translation.entries[0].status = DialogueTranslationDecisionStatus::Draft;
    translation.entries[0].translator = Some("translator-a".to_string());

    assert_eq!(
        authored_repertoire(&translation),
        ['!', '가', '나'].into_iter().collect()
    );
}

#[test]
fn pending_review_does_not_withhold_development_translation_input() {
    let readiness = classify_translation_readiness(
        DialogueTranslationProjectStatus::InProgress,
        1,
        0,
        1,
        0,
        false,
    );

    assert!(readiness.development_translation_input_available);
    assert!(!readiness.release_candidate_translation_input_eligible);
    assert!(!readiness.translation_project_complete);
}

#[test]
fn approved_text_can_feed_a_release_candidate_before_project_completion() {
    let readiness = classify_translation_readiness(
        DialogueTranslationProjectStatus::InProgress,
        1,
        0,
        1,
        1,
        false,
    );

    assert!(readiness.development_translation_input_available);
    assert!(readiness.release_candidate_translation_input_eligible);
    assert!(!readiness.translation_project_complete);
}

#[test]
fn approved_review_is_independent_and_hash_bound() {
    let source = source_shard();
    let mut translation = translation_shard();
    translation.entries[0].korean_segments = vec![Some("앞".to_string()), Some("뒤".to_string())];
    translation.entries[0].status = DialogueTranslationDecisionStatus::ReadyForReview;
    translation.entries[0].translator = Some("translator-a".to_string());

    let mut review = review_shard();
    let decision = &mut review.entries[0];
    decision.status = DialogueTranslationReviewStatus::Approved;
    decision.reviewer = Some("translator-a".to_string());
    decision.reviewed_at = Some("2026-08-09".to_string());
    decision.reviewed_translation_sha256 =
        Some(translation_decision_sha256(&translation.entries[0]).unwrap());
    assert!(
        validate_review_entries(&review, &source, &translation, &mut AuditCounts::default())
            .is_err()
    );

    review.entries[0].reviewer = Some("reviewer-b".to_string());
    review.entries[0].reviewed_translation_sha256 = Some("stale-hash".to_string());
    assert!(
        validate_review_entries(&review, &source, &translation, &mut AuditCounts::default())
            .is_err()
    );

    review.entries[0].reviewed_translation_sha256 =
        Some(translation_decision_sha256(&translation.entries[0]).unwrap());
    let mut counts = AuditCounts::default();
    validate_review_entries(&review, &source, &translation, &mut counts).unwrap();
    assert_eq!(counts.approved_group_count, 1);
}

#[test]
fn editing_one_translation_does_not_invalidate_a_sibling_review() {
    let mut source = source_shard();
    let mut sibling_source = source.entries[0].clone();
    sibling_source.semantic_source_sha256 = "semantic-sibling".to_string();
    source.entries.push(sibling_source);

    let mut translation = translation_shard();
    translation.entries[0].korean_segments = vec![Some("앞".to_string()), Some("뒤".to_string())];
    translation.entries[0].status = DialogueTranslationDecisionStatus::ReadyForReview;
    translation.entries[0].translator = Some("translator-a".to_string());
    let mut sibling_translation = translation.entries[0].clone();
    sibling_translation.semantic_source_sha256 = "semantic-sibling".to_string();
    sibling_translation.korean_segments[0] = Some("바뀐 앞".to_string());
    translation.entries.push(sibling_translation);

    let mut review = review_shard();
    review.entries[0].status = DialogueTranslationReviewStatus::Approved;
    review.entries[0].reviewer = Some("reviewer-b".to_string());
    review.entries[0].reviewed_at = Some("2026-08-10".to_string());
    review.entries[0].reviewed_translation_sha256 =
        Some(translation_decision_sha256(&translation.entries[0]).unwrap());
    let mut sibling_review = review.entries[0].clone();
    sibling_review.semantic_source_sha256 = "semantic-sibling".to_string();
    sibling_review.status = DialogueTranslationReviewStatus::Pending;
    sibling_review.reviewer = None;
    sibling_review.reviewed_at = None;
    sibling_review.reviewed_translation_sha256 = None;
    review.entries.push(sibling_review);

    let mut counts = AuditCounts::default();
    validate_review_entries(&review, &source, &translation, &mut counts).unwrap();
    assert_eq!(counts.approved_group_count, 1);
    assert_eq!(counts.pending_review_group_count, 1);
}

fn source_shard() -> DialogueTranslationSourceShard {
    DialogueTranslationSourceShard {
        kind: "source".to_string(),
        shard_id: "source-000".to_string(),
        source_bin_sha256: "bin".to_string(),
        codebook_sha256: "codebook".to_string(),
        source_corpus_sha256: "corpus".to_string(),
        script_inventory_sha256: "scripts".to_string(),
        owner: DialogueTranslationRouteOwner {
            source_path: "DAT1/MGK04.BIN".to_string(),
            bank_selector: 0,
            variant_selector: 0,
            route_table_offset: "0x4a654".to_string(),
        },
        entries: vec![DialogueTranslationSourceGroup {
            semantic_source_sha256: "semantic".to_string(),
            source_segments: vec!["前".to_string(), "後".to_string()],
            controls: Vec::new(),
            referenced_coordinate_ids: vec!["coordinate".to_string()],
            referenced_coordinate_count: None,
            unreferenced_duplicate_coordinate_ids: Vec::new(),
            unreferenced_duplicate_coordinate_count: 0,
            context_occurrence_count: 1,
        }],
    }
}

fn translation_shard() -> DialogueTranslationShard {
    DialogueTranslationShard {
        kind: "translation".to_string(),
        shard_id: "translation-000".to_string(),
        source_shard_sha256: "source-hash".to_string(),
        entries: vec![DialogueTranslationDecision {
            semantic_source_sha256: "semantic".to_string(),
            korean_segments: vec![None, None],
            status: DialogueTranslationDecisionStatus::Untranslated,
            translator: None,
            notes: String::new(),
        }],
    }
}

fn review_shard() -> DialogueTranslationReviewShard {
    DialogueTranslationReviewShard {
        kind: "review".to_string(),
        shard_id: "review-000".to_string(),
        source_shard_sha256: "source-hash".to_string(),
        translation_path: "translations/in-progress/unit-000.json".to_string(),
        entries: vec![DialogueTranslationReviewDecision {
            semantic_source_sha256: "semantic".to_string(),
            status: DialogueTranslationReviewStatus::Pending,
            reviewer: None,
            reviewed_at: None,
            reviewed_translation_sha256: None,
            notes: String::new(),
        }],
    }
}
