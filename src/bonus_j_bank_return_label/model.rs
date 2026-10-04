use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::tim::Cell;

pub type BonusJBankReturnLabelFontSource = crate::bonus_inventory::BonusInventoryTextStyleSource;

#[derive(Debug, Clone)]
pub struct BonusJBankReturnLabelBuildConfig {
    pub cue: PathBuf,
    pub assets: PathBuf,
    pub font: BonusJBankReturnLabelFontSource,
    pub build_spec_sha256: String,
    pub output_dir: PathBuf,
    pub force: bool,
}

pub struct BonusJBankReturnLabelBuild {
    pub(super) glyphs: Vec<JBankReturnLabelGlyphPixels>,
    pub(super) glyph_allocations: Vec<JBankReturnLabelGlyphAllocation>,
    pub(super) unit: JBankReturnLabelUnit,
    pub overlay: Vec<u8>,
    pub build_manifest_sha256: String,
    pub report: BonusJBankReturnLabelBuildReport,
}

pub(super) struct JBankReturnLabelGlyphPixels {
    pub(super) code: u16,
    pub(super) cell: Cell,
    pub(super) pixels: Vec<u8>,
    pub(super) indexed_sha256: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct JBankReturnLabelGlyphAllocation {
    pub(super) text: char,
    pub(super) code: u16,
    pub(super) cell: Cell,
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
pub(super) struct JBankReturnLabelManifest {
    pub(super) kind: String,
    pub(super) source: JBankReturnLabelSourceBinding,
    pub(super) runtime_consumer: String,
    pub(super) font_role: String,
    pub(super) glyphs: Vec<JBankReturnLabelGlyphAsset>,
    pub(super) units: Vec<PathBuf>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct JBankReturnLabelSourceBinding {
    pub(super) inventory_path: String,
    pub(super) inventory_stored_sha256: String,
    pub(super) inventory_decoded_sha256: String,
    pub(super) glyph_tim_offset: String,
    pub(super) overlay_path: String,
    pub(super) overlay_sha256: String,
    pub(super) overlay_runtime_base: String,
    pub(super) pointer_table_offset: String,
    pub(super) pointer_table_end: String,
    pub(super) direct_selector_region: [String; 2],
    pub(super) command_renderer_runtime_address: String,
    pub(super) pointer_storage_offset: String,
    pub(super) caller_runtime_addresses: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct JBankReturnLabelGlyphAsset {
    pub(super) text_index: usize,
    pub(super) code: String,
    pub(super) cell: Cell,
    pub(super) source_indexed_pixel_sha256: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct JBankReturnLabelUnit {
    pub(super) kind: String,
    pub(super) id: String,
    pub(super) source_text: String,
    pub(super) korean_text: Option<String>,
    pub(super) source: JBankReturnLabelSequenceBinding,
    pub(super) development_status: DevelopmentStatus,
    pub(super) release_status: ReleaseStatus,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct JBankReturnLabelSequenceBinding {
    pub(super) sequence_offset: String,
    pub(super) storage_length: usize,
    pub(super) terminator_offset: String,
    pub(super) pointer_storage_offset: String,
    pub(super) encoded_sha256: String,
    pub(super) caller_runtime_addresses: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct BonusJBankReturnLabelBuildReport {
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
    pub font_role: String,
    pub command_renderer_runtime_address: String,
    pub caller_runtime_addresses: Vec<String>,
    pub pointer_storage_offset: String,
    pub glyphs: Vec<JBankReturnLabelGlyphBuildReport>,
    pub unit: JBankReturnLabelUnitBuildReport,
    pub inventory_expected_write_ranges: Vec<[usize; 2]>,
    pub inventory_changed_byte_ranges: Vec<[usize; 2]>,
    pub overlay_expected_write_ranges: Vec<[usize; 2]>,
    pub overlay_changed_byte_ranges: Vec<[usize; 2]>,
    pub source_command_record_matches: bool,
    pub glyph_ownership_evidence: JBankReturnLabelGlyphOwnershipEvidenceReport,
    pub inventory_changes_confined_to_allocated_glyph_cells: bool,
    pub overlay_changes_confined_to_fixed_command_record: bool,
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
pub struct JBankReturnLabelGlyphOwnershipEvidenceReport {
    pub declared_physical_alias_set_matches: bool,
    pub physical_alias_source_cells_match_blank_hash: bool,
    pub existing_bonus_component_allocations_disjoint: bool,
    pub pointer_command_table_range: [usize; 2],
    pub pointer_command_table_parsed_glyphs_disjoint: bool,
    pub declared_direct_selector_byte_region: [usize; 2],
    pub declared_direct_selector_byte_region_scan_disjoint: bool,
}

#[derive(Debug, Serialize)]
pub struct JBankReturnLabelGlyphBuildReport {
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
pub struct JBankReturnLabelUnitBuildReport {
    pub id: String,
    pub source_text: String,
    pub korean_text: String,
    pub source_sequence_offset: String,
    pub source_storage_length: usize,
    pub source_terminator_offset: String,
    pub source_pointer_storage_offset: String,
    pub caller_runtime_addresses: Vec<String>,
    pub source_encoded_sha256: String,
    pub output_encoded_sha256: String,
    pub output_payload_byte_length: usize,
    pub output_command_count: usize,
    pub development_status: DevelopmentStatus,
    pub release_status: ReleaseStatus,
}
