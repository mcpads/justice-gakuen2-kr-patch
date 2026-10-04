use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::translation_model::DialogueDevelopmentInputPolicy;

#[derive(Debug, Clone)]
pub struct DialogueDiscBuildConfig {
    pub prepared_dialogue: PathBuf,
    pub cue: PathBuf,
    pub build_spec: PathBuf,
    pub input_policy: DialogueDevelopmentInputPolicy,
    pub output_dir: PathBuf,
    pub force: bool,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DialogueDiscBuildReport {
    pub kind: String,
    pub source_bin_sha256: String,
    pub input_policy: String,
    pub output_bin_sha256: String,
    pub output_bin: String,
    pub output_cue: String,
    pub development_build_spec_sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode_select_descendant_surface_inventory:
        Option<crate::surface_inventory::SurfaceInventoryBuildReport>,
    pub font_build_manifest_sha256: String,
    pub name_entry_build_manifest_sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bonus_menu_build_manifest_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bonus_inventory_build_manifest_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bonus_inventory_action_labels_build_manifest_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bonus_confirmation_build_manifest_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bonus_inventory_stock_label_build_manifest_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bonus_inventory_card_acquisition_build_manifest_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bonus_inventory_memory_card_swap_build_manifest_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bonus_j_bank_return_label_build_manifest_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bonus_page_indicator_build_manifest_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shop_ui_build_manifest_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bonus_shop_exit_confirmation_build_manifest_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bonus_shop_text_build_manifest_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diary_header_build_manifest_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diary_scene_build_manifest_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub character_select_build_manifest_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode_descendant_build_manifest_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub practical_instruction_build_manifest_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode_select_build_manifest_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options_build_manifest_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title_menu_build_manifest_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title_notice_build_manifest_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edit_runtime_text_build_manifest_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dialogue_name_runtime_build_manifest_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shared_name_runtime_build_manifest_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options_build_spec_sha256: Option<String>,
    pub dialogue_font_name: String,
    pub dialogue_font_sha256: String,
    pub dialogue_font_px: f32,
    pub name_entry_candidate_font_name: String,
    pub name_entry_candidate_font_sha256: String,
    pub name_entry_candidate_font_px: f32,
    pub mode_select_fonts: Vec<crate::mode_select::ModeSelectFontBuild>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bonus_menu_development_input_available: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bonus_menu_release_candidate_input_eligible: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bonus_inventory_development_input_available: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bonus_inventory_release_candidate_input_eligible: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bonus_inventory_action_labels_development_input_available: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bonus_inventory_action_labels_release_candidate_input_eligible: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bonus_confirmation_development_input_available: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bonus_confirmation_release_candidate_input_eligible: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bonus_inventory_stock_label_development_input_available: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bonus_inventory_stock_label_release_candidate_input_eligible: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bonus_inventory_card_acquisition_development_input_available: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bonus_inventory_card_acquisition_release_candidate_input_eligible: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bonus_inventory_memory_card_swap_development_input_available: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bonus_inventory_memory_card_swap_release_candidate_input_eligible: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bonus_j_bank_return_label_development_input_available: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bonus_j_bank_return_label_release_candidate_input_eligible: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bonus_page_indicator_development_input_available: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bonus_page_indicator_release_candidate_input_eligible: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shop_ui_development_input_available: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shop_ui_release_candidate_input_eligible: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bonus_shop_exit_confirmation_development_input_available: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bonus_shop_exit_confirmation_release_candidate_input_eligible: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bonus_shop_text_development_input_available: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bonus_shop_text_release_candidate_input_eligible: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode_select_development_input_available: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode_select_release_candidate_input_eligible: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode_descendant_development_input_available: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode_descendant_release_candidate_input_eligible: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub practical_instruction_development_input_available: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub practical_instruction_release_candidate_input_eligible: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options_development_input_available: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options_release_candidate_input_eligible: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title_menu_development_input_available: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title_menu_release_candidate_input_eligible: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title_notice_development_input_available: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title_notice_release_candidate_input_eligible: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edit_runtime_text_development_input_available: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edit_runtime_text_release_candidate_input_eligible: Option<bool>,
    pub development_build_input_available: bool,
    pub development_translation_input_available: bool,
    pub release_candidate_translation_input_eligible: bool,
    pub compression_maximum_match_words: usize,
    pub compression_maximum_control_block_output_words: usize,
    pub fixed_code_consumer_ownership_complete: bool,
    pub replacement_record_count: usize,
    #[serde(default)]
    pub record_writes: Vec<DialogueDiscRecordWrite>,
    pub changed_sector_count: usize,
    pub changed_lba_ranges: Vec<[u32; 2]>,
    pub changes_confined_to_replaced_records: bool,
    #[serde(default)]
    pub changes_confined_to_declared_records_and_metadata: bool,
    #[serde(default)]
    pub metadata_lbas: Vec<u32>,
    #[serde(default)]
    pub source_sector_count: u64,
    #[serde(default)]
    pub output_sector_count: u64,
    pub all_records_read_back: bool,
    pub edc_ecc_verified: bool,
    #[serde(default)]
    pub menu_surface_writes_disjoint: bool,
    #[serde(default)]
    pub menu_decoded_changed_byte_ranges: Vec<[usize; 2]>,
    #[serde(default)]
    pub menu_source_compression_maximum_match_words: usize,
    #[serde(default)]
    pub menu_source_compression_maximum_control_block_output_words: usize,
    #[serde(default)]
    pub menu_source_compression_control_blocks_crossing_input_pages: usize,
    #[serde(default)]
    pub menu_source_compression_stream_byte_count: usize,
    #[serde(default)]
    pub menu_rebuilt_compression_maximum_match_words: usize,
    #[serde(default)]
    pub menu_rebuilt_compression_maximum_control_block_output_words: usize,
    #[serde(default)]
    pub menu_rebuilt_compression_control_blocks_crossing_input_pages: usize,
    #[serde(default)]
    pub menu_rebuilt_compression_stream_byte_count: usize,
    #[serde(default)]
    pub menu_unpadded_stored_size: usize,
    #[serde(default)]
    pub title_overlay_writes_disjoint: bool,
    #[serde(default)]
    pub title_overlay_decoded_changed_byte_ranges: Vec<[usize; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title_contextual_glyph_upload:
        Option<crate::contextual_texture_upload::ContextualMenuGlyphUploadReport>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub continue_name_runtime:
        Option<crate::title_overlay_runtime::continue_names::ContinueNameRuntimeReport>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edit_tagged_name_runtime: Option<crate::edit_runtime_text::KanriTaggedNameRuntimeReport>,
    pub dialogue_assets: Vec<DialogueDiscAssetReadback>,
    pub dialogue_bundles: Vec<DialogueDiscBundleReadback>,
    pub name_entry_font: DialogueDiscAssetReadback,
    pub name_entry: DialogueDiscRecordReadback,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bonus_menu: Option<DialogueDiscAssetReadback>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bonus_inventory: Option<DialogueDiscAssetReadback>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bonus_inventory_overlay: Option<DialogueDiscRecordReadback>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shop_ui: Option<DialogueDiscAssetReadback>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shop_overlay: Option<DialogueDiscRecordReadback>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diary_header: Option<DialogueDiscAssetReadback>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diary_scene: Option<DialogueDiscRecordReadback>,
    #[serde(default)]
    pub diary_scene_runtime_bundles: Vec<DiarySceneRuntimeBundleReadback>,
    pub diary_artwork_archives: Vec<DialogueDiscRecordReadback>,
    #[serde(default)]
    pub character_select_records: Vec<DialogueDiscAssetReadback>,
    #[serde(default)]
    pub character_select_overlays: Vec<DialogueDiscRecordReadback>,
    #[serde(default)]
    pub mode_descendant_records: Vec<DialogueDiscAssetReadback>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub practical_instruction: Option<DialogueDiscRecordReadback>,
    #[serde(default)]
    pub practical_instruction_prompt_consumers: Vec<DialogueDiscRecordReadback>,
    #[serde(
        default,
        alias = "mode_select",
        skip_serializing_if = "Option::is_none"
    )]
    pub menu: Option<DialogueDiscAssetReadback>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options_overlay: Option<DialogueDiscRecordReadback>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options_info: Option<DialogueDiscAssetReadback>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title_menu_overlay: Option<DialogueDiscAssetReadback>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edit_runtime_text_overlay: Option<DialogueDiscRecordReadback>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edit_runtime_text_pass: Option<DialogueDiscRecordReadback>,
    #[serde(
        default,
        alias = "shared_name_runtime",
        skip_serializing_if = "Option::is_none"
    )]
    pub main_executable: Option<DialogueDiscRecordReadback>,
    pub battle_name: Option<DialogueDiscAssetReadback>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dialogue_name_runtime: Option<DialogueDiscRecordReadback>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DialogueDiscRecordWrite {
    pub owner: String,
    pub path: String,
    pub logical_byte_count: u32,
    pub source_sha256: String,
    pub replacement_sha256: String,
    pub extent_lba: u32,
    pub sector_count: usize,
    pub changed_lba_ranges: Vec<[u32; 2]>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DialogueDiscAssetReadback {
    pub path: String,
    pub extent_lba: u32,
    pub sector_count: usize,
    pub expected_stored_sha256: String,
    pub readback_stored_sha256: String,
    pub expected_decoded_sha256: String,
    pub readback_decoded_sha256: String,
    pub readback_verified: bool,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DialogueDiscRecordReadback {
    pub path: String,
    pub extent_lba: u32,
    pub sector_count: usize,
    pub expected_sha256: String,
    pub readback_sha256: String,
    pub readback_verified: bool,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DialogueDiscBundleReadback {
    pub path: String,
    pub extent_lba: u32,
    pub sector_count: usize,
    pub source_sha256: String,
    pub expected_rebuilt_sha256: String,
    pub readback_sha256: String,
    pub readback_verified: bool,
    pub members: Vec<DialogueDiscBundleMemberReadback>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DialogueDiscBundleMemberReadback {
    pub source_path: String,
    pub bundle_offset: usize,
    pub byte_count: usize,
    pub source_sha256: String,
    pub expected_replacement_sha256: String,
    pub readback_replacement_sha256: String,
    pub readback_verified: bool,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DiarySceneRuntimeBundleReadback {
    pub path: String,
    pub extent_lba: u32,
    pub sector_count: usize,
    pub source_sha256: String,
    pub expected_rebuilt_sha256: String,
    pub readback_sha256: String,
    pub members: Vec<DiarySceneRuntimeMemberReadback>,
    pub members_read_back: bool,
    pub readback_verified: bool,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DiarySceneRuntimeMemberReadback {
    pub index: usize,
    pub offset: usize,
    pub byte_count: usize,
    pub expected_sha256: String,
    pub readback_sha256: String,
    pub readback_verified: bool,
}
