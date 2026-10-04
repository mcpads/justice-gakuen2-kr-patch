use std::path::PathBuf;

use serde::Serialize;

#[derive(Debug, Clone)]
pub struct DialogueBuildCapacityAuditConfig {
    pub cue: PathBuf,
    pub codebook: PathBuf,
    pub translation: PathBuf,
    pub translation_audit_output: PathBuf,
    pub output: PathBuf,
}

#[derive(Debug, Serialize)]
pub struct DialogueBuildCapacityAuditReport {
    pub kind: String,
    pub source_bin_sha256: String,
    pub codebook_sha256: String,
    pub semantic_group_count: usize,
    pub coordinate_count: usize,
    pub graph_referenced_coordinate_count: usize,
    pub translated_coordinate_count: usize,
    pub translated_unreferenced_duplicate_coordinate_count: usize,
    pub preserved_unreferenced_coordinate_count: usize,
    pub development_translation_input_available: bool,
    pub release_candidate_translation_input_eligible: bool,
    pub fixed_bank_regions_fit: bool,
    pub relocated_bank_regions_fit: bool,
    pub total_original_message_byte_count: usize,
    pub total_rebuilt_message_byte_count: usize,
    pub total_fixed_region_shortfall_byte_count: usize,
    pub assets: Vec<DialogueBuildCapacityAssetAudit>,
}

#[derive(Debug, Serialize)]
pub struct DialogueBuildCapacityAssetAudit {
    pub source_path: String,
    pub bank_count: usize,
    pub message_count: usize,
    pub translated_message_count: usize,
    pub preserved_unreferenced_message_count: usize,
    pub original_message_byte_count: usize,
    pub rebuilt_message_byte_count: usize,
    pub fixed_bank_regions_fit: bool,
    pub fixed_region_shortfall_byte_count: usize,
    pub message_arena_start: String,
    pub message_arena_end: String,
    pub message_arena_byte_count: usize,
    pub source_zero_tail_byte_count: usize,
    pub source_zero_tail_nonzero_byte_count: usize,
    pub relocated_bank_regions_fit: bool,
    pub relocated_region_spare_byte_count: usize,
    pub banks: Vec<DialogueBuildCapacityBankAudit>,
}

#[derive(Debug, Serialize)]
pub struct DialogueBuildCapacityBankAudit {
    pub selector_index: usize,
    pub message_count: usize,
    pub translated_message_count: usize,
    pub preserved_unreferenced_message_count: usize,
    pub fixed_region_start: String,
    pub fixed_region_end: String,
    pub fixed_region_byte_count: usize,
    pub pointer_table_byte_count: usize,
    pub original_message_byte_count: usize,
    pub rebuilt_message_byte_count: usize,
    pub rebuilt_region_byte_count: usize,
    pub fixed_region_spare_byte_count: usize,
    pub fixed_region_shortfall_byte_count: usize,
    pub fits_fixed_region: bool,
}
