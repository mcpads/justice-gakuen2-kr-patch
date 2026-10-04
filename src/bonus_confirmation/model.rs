use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::tim::Cell;

pub type BonusConfirmationFontSource = crate::bonus_inventory::BonusInventoryTextStyleSource;

#[derive(Debug, Clone)]
pub struct BonusConfirmationBuildConfig {
    pub cue: PathBuf,
    pub assets: PathBuf,
    pub font: BonusConfirmationFontSource,
    pub build_spec_sha256: String,
    pub output_dir: PathBuf,
    pub force: bool,
}

pub struct BonusConfirmationBuild {
    pub(super) glyphs: Vec<ConfirmationGlyphPixels>,
    pub(super) units: Vec<ConfirmationTextUnit>,
    pub overlay: Vec<u8>,
    pub build_manifest_sha256: String,
    pub report: BonusConfirmationBuildReport,
}

pub(super) struct ConfirmationGlyphPixels {
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
pub(super) struct ConfirmationManifest {
    pub(super) kind: String,
    pub(super) source: ConfirmationSourceBinding,
    pub(super) runtime_consumer: String,
    pub(super) shared_choice_scope: String,
    pub(super) glyphs: Vec<ConfirmationGlyphAsset>,
    pub(super) fixed_record_space: ConfirmationGlyphAsset,
    pub(super) units: Vec<PathBuf>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ConfirmationSourceBinding {
    pub(super) inventory_path: String,
    pub(super) inventory_stored_sha256: String,
    pub(super) inventory_decoded_sha256: String,
    pub(super) glyph_tim_offset: String,
    pub(super) overlay_path: String,
    pub(super) overlay_sha256: String,
    pub(super) composer_runtime_address: String,
    pub(super) command_renderer_runtime_address: String,
    pub(super) exit_caller_runtime_address: String,
    pub(super) direct_caller_runtime_addresses: Vec<String>,
    pub(super) memory_card_destination_pointer_offset: String,
    pub(super) memory_card_copy_prompt_pointer_offset: String,
    pub(super) memory_card_destination_offset: String,
    pub(super) memory_card_destination_sha256: String,
    pub(super) memory_card_copy_prompt_offset: String,
    pub(super) memory_card_copy_prompt_sha256: String,
    pub(super) exit_prompt_record_offset: String,
    pub(super) exit_prompt_record_sha256: String,
    pub(super) alternate_prompt_record_offset: String,
    pub(super) alternate_prompt_record_sha256: String,
    pub(super) shared_choice_record_offset: String,
    pub(super) shared_choice_record_sha256: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ConfirmationGlyphAsset {
    pub(super) text: String,
    pub(super) code: String,
    pub(super) cell: Cell,
    pub(super) source_indexed_pixel_sha256: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ConfirmationTextUnit {
    pub(super) kind: String,
    pub(super) id: String,
    pub(super) source_text: String,
    pub(super) korean_text: Option<String>,
    pub(super) source: ConfirmationTextBinding,
    pub(super) development_status: DevelopmentStatus,
    pub(super) release_status: ReleaseStatus,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ConfirmationTextBinding {
    pub(super) record_offset: String,
    pub(super) record_length: usize,
    pub(super) encoded_sha256: String,
}

#[derive(Debug, Serialize)]
pub struct BonusConfirmationBuildReport {
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
    pub composer_runtime_address: String,
    pub command_renderer_runtime_address: String,
    pub exit_caller_runtime_address: String,
    pub direct_caller_runtime_addresses: Vec<String>,
    pub direct_caller_selector_values: Vec<u8>,
    pub memory_card_command_pointer_offsets: Vec<String>,
    pub shared_choice_caller_runtime_addresses: Vec<String>,
    pub shared_choice_scope: String,
    pub selected_card_prompt_translated: bool,
    pub memory_card_destination_translated: bool,
    pub memory_card_copy_prompt_translated: bool,
    pub fixed_record_space: ConfirmationReservedBlankReport,
    pub glyphs: Vec<ConfirmationGlyphBuildReport>,
    pub units: Vec<ConfirmationUnitBuildReport>,
    pub source_exit_prompt_record_sha256: String,
    pub output_exit_prompt_record_sha256: String,
    pub source_selected_card_prompt_record_sha256: String,
    pub output_selected_card_prompt_record_sha256: String,
    pub source_memory_card_destination_sha256: String,
    pub output_memory_card_destination_sha256: String,
    pub source_memory_card_copy_prompt_sha256: String,
    pub output_memory_card_copy_prompt_sha256: String,
    pub source_shared_choice_record_sha256: String,
    pub output_shared_choice_record_sha256: String,
    pub inventory_expected_write_ranges: Vec<[usize; 2]>,
    pub inventory_changed_byte_ranges: Vec<[usize; 2]>,
    pub overlay_expected_write_ranges: Vec<[usize; 2]>,
    pub overlay_changed_byte_ranges: Vec<[usize; 2]>,
    pub source_regions_match: bool,
    pub allocated_glyphs_are_unreferenced_by_known_source_consumers: bool,
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
pub struct ConfirmationReservedBlankReport {
    pub code: String,
    pub cell: Cell,
    pub source_indexed_pixel_sha256: String,
    pub output_indexed_pixel_sha256: String,
    pub remains_blank: bool,
    pub included_in_expected_write_ranges: bool,
}

#[derive(Debug, Serialize)]
pub struct ConfirmationGlyphBuildReport {
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
pub struct ConfirmationUnitBuildReport {
    pub id: String,
    pub source_text: String,
    pub korean_text: String,
    pub source_record_offset: String,
    pub source_record_length: usize,
    pub source_encoded_sha256: String,
    pub output_encoded_sha256: String,
    pub development_status: DevelopmentStatus,
    pub release_status: ReleaseStatus,
}
