use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::selector_translation_model::DialogueSelectorDevelopmentResolution;
use super::translation_model::DialogueTranslationControl;
use super::translation_workspace_model::{
    DialogueTranslationDecisionStatus, DialogueTranslationRouteOwner,
};

#[derive(Debug, Clone)]
pub struct DialogueTranslationAssetSyncConfig {
    pub primary_workspace: PathBuf,
    pub selector_workspace: PathBuf,
    pub output: PathBuf,
    pub force: bool,
}

#[derive(Debug, Clone)]
pub struct DialogueTranslationAssetAuditConfig {
    pub input: PathBuf,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DialogueTranslationAssetReport {
    pub kind: String,
    pub primary_owner_count: usize,
    pub selector_owner_count: usize,
    pub primary_shard_count: usize,
    pub selector_shard_count: usize,
    pub primary_decision_count: usize,
    pub selector_decision_count: usize,
    pub semantic_decision_count: usize,
    pub untranslated_decision_count: usize,
    pub draft_decision_count: usize,
    pub ready_for_review_decision_count: usize,
    pub largest_json_byte_count: usize,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub(super) enum DialogueTranslationAssetFamily {
    Primary,
    Selector,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DialogueTranslationAssetManifest {
    pub kind: String,
    pub source_bin_sha256: String,
    pub codebook_sha256: String,
    pub source_corpus_sha256: String,
    pub primary_script_inventory_sha256: String,
    pub selector_consumer_audit_sha256: String,
    pub primary_target_scope: String,
    pub selector_target_scope: String,
    pub primary_source_asset_count: usize,
    pub primary_owner_count: usize,
    pub selector_owner_count: usize,
    pub primary_shard_count: usize,
    pub selector_shard_count: usize,
    pub primary_decision_count: usize,
    pub selector_decision_count: usize,
    pub semantic_decision_count: usize,
    pub untranslated_decision_count: usize,
    pub draft_decision_count: usize,
    pub ready_for_review_decision_count: usize,
    pub owner_index_manifest_count: usize,
    pub owner_index_manifests: Vec<DialogueTranslationOwnerIndexRef>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DialogueTranslationOwnerIndexRef {
    pub family: DialogueTranslationAssetFamily,
    pub manifest_path: String,
    pub manifest_sha256: String,
    pub owner_count: usize,
    pub shard_count: usize,
    pub decision_count: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DialogueTranslationOwnerIndexManifest {
    pub kind: String,
    pub family: DialogueTranslationAssetFamily,
    pub owner_count: usize,
    pub shard_count: usize,
    pub decision_count: usize,
    pub owners: Vec<DialogueTranslationOwnerRef>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DialogueTranslationOwnerRef {
    pub family: DialogueTranslationAssetFamily,
    pub owner_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub canonical_selector: Option<usize>,
    pub manifest_count: usize,
    pub manifests: Vec<DialogueTranslationOwnerManifestRef>,
    pub shard_count: usize,
    pub decision_count: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DialogueTranslationOwnerManifestRef {
    pub manifest_path: String,
    pub manifest_sha256: String,
    pub shard_count: usize,
    pub decision_count: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DialogueTranslationOwnerManifest {
    pub kind: String,
    pub family: DialogueTranslationAssetFamily,
    pub owner_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub canonical_selector: Option<usize>,
    pub shard_count: usize,
    pub decision_count: usize,
    pub shards: Vec<DialogueTranslationAssetShardRef>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DialogueTranslationAssetShardRef {
    pub shard_id: String,
    pub path: String,
    pub content_sha256: String,
    pub source_shard_sha256: String,
    pub decision_count: usize,
    pub untranslated_decision_count: usize,
    pub draft_decision_count: usize,
    pub ready_for_review_decision_count: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TrackedPrimaryDialogueTranslationShard {
    pub kind: String,
    pub shard_id: String,
    pub source_shard_sha256: String,
    pub owner: DialogueTranslationRouteOwner,
    pub entries: Vec<TrackedPrimaryDialogueTranslationEntry>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TrackedPrimaryDialogueTranslationEntry {
    pub semantic_source_sha256: String,
    pub source_segments: Vec<String>,
    pub controls: Vec<DialogueTranslationControl>,
    pub korean_segments: Vec<Option<String>>,
    pub status: DialogueTranslationDecisionStatus,
    pub translator: Option<String>,
    pub notes: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TrackedSelectorDialogueTranslationShard {
    pub kind: String,
    pub shard_id: String,
    pub source_shard_sha256: String,
    pub canonical_selector: usize,
    pub entries: Vec<TrackedSelectorDialogueTranslationEntry>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TrackedSelectorDialogueTranslationEntry {
    pub semantic_source_sha256: String,
    pub source_segments: Vec<String>,
    pub controls: Vec<DialogueTranslationControl>,
    pub korean_segments: Vec<Option<String>>,
    pub status: DialogueTranslationDecisionStatus,
    pub translator: Option<String>,
    pub notes: String,
    pub target_selectors: Vec<usize>,
    pub development_resolution: DialogueSelectorDevelopmentResolution,
}
