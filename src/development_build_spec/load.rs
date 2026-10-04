use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};

use crate::bonus_inventory::{BonusInventoryFontSources, BonusInventoryTextStyleSource};
use crate::bonus_menu::BonusMenuFontSources;
use crate::character_select_graphics::{CharacterSelectFontSources, CharacterSelectFontStyle};
use crate::diary_header::{
    DiaryHeaderFontSources, DiaryHeaderFontStyle, DiaryHeaderGlyphLayout,
    DiaryHeaderIndexedRendering,
};
use crate::diary_scene::DiarySceneFontSources;
use crate::mode_descendant_graphics::ModeDescendantFontSources;
use crate::mode_select::ModeSelectFontSources;
use crate::name_input::NicknameHudGlyphStyle;
use crate::options::{
    OptionsBackgroundFontStyles, OptionsFontStyle, OptionsFontStyles, RecordsMainFontStyles,
    RecordsStatusFontStyles,
};
use crate::pipeline::sha256_bytes;
use crate::shop_ui::ShopUiFontSources;
use crate::title_menu::TitleMenuFontStyle;

use super::document::{
    BonusInventoryTextStyleDocument, CharacterSelectFontDocument, DevelopmentBuildSpecDocument,
    FontSelectionDocument, IndexedRampFontDocument, IndexedRenderingDocument,
    ShiftedSizedFontDocument, SizedFontDocument,
};
use super::model::{
    DevelopmentAssetPaths, DevelopmentFontSources, DevelopmentSpecificationPaths,
    LoadedDevelopmentBuildSpec, NameEntryFontSources, ShiftedSizedFontSource, SizedFontSource,
};

const BUILD_SPEC_KIND: &str = "justice_gakuen2_development_build_spec";

pub fn load_development_build_spec(path: &Path) -> Result<LoadedDevelopmentBuildSpec> {
    let bytes = std::fs::read(path)
        .with_context(|| format!("failed to read development build spec {}", path.display()))?;
    let document: DevelopmentBuildSpecDocument = serde_json::from_slice(&bytes)
        .with_context(|| format!("failed to parse development build spec {}", path.display()))?;
    ensure!(
        document.kind == BUILD_SPEC_KIND,
        "unsupported development build spec kind {:?}",
        document.kind
    );
    ensure!(
        !document.font_sources.is_empty(),
        "development build spec has no font sources"
    );

    crate::product_assets::bind_root_from_spec(path)?;
    let base = path.parent().unwrap_or_else(|| Path::new("."));
    let specifications = DevelopmentSpecificationPaths {
        mode_select_descendants: require_path(
            base,
            &document.specifications.mode_select_descendants,
            "MODE SELECT descendant surface inventory",
        )?,
    };
    let assets = DevelopmentAssetPaths {
        pocket_help: document
            .assets
            .pocket_help
            .as_ref()
            .map(|p| require_directory(base, p, "pocket_help assets"))
            .transpose()?,
        endings: document
            .assets
            .endings
            .as_ref()
            .map(|p| require_directory(base, p, "endings assets"))
            .transpose()?,
        staff_roll: document
            .assets
            .staff_roll
            .as_ref()
            .map(|p| require_directory(base, p, "staff-roll credits"))
            .transpose()?,
        card_art: document
            .assets
            .card_art
            .as_ref()
            .map(|p| require_directory(base, p, "card artwork"))
            .transpose()?,
        loading_art: document
            .assets
            .loading_art
            .as_ref()
            .map(|p| require_directory(base, p, "loading artwork"))
            .transpose()?,
        bonus_menu: require_directory(base, &document.assets.bonus_menu, "bonus main-menu assets")?,
        bonus_inventory: require_directory(
            base,
            &document.assets.bonus_inventory,
            "bonus inventory assets",
        )?,
        bonus_inventory_action_labels: require_directory(
            base,
            &document.assets.bonus_inventory_action_labels,
            "bonus inventory action-label assets",
        )?,
        bonus_confirmation: require_directory(
            base,
            &document.assets.bonus_confirmation,
            "bonus confirmation assets",
        )?,
        bonus_inventory_card_acquisition: require_directory(
            base,
            &document.assets.bonus_inventory_card_acquisition,
            "bonus inventory card-acquisition assets",
        )?,
        bonus_inventory_memory_card_swap: require_directory(
            base,
            &document.assets.bonus_inventory_memory_card_swap,
            "bonus inventory memory-card-swap assets",
        )?,
        bonus_j_bank_return_label: require_directory(
            base,
            &document.assets.bonus_j_bank_return_label,
            "bonus J-BANK return-label assets",
        )?,
        bonus_inventory_stock_label: require_directory(
            base,
            &document.assets.bonus_inventory_stock_label,
            "bonus inventory stock-label assets",
        )?,
        bonus_page_indicator: require_path(
            base,
            &document.assets.bonus_page_indicator,
            "bonus page-indicator asset",
        )?,
        bonus_shop_exit_confirmation: require_directory(
            base,
            &document.assets.bonus_shop_exit_confirmation,
            "bonus shop exit-confirmation assets",
        )?,
        bonus_shop_text: require_directory(
            base,
            &document.assets.bonus_shop_text,
            "bonus shop pointer-text assets",
        )?,
        shop_ui: require_directory(base, &document.assets.shop_ui, "shop fixed-UI assets")?,
        dialogue_codebook: require_path(
            base,
            &document.assets.dialogue_codebook,
            "dialogue codebook",
        )?,
        dialogue_translations: require_directory(
            base,
            &document.assets.dialogue_translations,
            "dialogue translations",
        )?,
        dialogue_selector_translations: require_directory(
            base,
            &document.assets.dialogue_selector_translations,
            "dialogue selector translations",
        )?,
        name_entry_candidates: require_directory(
            base,
            &document.assets.name_entry_candidates,
            "name-entry candidates",
        )?,
        name_entry_graphics: require_directory(
            base,
            &document.assets.name_entry_graphics,
            "name-entry graphics",
        )?,
        diary_header: require_directory(
            base,
            &document.assets.diary_header,
            "diary header assets",
        )?,
        diary_scenes: require_directory(base, &document.assets.diary_scenes, "diary scene assets")?,
        mode_select: require_directory(base, &document.assets.mode_select, "MODE SELECT assets")?,
        character_select: require_directory(
            base,
            &document.assets.character_select,
            "character-select assets",
        )?,
        mode_descendants: require_directory(
            base,
            &document.assets.mode_descendants,
            "MODE SELECT descendant assets",
        )?,
        practical_instructions: require_directory(
            base,
            &document.assets.practical_instructions,
            "practical-instruction assets",
        )?,
        options: require_directory(base, &document.assets.options, "options assets")?,
        title_menu: require_directory(
            base,
            &document.assets.title_menu,
            "title-adjacent menu assets",
        )?,
        title_notice: require_directory(
            base,
            &document.assets.title_notice,
            "title notice assets",
        )?,
    };

    let mut font_paths = BTreeMap::new();
    for (id, source) in &document.font_sources {
        ensure!(!id.trim().is_empty(), "font source id is empty");
        let font_path = resolve_path(base, &source.path);
        let font_bytes = std::fs::read(&font_path).with_context(|| {
            format!("failed to read font source {id} at {}", font_path.display())
        })?;
        let actual_sha256 = sha256_bytes(&font_bytes);
        ensure!(
            actual_sha256 == source.sha256,
            "font source {id} hash mismatch: expected {}, got {}",
            source.sha256,
            actual_sha256
        );
        font_paths.insert(id.clone(), font_path);
    }

    let mut used_sources = BTreeSet::new();
    let fonts = DevelopmentFontSources {
        bonus_menu: BonusMenuFontSources {
            heading: resolve_sized_font(
                "bonus_menu.heading",
                &document.fonts.bonus_menu.heading,
                &font_paths,
                &mut used_sources,
            )?,
            entry: resolve_sized_font(
                "bonus_menu.entry",
                &document.fonts.bonus_menu.entry,
                &font_paths,
                &mut used_sources,
            )?,
            compact_entry: resolve_sized_font(
                "bonus_menu.compact_entry",
                &document.fonts.bonus_menu.compact_entry,
                &font_paths,
                &mut used_sources,
            )?,
        },
        bonus_inventory: BonusInventoryFontSources {
            device_text: resolve_bonus_inventory_style(
                "bonus_inventory.device_text",
                &document.fonts.bonus_inventory.device_text,
                &font_paths,
                &mut used_sources,
            )?,
            item_label: resolve_bonus_inventory_style(
                "bonus_inventory.item_label",
                &document.fonts.bonus_inventory.item_label,
                &font_paths,
                &mut used_sources,
            )?,
            title: resolve_bonus_inventory_style(
                "bonus_inventory.title",
                &document.fonts.bonus_inventory.title,
                &font_paths,
                &mut used_sources,
            )?,
            compact_label: resolve_bonus_inventory_style(
                "bonus_inventory.compact_label",
                &document.fonts.bonus_inventory.compact_label,
                &font_paths,
                &mut used_sources,
            )?,
            large_label: resolve_bonus_inventory_style(
                "bonus_inventory.large_label",
                &document.fonts.bonus_inventory.large_label,
                &font_paths,
                &mut used_sources,
            )?,
            compact_action: resolve_bonus_inventory_style(
                "bonus_inventory.compact_action",
                &document.fonts.bonus_inventory.compact_action,
                &font_paths,
                &mut used_sources,
            )?,
            large_action: resolve_bonus_inventory_style(
                "bonus_inventory.large_action",
                &document.fonts.bonus_inventory.large_action,
                &font_paths,
                &mut used_sources,
            )?,
            help: resolve_bonus_inventory_style(
                "bonus_inventory.help",
                &document.fonts.bonus_inventory.help,
                &font_paths,
                &mut used_sources,
            )?,
            viewer_navigation: resolve_bonus_inventory_style(
                "bonus_inventory.viewer_navigation",
                &document.fonts.bonus_inventory.viewer_navigation,
                &font_paths,
                &mut used_sources,
            )?,
            viewer_card_placeholder: resolve_bonus_inventory_style(
                "bonus_inventory.viewer_card_placeholder",
                &document.fonts.bonus_inventory.viewer_card_placeholder,
                &font_paths,
                &mut used_sources,
            )?,
        },
        bonus_inventory_action_labels: resolve_bonus_inventory_style(
            "bonus_inventory_action_labels",
            &document.fonts.bonus_inventory_action_labels,
            &font_paths,
            &mut used_sources,
        )?,
        bonus_confirmation: resolve_bonus_inventory_style(
            "bonus_confirmation",
            &document.fonts.bonus_confirmation,
            &font_paths,
            &mut used_sources,
        )?,
        bonus_inventory_card_acquisition: resolve_bonus_inventory_style(
            "bonus_inventory_card_acquisition",
            &document.fonts.bonus_inventory_card_acquisition,
            &font_paths,
            &mut used_sources,
        )?,
        bonus_inventory_memory_card_swap: resolve_bonus_inventory_style(
            "bonus_inventory_memory_card_swap",
            &document.fonts.bonus_inventory_memory_card_swap,
            &font_paths,
            &mut used_sources,
        )?,
        bonus_j_bank_return_label: resolve_bonus_inventory_style(
            "bonus_j_bank_return_label",
            &document.fonts.bonus_j_bank_return_label,
            &font_paths,
            &mut used_sources,
        )?,
        bonus_inventory_stock_label: resolve_bonus_inventory_style(
            "bonus_inventory_stock_label",
            &document.fonts.bonus_inventory_stock_label,
            &font_paths,
            &mut used_sources,
        )?,
        bonus_page_indicator: resolve_bonus_inventory_style(
            "bonus_page_indicator",
            &document.fonts.bonus_page_indicator,
            &font_paths,
            &mut used_sources,
        )?,
        bonus_shop_exit_confirmation: resolve_bonus_inventory_style(
            "bonus_shop_exit_confirmation",
            &document.fonts.bonus_shop_exit_confirmation,
            &font_paths,
            &mut used_sources,
        )?,
        bonus_shop_text: crate::bonus_shop_text_source::BonusShopTextFontSources {
            product_description: resolve_bonus_inventory_style(
                "bonus_shop_text.product_description",
                &document.fonts.bonus_shop_text.product_description,
                &font_paths,
                &mut used_sources,
            )?,
            product_label: resolve_bonus_inventory_style(
                "bonus_shop_text.product_label",
                &document.fonts.bonus_shop_text.product_label,
                &font_paths,
                &mut used_sources,
            )?,
            clerk_dialogue: resolve_bonus_inventory_style(
                "bonus_shop_text.clerk_dialogue",
                &document.fonts.bonus_shop_text.clerk_dialogue,
                &font_paths,
                &mut used_sources,
            )?,
        },
        shop_ui: ShopUiFontSources {
            heading: resolve_sized_font(
                "shop_ui.heading",
                &document.fonts.shop_ui.heading,
                &font_paths,
                &mut used_sources,
            )?,
            current_points: resolve_sized_font(
                "shop_ui.current_points",
                &document.fonts.shop_ui.current_points,
                &font_paths,
                &mut used_sources,
            )?,
            heading_tracking_px: document.fonts.shop_ui.heading_tracking_px,
            current_points_tracking_px: document.fonts.shop_ui.current_points_tracking_px,
        },
        dialogue_body: resolve_sized_font(
            "dialogue_body",
            &document.fonts.dialogue_body,
            &font_paths,
            &mut used_sources,
        )?,
        name_entry: NameEntryFontSources {
            candidates: resolve_sized_font(
                "name_entry.candidates",
                &document.fonts.name_entry.candidates,
                &font_paths,
                &mut used_sources,
            )?,
            keyboard_keys: resolve_sized_font(
                "name_entry.keyboard_keys",
                &document.fonts.name_entry.keyboard_keys,
                &font_paths,
                &mut used_sources,
            )?,
            composed_glyphs: resolve_sized_font(
                "name_entry.composed_glyphs",
                &document.fonts.name_entry.composed_glyphs,
                &font_paths,
                &mut used_sources,
            )?,
            roster_glyphs: resolve_sized_font(
                "name_entry.roster_glyphs",
                &document.fonts.name_entry.roster_glyphs,
                &font_paths,
                &mut used_sources,
            )?,
            fixed_graphics: resolve_font(
                "name_entry.fixed_graphics",
                &document.fonts.name_entry.fixed_graphics,
                &font_paths,
                &mut used_sources,
            )?,
        },
        diary_header: DiaryHeaderFontSources {
            calendar_text: resolve_indexed_ramp_font(
                "diary_header.calendar_text",
                &document.fonts.diary_header.calendar_text,
                &font_paths,
                &mut used_sources,
            )?,
            status_label: resolve_indexed_ramp_font(
                "diary_header.status_label",
                &document.fonts.diary_header.status_label,
                &font_paths,
                &mut used_sources,
            )?,
            club_label: resolve_indexed_ramp_font(
                "diary_header.club_label",
                &document.fonts.diary_header.club_label,
                &font_paths,
                &mut used_sources,
            )?,
            action_label: resolve_indexed_ramp_font(
                "diary_header.action_label",
                &document.fonts.diary_header.action_label,
                &font_paths,
                &mut used_sources,
            )?,
        },
        diary_scene: DiarySceneFontSources {
            location_label: resolve_sized_font(
                "diary_scene.location_label",
                &document.fonts.diary_scene.location_label,
                &font_paths,
                &mut used_sources,
            )?,
            movement_map_label: resolve_sized_font(
                "diary_scene.movement_map_label",
                &document.fonts.diary_scene.movement_map_label,
                &font_paths,
                &mut used_sources,
            )?,
            exam_heading: resolve_sized_font(
                "diary_scene.exam_heading",
                &document.fonts.diary_scene.exam_heading,
                &font_paths,
                &mut used_sources,
            )?,
            exam_stamp: resolve_sized_font(
                "diary_scene.exam_stamp",
                &document.fonts.diary_scene.exam_stamp,
                &font_paths,
                &mut used_sources,
            )?,
            exam_evaluation: resolve_sized_font(
                "diary_scene.exam_evaluation",
                &document.fonts.diary_scene.exam_evaluation,
                &font_paths,
                &mut used_sources,
            )?,
            exam_finished: resolve_sized_font(
                "diary_scene.exam_finished",
                &document.fonts.diary_scene.exam_finished,
                &font_paths,
                &mut used_sources,
            )?,
            training_list_label: resolve_sized_font(
                "diary_scene.training_list_label",
                &document.fonts.diary_scene.training_list_label,
                &font_paths,
                &mut used_sources,
            )?,
            training_status_axis: resolve_sized_font(
                "diary_scene.training_status_axis",
                &document.fonts.diary_scene.training_status_axis,
                &font_paths,
                &mut used_sources,
            )?,
            cooperative_selection: resolve_sized_font(
                "diary_scene.cooperative_selection",
                &document.fonts.diary_scene.cooperative_selection,
                &font_paths,
                &mut used_sources,
            )?,
            calendar_title: document
                .fonts
                .diary_scene
                .calendar_title
                .as_ref()
                .map(|font| {
                    resolve_sized_font(
                        "diary_scene.calendar_title",
                        font,
                        &font_paths,
                        &mut used_sources,
                    )
                })
                .transpose()?,
        },
        mode_select: ModeSelectFontSources {
            artwork: resolve_font(
                "mode_select.artwork",
                &document.fonts.mode_select.artwork,
                &font_paths,
                &mut used_sources,
            )?,
            fixed_label: resolve_font(
                "mode_select.fixed_label",
                &document.fonts.mode_select.fixed_label,
                &font_paths,
                &mut used_sources,
            )?,
            list_label: resolve_font(
                "mode_select.list_label",
                &document.fonts.mode_select.list_label,
                &font_paths,
                &mut used_sources,
            )?,
            detail_title: resolve_font(
                "mode_select.detail_title",
                &document.fonts.mode_select.detail_title,
                &font_paths,
                &mut used_sources,
            )?,
            description: resolve_font(
                "mode_select.description",
                &document.fonts.mode_select.description,
                &font_paths,
                &mut used_sources,
            )?,
        },
        character_select: CharacterSelectFontSources {
            system_settings: resolve_character_select_font(
                "character_select.system_settings",
                &document.fonts.character_select.system_settings,
                &font_paths,
                &mut used_sources,
            )?,
            cooperative_diagnosis: resolve_character_select_font(
                "character_select.cooperative_diagnosis",
                &document.fonts.character_select.cooperative_diagnosis,
                &font_paths,
                &mut used_sources,
            )?,
            select_heading: resolve_character_select_font(
                "character_select.select_heading",
                &document.fonts.character_select.select_heading,
                &font_paths,
                &mut used_sources,
            )?,
            mode_menu_heading: resolve_character_select_font(
                "character_select.mode_menu_heading",
                &document.fonts.character_select.mode_menu_heading,
                &font_paths,
                &mut used_sources,
            )?,
            mode_menu_label: resolve_character_select_font(
                "character_select.mode_menu_label",
                &document.fonts.character_select.mode_menu_label,
                &font_paths,
                &mut used_sources,
            )?,
            cooperative_emblem_character: resolve_character_select_font(
                "character_select.cooperative_emblem_character",
                &document.fonts.character_select.cooperative_emblem_character,
                &font_paths,
                &mut used_sources,
            )?,
            tournament_bracket_label: resolve_character_select_font(
                "character_select.tournament_bracket_label",
                &document.fonts.character_select.tournament_bracket_label,
                &font_paths,
                &mut used_sources,
            )?,
            tournament_certificate_title: resolve_character_select_font(
                "character_select.tournament_certificate_title",
                &document.fonts.character_select.tournament_certificate_title,
                &font_paths,
                &mut used_sources,
            )?,
            tournament_certificate_label: resolve_character_select_font(
                "character_select.tournament_certificate_label",
                &document.fonts.character_select.tournament_certificate_label,
                &font_paths,
                &mut used_sources,
            )?,
            tournament_certificate_body: resolve_character_select_font(
                "character_select.tournament_certificate_body",
                &document.fonts.character_select.tournament_certificate_body,
                &font_paths,
                &mut used_sources,
            )?,
            label: resolve_character_select_font(
                "character_select.label",
                &document.fonts.character_select.label,
                &font_paths,
                &mut used_sources,
            )?,
            roster_name: resolve_character_select_font(
                "character_select.roster_name",
                &document.fonts.character_select.roster_name,
                &font_paths,
                &mut used_sources,
            )?,
            fixed_prompt: resolve_character_select_font(
                "character_select.fixed_prompt",
                &document.fonts.character_select.fixed_prompt,
                &font_paths,
                &mut used_sources,
            )?,
            compact_prompt: resolve_character_select_font(
                "character_select.compact_prompt",
                &document.fonts.character_select.compact_prompt,
                &font_paths,
                &mut used_sources,
            )?,
            solo_state_prompt: resolve_character_select_font(
                "character_select.solo_state_prompt",
                &document.fonts.character_select.solo_state_prompt,
                &font_paths,
                &mut used_sources,
            )?,
            common_pause_menu: resolve_character_select_font(
                "character_select.common_pause_menu",
                &document.fonts.character_select.common_pause_menu,
                &font_paths,
                &mut used_sources,
            )?,
            solo_story_intro: resolve_character_select_font(
                "character_select.solo_story_intro",
                &document.fonts.character_select.solo_story_intro,
                &font_paths,
                &mut used_sources,
            )?,
            solo_episode_card: resolve_character_select_font(
                "character_select.solo_episode_card",
                &document.fonts.character_select.solo_episode_card,
                &font_paths,
                &mut used_sources,
            )?,
            practical_selection_label: resolve_character_select_font(
                "character_select.practical_selection_label",
                &document.fonts.character_select.practical_selection_label,
                &font_paths,
                &mut used_sources,
            )?,
            stage_label: resolve_character_select_font(
                "character_select.stage_label",
                &document.fonts.character_select.stage_label,
                &font_paths,
                &mut used_sources,
            )?,
            league_standing_label: resolve_character_select_font(
                "character_select.league_standing_label",
                &document.fonts.character_select.league_standing_label,
                &font_paths,
                &mut used_sources,
            )?,
            selection_help: resolve_character_select_font(
                "character_select.selection_help",
                &document.fonts.character_select.selection_help,
                &font_paths,
                &mut used_sources,
            )?,
        },
        mode_descendants: ModeDescendantFontSources {
            // Continue has its own atlas and CLUT, but shares the calendar face,
            // size and baseline with the Diary header.
            calendar_text: {
                let style = resolve_indexed_ramp_font(
                    "diary_header.calendar_text",
                    &document.fonts.diary_header.calendar_text,
                    &font_paths,
                    &mut used_sources,
                )?;
                ShiftedSizedFontSource {
                    path: style.path,
                    font_px: style.font_px,
                    vertical_shift_px: style.glyph_layout.vertical_shift_px,
                }
            },
            gorin_heading: resolve_sized_font(
                "mode_descendants.gorin_heading",
                &document.fonts.mode_descendants.gorin_heading,
                &font_paths,
                &mut used_sources,
            )?,
            gorin_menu: resolve_sized_font(
                "mode_descendants.gorin_menu",
                &document.fonts.mode_descendants.gorin_menu,
                &font_paths,
                &mut used_sources,
            )?,
            gorin_speed_marker: document
                .fonts
                .mode_descendants
                .gorin_speed_marker
                .as_ref()
                .map(|role| {
                    resolve_sized_font(
                        "mode_descendants.gorin_speed_marker",
                        role,
                        &font_paths,
                        &mut used_sources,
                    )
                })
                .transpose()?,
            gorin_small_label: document
                .fonts
                .mode_descendants
                .gorin_small_label
                .as_ref()
                .map(|role| {
                    resolve_sized_font(
                        "mode_descendants.gorin_small_label",
                        role,
                        &font_paths,
                        &mut used_sources,
                    )
                })
                .transpose()?,
            edit_badge: document
                .fonts
                .mode_descendants
                .edit_badge
                .as_ref()
                .map(|role| {
                    resolve_sized_font(
                        "mode_descendants.edit_badge",
                        role,
                        &font_paths,
                        &mut used_sources,
                    )
                })
                .transpose()?,
            edit_condition: document
                .fonts
                .mode_descendants
                .edit_condition
                .as_ref()
                .map(|role| {
                    resolve_sized_font(
                        "mode_descendants.edit_condition",
                        role,
                        &font_paths,
                        &mut used_sources,
                    )
                })
                .transpose()?,
            edit_heading: resolve_sized_font(
                "mode_descendants.edit_heading",
                &document.fonts.mode_descendants.edit_heading,
                &font_paths,
                &mut used_sources,
            )?,
            edit_label: resolve_sized_font(
                "mode_descendants.edit_label",
                &document.fonts.mode_descendants.edit_label,
                &font_paths,
                &mut used_sources,
            )?,
            battle_counter: resolve_shifted_sized_font(
                "mode_descendants.battle_counter",
                &document.fonts.mode_descendants.battle_counter,
                &font_paths,
                &mut used_sources,
            )?,
            training_text: resolve_shifted_sized_font(
                "mode_descendants.training_text",
                &document.fonts.mode_descendants.training_text,
                &font_paths,
                &mut used_sources,
            )?,
            edit_compact_label: resolve_sized_font(
                "mode_descendants.edit_compact_label",
                &document.fonts.mode_descendants.edit_compact_label,
                &font_paths,
                &mut used_sources,
            )?,
            edit_team_up_name: resolve_sized_font(
                "mode_descendants.edit_team_up_name",
                &document.fonts.mode_descendants.edit_team_up_name,
                &font_paths,
                &mut used_sources,
            )?,
            edit_school_label: resolve_sized_font(
                "mode_descendants.edit_school_label",
                &document.fonts.mode_descendants.edit_school_label,
                &font_paths,
                &mut used_sources,
            )?,
            edit_runtime_text: resolve_shifted_sized_font(
                "mode_descendants.edit_runtime_text",
                &document.fonts.mode_descendants.edit_runtime_text,
                &font_paths,
                &mut used_sources,
            )?,
            password: document
                .fonts
                .mode_descendants
                .password
                .as_ref()
                .map(|role| {
                    resolve_shifted_sized_font(
                        "mode_descendants.password",
                        role,
                        &font_paths,
                        &mut used_sources,
                    )
                })
                .transpose()?,
            practical_title: resolve_shifted_sized_font(
                "mode_descendants.practical_title",
                &document.fonts.mode_descendants.practical_title,
                &font_paths,
                &mut used_sources,
            )?,
            practical_menu_label: resolve_shifted_sized_font(
                "mode_descendants.practical_menu_label",
                &document.fonts.mode_descendants.practical_menu_label,
                &font_paths,
                &mut used_sources,
            )?,
            practical_gameplay: resolve_shifted_sized_font(
                "practical_instruction",
                &document.fonts.practical_instruction,
                &font_paths,
                &mut used_sources,
            )?,
            practical_prompt: resolve_shifted_sized_font(
                "mode_descendants.practical_prompt",
                &document.fonts.mode_descendants.practical_prompt,
                &font_paths,
                &mut used_sources,
            )?,
            practical_hint: resolve_shifted_sized_font(
                "mode_descendants.practical_hint",
                &document.fonts.mode_descendants.practical_hint,
                &font_paths,
                &mut used_sources,
            )?,
            practical_result_heading: resolve_shifted_sized_font(
                "mode_descendants.practical_result_heading",
                &document.fonts.mode_descendants.practical_result_heading,
                &font_paths,
                &mut used_sources,
            )?,
            practical_result_label: resolve_shifted_sized_font(
                "mode_descendants.practical_result_label",
                &document.fonts.mode_descendants.practical_result_label,
                &font_paths,
                &mut used_sources,
            )?,
            practical_result_hint: resolve_shifted_sized_font(
                "mode_descendants.practical_result_hint",
                &document.fonts.mode_descendants.practical_result_hint,
                &font_paths,
                &mut used_sources,
            )?,
            practical_result_action: resolve_shifted_sized_font(
                "mode_descendants.practical_result_action",
                &document.fonts.mode_descendants.practical_result_action,
                &font_paths,
                &mut used_sources,
            )?,
            practical_result_small_judgment: resolve_shifted_sized_font(
                "mode_descendants.practical_result_small_judgment",
                &document
                    .fonts
                    .mode_descendants
                    .practical_result_small_judgment,
                &font_paths,
                &mut used_sources,
            )?,
            practical_result_judgment_stamp: resolve_shifted_sized_font(
                "mode_descendants.practical_result_judgment_stamp",
                &document
                    .fonts
                    .mode_descendants
                    .practical_result_judgment_stamp,
                &font_paths,
                &mut used_sources,
            )?,
            practical_result_branding: resolve_shifted_sized_font(
                "mode_descendants.practical_result_branding",
                &document.fonts.mode_descendants.practical_result_branding,
                &font_paths,
                &mut used_sources,
            )?,
        },
        practical_instruction: resolve_shifted_sized_font(
            "practical_instruction",
            &document.fonts.practical_instruction,
            &font_paths,
            &mut used_sources,
        )?,
        options: OptionsFontStyles {
            heading: resolve_options_font(
                "options.heading",
                &document.fonts.options.heading,
                &font_paths,
                &mut used_sources,
            )?,
            help: resolve_options_font(
                "options.help",
                &document.fonts.options.help,
                &font_paths,
                &mut used_sources,
            )?,
            label: resolve_options_font(
                "options.label",
                &document.fonts.options.label,
                &font_paths,
                &mut used_sources,
            )?,
            value: resolve_options_font(
                "options.value",
                &document.fonts.options.value,
                &font_paths,
                &mut used_sources,
            )?,
            action: resolve_options_font(
                "options.action",
                &document.fonts.options.action,
                &font_paths,
                &mut used_sources,
            )?,
            records_main: RecordsMainFontStyles {
                heading: resolve_options_font(
                    "options.records_main.heading",
                    &document.fonts.options.records_main.heading,
                    &font_paths,
                    &mut used_sources,
                )?,
                item: resolve_options_font(
                    "options.records_main.item",
                    &document.fonts.options.records_main.item,
                    &font_paths,
                    &mut used_sources,
                )?,
            },
            records_prompt: resolve_options_font(
                "options.records_prompt",
                &document.fonts.options.records_prompt,
                &font_paths,
                &mut used_sources,
            )?,
            records_status: RecordsStatusFontStyles {
                heading: resolve_options_font(
                    "options.records_status.heading",
                    &document.fonts.options.records_status.heading,
                    &font_paths,
                    &mut used_sources,
                )?,
                message: resolve_options_font(
                    "options.records_status.message",
                    &document.fonts.options.records_status.message,
                    &font_paths,
                    &mut used_sources,
                )?,
            },
            background: OptionsBackgroundFontStyles {
                school_name: resolve_options_font(
                    "options.background.school_name",
                    &document.fonts.options.background.school_name,
                    &font_paths,
                    &mut used_sources,
                )?,
                crest_mark: resolve_options_font(
                    "options.background.crest_mark",
                    &document.fonts.options.background.crest_mark,
                    &font_paths,
                    &mut used_sources,
                )?,
            },
            description: resolve_options_font(
                "options.description",
                &document.fonts.options.description,
                &font_paths,
                &mut used_sources,
            )?,
        },
        title_menu: {
            let resolved = resolve_sized_font(
                "title_menu",
                &document.fonts.title_menu,
                &font_paths,
                &mut used_sources,
            )?;
            TitleMenuFontStyle {
                font: resolved.path,
                font_px: resolved.font_px,
            }
        },
        pocket_help: document
            .fonts
            .pocket_help
            .iter()
            .map(|(role, font)| {
                Ok((
                    role.clone(),
                    resolve_sized_font(
                        &format!("pocket_help.{role}"),
                        font,
                        &font_paths,
                        &mut used_sources,
                    )?,
                ))
            })
            .collect::<Result<_>>()?,
        staff_roll: document
            .fonts
            .staff_roll
            .as_ref()
            .map(|font| resolve_sized_font("staff_roll", font, &font_paths, &mut used_sources))
            .transpose()?,
        card_art: document
            .fonts
            .card_art
            .iter()
            .map(|(role, font)| {
                Ok((
                    role.clone(),
                    resolve_sized_font(
                        &format!("card_art.{role}"),
                        font,
                        &font_paths,
                        &mut used_sources,
                    )?,
                ))
            })
            .collect::<Result<_>>()?,
        loading_art: document
            .fonts
            .loading_art
            .iter()
            .map(|(role, font)| {
                Ok((
                    role.clone(),
                    resolve_sized_font(
                        &format!("loading_art.{role}"),
                        font,
                        &font_paths,
                        &mut used_sources,
                    )?,
                ))
            })
            .collect::<Result<_>>()?,
        shared_menu_numerals: resolve_sized_font(
            "shared_menu_numerals",
            &document.fonts.shared_menu_numerals,
            &font_paths,
            &mut used_sources,
        )?,
        title_notice: resolve_sized_font(
            "title_notice",
            &document.fonts.title_notice,
            &font_paths,
            &mut used_sources,
        )?,
    };
    ensure!(
        used_sources.len() == font_paths.len(),
        "development build spec declares unused font sources: {}",
        font_paths
            .keys()
            .filter(|id| !used_sources.contains(*id))
            .cloned()
            .collect::<Vec<_>>()
            .join(", ")
    );

    ensure!(
        document.runtime_glyph_layouts.nickname_hud.scale_percent > 0,
        "runtime_glyph_layouts.nickname_hud scale_percent must be positive"
    );
    Ok(LoadedDevelopmentBuildSpec {
        specifications,
        assets,
        fonts,
        nickname_hud_glyph_style: NicknameHudGlyphStyle {
            scale_percent: document.runtime_glyph_layouts.nickname_hud.scale_percent,
            vertical_shift_px: document
                .runtime_glyph_layouts
                .nickname_hud
                .vertical_shift_px,
        },
        sha256: sha256_bytes(&bytes),
    })
}

fn resolve_sized_font(
    role: &str,
    document: &SizedFontDocument,
    font_paths: &BTreeMap<String, PathBuf>,
    used_sources: &mut BTreeSet<String>,
) -> Result<SizedFontSource> {
    ensure!(
        document.font_px.is_finite() && document.font_px > 0.0,
        "{role} font_px must be finite and positive"
    );
    Ok(SizedFontSource {
        path: resolve_source(role, &document.source, font_paths, used_sources)?,
        font_px: document.font_px,
    })
}

fn resolve_character_select_font(
    role: &str,
    document: &CharacterSelectFontDocument,
    font_paths: &BTreeMap<String, PathBuf>,
    used_sources: &mut BTreeSet<String>,
) -> Result<CharacterSelectFontStyle> {
    ensure!(
        document.font_px.is_finite() && document.font_px > 0.0,
        "{role} font_px must be finite and positive"
    );
    Ok(CharacterSelectFontStyle {
        path: resolve_source(role, &document.source, font_paths, used_sources)?,
        font_px: document.font_px,
        vertical_shift_px: document.vertical_shift_px,
    })
}

fn resolve_shifted_sized_font(
    role: &str,
    document: &ShiftedSizedFontDocument,
    font_paths: &BTreeMap<String, PathBuf>,
    used_sources: &mut BTreeSet<String>,
) -> Result<ShiftedSizedFontSource> {
    ensure!(
        document.font_px.is_finite() && document.font_px > 0.0,
        "{role} font_px must be finite and positive"
    );
    Ok(ShiftedSizedFontSource {
        path: resolve_source(role, &document.source, font_paths, used_sources)?,
        font_px: document.font_px,
        vertical_shift_px: document.vertical_shift_px,
    })
}

fn resolve_bonus_inventory_style(
    role: &str,
    document: &BonusInventoryTextStyleDocument,
    font_paths: &BTreeMap<String, PathBuf>,
    used_sources: &mut BTreeSet<String>,
) -> Result<BonusInventoryTextStyleSource> {
    ensure!(
        document.font_px.is_finite() && document.font_px > 0.0 && document.tracking_px.is_finite(),
        "{role} font size must be positive and all settings must be finite"
    );
    Ok(BonusInventoryTextStyleSource {
        path: resolve_source(role, &document.source, font_paths, used_sources)?,
        font_px: document.font_px,
        tracking_px: document.tracking_px,
        vertical_shift_px: document.vertical_shift_px,
    })
}

fn resolve_indexed_ramp_font(
    role: &str,
    document: &IndexedRampFontDocument,
    font_paths: &BTreeMap<String, PathBuf>,
    used_sources: &mut BTreeSet<String>,
) -> Result<DiaryHeaderFontStyle> {
    ensure!(
        document.font_px.is_finite() && document.font_px > 0.0,
        "{role} font_px must be finite and positive"
    );
    let rendering = match document.rendering {
        IndexedRenderingDocument::CoverageRamp {
            first_ink_index,
            last_ink_index,
        } => {
            ensure!(
                first_ink_index > 0 && first_ink_index <= last_ink_index && last_ink_index < 16,
                "{role} indexed coverage ramp must be within 1..=15"
            );
            DiaryHeaderIndexedRendering::CoverageRamp {
                first_ink_index,
                last_ink_index,
            }
        }
        IndexedRenderingDocument::Outlined {
            outline_index,
            fill_index,
        } => {
            ensure!(
                outline_index > 0
                    && outline_index < 16
                    && fill_index > 0
                    && fill_index < 16
                    && outline_index != fill_index,
                "{role} outline and fill indices must be distinct 4-bpp ink indices"
            );
            DiaryHeaderIndexedRendering::Outlined {
                outline_index,
                fill_index,
            }
        }
    };
    ensure!(
        document.glyph_layout.slot_width_px > 0,
        "{role} glyph slot width must be positive"
    );
    Ok(DiaryHeaderFontStyle {
        path: resolve_source(role, &document.source, font_paths, used_sources)?,
        font_px: document.font_px,
        rendering,
        glyph_layout: DiaryHeaderGlyphLayout {
            slot_width_px: document.glyph_layout.slot_width_px,
            vertical_shift_px: document.glyph_layout.vertical_shift_px,
        },
    })
}

fn resolve_options_font(
    role: &str,
    document: &SizedFontDocument,
    font_paths: &BTreeMap<String, PathBuf>,
    used_sources: &mut BTreeSet<String>,
) -> Result<OptionsFontStyle> {
    let font = resolve_sized_font(role, document, font_paths, used_sources)?;
    Ok(OptionsFontStyle {
        font: font.path,
        font_px: font.font_px,
    })
}

fn resolve_font(
    role: &str,
    document: &FontSelectionDocument,
    font_paths: &BTreeMap<String, PathBuf>,
    used_sources: &mut BTreeSet<String>,
) -> Result<PathBuf> {
    resolve_source(role, &document.source, font_paths, used_sources)
}

fn resolve_source(
    role: &str,
    source: &str,
    font_paths: &BTreeMap<String, PathBuf>,
    used_sources: &mut BTreeSet<String>,
) -> Result<PathBuf> {
    let font = font_paths
        .get(source)
        .with_context(|| format!("{role} references unknown font source {source:?}"))?;
    used_sources.insert(source.to_string());
    Ok(font.clone())
}

fn require_path(base: &Path, path: &Path, label: &str) -> Result<PathBuf> {
    let path = resolve_path(base, path);
    ensure!(path.is_file(), "{label} does not exist: {}", path.display());
    Ok(path)
}

fn require_directory(base: &Path, path: &Path, label: &str) -> Result<PathBuf> {
    let path = resolve_path(base, path);
    ensure!(
        path.is_dir(),
        "{label} directory does not exist: {}",
        path.display()
    );
    Ok(path)
}

fn resolve_path(base: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    }
}
