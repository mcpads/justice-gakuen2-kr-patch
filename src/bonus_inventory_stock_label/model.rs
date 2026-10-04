use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::tim::Cell;

pub type BonusInventoryStockLabelFontSource = crate::bonus_inventory::BonusInventoryTextStyleSource;

#[derive(Debug, Clone)]
pub struct BonusInventoryStockLabelBuildConfig {
    pub cue: PathBuf,
    pub assets: PathBuf,
    pub font: BonusInventoryStockLabelFontSource,
    pub build_spec_sha256: String,
    pub output_dir: PathBuf,
    pub force: bool,
}

pub struct BonusInventoryStockLabelBuild {
    pub(super) glyphs: Vec<StockLabelGlyphPixels>,
    pub overlay: Vec<u8>,
    pub build_manifest_sha256: String,
    pub report: BonusInventoryStockLabelBuildReport,
}

pub(super) struct StockLabelGlyphPixels {
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
pub(super) struct StockLabelManifest {
    pub(super) kind: String,
    pub(super) source: StockLabelSourceBinding,
    pub(super) runtime_consumer: String,
    pub(super) font_role: String,
    pub(super) glyphs: Vec<StockLabelGlyphAsset>,
    pub(super) units: Vec<PathBuf>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StockLabelSourceBinding {
    pub(super) inventory_path: String,
    pub(super) inventory_stored_sha256: String,
    pub(super) inventory_decoded_sha256: String,
    pub(super) glyph_tim_offset: String,
    pub(super) overlay_path: String,
    pub(super) overlay_sha256: String,
    pub(super) renderer_runtime_address: String,
    pub(super) category_handler_runtime_address: String,
    pub(super) category_jump_table_offset: String,
    pub(super) selector_instruction_offsets: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StockLabelGlyphAsset {
    pub(super) text: String,
    pub(super) code: String,
    pub(super) cell: Cell,
    pub(super) source_indexed_pixel_sha256: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StockLabelUnit {
    pub(super) kind: String,
    pub(super) id: String,
    pub(super) source_text: String,
    pub(super) korean_text: Option<String>,
    pub(super) source: StockLabelUnitSource,
    pub(super) development_status: DevelopmentStatus,
    pub(super) release_status: ReleaseStatus,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StockLabelUnitSource {
    pub(super) glyph_codes: Vec<String>,
    pub(super) selector_instruction_sha256: String,
    pub(super) screen_positions: Vec<[u16; 2]>,
    pub(super) sprite_size: [u16; 2],
}

#[derive(Debug, Serialize)]
pub struct BonusInventoryStockLabelBuildReport {
    pub kind: String,
    pub source_bin_sha256: String,
    pub build_spec_sha256: String,
    pub translation_manifest_sha256: String,
    pub translation_unit_sha256: String,
    pub source_inventory_path: String,
    pub source_inventory_stored_sha256: String,
    pub source_inventory_decoded_sha256: String,
    pub output_inventory_decoded_sha256: String,
    pub source_overlay_path: String,
    pub source_overlay_sha256: String,
    pub output_overlay_sha256: String,
    pub runtime_consumer: String,
    pub renderer_runtime_address: String,
    pub category_handler_runtime_address: String,
    pub category_jump_table_offset: String,
    pub category_state_target_runtime_addresses: Vec<String>,
    pub renderer_caller_runtime_addresses: Vec<String>,
    pub renderer_calling_states: Vec<u8>,
    pub noncalling_exit_state: u8,
    pub exit_state_skips_renderer: bool,
    pub source_glyph_codes: Vec<String>,
    pub output_glyph_codes: Vec<String>,
    pub source_selector_instruction_offsets: Vec<String>,
    pub changed_selector_instruction_offsets: Vec<String>,
    pub screen_positions: Vec<[u16; 2]>,
    pub sprite_size: [u16; 2],
    pub glyphs: Vec<StockLabelGlyphBuildReport>,
    pub inventory_expected_write_ranges: Vec<[usize; 2]>,
    pub inventory_changed_byte_ranges: Vec<[usize; 2]>,
    pub overlay_expected_write_ranges: Vec<[usize; 2]>,
    pub overlay_changed_byte_ranges: Vec<[usize; 2]>,
    pub source_regions_match: bool,
    pub allocated_glyphs_are_unreferenced_by_known_source_consumers: bool,
    pub existing_bonus_glyph_allocations_are_disjoint: bool,
    pub changes_confined_to_owned_regions: bool,
    pub font_role: String,
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
pub struct StockLabelGlyphBuildReport {
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
