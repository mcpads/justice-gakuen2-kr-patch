use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct OptionsAssetAuditConfig {
    pub cue: PathBuf,
    pub assets: PathBuf,
    pub output: PathBuf,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OptionsManifest {
    pub kind: String,
    pub source_bin_sha256: String,
    pub overlay_path: String,
    pub overlay_sha256: String,
    pub menu_path: String,
    pub menu_stored_sha256: String,
    pub menu_decoded_sha256: String,
    pub units: Vec<OptionsAssetReference>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OptionsAssetReference {
    pub id: String,
    pub file: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OptionsTranslationUnit {
    pub kind: String,
    pub id: String,
    pub source_offset: String,
    pub pointer_offsets: Vec<String>,
    pub source_codes: Vec<String>,
    pub source_text: Option<String>,
    pub korean_text: Option<String>,
    pub font_role: Option<OptionsFontRole>,
    pub development_status: OptionsDevelopmentStatus,
    pub release_status: OptionsReleaseStatus,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub(super) enum OptionsFontRole {
    #[serde(rename = "options_heading")]
    Heading,
    #[serde(rename = "options_help")]
    Help,
    #[serde(rename = "options_label")]
    Label,
    #[serde(rename = "options_value")]
    Value,
    #[serde(rename = "options_action")]
    Action,
    #[serde(rename = "records_main_heading")]
    RecordsMainHeading,
    #[serde(rename = "records_main_item")]
    RecordsMainItem,
    #[serde(rename = "records_prompt")]
    RecordsPrompt,
    #[serde(rename = "records_status_heading")]
    RecordsStatusHeading,
    #[serde(rename = "records_status_message")]
    RecordsStatusMessage,
}

impl OptionsFontRole {
    pub(super) const fn key(self) -> &'static str {
        match self {
            Self::Heading => "options_heading",
            Self::Help => "options_help",
            Self::Label => "options_label",
            Self::Value => "options_value",
            Self::Action => "options_action",
            Self::RecordsMainHeading => "records_main_heading",
            Self::RecordsMainItem => "records_main_item",
            Self::RecordsPrompt => "records_prompt",
            Self::RecordsStatusHeading => "records_status_heading",
            Self::RecordsStatusMessage => "records_status_message",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum OptionsDevelopmentStatus {
    Untranslated,
    Authored,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum OptionsReleaseStatus {
    Untranslated,
    NeedsHumanReview,
    Approved,
}

#[derive(Debug, Clone)]
pub(super) struct OptionsAuthoredUnit {
    pub(super) id: String,
    pub(super) source_offset: String,
    pub(super) pointer_offsets: Vec<String>,
    pub(super) source_codes: Vec<String>,
    pub(super) source_text: String,
    pub(super) korean_text: String,
    pub(super) font_role: OptionsFontRole,
    pub(super) release_status: OptionsReleaseStatus,
}

#[derive(Debug, Serialize)]
pub struct OptionsAssetAuditReport {
    pub kind: String,
    pub source_bin_sha256: String,
    pub overlay_path: String,
    pub overlay_sha256: String,
    pub menu_path: String,
    pub menu_stored_sha256: String,
    pub menu_decoded_sha256: String,
    pub manifest_sha256: String,
    pub unit_count: usize,
    pub untranslated_unit_count: usize,
    pub authored_unit_count: usize,
    pub release_approved_unit_count: usize,
    pub source_records_match: bool,
    pub source_pointer_bindings_match: bool,
    pub menu_texture_regions: Vec<OptionsMenuTextureRegion>,
    pub detected_four_bit_tim_regions: Vec<DetectedMenuTextureRegion>,
    pub development_asset_input_available: bool,
    pub release_candidate_input_eligible: bool,
    pub newopt_pointer_slot_count: usize,
    pub newopt_unique_source_record_count: usize,
    pub tracked_source_record_count: usize,
    pub untracked_source_record_count: usize,
    pub untracked_source_record_offsets: Vec<String>,
    pub required_korean_characters: String,
    pub required_korean_character_count: usize,
    pub descriptions: super::description_model::OptionsDescriptionAudit,
}

#[derive(Debug, Clone)]
pub struct OptionsSourceAssetSyncConfig {
    pub cue: PathBuf,
    pub assets: PathBuf,
}

#[derive(Debug, Serialize)]
pub struct OptionsSourceAssetSyncReport {
    pub kind: String,
    pub source_bin_sha256: String,
    pub overlay_path: String,
    pub overlay_sha256: String,
    pub pointer_slot_count: usize,
    pub unique_source_record_count: usize,
    pub existing_unit_count: usize,
    pub created_untranslated_unit_count: usize,
    pub final_unit_count: usize,
}

#[derive(Debug, Clone)]
pub struct OptionsFontStyle {
    pub font: PathBuf,
    pub font_px: f32,
}

#[derive(Debug, Clone)]
pub struct OptionsFontStyles {
    pub heading: OptionsFontStyle,
    pub help: OptionsFontStyle,
    pub label: OptionsFontStyle,
    pub value: OptionsFontStyle,
    pub action: OptionsFontStyle,
    pub records_main: RecordsMainFontStyles,
    pub records_prompt: OptionsFontStyle,
    pub records_status: RecordsStatusFontStyles,
    pub background: OptionsBackgroundFontStyles,
    pub description: OptionsFontStyle,
}

#[derive(Debug, Clone)]
pub struct RecordsMainFontStyles {
    pub heading: OptionsFontStyle,
    pub item: OptionsFontStyle,
}

#[derive(Debug, Clone)]
pub struct RecordsStatusFontStyles {
    pub heading: OptionsFontStyle,
    pub message: OptionsFontStyle,
}

#[derive(Debug, Clone)]
pub struct OptionsBackgroundFontStyles {
    pub school_name: OptionsFontStyle,
    pub crest_mark: OptionsFontStyle,
}

#[derive(Debug, Clone)]
pub struct OptionsBuildConfig {
    pub cue: PathBuf,
    pub assets: PathBuf,
    pub fonts: OptionsFontStyles,
    pub build_spec_sha256: String,
    pub output_dir: PathBuf,
    pub force: bool,
}

#[derive(Debug, Serialize)]
pub struct OptionsBuildReport {
    pub kind: String,
    pub output_bin_sha256: String,
    pub output_cue: String,
    #[serde(flatten)]
    pub records: OptionsRecordBuildReport,
    pub changed_lbas: Vec<u32>,
    pub edc_ecc_verified: bool,
}

#[derive(Debug, Serialize)]
pub struct OptionsRecordBuildReport {
    pub build_spec_sha256: String,
    pub source_bin_sha256: String,
    pub source_menu_stored_sha256: String,
    pub output_menu_stored_sha256: String,
    pub source_menu_decoded_sha256: String,
    pub output_menu_decoded_sha256: String,
    pub source_overlay_size: usize,
    pub source_overlay_sha256: String,
    pub output_overlay_sha256: String,
    pub source_optinfo_stored_sha256: String,
    pub output_optinfo_stored_sha256: String,
    pub source_optinfo_decoded_sha256: String,
    pub output_optinfo_decoded_sha256: String,
    pub source_main_executable_sha256: String,
    pub source_main_executable_size: usize,
    pub output_main_executable_sha256: String,
    pub translation_manifest_sha256: String,
    pub description_manifest_sha256: String,
    pub background: super::background_model::OptionsBackgroundBuildReport,
    pub records_runtime: RecordsRuntimeBuildReport,
    pub runtime_glyph_upload: super::runtime_glyph_upload::ContextualGlyphUploadReport,
    pub unit_count: usize,
    pub authored_unit_count: usize,
    pub release_approved_unit_count: usize,
    pub description_item_count: usize,
    pub description_release_approved_item_count: usize,
    pub development_input_available: bool,
    pub translations_release_approved: bool,
    pub allocation_status: String,
    pub allocation_proven_reclaimable: bool,
    pub release_candidate_input_eligible: bool,
    pub options_source_menu_glyph_pixels_preserved: bool,
    pub options_source_menu_background_pixels_preserved: bool,
    pub provisional_code_count: usize,
    pub menu_decoded_changed_byte_ranges: Vec<[usize; 2]>,
    pub overlay_changed_byte_ranges: Vec<[usize; 2]>,
    pub description_overlay_changed_byte_ranges: Vec<[usize; 2]>,
    pub optinfo_decoded_changed_byte_ranges: Vec<[usize; 2]>,
    pub main_executable_changed_byte_ranges: Vec<[usize; 2]>,
    pub original_menu_stored_size: usize,
    pub rebuilt_menu_stored_size: usize,
    pub menu_padding_size: usize,
    pub source_compression_maximum_match_words: usize,
    pub source_compression_maximum_control_block_output_words: usize,
    pub rebuilt_compression_maximum_match_words: usize,
    pub rebuilt_compression_maximum_control_block_output_words: usize,
    pub original_optinfo_stored_size: usize,
    pub rebuilt_optinfo_stored_size: usize,
    pub optinfo_padding_size: usize,
    pub source_optinfo_compression_maximum_match_words: usize,
    pub source_optinfo_compression_maximum_control_block_output_words: usize,
    pub rebuilt_optinfo_compression_maximum_match_words: usize,
    pub rebuilt_optinfo_compression_maximum_control_block_output_words: usize,
    pub fonts: Vec<OptionsFontBuild>,
    pub glyphs: Vec<OptionsGlyphBuild>,
    pub text_units: Vec<OptionsTextBuild>,
    pub description_glyphs: Vec<super::description_model::OptionsDescriptionGlyphBuild>,
    pub descriptions: Vec<super::description_model::OptionsDescriptionBuild>,
    pub menu_output_file: String,
    pub overlay_output_file: String,
    pub optinfo_output_file: String,
    pub main_executable_output_file: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct OptionsFontBuild {
    pub role: String,
    pub font_name: String,
    pub font_sha256: String,
    pub font_px: f32,
}

#[derive(Debug, Serialize)]
pub struct OptionsGlyphBuild {
    pub role: String,
    pub character: char,
    pub code: String,
    pub cell: crate::tim::Cell,
    pub ink_bounds: [usize; 4],
    pub install: crate::tim::GlyphInstallMetadata,
    pub global_menu_resident: bool,
    pub runtime_context: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct OptionsTextBuild {
    pub id: String,
    pub source_offset: String,
    pub output_offset: String,
    pub source_text: String,
    pub korean_text: String,
    pub font_role: String,
    pub output_codes: Vec<String>,
    pub pointer_offsets: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct RecordsRuntimeBuildReport {
    pub unit_count: usize,
    pub source_records_match: bool,
    pub source_pointer_bindings_match: bool,
    pub changes_confined_to_owned_records: bool,
    pub units: Vec<RecordsRuntimeUnitBuild>,
}

#[derive(Debug, Serialize)]
pub struct RecordsRuntimeUnitBuild {
    pub id: String,
    pub overlay_offset: String,
    pub main_executable_offset: String,
    pub main_executable_pointer_offsets: Vec<String>,
    pub source_record_size: usize,
    pub output_record_size: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DetectedMenuTextureRegion {
    pub decoded_offset: String,
    pub decoded_size: usize,
    pub has_clut: bool,
    pub image_vram_word_x: u16,
    pub image_vram_y: u16,
    pub image_pixel_width: usize,
    pub image_height: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OptionsMenuTextureRegion {
    pub role: String,
    pub decoded_offset: String,
    pub decoded_size: usize,
    pub clut_vram_x: u16,
    pub clut_vram_y: u16,
    pub clut_width: usize,
    pub clut_height: usize,
    pub image_vram_word_x: u16,
    pub image_vram_y: u16,
    pub image_pixel_width: usize,
    pub image_height: usize,
}
