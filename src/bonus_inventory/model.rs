use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::decoded_record_write_plan::DecodedDataClaim;
use crate::tim::Cell;

#[derive(Debug, Clone)]
pub struct BonusInventoryTextStyleSource {
    pub path: PathBuf,
    pub font_px: f32,
    pub tracking_px: f32,
    pub vertical_shift_px: i32,
}

#[derive(Debug, Clone)]
pub struct BonusInventoryFontSources {
    pub item_label: BonusInventoryTextStyleSource,
    pub device_text: BonusInventoryTextStyleSource,
    pub title: BonusInventoryTextStyleSource,
    pub compact_label: BonusInventoryTextStyleSource,
    pub large_label: BonusInventoryTextStyleSource,
    pub compact_action: BonusInventoryTextStyleSource,
    pub large_action: BonusInventoryTextStyleSource,
    pub help: BonusInventoryTextStyleSource,
    pub viewer_navigation: BonusInventoryTextStyleSource,
    pub viewer_card_placeholder: BonusInventoryTextStyleSource,
}

#[derive(Debug, Clone)]
pub struct BonusInventoryBuildConfig {
    pub cue: PathBuf,
    pub assets: PathBuf,
    pub fonts: BonusInventoryFontSources,
    pub build_spec_sha256: String,
    pub output_dir: PathBuf,
    pub force: bool,
}

pub struct BonusInventoryBuild {
    pub(crate) item_labels: super::item_labels::ItemLabelBuild,
    pub(crate) device_text: super::device_text::DeviceTextBuild,
    pub stored: Vec<u8>,
    pub decoded: Vec<u8>,
    pub(crate) decoded_write_claims: Vec<DecodedDataClaim>,
    pub build_manifest_sha256: String,
    pub report: BonusInventoryBuildReport,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Ord, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BonusInventoryFontRole {
    Title,
    CompactLabel,
    LargeLabel,
    CompactAction,
    LargeAction,
    Help,
    ViewerNavigation,
    ViewerCardPlaceholder,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DevelopmentStatus {
    Untranslated,
    Authored,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseStatus {
    Untranslated,
    NeedsHumanReview,
    Approved,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BonusInventoryManifest {
    pub(super) kind: String,
    pub(super) source: BonusInventorySourceBinding,
    pub(super) runtime_consumers: Vec<String>,
    pub(super) units: Vec<PathBuf>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BonusInventorySourceBinding {
    pub(super) path: String,
    pub(super) stored_sha256: String,
    pub(super) decoded_sha256: String,
    pub(super) fixed_ui_tim_offset: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BonusInventoryUnit {
    pub(super) kind: String,
    pub(super) id: String,
    pub(super) source_text: String,
    pub(super) korean_text: Option<String>,
    pub(super) occurrences: Vec<BonusInventoryOccurrence>,
    pub(super) development_status: DevelopmentStatus,
    pub(super) release_status: ReleaseStatus,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BonusInventoryOccurrence {
    pub(super) id: String,
    pub(super) font_role: Option<BonusInventoryFontRole>,
    pub(super) cell: Cell,
    pub(super) source_indexed_pixel_sha256: String,
}

#[derive(Debug, Serialize)]
pub struct BonusInventoryBuildReport {
    pub card_back: serde_json::Value,
    pub item_labels: super::item_labels::ItemLabelReport,
    pub device_text: serde_json::Value,
    pub kind: String,
    pub source_bin_sha256: String,
    pub source_path: String,
    pub source_stored_sha256: String,
    pub source_decoded_sha256: String,
    pub patched_stored_sha256: String,
    pub patched_decoded_sha256: String,
    pub build_spec_sha256: String,
    pub translation_manifest_sha256: String,
    pub fixed_unit_count: usize,
    pub authored_unit_count: usize,
    pub untranslated_unit_count: usize,
    pub occurrence_count: usize,
    pub authored_occurrence_count: usize,
    pub release_approved_unit_count: usize,
    pub source_regions_match: bool,
    pub occurrence_cells_are_unique_and_non_overlapping: bool,
    pub untranslated_regions_unchanged: bool,
    pub changes_confined_to_owned_cells: bool,
    pub complete_fixed_ui_localized: bool,
    pub development_input_available: bool,
    pub release_candidate_input_eligible: bool,
    pub unpadded_stored_size: usize,
    pub source_record_size: usize,
    pub source_compression_maximum_match_words: usize,
    pub source_compression_maximum_control_block_output_words: usize,
    pub rebuilt_compression_maximum_match_words: usize,
    pub rebuilt_compression_maximum_control_block_output_words: usize,
    pub fonts: Vec<BonusInventoryFontBuild>,
    pub units: Vec<BonusInventoryUnitBuild>,
}

#[derive(Debug, Serialize)]
pub struct BonusInventoryFontBuild {
    pub role: BonusInventoryFontRole,
    pub font_name: String,
    pub font_sha256: String,
    pub font_px: f32,
    pub tracking_px: f32,
    pub vertical_shift_px: i32,
}

#[derive(Debug, Serialize)]
pub struct BonusInventoryUnitBuild {
    pub id: String,
    pub source_text: String,
    pub korean_text: Option<String>,
    pub development_status: DevelopmentStatus,
    pub release_status: ReleaseStatus,
    pub occurrences: Vec<BonusInventoryOccurrenceBuild>,
}

#[derive(Debug, Serialize)]
pub struct BonusInventoryOccurrenceBuild {
    pub id: String,
    pub font_role: Option<BonusInventoryFontRole>,
    pub cell: Cell,
    pub source_indexed_pixel_sha256: String,
    pub output_indexed_pixel_sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub measured_advance_px: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ink_bounds: Option<[usize; 4]>,
    pub changed_decoded_byte_count: usize,
}
