use std::path::PathBuf;

use crate::bonus_confirmation::BonusConfirmationFontSource;
use crate::bonus_inventory::BonusInventoryFontSources;
use crate::bonus_inventory_action_labels::BonusInventoryActionLabelFontSource;
use crate::bonus_inventory_card_acquisition::BonusInventoryCardAcquisitionFontSource;
use crate::bonus_inventory_memory_card_swap::BonusInventoryMemoryCardSwapFontSource;
use crate::bonus_inventory_stock_label::BonusInventoryStockLabelFontSource;
use crate::bonus_j_bank_return_label::BonusJBankReturnLabelFontSource;
use crate::bonus_menu::BonusMenuFontSources;
use crate::bonus_page_indicator::BonusPageIndicatorFontSource;
use crate::bonus_shop_exit_confirmation::BonusShopExitConfirmationFontSource;
use crate::bonus_shop_text_source::BonusShopTextFontSources;
use crate::character_select_graphics::CharacterSelectFontSources;
use crate::diary_header::DiaryHeaderFontSources;
use crate::diary_scene::DiarySceneFontSources;
use crate::mode_descendant_graphics::ModeDescendantFontSources;
use crate::mode_select::ModeSelectFontSources;
use crate::name_input::NicknameHudGlyphStyle;
use crate::options::OptionsFontStyles;
use crate::shop_ui::ShopUiFontSources;
use crate::title_menu::TitleMenuFontStyle;

#[derive(Debug, Clone)]
pub struct SizedFontSource {
    pub path: PathBuf,
    pub font_px: f32,
}

#[derive(Debug, Clone)]
pub struct ShiftedSizedFontSource {
    pub path: PathBuf,
    pub font_px: f32,
    pub vertical_shift_px: i32,
}

#[derive(Debug, Clone)]
pub struct DevelopmentAssetPaths {
    pub staff_roll: Option<PathBuf>,
    pub pocket_help: Option<PathBuf>,
    pub endings: Option<PathBuf>,
    pub card_art: Option<PathBuf>,
    pub loading_art: Option<PathBuf>,
    pub bonus_menu: PathBuf,
    pub bonus_inventory: PathBuf,
    pub bonus_inventory_action_labels: PathBuf,
    pub bonus_confirmation: PathBuf,
    pub bonus_inventory_card_acquisition: PathBuf,
    pub bonus_inventory_memory_card_swap: PathBuf,
    pub bonus_j_bank_return_label: PathBuf,
    pub bonus_inventory_stock_label: PathBuf,
    pub bonus_page_indicator: PathBuf,
    pub bonus_shop_exit_confirmation: PathBuf,
    pub bonus_shop_text: PathBuf,
    pub shop_ui: PathBuf,
    pub dialogue_codebook: PathBuf,
    pub dialogue_translations: PathBuf,
    pub dialogue_selector_translations: PathBuf,
    pub name_entry_candidates: PathBuf,
    pub name_entry_graphics: PathBuf,
    pub diary_header: PathBuf,
    pub diary_scenes: PathBuf,
    pub mode_select: PathBuf,
    pub character_select: PathBuf,
    pub mode_descendants: PathBuf,
    pub practical_instructions: PathBuf,
    pub options: PathBuf,
    pub title_menu: PathBuf,
    pub title_notice: PathBuf,
}

#[derive(Debug, Clone)]
pub struct DevelopmentSpecificationPaths {
    pub mode_select_descendants: PathBuf,
}

#[derive(Debug, Clone)]
pub struct NameEntryFontSources {
    pub candidates: SizedFontSource,
    pub keyboard_keys: SizedFontSource,
    pub composed_glyphs: SizedFontSource,
    pub roster_glyphs: SizedFontSource,
    pub fixed_graphics: PathBuf,
}

#[derive(Debug, Clone)]
pub struct DevelopmentFontSources {
    pub staff_roll: Option<SizedFontSource>,
    pub pocket_help: std::collections::BTreeMap<String, SizedFontSource>,
    pub card_art: std::collections::BTreeMap<String, SizedFontSource>,
    pub loading_art: std::collections::BTreeMap<String, SizedFontSource>,
    pub bonus_menu: BonusMenuFontSources,
    pub bonus_inventory: BonusInventoryFontSources,
    pub bonus_inventory_action_labels: BonusInventoryActionLabelFontSource,
    pub bonus_confirmation: BonusConfirmationFontSource,
    pub bonus_inventory_card_acquisition: BonusInventoryCardAcquisitionFontSource,
    pub bonus_inventory_memory_card_swap: BonusInventoryMemoryCardSwapFontSource,
    pub bonus_j_bank_return_label: BonusJBankReturnLabelFontSource,
    pub bonus_inventory_stock_label: BonusInventoryStockLabelFontSource,
    pub bonus_page_indicator: BonusPageIndicatorFontSource,
    pub shared_menu_numerals: SizedFontSource,
    pub bonus_shop_exit_confirmation: BonusShopExitConfirmationFontSource,
    pub bonus_shop_text: BonusShopTextFontSources,
    pub shop_ui: ShopUiFontSources,
    pub dialogue_body: SizedFontSource,
    pub name_entry: NameEntryFontSources,
    pub diary_header: DiaryHeaderFontSources,
    pub diary_scene: DiarySceneFontSources,
    pub mode_select: ModeSelectFontSources,
    pub character_select: CharacterSelectFontSources,
    pub mode_descendants: ModeDescendantFontSources,
    pub practical_instruction: ShiftedSizedFontSource,
    pub options: OptionsFontStyles,
    pub title_menu: TitleMenuFontStyle,
    pub title_notice: SizedFontSource,
}

#[derive(Debug, Clone)]
pub struct LoadedDevelopmentBuildSpec {
    pub specifications: DevelopmentSpecificationPaths,
    pub assets: DevelopmentAssetPaths,
    pub fonts: DevelopmentFontSources,
    pub nickname_hud_glyph_style: NicknameHudGlyphStyle,
    pub sha256: String,
}
