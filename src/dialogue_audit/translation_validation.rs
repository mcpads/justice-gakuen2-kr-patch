#[cfg(test)]
use anyhow::{Result, ensure};

#[cfg(test)]
use super::translation_model::{DialogueTranslationGroup, DialogueTranslationStatus};

#[cfg(test)]
pub(super) fn validate_group_state(
    group: &DialogueTranslationGroup,
    expected: &DialogueTranslationGroup,
) -> Result<()> {
    ensure!(
        group.semantic_source_sha256 == expected.semantic_source_sha256
            && group.source_segments == expected.source_segments
            && group.controls == expected.controls
            && group.coordinate_ids == expected.coordinate_ids,
        "translation source fields changed for semantic group {}",
        expected.semantic_source_sha256
    );
    ensure!(
        group.korean_segments.len() == group.source_segments.len(),
        "Korean segment count differs from source shape for {}",
        group.semantic_source_sha256
    );
    match group.status {
        DialogueTranslationStatus::ProtectedOnly => ensure!(
            group.source_segments.iter().all(String::is_empty)
                && group
                    .korean_segments
                    .iter()
                    .all(|segment| segment.as_deref() == Some(""))
                && no_authors(group),
            "protected-only group contains translatable or authored content"
        ),
        DialogueTranslationStatus::Untranslated => ensure!(
            group
                .source_segments
                .iter()
                .any(|segment| !segment.is_empty())
                && group.korean_segments.iter().all(Option::is_none)
                && no_authors(group),
            "untranslated group must retain empty Korean decisions and no authors"
        ),
        DialogueTranslationStatus::Draft | DialogueTranslationStatus::NeedsReview => ensure!(
            group.korean_segments.iter().all(Option::is_some)
                && has_korean_text(group)
                && nonempty(group.translator.as_deref())
                && group.reviewer.is_none()
                && group.reviewed_at.is_none(),
            "draft or needs-review group requires complete segments and a translator only"
        ),
        DialogueTranslationStatus::Approved => ensure!(
            group.korean_segments.iter().all(Option::is_some)
                && has_korean_text(group)
                && nonempty(group.translator.as_deref())
                && group.reviewer.as_deref() == Some("project_owner")
                && valid_date(group.reviewed_at.as_deref()),
            "approved group requires complete segments and dated project-owner review"
        ),
    }
    Ok(())
}

#[cfg(test)]
fn no_authors(group: &DialogueTranslationGroup) -> bool {
    group.translator.is_none() && group.reviewer.is_none() && group.reviewed_at.is_none()
}

#[cfg(test)]
fn has_korean_text(group: &DialogueTranslationGroup) -> bool {
    group
        .korean_segments
        .iter()
        .flatten()
        .any(|segment| !segment.trim().is_empty())
}

#[cfg(test)]
fn nonempty(value: Option<&str>) -> bool {
    value.is_some_and(|value| !value.trim().is_empty())
}

pub(super) fn valid_date(value: Option<&str>) -> bool {
    value.is_some_and(|value| {
        let bytes = value.as_bytes();
        bytes.len() == 10
            && bytes[4] == b'-'
            && bytes[7] == b'-'
            && bytes
                .iter()
                .enumerate()
                .all(|(index, byte)| index == 4 || index == 7 || byte.is_ascii_digit())
    })
}

pub(super) fn contains_japanese_source_character(value: &str) -> bool {
    value.chars().any(|character| {
        matches!(
            character,
            '\u{3040}'..='\u{30ff}'
                | '\u{31f0}'..='\u{31ff}'
                | '\u{3400}'..='\u{4dbf}'
                | '\u{4e00}'..='\u{9fff}'
                | '\u{f900}'..='\u{faff}'
                | '\u{ff65}'..='\u{ff9f}'
        )
    })
}
