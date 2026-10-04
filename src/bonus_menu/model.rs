use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::development_build_spec::SizedFontSource;
use crate::tim::Cell;

#[derive(Debug, Clone)]
pub struct BonusMenuFontSources {
    pub heading: SizedFontSource,
    pub entry: SizedFontSource,
    pub compact_entry: SizedFontSource,
}

#[derive(Debug, Clone)]
pub struct BonusMenuBuildConfig {
    pub cue: PathBuf,
    pub assets: PathBuf,
    pub fonts: BonusMenuFontSources,
    pub build_spec_sha256: String,
    pub output_dir: PathBuf,
    pub force: bool,
}

pub struct BonusMenuBuild {
    pub stored: Vec<u8>,
    pub decoded: Vec<u8>,
    pub build_manifest_sha256: String,
    pub report: BonusMenuBuildReport,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Ord, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BonusMenuFontRole {
    Heading,
    Entry,
    CompactEntry,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DevelopmentStatus {
    Untranslated,
    Authored,
    SuppressedDuplicate,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseStatus {
    Untranslated,
    NeedsHumanReview,
    Approved,
    NotApplicable,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BonusMenuManifest {
    pub(super) kind: String,
    pub(super) source: BonusMenuSourceBinding,
    pub(super) units: Vec<PathBuf>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BonusMenuSourceBinding {
    pub(super) path: String,
    pub(super) stored_sha256: String,
    pub(super) decoded_sha256: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BonusMenuUnit {
    pub(super) kind: String,
    pub(super) id: String,
    pub(super) source_text: String,
    pub(super) korean_text: Option<String>,
    pub(super) font_role: BonusMenuFontRole,
    pub(super) bits_per_pixel: u8,
    pub(super) tim_offset: String,
    pub(super) cell: Cell,
    pub(super) source_region_sha256: String,
    pub(super) development_status: DevelopmentStatus,
    pub(super) release_status: ReleaseStatus,
}

#[derive(Debug, Serialize)]
pub struct BonusMenuBuildReport {
    pub kind: String,
    pub source_bin_sha256: String,
    pub source_path: String,
    pub source_stored_sha256: String,
    pub source_decoded_sha256: String,
    pub patched_stored_sha256: String,
    pub patched_decoded_sha256: String,
    pub build_spec_sha256: String,
    pub translation_manifest_sha256: String,
    pub source_unit_count: usize,
    pub authored_unit_count: usize,
    pub suppressed_duplicate_unit_count: usize,
    pub untranslated_unit_count: usize,
    pub release_approved_unit_count: usize,
    pub source_regions_match: bool,
    pub authored_cells_are_unique_and_non_overlapping: bool,
    pub untranslated_regions_unchanged: bool,
    pub source_palette_unchanged: bool,
    pub heading_background_preserved_outside_cleanup: bool,
    pub changed_bytes_confined_to_owned_cells: bool,
    pub complete_surface_localized: bool,
    pub development_input_available: bool,
    pub release_candidate_input_eligible: bool,
    pub unpadded_stored_size: usize,
    pub source_record_size: usize,
    pub source_compression_maximum_match_words: usize,
    pub source_compression_maximum_control_block_output_words: usize,
    pub rebuilt_compression_maximum_match_words: usize,
    pub rebuilt_compression_maximum_control_block_output_words: usize,
    pub fonts: Vec<BonusMenuFontBuild>,
    pub units: Vec<BonusMenuUnitBuild>,
}

#[derive(Debug, Serialize)]
pub struct BonusMenuFontBuild {
    pub role: BonusMenuFontRole,
    pub font_name: String,
    pub font_sha256: String,
    pub font_px: f32,
}

#[derive(Debug, Serialize)]
pub struct BonusMenuUnitBuild {
    pub id: String,
    pub source_text: String,
    pub korean_text: Option<String>,
    pub font_role: BonusMenuFontRole,
    pub bits_per_pixel: u8,
    pub tim_offset: String,
    pub cell: Cell,
    pub source_region_sha256: String,
    pub development_status: DevelopmentStatus,
    pub release_status: ReleaseStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub measured_advance_px: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ink_bounds: Option<[usize; 4]>,
    pub changed_decoded_byte_count: usize,
}
