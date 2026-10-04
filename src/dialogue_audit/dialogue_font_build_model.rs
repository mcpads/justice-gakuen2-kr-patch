use std::path::PathBuf;

use serde::Serialize;

use crate::name_input::{NameGlyphBandBuildReport, NameGlyphMaterializationBundle};

use super::translation_model::DialogueDevelopmentInputPolicy;

#[derive(Debug, Clone)]
pub struct DialogueFontBuildConfig {
    pub prepared_dialogue: PathBuf,
    pub cue: PathBuf,
    pub codebook: PathBuf,
    pub translation: PathBuf,
    pub selector_translation: PathBuf,
    pub name_input_keyboard: PathBuf,
    pub name_glyph_font: PathBuf,
    pub name_glyph_font_px: f32,
    pub font: PathBuf,
    pub font_px: f32,
    pub input_policy: DialogueDevelopmentInputPolicy,
    pub output_dir: PathBuf,
    pub force: bool,
}

#[derive(Debug)]
pub struct DialogueFontBuild {
    pub records: Vec<DialogueFontRecordBuild>,
    pub manifest_sha256: String,
    pub report: DialogueFontBuildReport,
    pub(crate) name_glyph_materialization: NameGlyphMaterializationBundle,
}

#[derive(Debug)]
pub struct DialogueFontRecordBuild {
    pub source_path: String,
    pub stored: Vec<u8>,
}

#[derive(Debug, Serialize)]
pub struct DialogueFontBuildReport {
    pub prepared_dialogue_sha256: String,
    pub stat_result_layout: super::stat_result_layout::StatResultLayout,
    pub backup_slot_text: super::backup_slot_text::BackupSlotTextCounts,
    pub primary_layout: super::dialogue_message_layout::PrimaryDialogueLayoutBuildReport,
    pub kind: String,
    pub source_bin_sha256: String,
    pub codebook_sha256: String,
    pub input_policy: String,
    pub font_name: String,
    pub font_sha256: String,
    pub font_px: f32,
    pub name_glyph_font_name: String,
    pub name_glyph_font_sha256: String,
    pub name_glyph_font_px: f32,
    pub name_glyph_pack: NameGlyphBandBuildReport,
    pub outline_palette_index: u8,
    pub fill_palette_index: u8,
    pub rasterized_character_count: usize,
    pub selector_development_authored_group_count: usize,
    pub development_build_input_available: bool,
    pub development_translation_input_available: bool,
    pub release_candidate_translation_input_eligible: bool,
    pub compression_maximum_match_words: usize,
    pub compression_maximum_control_block_output_words: usize,
    pub fixed_code_consumer_ownership_complete: bool,
    pub source_asset_count: usize,
    pub asset_count: usize,
    pub skipped_asset_count: usize,
    pub skipped_assets: Vec<DialogueFontBuildSkippedAsset>,
    pub installed_local_glyph_count: usize,
    pub installed_shared_name_cell_count: usize,
    pub all_message_regions_preserved: bool,
    pub all_assets_compress_within_original_extents: bool,
    pub assets: Vec<DialogueFontBuildAsset>,
}

#[derive(Debug, Serialize)]
pub struct DialogueFontBuildSkippedAsset {
    pub source_path: String,
    pub reason: String,
}

#[derive(Debug, Serialize)]
pub struct DialogueFontBuildAsset {
    pub source_path: String,
    pub source_stored_sha256: String,
    pub message_decoded_sha256: String,
    pub font_installed_decoded_sha256: String,
    pub font_installed_stored_sha256: String,
    pub decoded_output_file: String,
    pub stored_output_file: String,
    pub installed_code_map_sha256: String,
    pub installed_local_glyph_count: usize,
    pub installed_shared_name_cell_count: usize,
    pub installed_fixed_cell_count: usize,
    pub installed_extension_cell_count: usize,
    pub changed_decoded_byte_count: usize,
    pub font_write_decoded_byte_range: [usize; 2],
    pub message_region_preserved: bool,
    pub compressed_byte_count: usize,
    pub compression_maximum_match_words: usize,
    pub compression_maximum_control_block_output_words: usize,
    pub source_compression_control_blocks_crossing_input_pages: usize,
    pub rebuilt_compression_control_blocks_crossing_input_pages: usize,
    pub source_compression_stream_byte_count: usize,
    pub stored_padding_byte_count: usize,
    pub original_stored_byte_count: usize,
    pub compression_roundtrip_verified: bool,
    pub compressed_within_original_extent: bool,
}
