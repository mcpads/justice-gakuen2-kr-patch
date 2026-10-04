use super::translation_workspace_layout::{admit_layout_input, authored_layout_segments};
use super::translation_workspace_model::{
    DialogueTranslationDecision, DialogueTranslationDecisionStatus,
    DialogueTranslationWorkspaceAuditReport,
};

#[test]
fn pending_review_does_not_block_development_layout_analysis() {
    let audit = translation_audit(1, 1, 0, 1, 0, true);

    let approval_complete = admit_layout_input(&audit, false).unwrap();

    assert!(!approval_complete);
}

#[test]
fn approval_gated_layout_analysis_rejects_pending_review() {
    let audit = translation_audit(1, 1, 0, 1, 0, true);

    let error = admit_layout_input(&audit, true).unwrap_err();

    assert!(error.to_string().contains("--require-approved"));
}

#[test]
fn development_layout_analysis_accepts_complete_draft_segments() {
    let decision = DialogueTranslationDecision {
        semantic_source_sha256: "semantic".to_string(),
        korean_segments: vec![Some("개발 입력".to_string()), Some(String::new())],
        status: DialogueTranslationDecisionStatus::Draft,
        translator: Some("translator".to_string()),
        notes: String::new(),
    };

    let segments = authored_layout_segments(&decision).unwrap();

    assert_eq!(segments, ["개발 입력", ""]);
}

#[test]
fn untranslated_text_cannot_be_measured_as_development_layout_input() {
    let audit = translation_audit(1, 0, 1, 1, 0, false);

    let error = admit_layout_input(&audit, false).unwrap_err();

    assert!(error.to_string().contains("authored text"));
}

fn translation_audit(
    semantic_group_count: usize,
    ready_for_review_group_count: usize,
    untranslated_group_count: usize,
    pending_review_group_count: usize,
    approved_group_count: usize,
    ready_for_font_repertoire: bool,
) -> DialogueTranslationWorkspaceAuditReport {
    DialogueTranslationWorkspaceAuditReport {
        kind: "translation audit".to_string(),
        source_bin_sha256: "source-bin".to_string(),
        codebook_sha256: "codebook".to_string(),
        source_corpus_sha256: "source-corpus".to_string(),
        script_inventory_sha256: "script-inventory".to_string(),
        asset_count: 1,
        source_shard_count: 1,
        context_shard_count: 1,
        translation_shard_count: 1,
        review_shard_count: 1,
        semantic_group_count,
        referenced_coordinate_count: semantic_group_count,
        context_occurrence_count: semantic_group_count,
        untranslated_group_count,
        draft_group_count: semantic_group_count
            - ready_for_review_group_count
            - untranslated_group_count,
        ready_for_review_group_count,
        pending_review_group_count,
        changes_requested_group_count: 0,
        approved_group_count,
        korean_character_count: usize::from(ready_for_font_repertoire),
        korean_repertoire: if ready_for_font_repertoire {
            "가".to_string()
        } else {
            String::new()
        },
        asset_repertoires: Vec::new(),
        largest_shard_entry_count: semantic_group_count,
        largest_shard_byte_count: 1,
        ready_for_font_repertoire,
        development_translation_input_available: ready_for_font_repertoire,
        release_candidate_translation_input_eligible: false,
        translation_project_complete: false,
    }
}
