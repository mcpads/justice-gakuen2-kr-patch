use std::path::PathBuf;

use serde::Serialize;

use super::translation_workspace_model::DialogueTranslationRouteOwner;

#[derive(Debug, Clone)]
pub struct DialogueFontConflictAuditConfig {
    pub cue: PathBuf,
    pub codebook: PathBuf,
    pub translation: PathBuf,
    pub selector_translation: PathBuf,
    pub name_input_keyboard: PathBuf,
    pub translation_audit_output: PathBuf,
    pub selector_translation_audit_output: PathBuf,
    pub code_allocation_output: PathBuf,
    pub output_dir: PathBuf,
    pub force: bool,
}

#[derive(Debug, Serialize)]
pub struct DialogueFontConflictAuditManifest {
    pub kind: String,
    pub source_bin_sha256: String,
    pub codebook_sha256: String,
    pub input_policy: String,
    pub source_asset_count: usize,
    pub conflict_asset_count: usize,
    pub conflicting_code_count: usize,
    pub required_untranslated_group_count: usize,
    pub single_group_owned_code_count: usize,
    pub all_conflicts_attributed: bool,
    pub prose_generated: bool,
    pub planning_order_basis: String,
    pub assets: Vec<DialogueFontConflictAssetRef>,
    pub group_indexes: Vec<DialogueFontConflictGroupIndexRef>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DialogueFontConflictGroupIndexRef {
    pub path: String,
    pub content_sha256: String,
    pub shard_count: usize,
    pub entry_count: usize,
}

#[derive(Debug, Serialize)]
pub struct DialogueFontConflictGroupIndex {
    pub kind: String,
    pub group_shards: Vec<DialogueFontConflictShardRef>,
}

#[derive(Debug, Serialize)]
pub struct DialogueFontConflictAssetRef {
    pub source_path: String,
    pub manifest_path: String,
    pub manifest_sha256: String,
    pub conflicting_code_count: usize,
    pub required_untranslated_group_count: usize,
    pub single_group_owned_code_count: usize,
}

#[derive(Debug, Serialize)]
pub struct DialogueFontConflictAssetManifest {
    pub kind: String,
    pub source_path: String,
    pub conflicting_code_count: usize,
    pub required_untranslated_group_count: usize,
    pub single_group_owned_code_count: usize,
    pub conflict_shards: Vec<DialogueFontConflictShardRef>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DialogueFontConflictShardRef {
    pub path: String,
    pub content_sha256: String,
    pub entry_count: usize,
}

#[derive(Debug, Serialize)]
pub struct DialogueFontConflictCodeShard {
    pub kind: String,
    pub source_path: String,
    pub entries: Vec<DialogueFontConflictCode>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DialogueFontConflictCode {
    pub code: String,
    pub replacement_character: String,
    pub preserved_source_character: String,
    pub preserved_coordinate_count: usize,
    pub preserved_glyph_occurrence_count: usize,
    pub required_group_count: usize,
    pub requirement_shards: Vec<DialogueFontConflictShardRef>,
    pub single_group_owned: bool,
}

#[derive(Debug, Serialize)]
pub struct DialogueFontConflictRequirementShard {
    pub kind: String,
    pub source_path: String,
    pub code: String,
    pub semantic_source_sha256: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct DialogueFontConflictGroupShard {
    pub kind: String,
    pub entries: Vec<DialogueFontConflictGroup>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DialogueFontConflictGroup {
    pub planning_rank: usize,
    pub semantic_source_sha256: String,
    pub translation_scope: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary_owner: Option<DialogueTranslationRouteOwner>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub canonical_selector: Option<usize>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub target_selectors: Vec<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selector_consumer_evidence: Option<String>,
    pub source_segment_count: usize,
    pub context_occurrence_count: usize,
    pub conflicting_asset_count: usize,
    pub conflicting_code_count: usize,
    pub single_group_owned_code_count: usize,
    pub assets: Vec<DialogueFontConflictGroupAsset>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DialogueFontConflictGroupAsset {
    pub source_path: String,
    pub conflicting_codes: Vec<String>,
    pub single_group_owned_codes: Vec<String>,
    pub preserved_coordinate_count: usize,
}
