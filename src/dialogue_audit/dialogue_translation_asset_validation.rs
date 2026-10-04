use anyhow::{Result, ensure};

use super::super::translation_workspace_model::DialogueTranslationDecisionStatus;
use super::DecisionCounts;

pub(super) fn validate_translation_entry(
    semantic_source_sha256: &str,
    source_segments: &[String],
    korean_segments: &[Option<String>],
    status: DialogueTranslationDecisionStatus,
    translator: Option<&str>,
) -> Result<DecisionCounts> {
    ensure!(
        is_sha256(semantic_source_sha256),
        "dialogue semantic ID is not one SHA-256: {semantic_source_sha256}"
    );
    ensure!(
        source_segments.len() == korean_segments.len(),
        "tracked translation source shape changed for {semantic_source_sha256}"
    );
    let mut counts = DecisionCounts::default();
    match status {
        DialogueTranslationDecisionStatus::Untranslated => ensure!(
            korean_segments.iter().all(Option::is_none) && translator.is_none(),
            "untranslated tracked decision contains authored text for {semantic_source_sha256}"
        ),
        DialogueTranslationDecisionStatus::Draft
        | DialogueTranslationDecisionStatus::ReadyForReview => ensure!(
            korean_segments.iter().all(Option::is_some)
                && korean_segments
                    .iter()
                    .flatten()
                    .any(|segment| !segment.trim().is_empty())
                && translator.is_some_and(|value| !value.trim().is_empty()),
            "authored tracked decision is incomplete for {semantic_source_sha256}"
        ),
    }
    counts.observe(status);
    Ok(counts)
}

pub(super) fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}
