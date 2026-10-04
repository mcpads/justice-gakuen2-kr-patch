use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::decoded_record_write_plan::DecodedDataClaim;
use crate::development_build_spec::ShiftedSizedFontSource;
use crate::name_input::{
    NameGlyphConsumerLayout, NameGlyphMaterializationBundle, NameInputRuntimeAtlasLayout,
};
use crate::tim::Cell;

use super::kanri_name_runtime::KanriTaggedNameRuntimeReport;

#[derive(Debug, Serialize)]
pub(crate) struct KanriDisplayAllocationEvidence {
    pub(crate) source_consumer_code_count: usize,
    pub(crate) fixed_menu_code_count: usize,
    pub(crate) allocated_display_code_count: usize,
    pub(crate) pairwise_disjoint: bool,
    pub(crate) disjoint_from_source_consumers: bool,
    pub(crate) disjoint_from_fixed_menu_codes: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct EditRuntimeTextBuildConfig {
    pub(crate) assets: PathBuf,
    pub(crate) font: ShiftedSizedFontSource,
    pub(crate) password_font: Option<ShiftedSizedFontSource>,
    pub(crate) edit_shared_ui_stored: Vec<u8>,
    pub(crate) edit_shared_ui_write_claims: Vec<DecodedDataClaim>,
    pub(crate) name_glyph_materialization: NameGlyphMaterializationBundle,
    pub(crate) name_glyph_consumer_layout: NameGlyphConsumerLayout,
    pub(crate) name_entry_font_stored: Vec<u8>,
    pub(crate) name_entry_font_patched_decoded_sha256: String,
    pub(crate) name_entry_runtime_atlas: NameInputRuntimeAtlasLayout,
    pub(crate) name_entry_runtime_coordinate_list_storage_byte_range: [usize; 2],
    pub(crate) outline_pixel_address: u32,
    pub(crate) outline_cleanup_address: u32,
    pub(crate) build_spec_sha256: String,
    pub(crate) output_dir: PathBuf,
    pub(crate) force: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EditRuntimeTextManifest {
    pub(super) kind: String,
    pub(super) source_bin_sha256: String,
    pub(super) overlay_path: String,
    pub(super) overlay_sha256: String,
    pub(super) runtime_font: EditRuntimeFontManifest,
    pub(super) renderer_glyph_advance_px: i16,
    pub(super) pass_fixed_presentation_text: String,
    pub(super) entries: Vec<EditRuntimeTextReference>,
    pub(super) direct_glyph_entries: Vec<EditRuntimeTextReference>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PassFixedPresentationTextManifest {
    pub(super) kind: String,
    pub(super) entries: Vec<PassFixedPresentationTextEntry>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PassFixedPresentationTextEntry {
    pub(super) id: String,
    pub(super) korean_text: String,
    pub(super) development_status: DevelopmentStatus,
    pub(super) release_status: ReleaseStatus,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EditRuntimeFontManifest {
    pub(super) path: String,
    pub(super) stored_sha256: String,
    pub(super) decoded_sha256: String,
    pub(super) tim_offset: String,
    pub(super) tim_sha256: String,
    pub(super) glyph_codes: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EditRuntimeTextReference {
    pub(super) id: String,
    pub(super) file: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EditRuntimeTextTranslation {
    pub(super) kind: String,
    pub(super) id: String,
    pub(super) source_offset: String,
    pub(super) source_record_size: usize,
    pub(super) source_record_sha256: String,
    pub(super) source_codes: Vec<String>,
    pub(super) source_text: Option<String>,
    pub(super) korean_text: Option<String>,
    pub(super) placement_record_offset: String,
    pub(super) source_x: i16,
    pub(super) source_y: i16,
    pub(super) source_placement_sha256: String,
    pub(super) horizontal_alignment: HorizontalAlignment,
    pub(super) development_status: DevelopmentStatus,
    pub(super) release_status: ReleaseStatus,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EditRuntimeDirectGlyphTranslation {
    pub(super) kind: String,
    pub(super) id: String,
    pub(super) source_instruction_offset: String,
    pub(super) source_code: String,
    pub(super) source_text: String,
    pub(super) korean_text: String,
    pub(super) development_status: DevelopmentStatus,
    pub(super) release_status: ReleaseStatus,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum HorizontalAlignment {
    PreserveCenter,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DevelopmentStatus {
    Untranslated,
    Authored,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ReleaseStatus {
    Untranslated,
    NeedsHumanReview,
    Approved,
}

pub(crate) struct EditRuntimeTextBuild {
    pub(crate) overlay: Vec<u8>,
    pub(crate) pass: Vec<u8>,
    pub(crate) edit_shared_ui_stored: Vec<u8>,
    pub(crate) build_manifest_sha256: String,
    pub(crate) report: EditRuntimeTextBuildReport,
}

#[derive(Debug, Serialize)]
pub(crate) struct EditRuntimeTextBuildReport {
    pub(crate) kind: String,
    pub(crate) build_spec_sha256: String,
    pub(crate) source_bin_sha256: String,
    pub(crate) source_overlay_size: usize,
    pub(crate) source_overlay_sha256: String,
    pub(crate) output_overlay_sha256: String,
    pub(crate) source_pass_size: usize,
    pub(crate) source_pass_sha256: String,
    pub(crate) output_pass_sha256: String,
    pub(crate) pass_fixed_presentation: super::pass_fixed_presentation::PassFixedPresentationReport,
    pub(crate) source_edit_shared_ui_stored_sha256: String,
    pub(crate) source_edit_shared_ui_decoded_sha256: String,
    pub(crate) input_edit_shared_ui_stored_sha256: String,
    pub(crate) input_edit_shared_ui_decoded_sha256: String,
    pub(crate) output_edit_shared_ui_stored_sha256: String,
    pub(crate) output_edit_shared_ui_decoded_sha256: String,
    pub(crate) runtime_font_tim_offset: String,
    pub(crate) runtime_font_tim_sha256: String,
    pub(crate) translation_manifest_sha256: String,
    pub(crate) pass_fixed_presentation_text_sha256: String,
    pub(crate) font_sha256: String,
    pub(crate) font_px: f32,
    pub(crate) vertical_shift_px: i32,
    pub(crate) renderer_glyph_advance_px: i16,
    pub(crate) entry_count: usize,
    pub(crate) authored_entry_count: usize,
    pub(crate) release_approved_entry_count: usize,
    pub(crate) development_input_available: bool,
    pub(crate) release_candidate_input_eligible: bool,
    pub(crate) source_records_match: bool,
    pub(crate) source_placements_match: bool,
    pub(crate) changed_bytes_confined_to_owned_ranges: bool,
    pub(crate) reused_glyph_count: usize,
    pub(crate) installed_glyph_count: usize,
    pub(crate) runtime_font_page_glyph_count: usize,
    pub(crate) kanri_initial_glyph_count: usize,
    pub(crate) kanri_display_allocation: KanriDisplayAllocationEvidence,
    pub(crate) tagged_name_runtime: KanriTaggedNameRuntimeReport,
    pub(crate) edit_shared_ui_decoded_changed_byte_ranges: Vec<[usize; 2]>,
    pub(crate) overlay_changed_byte_ranges: Vec<[usize; 2]>,
    pub(crate) glyphs: Vec<EditRuntimeTextGlyphBuild>,
    pub(crate) entries: Vec<EditRuntimeTextEntryBuild>,
    pub(crate) direct_glyph_entries: Vec<EditRuntimeDirectGlyphBuild>,
    pub(crate) overlay_output_file: String,
    pub(crate) pass_output_file: String,
    pub(crate) edit_shared_ui_output_file: String,
    pub(crate) runtime_consumer_verified: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct EditRuntimeDirectGlyphBuild {
    pub(crate) id: String,
    pub(crate) source_instruction_offset: String,
    pub(crate) source_code: String,
    pub(crate) source_text: String,
    pub(crate) korean_text: String,
    pub(crate) output_code: String,
    pub(crate) renderer_input_code: String,
    pub(crate) development_status: DevelopmentStatus,
    pub(crate) release_status: ReleaseStatus,
}

#[derive(Debug, Serialize)]
pub(crate) struct EditRuntimeTextGlyphBuild {
    pub(crate) character: char,
    pub(crate) code: String,
    pub(crate) cell: Cell,
    pub(crate) reused: bool,
    pub(crate) storage: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) ink_bounds: Option<[usize; 4]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) source_region_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) allowed_decoded_byte_ranges: Option<Vec<[usize; 2]>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) changed_decoded_byte_count: Option<usize>,
}

#[derive(Debug, Serialize)]
pub(crate) struct EditRuntimeTextEntryBuild {
    pub(crate) id: String,
    pub(crate) source_offset: String,
    pub(crate) source_text: Option<String>,
    pub(crate) korean_text: Option<String>,
    pub(crate) output_codes: Vec<String>,
    pub(crate) placement_record_offset: String,
    pub(crate) x: i16,
    pub(crate) y: i16,
    pub(crate) glyph_advance_px: i16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) button_layout: Option<super::button_layout::ButtonLayout>,
    pub(crate) development_status: DevelopmentStatus,
    pub(crate) release_status: ReleaseStatus,
}
