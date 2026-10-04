use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::decoded_record_write_plan::DecodedDataClaim;
use crate::font::HorizontalTextAlignment;
use crate::tim::Cell;

#[derive(Debug, Clone)]
pub struct ModeSelectBuildConfig {
    pub cue: PathBuf,
    pub assets: PathBuf,
    pub fonts: ModeSelectFontSources,
    pub output_dir: PathBuf,
    pub force: bool,
}

#[derive(Debug, Clone)]
pub struct ModeSelectFontSources {
    pub artwork: PathBuf,
    pub fixed_label: PathBuf,
    pub list_label: PathBuf,
    pub detail_title: PathBuf,
    pub description: PathBuf,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ModeSelectManifest {
    pub kind: String,
    pub source_bin_sha256: String,
    pub menu_path: String,
    pub menu_stored_sha256: String,
    pub menu_decoded_sha256: String,
    pub overlay_path: String,
    pub overlay_sha256: String,
    pub atlas_tim_offset: usize,
    pub fixed_labels: Vec<ModeSelectFixedAssetReference>,
    pub modes: Vec<ModeSelectAssetReference>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ModeSelectFixedAssetReference {
    pub id: String,
    pub file: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ModeSelectAssetReference {
    pub mode_index: usize,
    pub panel_index: usize,
    pub id: String,
    pub file: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ModeSelectTranslation {
    pub kind: String,
    pub id: String,
    pub mode_index: usize,
    pub panel_index: usize,
    pub source_title: String,
    pub source_description_lines: Vec<String>,
    pub korean_title: String,
    pub korean_description_lines: Vec<String>,
    pub development_status: DevelopmentStatus,
    pub release_status: ReleaseStatus,
    pub title_font_px: f32,
    pub description_font_px: f32,
    pub description_line_height: usize,
    pub list_label: BoundIndexedRegion,
    pub detail_title: BoundIndexedRegion,
    pub description: BoundIndexedRegion,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ModeSelectFixedTranslation {
    pub kind: String,
    pub id: String,
    pub source_text: String,
    pub korean_text: String,
    pub development_status: DevelopmentStatus,
    pub release_status: ReleaseStatus,
    pub font_px: f32,
    pub region: BoundIndexedRegion,
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

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BoundIndexedRegion {
    pub tim_offset: usize,
    pub cell: Cell,
    pub source_region_sha256: String,
}

#[derive(Debug, Serialize)]
pub struct ModeSelectBuildReport {
    pub logo: serde_json::Value,
    pub images: super::images::ImageFamilyBuild,
    pub kind: String,
    pub source_bin_sha256: String,
    pub source_menu_path: String,
    pub source_menu_stored_sha256: String,
    pub source_menu_decoded_sha256: String,
    pub source_overlay_path: String,
    pub source_overlay_sha256: String,
    pub manifest_sha256: String,
    pub fonts: Vec<ModeSelectFontBuild>,
    pub mode_count: usize,
    pub fixed_label_count: usize,
    pub authored_mode_count: usize,
    pub release_approved_mode_count: usize,
    pub release_approved_fixed_label_count: usize,
    pub development_input_available: bool,
    pub release_candidate_input_eligible: bool,
    pub source_regions_match: bool,
    pub source_regions_are_disjoint: bool,
    pub decoded_changed_byte_count: usize,
    pub decoded_changed_byte_ranges: Vec<[usize; 2]>,
    pub compression_maximum_match_words: usize,
    pub compression_maximum_control_block_output_words: usize,
    pub compression_control_blocks_crossing_input_pages: usize,
    pub compression_stream_byte_count: usize,
    pub rebuilt_compression_maximum_match_words: usize,
    pub rebuilt_compression_maximum_control_block_output_words: usize,
    pub rebuilt_compression_control_blocks_crossing_input_pages: usize,
    pub rebuilt_compression_stream_byte_count: usize,
    pub rebuilt_decoded_sha256: String,
    pub rebuilt_stored_size: usize,
    pub original_stored_size: usize,
    pub padding_size: usize,
    pub padded_stored_sha256: String,
    pub stored_output_file: String,
    pub modes: Vec<ModeSelectModeBuild>,
    pub fixed_labels: Vec<ModeSelectFixedLabelBuild>,
}

pub(crate) struct ModeSelectRecordBuild {
    pub(crate) menu_decoded: Vec<u8>,
    pub(crate) menu_write_claims: Vec<DecodedDataClaim>,
    pub(crate) build_manifest_sha256: String,
    pub(crate) report: ModeSelectBuildReport,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ModeSelectFontBuild {
    pub role: String,
    pub font_name: String,
    pub font_sha256: String,
}

#[derive(Debug, Serialize)]
pub struct ModeSelectFixedLabelBuild {
    pub id: String,
    pub source_text: String,
    pub korean_text: String,
    pub development_status: DevelopmentStatus,
    pub release_status: ReleaseStatus,
    pub font_px: f32,
    pub cell: Cell,
    pub measured_advance_px: f32,
}

#[derive(Debug, Serialize)]
pub struct ModeSelectModeBuild {
    pub mode_index: usize,
    pub panel_index: usize,
    pub id: String,
    pub source_title: String,
    pub korean_title: String,
    pub source_description_lines: Vec<String>,
    pub korean_description_lines: Vec<String>,
    pub development_status: DevelopmentStatus,
    pub release_status: ReleaseStatus,
    pub title_font_px: f32,
    pub description_font_px: f32,
    pub list_label_cell: Cell,
    pub detail_title_cell: Cell,
    pub description_tim_offset: String,
    pub description_line_cells: Vec<Cell>,
    pub title_measured_advance_px: f32,
    pub description_measured_advance_px: Vec<f32>,
}

pub(super) const TITLE_ALIGNMENT: HorizontalTextAlignment = HorizontalTextAlignment::Center;
pub(super) const DESCRIPTION_ALIGNMENT: HorizontalTextAlignment = HorizontalTextAlignment::Left;
