use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct MenuCompositionProbeBuildConfig {
    pub spec: PathBuf,
    pub force: bool,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum MenuContributorEncoding {
    CompressedRecord,
    DecodedImage,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct MenuCompositionContributorSpec {
    pub(super) role: String,
    pub(super) path: PathBuf,
    pub(super) sha256: String,
    pub(super) encoding: MenuContributorEncoding,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct MenuCompositionProbeBuildSpec {
    pub(super) kind: String,
    pub(super) probe_id: String,
    pub(super) source_cue: PathBuf,
    pub(super) source_bin_sha256: String,
    pub(super) contributors: Vec<MenuCompositionContributorSpec>,
    pub(super) output_menu: PathBuf,
}

#[derive(Debug, Serialize)]
pub struct MenuCompositionContributorBuild {
    pub role: String,
    pub path: String,
    pub input_sha256: String,
    pub encoding: String,
    pub decoded_sha256: String,
    pub changed_byte_ranges: Vec<[usize; 2]>,
}

#[derive(Debug, Serialize)]
pub struct MenuCompositionProbeBuildReport {
    pub kind: String,
    pub probe_id: String,
    pub spec_path: String,
    pub spec_sha256: String,
    pub source_bin_sha256: String,
    pub source_menu_sha256: String,
    pub source_decoded_sha256: String,
    pub source_stream_byte_count: usize,
    pub source_control_blocks_crossing_input_pages: usize,
    pub output_menu_path: String,
    pub output_menu_sha256: String,
    pub output_decoded_sha256: String,
    pub output_stream_byte_count: usize,
    pub output_control_blocks_crossing_input_pages: usize,
    pub changed_byte_ranges: Vec<[usize; 2]>,
    pub contributors: Vec<MenuCompositionContributorBuild>,
    pub compression_roundtrip_verified: bool,
    pub release_eligible: bool,
}
