use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};

use super::translation_decision_hash::translation_decision_sha256;
use super::translation_model::DialogueTranslationScope;
use super::translation_validation::{contains_japanese_source_character, valid_date};
use super::translation_workspace_model::{
    DialogueTranslationAssetManifest, DialogueTranslationContextEntry,
    DialogueTranslationDecisionStatus, DialogueTranslationReviewShard,
    DialogueTranslationReviewStatus, DialogueTranslationRouteOwner, DialogueTranslationShard,
    DialogueTranslationShardRef, DialogueTranslationSourceGroup, DialogueTranslationSourceShard,
    DialogueTranslationWorkspaceAssetRef, DialogueTranslationWorkspaceManifest,
};
use super::translation_workspace_source::TranslationWorkspaceSource;

pub(super) const MAX_SHARD_ENTRIES: usize = 32;
pub(super) const MAX_SHARD_BYTES: usize = 24 * 1024;

pub(super) type ExpectedSourceGroups = BTreeMap<
    String,
    (
        DialogueTranslationRouteOwner,
        DialogueTranslationSourceGroup,
    ),
>;

pub(super) type ExpectedContextEntries = BTreeMap<
    String,
    (
        DialogueTranslationRouteOwner,
        DialogueTranslationContextEntry,
    ),
>;

pub(super) fn validate_manifest(
    manifest: &DialogueTranslationWorkspaceManifest,
    source: &TranslationWorkspaceSource,
) -> Result<()> {
    let expected_asset_count = source
        .contexts_by_owner
        .keys()
        .chain(source.groups_by_owner.keys())
        .map(|owner| owner.source_path.as_str())
        .collect::<BTreeSet<_>>()
        .len();
    let expected_semantic_group_count: usize = source.groups_by_owner.values().map(Vec::len).sum();
    let manifest_scope = DialogueTranslationScope::from_target_scope(&manifest.target_scope);
    let source_scope = DialogueTranslationScope::from_target_scope(&source.target_scope);
    let legacy_mgk_manifest = manifest.target_scope == "primary_execution_referenced_dialogue";
    ensure!(
        manifest.kind == "Justice Gakuen 2 sharded Korean dialogue translation workspace"
            && manifest.source_bin_sha256 == source.source_bin_sha256
            && manifest.codebook_sha256 == source.codebook_sha256
            && manifest.source_corpus_sha256 == source.source_corpus_sha256
            && manifest.script_inventory_sha256 == source.script_inventory_sha256
            && manifest_scope == source_scope
            && (manifest.source_asset_count == source.source_asset_count
                || (legacy_mgk_manifest && manifest.source_asset_count == 0))
            && manifest.unresolved_primary_script_assets == source.unresolved_primary_script_assets
            && manifest.asset_count == expected_asset_count
            && manifest.semantic_group_count == expected_semantic_group_count
            && manifest.referenced_coordinate_count == source.referenced_coordinates.len()
            && manifest.max_entries_per_shard == MAX_SHARD_ENTRIES
            && manifest.max_bytes_per_shard == MAX_SHARD_BYTES,
        "translation workspace root binding changed"
    );
    Ok(())
}

pub(super) fn validate_asset_ref(
    asset_ref: &DialogueTranslationWorkspaceAssetRef,
    asset: &DialogueTranslationAssetManifest,
) -> Result<()> {
    ensure!(
        asset.source_path == asset_ref.source_path
            && asset.semantic_group_count == asset_ref.semantic_group_count
            && asset.referenced_coordinate_count == asset_ref.referenced_coordinate_count
            && asset.context_occurrence_count == asset_ref.context_occurrence_count,
        "asset manifest aggregate changed for {}",
        asset_ref.source_path
    );
    Ok(())
}

pub(super) fn validate_shard_ref(
    shard_ref: &DialogueTranslationShardRef,
    shard_id: &str,
    entry_count: usize,
) -> Result<()> {
    ensure!(
        shard_ref.shard_id == shard_id
            && shard_ref.entry_count == entry_count
            && entry_count <= MAX_SHARD_ENTRIES,
        "workspace shard metadata changed at {}",
        shard_ref.path
    );
    Ok(())
}

pub(super) fn expected_source_groups(source: &TranslationWorkspaceSource) -> ExpectedSourceGroups {
    source
        .groups_by_owner
        .iter()
        .flat_map(|(owner, groups)| {
            groups.iter().map(|group| {
                (
                    group.semantic_source_sha256.clone(),
                    (owner.clone(), group.clone()),
                )
            })
        })
        .collect()
}

pub(super) fn expected_context_entries(
    source: &TranslationWorkspaceSource,
) -> ExpectedContextEntries {
    source
        .contexts_by_owner
        .iter()
        .flat_map(|(owner, entries)| {
            entries
                .iter()
                .map(|entry| (entry.occurrence_id.clone(), (owner.clone(), entry.clone())))
        })
        .collect()
}

pub(super) fn validate_source_entries(
    owner: &DialogueTranslationRouteOwner,
    entries: &[DialogueTranslationSourceGroup],
    expected: &ExpectedSourceGroups,
    observed: &mut BTreeSet<String>,
) -> Result<()> {
    for entry in entries {
        let expected_entry = expected
            .get(&entry.semantic_source_sha256)
            .context("source shard contains an unregistered semantic group")?;
        let expected_group = &expected_entry.1;
        let observed_unreferenced_count = entry.unreferenced_duplicate_coordinate_count
            + entry.unreferenced_duplicate_coordinate_ids.len();
        ensure!(
            expected_entry.0 == *owner
                && entry.semantic_source_sha256 == expected_group.semantic_source_sha256
                && entry.source_segments == expected_group.source_segments
                && entry.controls == expected_group.controls
                && entry.coordinate_count() == expected_group.coordinate_count()
                && (entry.referenced_coordinate_ids == expected_group.referenced_coordinate_ids
                    || (entry.referenced_coordinate_ids.is_empty()
                        && entry.referenced_coordinate_count.is_some()))
                && observed_unreferenced_count
                    == expected_group.unreferenced_duplicate_coordinate_count
                && entry.context_occurrence_count == expected_group.context_occurrence_count,
            "protected source group changed for {}",
            entry.semantic_source_sha256
        );
        ensure!(
            observed.insert(entry.semantic_source_sha256.clone()),
            "semantic source group occurs more than once"
        );
    }
    Ok(())
}

pub(super) fn validate_translation_entries(
    translation: &DialogueTranslationShard,
    source: &DialogueTranslationSourceShard,
    counts: &mut AuditCounts,
) -> Result<()> {
    ensure!(
        translation.entries.len() == source.entries.len(),
        "translation/source entry count differs"
    );
    for (decision, source_group) in translation.entries.iter().zip(&source.entries) {
        ensure!(
            decision.semantic_source_sha256 == source_group.semantic_source_sha256
                && decision.korean_segments.len() == source_group.source_segments.len(),
            "translation source shape changed for {}",
            source_group.semantic_source_sha256
        );
        match decision.status {
            DialogueTranslationDecisionStatus::Untranslated => {
                ensure!(
                    decision.korean_segments.iter().all(Option::is_none)
                        && decision.translator.is_none(),
                    "untranslated decision contains authored text"
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
                    "authored translation requires complete Korean segments and translator"
                );
                for character in decision
                    .korean_segments
                    .iter()
                    .flatten()
                    .flat_map(|segment| segment.chars())
                    .filter(|character| !character.is_whitespace())
                {
                    counts.korean_repertoire.insert(character);
                }
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

pub(super) fn validate_review_entries(
    review: &DialogueTranslationReviewShard,
    source: &DialogueTranslationSourceShard,
    translation: &DialogueTranslationShard,
    counts: &mut AuditCounts,
) -> Result<()> {
    ensure!(
        review.entries.len() == source.entries.len()
            && review.entries.len() == translation.entries.len(),
        "review/source/translation entry counts differ"
    );
    for ((decision, source_group), translation_decision) in review
        .entries
        .iter()
        .zip(&source.entries)
        .zip(&translation.entries)
    {
        let translation_hash = translation_decision_sha256(translation_decision)?;
        ensure!(
            decision.semantic_source_sha256 == source_group.semantic_source_sha256,
            "review semantic identity changed"
        );
        match decision.status {
            DialogueTranslationReviewStatus::Pending => {
                ensure!(
                    decision.reviewer.is_none()
                        && decision.reviewed_at.is_none()
                        && decision.reviewed_translation_sha256.is_none(),
                    "pending review contains approval metadata"
                );
                counts.pending_review_group_count += 1;
            }
            DialogueTranslationReviewStatus::ChangesRequested
            | DialogueTranslationReviewStatus::Approved => {
                ensure!(
                    nonempty(decision.reviewer.as_deref())
                        && valid_date(decision.reviewed_at.as_deref())
                        && decision.reviewed_translation_sha256.as_deref()
                            == Some(translation_hash.as_str())
                        && decision.reviewer.as_deref()
                            != translation_decision.translator.as_deref(),
                    "review must be independent, dated, and bound to current translation bytes"
                );
                match decision.status {
                    DialogueTranslationReviewStatus::ChangesRequested => {
                        counts.changes_requested_group_count += 1
                    }
                    DialogueTranslationReviewStatus::Approved => {
                        ensure!(
                            translation_decision.status
                                == DialogueTranslationDecisionStatus::ReadyForReview,
                            "approved review requires ready-for-review translation"
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

pub(super) fn validate_workspace_project_review(
    manifest: &DialogueTranslationWorkspaceManifest,
) -> Result<()> {
    let review = manifest
        .project_review
        .as_ref()
        .context("complete workspace lacks project review")?;
    ensure!(
        review.decision == "approved"
            && review.approved_by == "project_owner"
            && valid_date(Some(&review.approved_on))
            && review.scope == "all_execution_referenced_dialogue_semantic_groups"
            && review.basis == "source_and_integrated_runtime_review",
        "workspace project review is incomplete"
    );
    Ok(())
}

fn nonempty(value: Option<&str>) -> bool {
    value.is_some_and(|value| !value.trim().is_empty())
}

#[derive(Default)]
pub(super) struct AuditCounts {
    pub(super) source_shard_count: usize,
    pub(super) context_shard_count: usize,
    pub(super) translation_shard_count: usize,
    pub(super) review_shard_count: usize,
    pub(super) referenced_coordinate_count: usize,
    pub(super) context_occurrence_count: usize,
    pub(super) untranslated_group_count: usize,
    pub(super) draft_group_count: usize,
    pub(super) ready_for_review_group_count: usize,
    pub(super) pending_review_group_count: usize,
    pub(super) changes_requested_group_count: usize,
    pub(super) approved_group_count: usize,
    pub(super) korean_repertoire: BTreeSet<char>,
    pub(super) largest_shard_entry_count: usize,
    pub(super) largest_shard_byte_count: usize,
}

impl AuditCounts {
    pub(super) fn observe_shard(&mut self, entry_count: usize, byte_count: usize) {
        self.largest_shard_entry_count = self.largest_shard_entry_count.max(entry_count);
        self.largest_shard_byte_count = self.largest_shard_byte_count.max(byte_count);
    }

    pub(super) fn semantic_group_count(&self) -> usize {
        self.untranslated_group_count + self.draft_group_count + self.ready_for_review_group_count
    }
}
