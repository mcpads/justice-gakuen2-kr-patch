use std::path::PathBuf;

use serde::Serialize;

use crate::name_input::NameGlyphConsumerLayout;

use super::dialogue_fixed_code_consumers_model::DialogueFixedCodeConsumerAuditReport;
use super::selector_consumers_model::DialogueSelectorConsumerReport;
use super::translation_model::DialogueDevelopmentInputPolicy;

#[derive(Debug, Clone)]
pub struct DialogueCodeAllocationConfig {
    pub cue: PathBuf,
    pub codebook: PathBuf,
    pub translation: PathBuf,
    pub selector_translation: PathBuf,
    pub name_input_keyboard: PathBuf,
    pub translation_audit_output: PathBuf,
    pub selector_translation_audit_output: PathBuf,
    pub output: PathBuf,
    pub input_policy: DialogueDevelopmentInputPolicy,
}

#[derive(Debug, Serialize)]
pub struct DialogueCodeAllocationReport {
    pub kind: String,
    pub source_bin_sha256: String,
    pub codebook_sha256: String,
    pub input_policy: String,
    pub semantic_group_count: usize,
    pub development_authored_group_count: usize,
    pub selector_semantic_group_count: usize,
    pub selector_development_authored_group_count: usize,
    pub selector_development_full_input_available: bool,
    pub release_candidate_selector_input_eligible: bool,
    pub development_build_input_available: bool,
    pub development_translation_input_available: bool,
    pub release_candidate_translation_input_eligible: bool,
    pub addressable_code_count: usize,
    pub asset_local_code_count: usize,
    pub global_name_code_start: String,
    pub global_name_code_end: String,
    pub global_name_code_count: usize,
    pub name_input_keyboard_sha256: String,
    pub name_glyph_layout_complete: bool,
    pub name_glyph_layout_release_candidate_eligible: bool,
    pub name_glyph_layout: NameGlyphConsumerLayout,
    pub direct_selector_consumers: DialogueSelectorConsumerReport,
    pub fixed_code_consumers: DialogueFixedCodeConsumerAuditReport,
    pub fixed_code_consumer_ownership_complete: bool,
    pub static_allocation_complete: bool,
    pub assets: Vec<DialogueCodeAllocationAsset>,
}

#[derive(Debug, Serialize, serde::Deserialize)]
pub struct DialogueCodeAllocationAsset {
    pub source_path: String,
    pub fixed_cell_count: usize,
    pub addressable_code_count: usize,
    pub protected_source_glyph_code_count: usize,
    pub translated_coordinate_count: usize,
    pub static_character_count: usize,
    pub known_runtime_character_count: usize,
    pub required_character_count: usize,
    pub reusable_source_assignment_count: usize,
    pub global_name_assignment_reuse_count: usize,
    pub replaced_fixed_cell_count: usize,
    pub extension_cell_install_count: usize,
    pub unused_asset_local_code_count: usize,
    pub unresolved_runtime_sources: Vec<String>,
    pub assignments: Vec<DialogueCharacterCodeAssignment>,
}

#[derive(Debug, Serialize, serde::Deserialize)]
pub struct DialogueCharacterCodeAssignment {
    pub character: String,
    pub code: String,
    pub source_glyph_reused: bool,
    pub global_name_glyph_reused: bool,
    pub requires_glyph_install: bool,
    pub target_was_fixed_source_cell: bool,
    pub target_source_glyph_protected: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_source_cell_sha256: Option<String>,
}
