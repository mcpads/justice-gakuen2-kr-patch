use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::decoded_record_write_plan::DecodedDataClaim;
use crate::development_build_spec::SizedFontSource;
use crate::tim::Cell;

#[derive(Debug, Clone)]
pub struct ShopUiFontSources {
    pub heading: SizedFontSource,
    pub current_points: SizedFontSource,
    pub heading_tracking_px: f32,
    pub current_points_tracking_px: f32,
}

#[derive(Debug, Clone)]
pub struct ShopUiBuildConfig {
    pub cue: PathBuf,
    pub assets: PathBuf,
    pub fonts: ShopUiFontSources,
    pub build_spec_sha256: String,
    pub output_dir: PathBuf,
    pub force: bool,
}

pub struct ShopUiBuild {
    pub stored: Vec<u8>,
    pub decoded: Vec<u8>,
    pub(crate) decoded_write_claims: Vec<DecodedDataClaim>,
    pub build_manifest_sha256: String,
    pub report: ShopUiBuildReport,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Ord, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ShopUiFontRole {
    Heading,
    CurrentPoints,
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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ShopUiManifest {
    pub(super) kind: String,
    pub(super) source: ShopUiSourceBinding,
    pub(super) units: Vec<PathBuf>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ShopUiSourceBinding {
    pub(super) path: String,
    pub(super) stored_sha256: String,
    pub(super) decoded_sha256: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ShopUiUnit {
    pub(super) kind: String,
    pub(super) id: String,
    pub(super) source_text: String,
    pub(super) korean_text: String,
    pub(super) font_role: ShopUiFontRole,
    pub(super) tim_offset: String,
    pub(super) cell: Cell,
    pub(super) source_region_sha256: String,
    pub(super) development_status: DevelopmentStatus,
    pub(super) release_status: ReleaseStatus,
}

#[derive(Debug, Serialize)]
pub struct ShopUiBuildReport {
    pub kind: String,
    pub source_bin_sha256: String,
    pub source_path: String,
    pub source_stored_sha256: String,
    pub source_decoded_sha256: String,
    pub patched_stored_sha256: String,
    pub patched_decoded_sha256: String,
    pub build_spec_sha256: String,
    pub translation_manifest_sha256: String,
    pub unit_count: usize,
    pub release_approved_unit_count: usize,
    pub source_regions_match: bool,
    pub cells_are_unique_and_non_overlapping: bool,
    pub changed_bytes_confined_to_owned_cells: bool,
    pub development_input_available: bool,
    pub release_candidate_input_eligible: bool,
    pub unpadded_stored_size: usize,
    pub source_record_size: usize,
    pub source_compression_maximum_match_words: usize,
    pub source_compression_maximum_control_block_output_words: usize,
    pub rebuilt_compression_maximum_match_words: usize,
    pub rebuilt_compression_maximum_control_block_output_words: usize,
    pub fonts: Vec<ShopUiFontBuild>,
    pub units: Vec<ShopUiUnitBuild>,
}

#[derive(Debug, Serialize)]
pub struct ShopUiFontBuild {
    pub role: ShopUiFontRole,
    pub font_name: String,
    pub font_sha256: String,
    pub font_px: f32,
    pub tracking_px: f32,
}

#[derive(Debug, Serialize)]
pub struct ShopUiUnitBuild {
    pub id: String,
    pub source_text: String,
    pub korean_text: String,
    pub font_role: ShopUiFontRole,
    pub tim_offset: String,
    pub cell: Cell,
    pub source_region_sha256: String,
    pub development_status: DevelopmentStatus,
    pub release_status: ReleaseStatus,
    pub measured_advance_px: f32,
    pub ink_bounds: [usize; 4],
    pub changed_decoded_byte_count: usize,
}
