use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::tim::Cell;

pub type BonusPageIndicatorFontSource = crate::bonus_inventory::BonusInventoryTextStyleSource;

#[derive(Debug, Clone)]
pub struct BonusPageIndicatorBuildConfig {
    pub cue: PathBuf,
    pub asset: PathBuf,
    pub font: BonusPageIndicatorFontSource,
    pub build_spec_sha256: String,
    pub output_dir: PathBuf,
    pub force: bool,
}

pub struct BonusPageIndicatorBuild {
    pub suffix_pixels: Vec<u8>,
    pub overlay: Vec<u8>,
    pub build_manifest_sha256: String,
    pub report: BonusPageIndicatorBuildReport,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum DevelopmentStatus {
    Untranslated,
    Authored,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum ReleaseStatus {
    Untranslated,
    NeedsHumanReview,
    Approved,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BonusPageIndicatorAsset {
    pub(super) kind: String,
    pub(super) id: String,
    pub(super) source: BonusPageIndicatorSourceBinding,
    pub(super) runtime_consumer: String,
    pub(super) source_text: String,
    pub(super) korean_text: Option<String>,
    pub(super) development_status: DevelopmentStatus,
    pub(super) release_status: ReleaseStatus,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BonusPageIndicatorSourceBinding {
    pub(super) inventory_path: String,
    pub(super) inventory_stored_sha256: String,
    pub(super) inventory_decoded_sha256: String,
    pub(super) glyph_tim_offset: String,
    pub(super) suffix_glyph_code: String,
    pub(super) suffix_glyph_packed_sha256: String,
    pub(super) overlay_path: String,
    pub(super) overlay_sha256: String,
    pub(super) consumer_runtime_address: String,
    pub(super) suffix_table_offset: String,
    pub(super) current_page_lookup_offset: String,
    pub(super) total_page_lookup_offset: String,
    pub(super) suffix_loop_bound_instruction_offset: String,
}

#[derive(Debug, Serialize)]
pub struct BonusPageIndicatorBuildReport {
    pub kind: String,
    pub source_bin_sha256: String,
    pub build_spec_sha256: String,
    pub translation_asset_sha256: String,
    pub source_inventory_path: String,
    pub source_inventory_stored_sha256: String,
    pub source_inventory_decoded_sha256: String,
    pub source_overlay_path: String,
    pub source_overlay_sha256: String,
    pub output_overlay_sha256: String,
    pub runtime_consumer: String,
    pub consumer_runtime_address: String,
    pub source_text: String,
    pub korean_text: String,
    pub source_suffix_sprite_count: usize,
    pub output_suffix_sprite_count: usize,
    pub dynamic_current_page_lookup_preserved: bool,
    pub dynamic_total_page_lookup_preserved: bool,
    pub source_suffix_glyph_code: String,
    pub source_suffix_sprite_selector_offsets: Vec<String>,
    pub source_suffix_string_selector_offsets: Vec<String>,
    pub suffix_glyph_cell: Cell,
    pub source_suffix_glyph_packed_sha256: String,
    pub output_suffix_glyph_packed_sha256: String,
    pub output_suffix_glyph_indexed_sha256: String,
    pub suffix_glyph_allowed_decoded_byte_ranges: Vec<[usize; 2]>,
    pub suffix_glyph_changed_decoded_byte_count: usize,
    pub overlay_expected_write_ranges: Vec<[usize; 2]>,
    pub overlay_changed_byte_ranges: Vec<[usize; 2]>,
    pub source_regions_match: bool,
    pub changes_confined_to_owned_regions: bool,
    pub font_name: String,
    pub font_sha256: String,
    pub font_px: f32,
    pub tracking_px: f32,
    pub vertical_shift_px: i32,
    pub measured_advance_px: f32,
    pub ink_bounds: [usize; 4],
    pub development_input_available: bool,
    pub release_candidate_input_eligible: bool,
    pub runtime_verification_required: bool,
}
