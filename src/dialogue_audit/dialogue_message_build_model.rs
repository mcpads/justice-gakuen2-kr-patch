use std::path::PathBuf;

use serde::Serialize;

use super::translation_model::DialogueDevelopmentInputPolicy;

#[derive(Debug, Clone)]
pub struct DialogueMessageBuildConfig {
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
    pub input_policy: DialogueDevelopmentInputPolicy,
}

#[derive(Debug, Serialize)]
pub struct DialogueMessageBuildReport {
    pub backup_slot_text: super::backup_slot_text::BackupSlotTextCounts,
    pub primary_layout: super::dialogue_message_layout::PrimaryDialogueLayoutBuildReport,
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
    pub compression_maximum_match_words: usize,
    pub compression_maximum_control_block_output_words: usize,
    pub message_arena_start: String,
    pub message_arena_end: String,
    pub asset_count: usize,
    pub translated_coordinate_count: usize,
    pub rewritten_runtime_insertion_coordinate_count: usize,
    pub preserved_untranslated_coordinate_count: usize,
    pub preserved_unreferenced_coordinate_count: usize,
    pub all_assets_parse_back: bool,
    pub all_assets_compress_within_original_extents: bool,
    pub assets: Vec<DialogueMessageBuildAsset>,
}

#[derive(Debug, Serialize)]
pub struct DialogueMessageBuildAsset {
    pub source_path: String,
    pub decoded_output_file: String,
    pub stored_output_file: String,
    pub original_decoded_sha256: String,
    pub rebuilt_decoded_sha256: String,
    pub original_stored_sha256: String,
    pub rebuilt_stored_sha256: String,
    pub original_stored_byte_count: usize,
    pub compressed_byte_count: usize,
    pub compression_maximum_match_words: usize,
    pub compression_maximum_control_block_output_words: usize,
    pub source_compression_control_blocks_crossing_input_pages: usize,
    pub rebuilt_compression_control_blocks_crossing_input_pages: usize,
    pub source_compression_stream_byte_count: usize,
    pub stored_padding_byte_count: usize,
    pub bank_count: usize,
    pub message_count: usize,
    pub translated_message_count: usize,
    pub rewritten_runtime_insertion_message_count: usize,
    pub preserved_untranslated_message_count: usize,
    pub preserved_unreferenced_message_count: usize,
    pub message_arena_used_byte_count: usize,
    pub message_arena_spare_byte_count: usize,
    pub parse_back_verified: bool,
    pub compression_roundtrip_verified: bool,
    pub compressed_within_original_extent: bool,
}
