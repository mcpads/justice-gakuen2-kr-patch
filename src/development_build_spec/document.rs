use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DevelopmentBuildSpecDocument {
    pub(super) kind: String,
    pub(super) specifications: DevelopmentSpecificationDocuments,
    pub(super) assets: DevelopmentAssetDocuments,
    pub(super) font_sources: BTreeMap<String, FontSourceDocument>,
    pub(super) fonts: DevelopmentFontDocuments,
    pub(super) runtime_glyph_layouts: RuntimeGlyphLayoutDocuments,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DevelopmentSpecificationDocuments {
    pub(super) mode_select_descendants: PathBuf,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RuntimeGlyphLayoutDocuments {
    pub(super) nickname_hud: NicknameHudGlyphLayoutDocument,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct NicknameHudGlyphLayoutDocument {
    pub(super) scale_percent: u16,
    pub(super) vertical_shift_px: i16,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DevelopmentAssetDocuments {
    pub(super) staff_roll: Option<PathBuf>,
    pub(super) pocket_help: Option<PathBuf>,
    pub(super) endings: Option<PathBuf>,
    pub(super) card_art: Option<PathBuf>,
    pub(super) loading_art: Option<PathBuf>,
    pub(super) bonus_menu: PathBuf,
    pub(super) bonus_inventory: PathBuf,
    pub(super) bonus_inventory_action_labels: PathBuf,
    pub(super) bonus_confirmation: PathBuf,
    pub(super) bonus_inventory_card_acquisition: PathBuf,
    pub(super) bonus_inventory_memory_card_swap: PathBuf,
    pub(super) bonus_j_bank_return_label: PathBuf,
    pub(super) bonus_inventory_stock_label: PathBuf,
    pub(super) bonus_page_indicator: PathBuf,
    pub(super) bonus_shop_exit_confirmation: PathBuf,
    pub(super) bonus_shop_text: PathBuf,
    pub(super) shop_ui: PathBuf,
    pub(super) dialogue_codebook: PathBuf,
    pub(super) dialogue_translations: PathBuf,
    pub(super) dialogue_selector_translations: PathBuf,
    pub(super) name_entry_candidates: PathBuf,
    pub(super) name_entry_graphics: PathBuf,
    pub(super) diary_header: PathBuf,
    pub(super) diary_scenes: PathBuf,
    pub(super) mode_select: PathBuf,
    pub(super) character_select: PathBuf,
    pub(super) mode_descendants: PathBuf,
    pub(super) practical_instructions: PathBuf,
    pub(super) options: PathBuf,
    pub(super) title_menu: PathBuf,
    pub(super) title_notice: PathBuf,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FontSourceDocument {
    pub(super) path: PathBuf,
    pub(super) sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DevelopmentFontDocuments {
    pub(super) staff_roll: Option<SizedFontDocument>,
    #[serde(default)]
    pub(super) pocket_help: BTreeMap<String, SizedFontDocument>,
    #[serde(default)]
    pub(super) card_art: BTreeMap<String, SizedFontDocument>,
    #[serde(default)]
    pub(super) loading_art: BTreeMap<String, SizedFontDocument>,
    pub(super) bonus_menu: BonusMenuFontDocuments,
    pub(super) bonus_inventory: BonusInventoryFontDocuments,
    pub(super) bonus_inventory_action_labels: BonusInventoryTextStyleDocument,
    pub(super) bonus_confirmation: BonusInventoryTextStyleDocument,
    pub(super) bonus_inventory_card_acquisition: BonusInventoryTextStyleDocument,
    pub(super) bonus_inventory_memory_card_swap: BonusInventoryTextStyleDocument,
    pub(super) bonus_j_bank_return_label: BonusInventoryTextStyleDocument,
    pub(super) bonus_inventory_stock_label: BonusInventoryTextStyleDocument,
    pub(super) bonus_page_indicator: BonusInventoryTextStyleDocument,
    pub(super) shared_menu_numerals: SizedFontDocument,
    pub(super) bonus_shop_exit_confirmation: BonusInventoryTextStyleDocument,
    pub(super) bonus_shop_text: BonusShopTextFontDocuments,
    pub(super) shop_ui: ShopUiFontDocuments,
    pub(super) dialogue_body: SizedFontDocument,
    pub(super) name_entry: NameEntryFontDocuments,
    pub(super) diary_header: DiaryHeaderFontDocuments,
    pub(super) diary_scene: DiarySceneFontDocuments,
    pub(super) mode_select: ModeSelectFontDocuments,
    pub(super) character_select: CharacterSelectFontDocuments,
    pub(super) mode_descendants: ModeDescendantFontDocuments,
    pub(super) practical_instruction: ShiftedSizedFontDocument,
    pub(super) options: OptionsFontDocuments,
    pub(super) title_menu: SizedFontDocument,
    pub(super) title_notice: SizedFontDocument,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BonusMenuFontDocuments {
    pub(super) heading: SizedFontDocument,
    pub(super) entry: SizedFontDocument,
    pub(super) compact_entry: SizedFontDocument,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BonusInventoryFontDocuments {
    pub(super) item_label: BonusInventoryTextStyleDocument,
    pub(super) device_text: BonusInventoryTextStyleDocument,
    pub(super) title: BonusInventoryTextStyleDocument,
    pub(super) compact_label: BonusInventoryTextStyleDocument,
    pub(super) large_label: BonusInventoryTextStyleDocument,
    pub(super) compact_action: BonusInventoryTextStyleDocument,
    pub(super) large_action: BonusInventoryTextStyleDocument,
    pub(super) help: BonusInventoryTextStyleDocument,
    pub(super) viewer_navigation: BonusInventoryTextStyleDocument,
    pub(super) viewer_card_placeholder: BonusInventoryTextStyleDocument,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BonusInventoryTextStyleDocument {
    pub(super) source: String,
    pub(super) font_px: f32,
    pub(super) tracking_px: f32,
    pub(super) vertical_shift_px: i32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BonusShopTextFontDocuments {
    pub(super) product_description: BonusInventoryTextStyleDocument,
    pub(super) product_label: BonusInventoryTextStyleDocument,
    pub(super) clerk_dialogue: BonusInventoryTextStyleDocument,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ShopUiFontDocuments {
    pub(super) heading: SizedFontDocument,
    pub(super) current_points: SizedFontDocument,
    pub(super) heading_tracking_px: f32,
    pub(super) current_points_tracking_px: f32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct NameEntryFontDocuments {
    pub(super) candidates: SizedFontDocument,
    pub(super) keyboard_keys: SizedFontDocument,
    pub(super) composed_glyphs: SizedFontDocument,
    pub(super) roster_glyphs: SizedFontDocument,
    pub(super) fixed_graphics: FontSelectionDocument,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DiaryHeaderFontDocuments {
    pub(super) calendar_text: IndexedRampFontDocument,
    pub(super) status_label: IndexedRampFontDocument,
    pub(super) club_label: IndexedRampFontDocument,
    pub(super) action_label: IndexedRampFontDocument,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct IndexedRampFontDocument {
    pub(super) source: String,
    pub(super) font_px: f32,
    pub(super) rendering: IndexedRenderingDocument,
    pub(super) glyph_layout: IndexedGlyphLayoutDocument,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct IndexedGlyphLayoutDocument {
    pub(super) slot_width_px: usize,
    pub(super) vertical_shift_px: i32,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum IndexedRenderingDocument {
    CoverageRamp {
        first_ink_index: u8,
        last_ink_index: u8,
    },
    Outlined {
        outline_index: u8,
        fill_index: u8,
    },
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DiarySceneFontDocuments {
    pub(super) location_label: SizedFontDocument,
    pub(super) movement_map_label: SizedFontDocument,
    pub(super) exam_heading: SizedFontDocument,
    pub(super) exam_stamp: SizedFontDocument,
    pub(super) exam_evaluation: SizedFontDocument,
    pub(super) exam_finished: SizedFontDocument,
    pub(super) training_list_label: SizedFontDocument,
    pub(super) training_status_axis: SizedFontDocument,
    pub(super) cooperative_selection: SizedFontDocument,
    pub(super) calendar_title: Option<SizedFontDocument>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ModeSelectFontDocuments {
    pub(super) artwork: FontSelectionDocument,
    pub(super) fixed_label: FontSelectionDocument,
    pub(super) list_label: FontSelectionDocument,
    pub(super) detail_title: FontSelectionDocument,
    pub(super) description: FontSelectionDocument,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CharacterSelectFontDocuments {
    pub(super) system_settings: CharacterSelectFontDocument,
    pub(super) cooperative_diagnosis: CharacterSelectFontDocument,
    pub(super) select_heading: CharacterSelectFontDocument,
    pub(super) mode_menu_heading: CharacterSelectFontDocument,
    pub(super) mode_menu_label: CharacterSelectFontDocument,
    pub(super) cooperative_emblem_character: CharacterSelectFontDocument,
    pub(super) tournament_bracket_label: CharacterSelectFontDocument,
    pub(super) tournament_certificate_title: CharacterSelectFontDocument,
    pub(super) tournament_certificate_label: CharacterSelectFontDocument,
    pub(super) tournament_certificate_body: CharacterSelectFontDocument,
    pub(super) label: CharacterSelectFontDocument,
    pub(super) roster_name: CharacterSelectFontDocument,
    pub(super) fixed_prompt: CharacterSelectFontDocument,
    pub(super) compact_prompt: CharacterSelectFontDocument,
    pub(super) solo_state_prompt: CharacterSelectFontDocument,
    pub(super) common_pause_menu: CharacterSelectFontDocument,
    pub(super) solo_story_intro: CharacterSelectFontDocument,
    pub(super) solo_episode_card: CharacterSelectFontDocument,
    pub(super) practical_selection_label: CharacterSelectFontDocument,
    pub(super) stage_label: CharacterSelectFontDocument,
    pub(super) league_standing_label: CharacterSelectFontDocument,
    pub(super) selection_help: CharacterSelectFontDocument,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CharacterSelectFontDocument {
    pub(super) source: String,
    pub(super) font_px: f32,
    #[serde(default)]
    pub(super) vertical_shift_px: i32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ModeDescendantFontDocuments {
    pub(super) gorin_heading: SizedFontDocument,
    pub(super) gorin_menu: SizedFontDocument,
    pub(super) gorin_speed_marker: Option<SizedFontDocument>,
    pub(super) gorin_small_label: Option<SizedFontDocument>,
    pub(super) edit_badge: Option<SizedFontDocument>,
    pub(super) edit_condition: Option<SizedFontDocument>,
    pub(super) edit_heading: SizedFontDocument,
    pub(super) edit_label: SizedFontDocument,
    pub(super) edit_compact_label: SizedFontDocument,
    pub(super) training_text: ShiftedSizedFontDocument,
    pub(super) battle_counter: ShiftedSizedFontDocument,
    pub(super) edit_team_up_name: SizedFontDocument,
    pub(super) edit_school_label: SizedFontDocument,
    pub(super) edit_runtime_text: ShiftedSizedFontDocument,
    pub(super) password: Option<ShiftedSizedFontDocument>,
    pub(super) practical_title: ShiftedSizedFontDocument,
    pub(super) practical_menu_label: ShiftedSizedFontDocument,
    pub(super) practical_prompt: ShiftedSizedFontDocument,
    pub(super) practical_hint: ShiftedSizedFontDocument,
    pub(super) practical_result_heading: ShiftedSizedFontDocument,
    pub(super) practical_result_label: ShiftedSizedFontDocument,
    pub(super) practical_result_hint: ShiftedSizedFontDocument,
    pub(super) practical_result_action: ShiftedSizedFontDocument,
    pub(super) practical_result_small_judgment: ShiftedSizedFontDocument,
    pub(super) practical_result_judgment_stamp: ShiftedSizedFontDocument,
    pub(super) practical_result_branding: ShiftedSizedFontDocument,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ShiftedSizedFontDocument {
    pub(super) source: String,
    pub(super) font_px: f32,
    #[serde(default)]
    pub(super) vertical_shift_px: i32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OptionsFontDocuments {
    pub(super) heading: SizedFontDocument,
    pub(super) help: SizedFontDocument,
    pub(super) label: SizedFontDocument,
    pub(super) value: SizedFontDocument,
    pub(super) action: SizedFontDocument,
    pub(super) records_main: RecordsMainFontDocuments,
    pub(super) records_prompt: SizedFontDocument,
    pub(super) records_status: RecordsStatusFontDocuments,
    pub(super) background: OptionsBackgroundFontDocuments,
    pub(super) description: SizedFontDocument,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RecordsMainFontDocuments {
    pub(super) heading: SizedFontDocument,
    pub(super) item: SizedFontDocument,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RecordsStatusFontDocuments {
    pub(super) heading: SizedFontDocument,
    pub(super) message: SizedFontDocument,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OptionsBackgroundFontDocuments {
    pub(super) school_name: SizedFontDocument,
    pub(super) crest_mark: SizedFontDocument,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FontSelectionDocument {
    pub(super) source: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SizedFontDocument {
    pub(super) source: String,
    pub(super) font_px: f32,
}
