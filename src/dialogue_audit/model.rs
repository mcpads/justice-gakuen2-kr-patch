use std::path::PathBuf;

use serde::Serialize;

#[derive(Debug, Clone)]
pub struct DialogueAuditConfig {
    pub cue: PathBuf,
    pub output: PathBuf,
}

#[derive(Debug, Clone)]
pub struct DialogueFontPreviewConfig {
    pub cue: PathBuf,
    pub output_dir: PathBuf,
}

#[derive(Debug, Serialize)]
pub struct DialogueFontPreviewAsset {
    pub source_path: String,
    pub fixed_cell_count: usize,
    pub columns: usize,
    pub rows: usize,
    pub first_code: String,
    pub last_code: String,
    pub png: String,
    pub png_sha256: String,
}

#[derive(Debug, Serialize)]
pub struct DialogueFontPreviewManifest {
    pub kind: String,
    pub implementation: String,
    pub source_bin_sha256: String,
    pub cell_width: usize,
    pub cell_height: usize,
    pub scale: usize,
    pub gutter: usize,
    pub columns: usize,
    pub coordinate_rule: String,
    pub assets: Vec<DialogueFontPreviewAsset>,
}

#[derive(Debug, Serialize)]
pub struct DialogueTokenAudit {
    pub kind: String,
    pub code: String,
    pub arguments: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct DialogueMessageAudit {
    pub index: usize,
    pub decoded_offset: String,
    pub runtime_address: String,
    pub byte_length: usize,
    pub sha256: String,
    pub raw_codes: Vec<String>,
    pub alignment_padding_word_count: usize,
    pub tokens: Vec<DialogueTokenAudit>,
}

#[derive(Debug, Serialize)]
pub struct DialogueBankAudit {
    pub selector_index: usize,
    pub message_data_start: String,
    pub message_data_end: String,
    pub pointer_table_offset: String,
    pub pointer_table_runtime_address: String,
    pub pointer_table_end: String,
    pub entry_count: usize,
    pub message_byte_count: usize,
    pub unique_entry_count: usize,
    pub entries: Vec<DialogueMessageAudit>,
}

#[derive(Debug, Serialize)]
pub struct DialogueAssetAudit {
    pub path: String,
    pub extent_lba: u32,
    pub stored_size: usize,
    pub stored_sha256: String,
    pub decoded_size: usize,
    pub decoded_sha256: String,
    pub bank_count: usize,
    pub entry_count: usize,
    pub message_byte_count: usize,
    pub unique_entry_count: usize,
    pub font: DialogueAssetFontAudit,
    pub banks: Vec<DialogueBankAudit>,
}

#[derive(Debug, Serialize)]
pub struct DialogueAssetFontAudit {
    pub tim_size: usize,
    pub clut_vram_x: u16,
    pub clut_vram_y: u16,
    pub clut_color_count: usize,
    pub image_vram_x: u16,
    pub image_vram_y: u16,
    pub pixel_data_offset: String,
    pub fixed_cell_count: usize,
    pub addressable_slot_count_before_selector: usize,
    pub runtime_extension_slot_count: usize,
    pub used_fixed_cell_count: usize,
    pub used_runtime_extension_slot_count: usize,
    pub fixed_glyph_occurrence_count: usize,
    pub runtime_extension_glyph_occurrence_count: usize,
    pub fixed_atlas_sha256: String,
    pub source_extension_start: String,
    pub source_extension_end: String,
    pub source_extension_sha256: String,
    pub source_extension_nonzero_byte_count: usize,
}

#[derive(Debug, Serialize)]
pub struct DialogueGlyphVariantAudit {
    pub cell_sha256: String,
    pub assets: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct DialogueGlyphCodeAudit {
    pub code: String,
    pub occurrence_count: usize,
    pub message_assets: Vec<String>,
    pub fixed_source_assets: Vec<String>,
    pub runtime_extension_source_assets: Vec<String>,
    pub source_cell_variants: Vec<DialogueGlyphVariantAudit>,
}

#[derive(Debug, Serialize)]
pub struct DialogueFontAudit {
    pub bits_per_pixel: usize,
    pub cell_width: usize,
    pub cell_height: usize,
    pub bytes_per_cell: usize,
    pub minimum_fixed_cell_count: usize,
    pub maximum_fixed_cell_count: usize,
    pub addressable_slot_count_before_selector: usize,
    pub minimum_runtime_extension_slot_count: usize,
    pub maximum_runtime_extension_slot_count: usize,
    pub used_glyph_code_count: usize,
    pub used_fixed_asset_code_pair_count: usize,
    pub unreferenced_fixed_asset_code_pair_count: usize,
    pub used_runtime_extension_asset_code_pair_count: usize,
    pub shared_identical_prefix_cell_count: usize,
    pub shared_identical_prefix_end_code: String,
    pub common_fixed_cells_identical_across_assets: usize,
    pub common_fixed_cells_with_asset_variants: usize,
    pub all_source_pixel_hash_count: usize,
    pub used_source_pixel_hash_count: usize,
    pub source_extension_zero_in_every_asset: bool,
    pub glyph_codes: Vec<DialogueGlyphCodeAudit>,
    pub limitations: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct DialogueTokenCodeAudit {
    pub code: String,
    pub kind: String,
    pub semantic_name: String,
    pub argument_word_count: usize,
    pub occurrence_count: usize,
    pub meaning_status: String,
}

#[derive(Debug, Serialize)]
pub struct DialogueAuditManifest {
    pub kind: String,
    pub implementation: String,
    pub source_bin_sha256: String,
    pub storage_format: String,
    pub decoded_runtime_base: String,
    pub decoded_image_size: usize,
    pub selector_table_offset: String,
    pub selector_slot_count: usize,
    pub asset_count: usize,
    pub bank_count: usize,
    pub entry_count: usize,
    pub message_byte_count: usize,
    pub unique_entry_count: usize,
    pub token_count: usize,
    pub glyph_occurrence_count: usize,
    pub fixed_glyph_occurrence_count: usize,
    pub runtime_extension_glyph_occurrence_count: usize,
    pub alignment_padding_word_count: usize,
    pub font: DialogueFontAudit,
    pub token_codes: Vec<DialogueTokenCodeAudit>,
    pub assets: Vec<DialogueAssetAudit>,
    pub limitations: Vec<String>,
}
