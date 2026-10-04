use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::contextual_texture_upload::{ContextualMenuGlyph, ContextualMenuGlyphUploadReport};
use crate::decoded_record_write_plan::DecodedDataClaim;

#[derive(Debug, Clone)]
pub struct TitleMenuFontStyle {
    pub font: PathBuf,
    pub font_px: f32,
}

#[derive(Debug, Clone)]
pub struct TitleMenuBuildConfig {
    pub cue: PathBuf,
    pub assets: PathBuf,
    pub font: TitleMenuFontStyle,
    pub build_spec_sha256: String,
    pub output_dir: PathBuf,
    pub force: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TitleMenuManifest {
    pub kind: String,
    pub source_bin_sha256: String,
    pub menu_path: String,
    pub menu_stored_sha256: String,
    pub menu_decoded_sha256: String,
    pub overlay_path: String,
    pub overlay_stored_sha256: String,
    pub overlay_decoded_sha256: String,
    pub entries: Vec<TitleMenuAssetReference>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TitleMenuAssetReference {
    pub id: String,
    pub file: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TitleMenuTranslation {
    pub kind: String,
    pub id: String,
    pub source_offset: String,
    pub source_record_size: usize,
    pub source_record_sha256: String,
    pub source_codes: Vec<String>,
    pub source_text: String,
    pub korean_text: String,
    pub placement_record_offset: String,
    pub source_x: i16,
    pub source_y: i16,
    pub source_placement_sha256: String,
    pub development_status: DevelopmentStatus,
    pub release_status: ReleaseStatus,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DevelopmentStatus {
    Authored,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseStatus {
    NeedsHumanReview,
    Approved,
}

#[derive(Debug, Serialize)]
pub struct TitleMenuBuildReport {
    pub kind: String,
    pub build_spec_sha256: String,
    pub source_bin_sha256: String,
    pub source_menu_stored_sha256: String,
    pub source_menu_decoded_sha256: String,
    pub output_menu_stored_sha256: String,
    pub output_menu_decoded_sha256: String,
    pub source_overlay_stored_sha256: String,
    pub source_overlay_decoded_sha256: String,
    pub output_overlay_stored_sha256: String,
    pub output_overlay_decoded_sha256: String,
    pub translation_manifest_sha256: String,
    pub font_name: String,
    pub font_sha256: String,
    pub font_px: f32,
    pub entry_count: usize,
    pub release_approved_entry_count: usize,
    pub development_input_available: bool,
    pub release_candidate_input_eligible: bool,
    pub source_records_match: bool,
    pub source_placements_match: bool,
    pub changed_bytes_confined_to_owned_ranges: bool,
    pub runtime_catalog_prefix_matches: bool,
    pub allocation_status: String,
    pub allocation_proven_reclaimable: bool,
    pub provisional_codes: Vec<String>,
    pub menu_decoded_changed_byte_ranges: Vec<[usize; 2]>,
    pub overlay_changed_byte_ranges: Vec<[usize; 2]>,
    pub original_menu_stored_size: usize,
    pub rebuilt_menu_stored_size: usize,
    pub menu_padding_size: usize,
    pub source_compression_maximum_match_words: usize,
    pub source_compression_maximum_control_block_output_words: usize,
    pub rebuilt_compression_maximum_match_words: usize,
    pub rebuilt_compression_maximum_control_block_output_words: usize,
    pub original_overlay_stored_size: usize,
    pub rebuilt_overlay_stored_size: usize,
    pub overlay_padding_size: usize,
    pub source_overlay_compression_maximum_match_words: usize,
    pub source_overlay_compression_maximum_control_block_output_words: usize,
    pub source_overlay_control_blocks_crossing_input_pages: usize,
    pub source_overlay_stream_byte_count: usize,
    pub rebuilt_overlay_compression_maximum_match_words: usize,
    pub rebuilt_overlay_compression_maximum_control_block_output_words: usize,
    pub rebuilt_overlay_control_blocks_crossing_input_pages: usize,
    pub rebuilt_overlay_stream_byte_count: usize,
    pub runtime_glyph_upload: ContextualMenuGlyphUploadReport,
    pub glyphs: Vec<TitleMenuGlyphBuild>,
    pub entries: Vec<TitleMenuEntryBuild>,
    pub menu_output_file: String,
    pub overlay_output_file: String,
}

#[derive(Debug, Serialize)]
pub struct TitleMenuGlyphBuild {
    pub character: char,
    pub code: String,
    pub cell: crate::tim::Cell,
    pub ink_bounds: [usize; 4],
    pub install: crate::tim::GlyphInstallMetadata,
    pub global_menu_resident: bool,
    pub runtime_context: String,
    pub packed_payload_sha256: String,
}

#[derive(Debug, Serialize)]
pub struct TitleMenuEntryBuild {
    pub id: String,
    pub source_offset: String,
    pub source_text: String,
    pub korean_text: String,
    pub output_codes: Vec<String>,
    pub placement_record_offset: String,
    pub x: i16,
    pub y: i16,
    pub development_status: DevelopmentStatus,
    pub release_status: ReleaseStatus,
}

pub struct TitleMenuRecordBuild {
    pub source_menu_stored: Vec<u8>,
    pub source_menu_decoded: Vec<u8>,
    pub source_overlay_stored: Vec<u8>,
    pub source_overlay_decoded: Vec<u8>,
    pub menu_stored: Vec<u8>,
    pub menu_decoded: Vec<u8>,
    pub(crate) menu_write_claims: Vec<DecodedDataClaim>,
    pub overlay_stored: Vec<u8>,
    pub overlay_decoded: Vec<u8>,
    pub(crate) overlay_text_decoded: Vec<u8>,
    pub(crate) overlay_text_write_claims: Vec<DecodedDataClaim>,
    pub(crate) contextual_glyphs: Vec<ContextualMenuGlyph>,
    pub build_manifest_sha256: String,
    pub report: TitleMenuBuildReport,
}
