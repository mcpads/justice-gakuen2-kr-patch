use std::path::PathBuf;

use clap::ValueEnum;
use serde::{Deserialize, Serialize};

use super::translation_model::DialogueTranslationControl;
use super::translation_workspace_model::{
    DialogueTranslationDecisionStatus, DialogueTranslationReviewDecision,
    DialogueTranslationWorkspaceRole,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum DialogueSelectorTranslationScope {
    MgkDevelopment,
    AllRuntimeImages,
}

impl DialogueSelectorTranslationScope {
    pub fn target_scope(self) -> &'static str {
        match self {
            Self::MgkDevelopment => "mgk_selector_banks_2_through_6",
            Self::AllRuntimeImages => "all_runtime_image_selector_banks_2_through_6",
        }
    }

    pub fn from_target_scope(value: &str) -> Option<Self> {
        match value {
            "selector_banks_2_through_6" | "mgk_selector_banks_2_through_6" => {
                Some(Self::MgkDevelopment)
            }
            "all_runtime_image_selector_banks_2_through_6" => Some(Self::AllRuntimeImages),
            _ => None,
        }
    }

    pub fn source_asset_count(self) -> usize {
        match self {
            Self::MgkDevelopment => 10,
            Self::AllRuntimeImages => 78,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DialogueSelectorTranslationInitConfig {
    pub cue: PathBuf,
    pub codebook: PathBuf,
    pub output: PathBuf,
    pub force: bool,
    pub scope: DialogueSelectorTranslationScope,
}

#[derive(Debug, Clone)]
pub struct DialogueSelectorTranslationRefreshConfig {
    pub cue: PathBuf,
    pub codebook: PathBuf,
    pub previous: PathBuf,
    pub output: PathBuf,
    pub force: bool,
    pub scope: DialogueSelectorTranslationScope,
}

#[derive(Debug, Clone)]
pub struct DialogueSelectorTranslationAuditConfig {
    pub cue: PathBuf,
    pub codebook: PathBuf,
    pub translation: PathBuf,
    pub output: PathBuf,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DialogueSelectorTranslationWorkspaceManifest {
    pub kind: String,
    pub source_bin_sha256: String,
    pub codebook_sha256: String,
    pub source_corpus_sha256: String,
    pub selector_consumer_audit_sha256: String,
    pub target_scope: String,
    #[serde(default)]
    pub source_asset_count: usize,
    pub selector_count: usize,
    pub semantic_group_count: usize,
    pub coordinate_count: usize,
    pub authored_translation_target_group_count: usize,
    pub runtime_insertion_rewrite_group_count: usize,
    pub direct_entry_load_group_count: usize,
    pub direct_selector_load_group_count: usize,
    pub consumer_pending_group_count: usize,
    pub max_entries_per_shard: usize,
    pub max_bytes_per_shard: usize,
    pub selectors: Vec<DialogueSelectorTranslationSelectorRef>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DialogueSelectorTranslationAuditReport {
    pub kind: String,
    pub source_bin_sha256: String,
    pub codebook_sha256: String,
    pub source_corpus_sha256: String,
    pub selector_consumer_audit_sha256: String,
    pub target_scope: String,
    pub source_asset_count: usize,
    pub selector_count: usize,
    pub semantic_group_count: usize,
    pub coordinate_count: usize,
    pub authored_translation_target_group_count: usize,
    pub runtime_insertion_rewrite_group_count: usize,
    pub untranslated_group_count: usize,
    pub draft_group_count: usize,
    pub ready_for_review_group_count: usize,
    pub pending_review_group_count: usize,
    pub changes_requested_group_count: usize,
    pub approved_group_count: usize,
    pub development_authored_group_count: usize,
    pub development_full_selector_input_available: bool,
    pub release_candidate_selector_input_eligible: bool,
    pub largest_json_byte_count: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DialogueSelectorTranslationSelectorRef {
    pub canonical_selector: usize,
    pub manifest_path: String,
    pub manifest_sha256: String,
    pub semantic_group_count: usize,
    pub coordinate_count: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DialogueSelectorTranslationSelectorManifest {
    pub kind: String,
    pub canonical_selector: usize,
    pub semantic_group_count: usize,
    pub coordinate_count: usize,
    pub roles: Vec<DialogueSelectorTranslationRoleRef>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DialogueSelectorTranslationRoleRef {
    pub role: DialogueTranslationWorkspaceRole,
    pub manifest_count: usize,
    pub manifests: Vec<DialogueSelectorTranslationRoleManifestFileRef>,
    pub shard_count: usize,
    pub entry_count: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DialogueSelectorTranslationRoleManifestFileRef {
    pub manifest_path: String,
    pub manifest_sha256: String,
    pub shard_count: usize,
    pub entry_count: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DialogueSelectorTranslationRoleManifest {
    pub kind: String,
    pub canonical_selector: usize,
    pub role: DialogueTranslationWorkspaceRole,
    pub shard_count: usize,
    pub entry_count: usize,
    pub shards: Vec<DialogueSelectorTranslationShardRef>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DialogueSelectorTranslationShardRef {
    pub shard_id: String,
    pub path: String,
    pub entry_count: usize,
    pub content_sha256: Option<String>,
    pub source_shard_sha256: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DialogueSelectorTranslationSourceShard {
    pub kind: String,
    pub shard_id: String,
    pub source_bin_sha256: String,
    pub codebook_sha256: String,
    pub source_corpus_sha256: String,
    pub selector_consumer_audit_sha256: String,
    pub canonical_selector: usize,
    pub entries: Vec<DialogueSelectorTranslationSourceGroup>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DialogueSelectorTranslationSourceGroup {
    pub semantic_source_sha256: String,
    pub source_segments: Vec<String>,
    pub controls: Vec<DialogueTranslationControl>,
    pub target_selectors: Vec<usize>,
    #[serde(default)]
    pub coordinate_count: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub coordinate_ids: Vec<String>,
    pub consumer_evidence: DialogueSelectorConsumerEvidence,
    pub evidence_basis: Vec<String>,
    pub development_resolution: DialogueSelectorDevelopmentResolution,
    pub development_korean_segments: Option<Vec<String>>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum DialogueSelectorConsumerEvidence {
    ConsumerPending,
    DirectSelectorLoad,
    DirectEntryLoad,
    RuntimeInsertionRewrite,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DialogueSelectorDevelopmentResolution {
    AuthoredTranslation,
    RuntimeInsertionRewrite,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DialogueSelectorTranslationContextShard {
    pub kind: String,
    pub shard_id: String,
    pub source_corpus_sha256: String,
    pub selector_consumer_audit_sha256: String,
    pub canonical_selector: usize,
    pub entries: Vec<DialogueSelectorTranslationContextEntry>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DialogueSelectorTranslationContextEntry {
    pub occurrence_id: String,
    pub source_path: String,
    pub selector_index: usize,
    pub entry_index: usize,
    pub decoded_offset: String,
    pub runtime_address: String,
    pub coordinate_id: String,
    pub semantic_source_sha256: String,
    pub consumer_evidence: DialogueSelectorConsumerEvidence,
    pub evidence_basis: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DialogueSelectorTranslationShard {
    pub kind: String,
    pub shard_id: String,
    pub source_shard_sha256: String,
    pub entries: Vec<DialogueSelectorTranslationDecision>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DialogueSelectorTranslationDecision {
    pub semantic_source_sha256: String,
    pub korean_segments: Vec<Option<String>>,
    pub status: DialogueTranslationDecisionStatus,
    pub translator: Option<String>,
    pub notes: String,
    pub development_resolution: DialogueSelectorDevelopmentResolution,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DialogueSelectorTranslationReviewShard {
    pub kind: String,
    pub shard_id: String,
    pub source_shard_sha256: String,
    pub translation_path: String,
    pub entries: Vec<DialogueTranslationReviewDecision>,
}
