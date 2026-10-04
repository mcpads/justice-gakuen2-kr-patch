use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::tim::Cell;

pub type BonusInventoryActionLabelFontSource =
    crate::bonus_inventory::BonusInventoryTextStyleSource;

#[derive(Debug, Clone)]
pub struct BonusInventoryActionLabelBuildConfig {
    pub cue: PathBuf,
    pub assets: PathBuf,
    pub font: BonusInventoryActionLabelFontSource,
    pub build_spec_sha256: String,
    pub output_dir: PathBuf,
    pub force: bool,
}

pub struct BonusInventoryActionLabelBuild {
    pub(super) glyphs: Vec<ActionGlyphPixels>,
    pub(super) units: Vec<ActionLabelUnit>,
    pub overlay: Vec<u8>,
    pub build_manifest_sha256: String,
    pub report: BonusInventoryActionLabelBuildReport,
}

pub(super) struct ActionGlyphPixels {
    pub(super) code: u16,
    pub(super) cell: Cell,
    pub(super) pixels: Vec<u8>,
    pub(super) indexed_sha256: String,
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
pub(super) struct ActionLabelManifest {
    pub(super) kind: String,
    pub(super) source: ActionLabelSourceBinding,
    pub(super) runtime_consumer: String,
    pub(super) glyphs: Vec<ActionGlyphAsset>,
    pub(super) units: Vec<PathBuf>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ActionLabelSourceBinding {
    pub(super) inventory_path: String,
    pub(super) inventory_stored_sha256: String,
    pub(super) inventory_decoded_sha256: String,
    pub(super) glyph_tim_offset: String,
    pub(super) overlay_path: String,
    pub(super) overlay_sha256: String,
    pub(super) pointer_table_offset: String,
    pub(super) secondary_parser_runtime_address: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ActionGlyphAsset {
    pub(super) text: String,
    pub(super) code: String,
    pub(super) cell: Cell,
    pub(super) source_indexed_pixel_sha256: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ActionLabelUnit {
    pub(super) kind: String,
    pub(super) id: String,
    pub(super) source_text: String,
    pub(super) korean_text: Option<String>,
    pub(super) source: ActionSequenceBinding,
    pub(super) development_status: DevelopmentStatus,
    pub(super) release_status: ReleaseStatus,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ActionSequenceBinding {
    pub(super) sequence_offset: String,
    pub(super) terminator_offset: String,
    pub(super) pointer_storage_offset: String,
    pub(super) encoded_sha256: String,
}

#[derive(Debug, Serialize)]
pub struct BonusInventoryActionLabelBuildReport {
    pub kind: String,
    pub source_bin_sha256: String,
    pub build_spec_sha256: String,
    pub translation_manifest_sha256: String,
    pub translation_unit_sha256s: Vec<String>,
    pub source_inventory_path: String,
    pub source_inventory_stored_sha256: String,
    pub source_inventory_decoded_sha256: String,
    pub source_overlay_path: String,
    pub source_overlay_sha256: String,
    pub output_overlay_sha256: String,
    pub runtime_consumer: String,
    pub secondary_parser_runtime_address: String,
    pub statically_bound_caller_runtime_addresses: Vec<String>,
    pub runtime_observed_return_address: String,
    pub source_pointer_table_offset: String,
    pub glyphs: Vec<ActionGlyphBuildReport>,
    pub units: Vec<ActionLabelUnitBuildReport>,
    pub inventory_expected_write_ranges: Vec<[usize; 2]>,
    pub inventory_changed_byte_ranges: Vec<[usize; 2]>,
    pub overlay_expected_write_ranges: Vec<[usize; 2]>,
    pub overlay_changed_byte_ranges: Vec<[usize; 2]>,
    pub source_regions_match: bool,
    pub allocated_glyphs_are_unreferenced_by_source_consumers: bool,
    pub changes_confined_to_owned_regions: bool,
    pub font_name: String,
    pub font_sha256: String,
    pub font_px: f32,
    pub tracking_px: f32,
    pub vertical_shift_px: i32,
    pub development_input_available: bool,
    pub release_candidate_input_eligible: bool,
    pub runtime_verification_required: bool,
}

#[derive(Debug, Serialize)]
pub struct ActionGlyphBuildReport {
    pub text: String,
    pub code: String,
    pub cell: Cell,
    pub source_indexed_pixel_sha256: String,
    pub output_indexed_pixel_sha256: String,
    pub measured_advance_px: f32,
    pub ink_bounds: [usize; 4],
    pub allowed_decoded_byte_ranges: Vec<[usize; 2]>,
    pub changed_decoded_byte_count: usize,
}

#[derive(Debug, Serialize)]
pub struct ActionLabelUnitBuildReport {
    pub id: String,
    pub source_text: String,
    pub korean_text: String,
    pub source_sequence_offset: String,
    pub source_terminator_offset: String,
    pub source_pointer_storage_offset: String,
    pub source_encoded_sha256: String,
    pub output_encoded_sha256: String,
    pub development_status: DevelopmentStatus,
    pub release_status: ReleaseStatus,
}
