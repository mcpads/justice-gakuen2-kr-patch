use serde::{Deserialize, Serialize};

use super::translation_model::{
    DialogueTranslationControl, DialogueTranslationProjectStatus, DialogueTranslationReview,
};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DialogueTranslationWorkspaceManifest {
    pub kind: String,
    pub source_bin_sha256: String,
    pub codebook_sha256: String,
    pub source_corpus_sha256: String,
    pub script_inventory_sha256: String,
    pub target_scope: String,
    #[serde(default)]
    pub source_asset_count: usize,
    #[serde(default)]
    pub unresolved_primary_script_assets: Vec<String>,
    pub project_status: DialogueTranslationProjectStatus,
    pub project_review: Option<DialogueTranslationReview>,
    pub asset_count: usize,
    pub semantic_group_count: usize,
    pub referenced_coordinate_count: usize,
    pub context_occurrence_count: usize,
    pub max_entries_per_shard: usize,
    pub max_bytes_per_shard: usize,
    #[serde(default)]
    pub asset_manifest_count: usize,
    #[serde(default)]
    pub asset_manifests: Vec<DialogueTranslationAssetIndexManifestRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assets: Vec<DialogueTranslationWorkspaceAssetRef>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DialogueTranslationAssetIndexManifestRef {
    pub manifest_path: String,
    pub manifest_sha256: String,
    pub asset_count: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DialogueTranslationAssetIndexManifest {
    pub kind: String,
    pub asset_count: usize,
    pub assets: Vec<DialogueTranslationWorkspaceAssetRef>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DialogueTranslationWorkspaceAssetRef {
    pub source_path: String,
    pub manifest_path: String,
    pub manifest_sha256: String,
    pub semantic_group_count: usize,
    pub referenced_coordinate_count: usize,
    pub context_occurrence_count: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DialogueTranslationAssetManifest {
    pub kind: String,
    pub source_path: String,
    pub semantic_group_count: usize,
    pub referenced_coordinate_count: usize,
    pub context_occurrence_count: usize,
    pub roles: Vec<DialogueTranslationRoleManifestRef>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DialogueTranslationRoleManifestRef {
    pub role: DialogueTranslationWorkspaceRole,
    pub manifest_count: usize,
    pub manifests: Vec<DialogueTranslationRoleManifestFileRef>,
    pub shard_count: usize,
    pub entry_count: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DialogueTranslationRoleManifestFileRef {
    pub manifest_path: String,
    pub manifest_sha256: String,
    pub shard_count: usize,
    pub entry_count: usize,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum DialogueTranslationWorkspaceRole {
    Source,
    Context,
    Translation,
    Review,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DialogueTranslationRoleManifest {
    pub kind: String,
    pub role: DialogueTranslationWorkspaceRole,
    pub source_path: String,
    pub shard_count: usize,
    pub entry_count: usize,
    pub shards: Vec<DialogueTranslationShardRef>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DialogueTranslationShardRef {
    pub shard_id: String,
    pub path: String,
    pub entry_count: usize,
    pub content_sha256: Option<String>,
    pub source_shard_sha256: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DialogueTranslationSourceShard {
    pub kind: String,
    pub shard_id: String,
    pub source_bin_sha256: String,
    pub codebook_sha256: String,
    pub source_corpus_sha256: String,
    pub script_inventory_sha256: String,
    pub owner: DialogueTranslationRouteOwner,
    pub entries: Vec<DialogueTranslationSourceGroup>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
pub struct DialogueTranslationRouteOwner {
    pub source_path: String,
    pub bank_selector: usize,
    pub variant_selector: usize,
    pub route_table_offset: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DialogueTranslationSourceGroup {
    pub semantic_source_sha256: String,
    pub source_segments: Vec<String>,
    pub controls: Vec<DialogueTranslationControl>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub referenced_coordinate_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub referenced_coordinate_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unreferenced_duplicate_coordinate_ids: Vec<String>,
    #[serde(default)]
    pub unreferenced_duplicate_coordinate_count: usize,
    pub context_occurrence_count: usize,
}

impl DialogueTranslationSourceGroup {
    pub(super) fn coordinate_count(&self) -> usize {
        self.referenced_coordinate_count
            .unwrap_or(self.referenced_coordinate_ids.len())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DialogueTranslationContextShard {
    pub kind: String,
    pub shard_id: String,
    pub script_inventory_sha256: String,
    pub owner: DialogueTranslationRouteOwner,
    pub entries: Vec<DialogueTranslationContextEntry>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DialogueTranslationContextEntry {
    pub occurrence_id: String,
    pub segment_index: usize,
    pub message_order: usize,
    pub entrypoint: String,
    pub command_offset: String,
    pub opcode: String,
    pub entry_index: usize,
    pub coordinate_id: String,
    pub semantic_source_sha256: String,
    pub scene_identity: String,
    pub speaker_identity: String,
    pub listener_identity: String,
    pub semantic_evidence: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DialogueTranslationShard {
    pub kind: String,
    pub shard_id: String,
    pub source_shard_sha256: String,
    pub entries: Vec<DialogueTranslationDecision>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DialogueTranslationDecision {
    pub semantic_source_sha256: String,
    pub korean_segments: Vec<Option<String>>,
    pub status: DialogueTranslationDecisionStatus,
    pub translator: Option<String>,
    pub notes: String,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum DialogueTranslationDecisionStatus {
    Untranslated,
    Draft,
    ReadyForReview,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DialogueTranslationReviewShard {
    pub kind: String,
    pub shard_id: String,
    pub source_shard_sha256: String,
    pub translation_path: String,
    pub entries: Vec<DialogueTranslationReviewDecision>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DialogueTranslationReviewDecision {
    pub semantic_source_sha256: String,
    pub status: DialogueTranslationReviewStatus,
    pub reviewer: Option<String>,
    pub reviewed_at: Option<String>,
    pub reviewed_translation_sha256: Option<String>,
    pub notes: String,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum DialogueTranslationReviewStatus {
    Pending,
    ChangesRequested,
    Approved,
}

#[derive(Debug, Serialize)]
pub struct DialogueTranslationAssetRepertoire {
    pub source_path: String,
    pub korean_character_count: usize,
    pub korean_repertoire: String,
}

#[derive(Debug, Serialize)]
pub struct DialogueTranslationWorkspaceAuditReport {
    pub kind: String,
    pub source_bin_sha256: String,
    pub codebook_sha256: String,
    pub source_corpus_sha256: String,
    pub script_inventory_sha256: String,
    pub asset_count: usize,
    pub source_shard_count: usize,
    pub context_shard_count: usize,
    pub translation_shard_count: usize,
    pub review_shard_count: usize,
    pub semantic_group_count: usize,
    pub referenced_coordinate_count: usize,
    pub context_occurrence_count: usize,
    pub untranslated_group_count: usize,
    pub draft_group_count: usize,
    pub ready_for_review_group_count: usize,
    pub pending_review_group_count: usize,
    pub changes_requested_group_count: usize,
    pub approved_group_count: usize,
    pub korean_character_count: usize,
    pub korean_repertoire: String,
    pub asset_repertoires: Vec<DialogueTranslationAssetRepertoire>,
    pub largest_shard_entry_count: usize,
    pub largest_shard_byte_count: usize,
    pub ready_for_font_repertoire: bool,
    pub development_translation_input_available: bool,
    pub release_candidate_translation_input_eligible: bool,
    pub translation_project_complete: bool,
}
