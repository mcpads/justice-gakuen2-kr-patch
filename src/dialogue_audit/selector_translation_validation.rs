use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};

use crate::pipeline::sha256_bytes;

use super::selector_translation_model::{
    DialogueSelectorDevelopmentResolution, DialogueSelectorTranslationContextEntry,
    DialogueSelectorTranslationDecision, DialogueSelectorTranslationReviewShard,
    DialogueSelectorTranslationScope, DialogueSelectorTranslationSourceGroup,
    DialogueSelectorTranslationWorkspaceManifest,
};
use super::selector_translation_source::SelectorTranslationSource;
use super::translation_validation::{contains_japanese_source_character, valid_date};
use super::translation_workspace_model::{
    DialogueTranslationDecisionStatus, DialogueTranslationReviewStatus,
};
use super::translation_workspace_validation::{MAX_SHARD_BYTES, MAX_SHARD_ENTRIES};

pub(super) type ExpectedSelectorGroups =
    BTreeMap<String, (usize, DialogueSelectorTranslationSourceGroup)>;
pub(super) type ExpectedSelectorContexts =
    BTreeMap<String, (usize, DialogueSelectorTranslationContextEntry)>;

#[derive(Debug, Default)]
pub(super) struct SelectorTranslationAuditCounts {
    pub(super) source_group_count: usize,
    pub(super) context_count: usize,
    pub(super) untranslated_group_count: usize,
    pub(super) draft_group_count: usize,
    pub(super) ready_for_review_group_count: usize,
    pub(super) pending_review_group_count: usize,
    pub(super) changes_requested_group_count: usize,
    pub(super) approved_group_count: usize,
    pub(super) largest_json_byte_count: usize,
}

impl SelectorTranslationAuditCounts {
    pub(super) fn authored_group_count(&self) -> usize {
        self.draft_group_count + self.ready_for_review_group_count
    }

    pub(super) fn observe_json(&mut self, byte_count: usize) {
        self.largest_json_byte_count = self.largest_json_byte_count.max(byte_count);
    }
}

pub(super) fn validate_selector_manifest(
    manifest: &DialogueSelectorTranslationWorkspaceManifest,
    source: &SelectorTranslationSource,
) -> Result<()> {
    let scope = DialogueSelectorTranslationScope::from_target_scope(&manifest.target_scope)
        .context("selector translation target scope is unknown")?;
    ensure!(scope == source.scope, "selector translation scope changed");
    let expected = match scope {
        DialogueSelectorTranslationScope::MgkDevelopment => (10, 496, 3_070, 492, 431),
        DialogueSelectorTranslationScope::AllRuntimeImages => (78, 550, 22_500, 546, 485),
    };
    let source_asset_count = if manifest.source_asset_count == 0
        && manifest.target_scope == "selector_banks_2_through_6"
    {
        10
    } else {
        manifest.source_asset_count
    };
    ensure!(
        manifest.kind == "Justice Gakuen 2 sharded Korean selector translation workspace"
            && manifest.source_bin_sha256 == source.source_bin_sha256
            && manifest.codebook_sha256 == source.codebook_sha256
            && manifest.source_corpus_sha256 == source.source_corpus_sha256
            && manifest.selector_consumer_audit_sha256 == source.selector_consumer_audit_sha256
            && source_asset_count == source.source_asset_count
            && manifest.selector_count == 5
            && manifest.selector_count == manifest.selectors.len()
            && (
                source_asset_count,
                manifest.semantic_group_count,
                manifest.coordinate_count,
                manifest.authored_translation_target_group_count,
                manifest.direct_selector_load_group_count,
            ) == expected
            && manifest.runtime_insertion_rewrite_group_count == 4
            && manifest.direct_entry_load_group_count == 1
            && manifest.consumer_pending_group_count == 60
            && manifest.max_entries_per_shard == MAX_SHARD_ENTRIES
            && manifest.max_bytes_per_shard == MAX_SHARD_BYTES,
        "selector translation root binding changed for {}",
        scope.target_scope(),
    );
    let selectors = manifest
        .selectors
        .iter()
        .map(|selector| selector.canonical_selector)
        .collect::<BTreeSet<_>>();
    ensure!(
        selectors == BTreeSet::from([2, 3, 4, 5, 6]),
        "selector translation owner population changed"
    );
    Ok(())
}

pub(super) fn expected_selector_groups(
    source: &SelectorTranslationSource,
) -> ExpectedSelectorGroups {
    source
        .groups_by_selector
        .iter()
        .flat_map(|(&selector, groups)| {
            groups.iter().map(move |group| {
                (
                    group.semantic_source_sha256.clone(),
                    (selector, group.clone()),
                )
            })
        })
        .collect()
}

pub(super) fn expected_selector_contexts(
    source: &SelectorTranslationSource,
) -> ExpectedSelectorContexts {
    source
        .contexts_by_selector
        .iter()
        .flat_map(|(&selector, contexts)| {
            contexts
                .iter()
                .map(move |context| (context.occurrence_id.clone(), (selector, context.clone())))
        })
        .collect()
}

pub(super) fn validate_selector_source_entries(
    canonical_selector: usize,
    entries: &[DialogueSelectorTranslationSourceGroup],
    expected: &ExpectedSelectorGroups,
    observed: &mut BTreeSet<String>,
) -> Result<()> {
    for entry in entries {
        let (expected_selector, expected_entry) = expected
            .get(&entry.semantic_source_sha256)
            .context("selector source contains an unregistered semantic group")?;
        let legacy_coordinate_count = entry.coordinate_count == 0
            && !entry.coordinate_ids.is_empty()
            && entry.coordinate_ids == expected_entry.coordinate_ids;
        ensure!(
            *expected_selector == canonical_selector
                && entry.semantic_source_sha256 == expected_entry.semantic_source_sha256
                && entry.source_segments == expected_entry.source_segments
                && entry.controls == expected_entry.controls
                && entry.target_selectors == expected_entry.target_selectors
                && entry.coordinate_ids == expected_entry.coordinate_ids
                && (entry.coordinate_count == expected_entry.coordinate_count
                    || legacy_coordinate_count)
                && entry.consumer_evidence == expected_entry.consumer_evidence
                && entry.evidence_basis == expected_entry.evidence_basis
                && entry.development_resolution == expected_entry.development_resolution
                && entry.development_korean_segments == expected_entry.development_korean_segments,
            "protected selector source changed for {}",
            entry.semantic_source_sha256
        );
        ensure!(
            observed.insert(entry.semantic_source_sha256.clone()),
            "selector source semantic group occurs more than once"
        );
    }
    Ok(())
}

pub(super) fn validate_selector_context_entries(
    canonical_selector: usize,
    entries: &[DialogueSelectorTranslationContextEntry],
    expected: &ExpectedSelectorContexts,
    observed: &mut BTreeSet<String>,
) -> Result<()> {
    for entry in entries {
        let (expected_selector, expected_entry) = expected
            .get(&entry.occurrence_id)
            .context("selector context contains an unregistered occurrence")?;
        ensure!(
            *expected_selector == canonical_selector && expected_entry == entry,
            "protected selector context changed for {}",
            entry.occurrence_id
        );
        ensure!(
            observed.insert(entry.occurrence_id.clone()),
            "selector context occurrence occurs more than once"
        );
    }
    Ok(())
}

pub(super) fn validate_selector_translation_entries(
    decisions: &[DialogueSelectorTranslationDecision],
    source: &[DialogueSelectorTranslationSourceGroup],
    counts: &mut SelectorTranslationAuditCounts,
) -> Result<()> {
    ensure!(
        decisions.len() == source.len(),
        "selector translation/source entry count differs"
    );
    for (decision, source_group) in decisions.iter().zip(source) {
        ensure!(
            decision.semantic_source_sha256 == source_group.semantic_source_sha256
                && decision.korean_segments.len() == source_group.source_segments.len()
                && decision.development_resolution == source_group.development_resolution,
            "selector translation source shape changed for {}",
            source_group.semantic_source_sha256
        );
        if source_group.development_resolution
            == DialogueSelectorDevelopmentResolution::RuntimeInsertionRewrite
        {
            let expected_segments = source_group
                .development_korean_segments
                .as_ref()
                .context("runtime insertion source lacks Korean development bytes")?;
            ensure!(
                decision.status != DialogueTranslationDecisionStatus::Untranslated
                    && decision.korean_segments
                        == expected_segments
                            .iter()
                            .cloned()
                            .map(Some)
                            .collect::<Vec<_>>()
                    && decision.translator.as_deref() == Some("source-bound-runtime-insertion"),
                "runtime insertion development rewrite changed"
            );
        }
        match decision.status {
            DialogueTranslationDecisionStatus::Untranslated => {
                ensure!(
                    source_group.development_resolution
                        == DialogueSelectorDevelopmentResolution::AuthoredTranslation
                        && decision.korean_segments.iter().all(Option::is_none)
                        && decision.translator.is_none(),
                    "untranslated selector decision contains authored text"
                );
                counts.untranslated_group_count += 1;
            }
            DialogueTranslationDecisionStatus::Draft
            | DialogueTranslationDecisionStatus::ReadyForReview => {
                ensure!(
                    decision.korean_segments.iter().all(Option::is_some)
                        && decision
                            .korean_segments
                            .iter()
                            .flatten()
                            .any(|segment| !segment.trim().is_empty())
                        && decision
                            .korean_segments
                            .iter()
                            .flatten()
                            .all(|segment| !contains_japanese_source_character(segment))
                        && nonempty(decision.translator.as_deref()),
                    "authored selector translation requires complete Korean segments and translator"
                );
                match decision.status {
                    DialogueTranslationDecisionStatus::Draft => counts.draft_group_count += 1,
                    DialogueTranslationDecisionStatus::ReadyForReview => {
                        counts.ready_for_review_group_count += 1
                    }
                    DialogueTranslationDecisionStatus::Untranslated => unreachable!(),
                }
            }
        }
    }
    Ok(())
}

pub(super) fn validate_selector_review_entries(
    review: &DialogueSelectorTranslationReviewShard,
    translation_path: &str,
    source: &[DialogueSelectorTranslationSourceGroup],
    translations: &[DialogueSelectorTranslationDecision],
    counts: &mut SelectorTranslationAuditCounts,
) -> Result<()> {
    ensure!(
        review.translation_path == translation_path
            && review.entries.len() == source.len()
            && translations.len() == source.len(),
        "selector review/source/translation shape differs"
    );
    for ((decision, source_group), translation) in
        review.entries.iter().zip(source).zip(translations)
    {
        ensure!(
            decision.semantic_source_sha256 == source_group.semantic_source_sha256,
            "selector review semantic identity changed"
        );
        match decision.status {
            DialogueTranslationReviewStatus::Pending => {
                ensure!(
                    decision.reviewer.is_none()
                        && decision.reviewed_at.is_none()
                        && decision.reviewed_translation_sha256.is_none(),
                    "pending selector review contains approval metadata"
                );
                counts.pending_review_group_count += 1;
            }
            DialogueTranslationReviewStatus::ChangesRequested
            | DialogueTranslationReviewStatus::Approved => {
                let translation_hash = selector_translation_decision_sha256(translation)?;
                ensure!(
                    nonempty(decision.reviewer.as_deref())
                        && valid_date(decision.reviewed_at.as_deref())
                        && decision.reviewed_translation_sha256.as_deref()
                            == Some(translation_hash.as_str())
                        && decision.reviewer.as_deref() != translation.translator.as_deref(),
                    "selector review must be independent, dated, and hash bound"
                );
                match decision.status {
                    DialogueTranslationReviewStatus::ChangesRequested => {
                        counts.changes_requested_group_count += 1
                    }
                    DialogueTranslationReviewStatus::Approved => {
                        ensure!(
                            translation.status == DialogueTranslationDecisionStatus::ReadyForReview,
                            "approved selector review requires ready-for-review translation"
                        );
                        counts.approved_group_count += 1;
                    }
                    DialogueTranslationReviewStatus::Pending => unreachable!(),
                }
            }
        }
    }
    Ok(())
}

pub(super) fn selector_translation_decision_sha256(
    decision: &DialogueSelectorTranslationDecision,
) -> Result<String> {
    Ok(sha256_bytes(&serde_json::to_vec(decision)?))
}

fn nonempty(value: Option<&str>) -> bool {
    value.is_some_and(|value| !value.trim().is_empty())
}
