use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::tim::Cell;

#[derive(Debug, Clone)]
pub struct MenuTextureVariantBuildConfig {
    pub spec: PathBuf,
    pub force: bool,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct MenuTextureRestoreRegion {
    pub(super) role: String,
    pub(super) tim_offset: usize,
    pub(super) cell: Cell,
    #[serde(default)]
    pub(super) representation: MenuTextureRepresentation,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum MenuTextureRepresentation {
    #[default]
    Indexed4bppWithClut,
    Indexed4bppWithoutClut,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct MenuTextureVariantBuildSpec {
    pub(super) kind: String,
    pub(super) variant_id: String,
    pub(super) source_cue: PathBuf,
    pub(super) source_bin_sha256: String,
    pub(super) input_menu: PathBuf,
    pub(super) input_menu_sha256: String,
    pub(super) input_decoded_output: Option<PathBuf>,
    pub(super) output_decoded_output: Option<PathBuf>,
    pub(super) output_menu: PathBuf,
    #[serde(default)]
    pub(super) restored_regions: Vec<MenuTextureRestoreRegion>,
    pub(super) glyph_report: Option<MenuGlyphReportRestoreSpec>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct MenuGlyphReportRestoreSpec {
    pub(super) path: PathBuf,
    pub(super) sha256: String,
    pub(super) tim_offset: usize,
    #[serde(default)]
    pub(super) roles: Vec<String>,
    #[serde(default)]
    pub(super) code_ranges: Vec<[u16; 2]>,
    #[serde(default)]
    pub(super) representation: MenuTextureRepresentation,
}

#[derive(Debug, Serialize)]
pub struct MenuTextureVariantBuildReport {
    pub kind: String,
    pub variant_id: String,
    pub spec_path: String,
    pub spec_sha256: String,
    pub source_bin_sha256: String,
    pub source_menu_sha256: String,
    pub source_stream_byte_count: usize,
    pub source_control_blocks_crossing_input_pages: usize,
    pub input_menu_path: String,
    pub input_menu_sha256: String,
    pub input_stream_byte_count: usize,
    pub input_control_blocks_crossing_input_pages: usize,
    pub input_decoded_output_path: Option<String>,
    pub input_decoded_sha256: Option<String>,
    pub output_decoded_output_path: Option<String>,
    pub output_decoded_sha256: Option<String>,
    pub output_menu_path: String,
    pub output_menu_sha256: String,
    pub output_stream_byte_count: usize,
    pub output_control_blocks_crossing_input_pages: usize,
    pub restored_region_count: usize,
    pub restored_roles: Vec<String>,
    pub glyph_report: Option<MenuGlyphReportRestoreBuild>,
    pub decoded_changed_byte_ranges: Vec<[usize; 2]>,
    pub compression_roundtrip_verified: bool,
    pub release_eligible: bool,
}

#[derive(Debug, Serialize)]
pub struct MenuGlyphReportRestoreBuild {
    pub path: String,
    pub sha256: String,
    pub requested_roles: Vec<String>,
    pub requested_code_ranges: Vec<[u16; 2]>,
    pub restored_region_count: usize,
}
