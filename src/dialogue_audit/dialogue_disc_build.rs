use anyhow::{Context, Result, ensure};

use crate::development_build_spec::load_development_build_spec;
use crate::edit_runtime_text::{
    EditRuntimeTextBuildConfig, build_edit_runtime_text, validate_edit_runtime_text_source_bindings,
};
use crate::mode_descendant_graphics::ModeDescendantRecord;
use crate::source_disc::MAIN_EXECUTABLE_PATH;
use crate::source_disc::{
    VerifiedSourceDisc,
    profile::{
        BONUS_MENU_OVERLAY_RECORD, BONUS_MENU_RECORD, CHARACTER_SELECT_COOPERATIVE_MENU_RECORD,
        CHARACTER_SELECT_OVERLAY_RECORDS, CHARACTER_SELECT_TEXTURE_RECORDS,
        EDIT_REGISTRATION_OVERLAY_RECORD, EDIT_REGISTRATION_UI_RECORD, GORIN_MAIN_MENU_RECORD,
        GORIN_SELECTION_OVERLAY_RECORD, MENU_RECORD, MODE_SELECT_OVERLAY_RECORD,
        OPTIONS_INFO_RECORD, OPTIONS_OVERLAY_RECORD, TITLE_MENU_OVERLAY_RECORD,
    },
};
use crate::surface_inventory::{
    ModeSelectDirectEntrySources, ModeSelectInventorySources, load_surface_inventory,
    validate_mode_select_descendant_inventory, validate_mode_select_inventory_source_binding,
};
use crate::title_notice::{
    TitleNoticeBuildConfig, TitleNoticeFontStyle, build_title_notice_from_source,
};

use super::bonus_inventory_development_build::BonusInventoryDevelopmentBuild;
use super::dialogue_disc_build_model::{DialogueDiscBuildConfig, DialogueDiscBuildReport};
use super::dialogue_disc_finalization::{
    DialogueDiscComponentBuilds, DialogueDiscComposition, DialogueDiscFinalization,
    finalize_dialogue_disc,
};
use super::dialogue_disc_outputs::DialogueDiscOutputs;
use super::dialogue_disc_records::{DialogueDiscRecordSources, assemble_dialogue_disc_records};
use super::dialogue_font_build::build_dialogue_font_images_from_source;
use super::dialogue_font_build_model::DialogueFontBuildConfig;
use super::main_executable_record::compose_main_executable_record;
use super::menu_scene_components::{MenuSceneComponents, build_menu_scene_components};
use super::name_runtime_components::{NameRuntimeComponents, build_name_runtime_components};
use super::shop_development_build::ShopDevelopmentBuild;

const FONT_OUTPUT_DIRECTORY: &str = "dialogue-assets";
const TITLE_NOTICE_OUTPUT_DIRECTORY: &str = "title-notice";
const EDIT_RUNTIME_TEXT_OUTPUT_DIRECTORY: &str = "edit-runtime-text";

pub fn build_dialogue_development_disc(
    config: &DialogueDiscBuildConfig,
) -> Result<DialogueDiscBuildReport> {
    let build_started = std::time::Instant::now();
    eprintln!("build: verify inputs and source");
    let build_spec = load_development_build_spec(&config.build_spec)?;
    super::prepared_dialogue::preflight_prepared_dialogue(
        &config.prepared_dialogue,
        &build_spec.assets,
        config.input_policy,
    )?;
    let outputs = DialogueDiscOutputs::prepare(&config.output_dir, config.force)?;
    let source_disc = VerifiedSourceDisc::open_supported(&config.cue)?;
    let surface_inventory =
        load_surface_inventory(&build_spec.specifications.mode_select_descendants)?;
    validate_mode_select_inventory_source_binding(
        &surface_inventory,
        source_disc.source_bin_sha256(),
    )?;
    let (_, main_executable) = source_disc.source().read_record(MAIN_EXECUTABLE_PATH)?;
    let plain_loading = build_spec
        .assets
        .loading_art
        .as_ref()
        .map(|assets| {
            crate::loading_art::build_plain_message(
                source_disc.source(),
                assets,
                &build_spec.fonts.loading_art,
                &config.output_dir.join("plain-loading"),
            )
        })
        .transpose()?;
    let (_, source_mode_select_overlay) = source_disc
        .source()
        .read_record(MODE_SELECT_OVERLAY_RECORD.path)?;
    let source_character_select_overlays = CHARACTER_SELECT_OVERLAY_RECORDS
        .iter()
        .map(|record| {
            source_disc
                .source()
                .read_record(record.path)
                .map(|(_, bytes)| bytes)
        })
        .collect::<Result<Vec<_>>>()?;
    let source_character_select_textures = CHARACTER_SELECT_TEXTURE_RECORDS
        .iter()
        .map(|record| {
            source_disc
                .source()
                .read_record(record.path)
                .map(|(_, bytes)| bytes)
        })
        .collect::<Result<Vec<_>>>()?;
    let (_, source_cooperative_menu) = source_disc
        .source()
        .read_record(CHARACTER_SELECT_COOPERATIVE_MENU_RECORD.path)?;
    let (_, source_options_overlay) = source_disc
        .source()
        .read_record(OPTIONS_OVERLAY_RECORD.path)?;
    let (_, source_options_information) =
        source_disc.source().read_record(OPTIONS_INFO_RECORD.path)?;
    let (_, source_diary_overlay) = source_disc
        .source()
        .read_record(TITLE_MENU_OVERLAY_RECORD.path)?;
    let (_, source_menu) = source_disc.source().read_record(MENU_RECORD.path)?;
    let (_, source_gorin_overlay) = source_disc
        .source()
        .read_record(GORIN_SELECTION_OVERLAY_RECORD.path)?;
    let (_, source_gorin_menu) = source_disc
        .source()
        .read_record(GORIN_MAIN_MENU_RECORD.path)?;
    let (_, source_edit_overlay) = source_disc
        .source()
        .read_record(EDIT_REGISTRATION_OVERLAY_RECORD.path)?;
    let (_, source_edit_ui) = source_disc
        .source()
        .read_record(EDIT_REGISTRATION_UI_RECORD.path)?;
    let (_, source_bonus_overlay) = source_disc
        .source()
        .read_record(BONUS_MENU_OVERLAY_RECORD.path)?;
    let (_, source_bonus_menu) = source_disc.source().read_record(BONUS_MENU_RECORD.path)?;
    let source_cue_path = source_disc.snapshot_cue_path().to_path_buf();
    let source_image_path = source_disc.snapshot_image_path();
    validate_edit_runtime_text_source_bindings(
        source_disc.source(),
        &build_spec.assets.mode_descendants,
    )?;
    let font_output_dir = config.output_dir.join(FONT_OUTPUT_DIRECTORY);
    eprintln!("build: render and compress prepared dialogue");
    let dialogue_started = std::time::Instant::now();
    let font_build = build_dialogue_font_images_from_source(
        &DialogueFontBuildConfig {
            prepared_dialogue: config.prepared_dialogue.clone(),
            cue: source_cue_path.clone(),
            codebook: build_spec.assets.dialogue_codebook.clone(),
            translation: build_spec.assets.dialogue_translations.clone(),
            selector_translation: build_spec.assets.dialogue_selector_translations.clone(),
            name_input_keyboard: build_spec
                .assets
                .name_entry_candidates
                .join("keyboard.json"),
            name_glyph_font: build_spec.fonts.name_entry.composed_glyphs.path.clone(),
            name_glyph_font_px: build_spec.fonts.name_entry.composed_glyphs.font_px,
            font: build_spec.fonts.dialogue_body.path.clone(),
            font_px: build_spec.fonts.dialogue_body.font_px,
            input_policy: config.input_policy,
            output_dir: font_output_dir.clone(),
            force: config.force,
        },
        source_disc.source(),
    )?;
    eprintln!(
        "build: dialogue completed in {:.2}s",
        dialogue_started.elapsed().as_secs_f64()
    );
    let font_report = &font_build.report;
    ensure!(
        font_report.development_build_input_available
            && font_report.all_message_regions_preserved
            && font_report.all_assets_compress_within_original_extents,
        "dialogue disc build requires complete verified development assets"
    );
    require_snapshot_source(
        &source_disc,
        "dialogue font",
        &font_report.source_bin_sha256,
    )?;
    let NameRuntimeComponents {
        name_glyph_layout,
        shared_name_runtime,
        dialogue_name_runtime,
        name_entry_build,
        battle_names,
        dialogue_name_runtime_build_manifest_sha256,
        shared_name_runtime_build_manifest_sha256,
    } = build_name_runtime_components(
        config,
        &build_spec,
        source_disc.source(),
        &source_cue_path,
        &main_executable,
        &font_build,
    )?;
    let name_entry_report = &name_entry_build.report;
    let MenuSceneComponents {
        menu_atlas_plan,
        shared_menu_atlas,
        diary_header_build,
        diary_scene_build,
        mode_select_build,
        character_select_build,
        mode_descendant_build,
        practical_instruction_build,
        options_build,
        title_menu_build,
        bonus_menu_build,
        bonus_inventory_development,
        shop_development,
    } = build_menu_scene_components(
        config,
        &build_spec,
        &source_disc,
        plain_loading.as_ref(),
        &name_entry_build.selector_runtime,
    )?;
    let source_context = source_disc.source();
    let panel_selections = mode_select_build
        .report
        .modes
        .iter()
        .map(|mode| crate::surface_inventory::ModeSelectPanelSelection {
            panel_index: mode.panel_index,
            asset_id: mode.id.as_str(),
        })
        .collect::<Vec<_>>();
    let surface_inventory_report = validate_mode_select_descendant_inventory(
        &surface_inventory,
        &panel_selections,
        &ModeSelectInventorySources {
            source_bin_sha256: source_disc.source_bin_sha256(),
            mode_select_menu_path: &mode_select_build.report.source_menu_path,
            mode_select_overlay_path: &mode_select_build.report.source_overlay_path,
            mode_select_overlay: &source_mode_select_overlay,
            main_executable: &main_executable,
            character_select_overlays: &source_character_select_overlays,
            character_select_textures_stored: &source_character_select_textures,
            cooperative_menu_stored: &source_cooperative_menu,
            options_overlay: &source_options_overlay,
            options_information_stored: &source_options_information,
            direct_entries: ModeSelectDirectEntrySources {
                diary_overlay_stored: &source_diary_overlay,
                menu_stored: &source_menu,
                gorin_overlay: &source_gorin_overlay,
                gorin_menu_stored: &source_gorin_menu,
                edit_overlay: &source_edit_overlay,
                edit_ui_stored: &source_edit_ui,
                bonus_overlay: &source_bonus_overlay,
                bonus_menu_stored: &source_bonus_menu,
            },
        },
    )?;
    ensure!(
        options_build.source_main_executable == main_executable,
        "records-main and shared-name builds do not share one source main executable"
    );
    let title_notice_output_dir = config.output_dir.join(TITLE_NOTICE_OUTPUT_DIRECTORY);
    let title_notice_font = TitleNoticeFontStyle {
        font: build_spec.fonts.title_notice.path.clone(),
        font_px: build_spec.fonts.title_notice.font_px,
    };
    let title_notice_build = build_title_notice_from_source(
        &TitleNoticeBuildConfig {
            assets: build_spec.assets.title_notice.clone(),
            font: title_notice_font.clone(),
            build_spec_sha256: build_spec.sha256.clone(),
            output_dir: title_notice_output_dir.clone(),
            force: config.force,
        },
        source_context,
        &menu_atlas_plan,
    )?;
    ensure!(
        title_notice_build.report.development_input_available
            && title_notice_build.report.source_records_match
            && title_notice_build.report.source_placements_match
            && title_notice_build
                .report
                .changed_bytes_confined_to_owned_ranges,
        "development disc build requires verified title notice assets"
    );
    let edit_runtime_text_output_dir = config.output_dir.join(EDIT_RUNTIME_TEXT_OUTPUT_DIRECTORY);
    let edit_runtime_text_build = build_edit_runtime_text(
        &EditRuntimeTextBuildConfig {
            assets: build_spec.assets.mode_descendants.clone(),
            font: build_spec.fonts.mode_descendants.edit_runtime_text.clone(),
            password_font: build_spec.fonts.mode_descendants.password.clone(),
            edit_shared_ui_stored: mode_descendant_build
                .stored_record(ModeDescendantRecord::EditSharedUi)
                .context("mode-descendant build lost EDITMOJI")?
                .to_vec(),
            edit_shared_ui_write_claims: mode_descendant_build
                .decoded_write_claims(ModeDescendantRecord::EditSharedUi)
                .context("mode-descendant build lost EDITMOJI write claims")?
                .to_vec(),
            name_glyph_materialization: font_build.name_glyph_materialization.clone(),
            name_glyph_consumer_layout: name_glyph_layout.clone(),
            name_entry_font_stored: name_entry_build.font_stored.clone(),
            name_entry_font_patched_decoded_sha256: name_entry_report
                .font
                .patched_decoded_sha256
                .clone(),
            name_entry_runtime_atlas: name_entry_report.font.runtime_atlas.clone(),
            name_entry_runtime_coordinate_list_storage_byte_range: name_entry_report
                .font
                .runtime_coordinate_list_storage_byte_range,
            outline_pixel_address: shared_name_runtime.outline_program.outline_pixel_address,
            outline_cleanup_address: shared_name_runtime.outline_program.outline_cleanup_address,
            build_spec_sha256: build_spec.sha256.clone(),
            output_dir: edit_runtime_text_output_dir.clone(),
            force: config.force,
        },
        source_disc.source(),
        &menu_atlas_plan,
    )?;
    ensure!(
        edit_runtime_text_build.report.development_input_available
            && edit_runtime_text_build.report.source_records_match
            && edit_runtime_text_build.report.source_placements_match
            && edit_runtime_text_build
                .report
                .changed_bytes_confined_to_owned_ranges,
        "development disc build requires verified EDIT runtime text assets"
    );
    let main_executable_record = compose_main_executable_record(
        &main_executable,
        &shared_name_runtime,
        &options_build,
        &character_select_build,
        plain_loading.as_ref(),
        &mode_descendant_build,
        &battle_names,
    )?;
    let BonusInventoryDevelopmentBuild {
        fixed_ui: bonus_inventory_build,
        action_labels: bonus_inventory_action_labels_build,
        confirmation: bonus_confirmation_build,
        j_bank_return_label: bonus_j_bank_return_label_build,
        page_indicator: bonus_page_indicator_build,
        stock_label: bonus_inventory_stock_label_build,
        card_acquisition: bonus_inventory_card_acquisition_build,
        memory_card_swap: bonus_inventory_memory_card_swap_build,
        surfaces: bonus_inventory_surfaces,
    } = bonus_inventory_development;
    let ShopDevelopmentBuild {
        fixed_ui: shop_ui_build,
        exit_confirmation: bonus_shop_exit_confirmation_build,
        pointer_text: bonus_shop_text_build,
        surfaces: shop_surfaces,
    } = shop_development;
    for (stage, source_bin_sha256) in [
        ("name entry", name_entry_report.source_bin_sha256.as_str()),
        (
            "mode select",
            mode_select_build.report.source_bin_sha256.as_str(),
        ),
        (
            "character select",
            character_select_build.report.source_bin_sha256.as_str(),
        ),
        (
            "mode descendants",
            mode_descendant_build.report.source_bin_sha256.as_str(),
        ),
        (
            "practical instructions",
            practical_instruction_build
                .report
                .source_bin_sha256
                .as_str(),
        ),
        ("options", options_build.report.source_bin_sha256.as_str()),
        (
            "title menu",
            title_menu_build.report.source_bin_sha256.as_str(),
        ),
        (
            "title notice",
            title_notice_build.report.source_bin_sha256.as_str(),
        ),
        (
            "EDIT runtime text",
            edit_runtime_text_build.report.source_bin_sha256.as_str(),
        ),
        (
            "bonus menu",
            bonus_menu_build.report.source_bin_sha256.as_str(),
        ),
        ("shop UI", shop_ui_build.report.source_bin_sha256.as_str()),
    ] {
        require_snapshot_source(&source_disc, stage, source_bin_sha256)?;
    }
    let edit_shared_ui_report = mode_descendant_build
        .report
        .records
        .iter()
        .find(|record| record.record == ModeDescendantRecord::EditSharedUi)
        .context("mode-descendant build lost EDITMOJI")?;
    ensure!(
        edit_runtime_text_build
            .report
            .input_edit_shared_ui_stored_sha256
            == edit_shared_ui_report.patched_stored_sha256,
        "EDIT runtime text did not consume the fixed-UI EDITMOJI build"
    );
    let loading_art = build_spec
        .assets
        .loading_art
        .as_ref()
        .map(|assets| {
            crate::loading_art::build(
                source_context,
                assets,
                &build_spec.fonts.loading_art,
                &config.output_dir.join("loading-art"),
            )
        })
        .transpose()?;
    let card_art = build_spec
        .assets
        .card_art
        .as_ref()
        .map(|assets| {
            crate::card_art::build(
                source_context,
                assets,
                &build_spec.fonts.card_art,
                &config.output_dir.join("card-art"),
            )
        })
        .transpose()?;
    let staff_roll = build_spec
        .assets
        .staff_roll
        .as_ref()
        .map(|assets| {
            crate::staff_roll::build(
                source_context,
                assets,
                build_spec
                    .fonts
                    .staff_roll
                    .as_ref()
                    .context("staff-roll font is not selected")?,
                &config.output_dir.join("staff-roll"),
            )
        })
        .transpose()?;
    let endings = build_spec
        .assets
        .endings
        .as_ref()
        .map(|assets| {
            crate::ending_subtitles::build(
                source_context,
                assets,
                &build_spec.fonts.dialogue_body,
                &config.output_dir.join("ending-subtitles"),
            )
        })
        .transpose()?;
    let pocket_help = build_spec
        .assets
        .pocket_help
        .as_ref()
        .map(|assets| {
            crate::pocket_help::build(
                source_context,
                assets,
                &build_spec.fonts,
                &config.output_dir.join("pocket-help"),
            )
        })
        .transpose()?;
    let record_assembly = assemble_dialogue_disc_records(DialogueDiscRecordSources {
        menu_atlas_plan: &menu_atlas_plan,
        staff_roll: staff_roll.as_ref(),
        endings: endings.as_ref(),
        pocket_help: pocket_help.as_ref(),
        card_art: card_art.as_ref(),
        shared_menu_atlas: &shared_menu_atlas,
        loading_art: loading_art.as_ref(),
        source_image: source_image_path,
        font: &font_build,
        name_entry: &name_entry_build,
        battle_names: &battle_names,
        diary_header: &diary_header_build,
        diary_scene: &diary_scene_build,
        character_select: &character_select_build,
        mode_descendants: &mode_descendant_build,
        practical_instructions: &practical_instruction_build,
        mode_select: &mode_select_build,
        options: &options_build,
        title_menu: &title_menu_build,
        title_notice: &title_notice_build,
        edit_runtime_text: &edit_runtime_text_build,
        main_executable: &main_executable_record,
        dialogue_name_runtime: &dialogue_name_runtime,
        bonus_menu: &bonus_menu_build,
        bonus_inventory_surfaces: &bonus_inventory_surfaces,
        shop_surfaces: &shop_surfaces,
    })?;
    let replacements = record_assembly.contributions();

    eprintln!("build: compose disc and verify readback/EDC/ECC");
    let finalization_started = std::time::Instant::now();
    let result = finalize_dialogue_disc(DialogueDiscFinalization {
        source_disc: &source_disc,
        component_builds: DialogueDiscComponentBuilds {
            font: &font_build,
            name_entry: &name_entry_build,
            diary_header: &diary_header_build,
            diary_scene: &diary_scene_build,
            character_select: &character_select_build,
            mode_descendants: &mode_descendant_build,
            practical_instructions: &practical_instruction_build,
            mode_select: &mode_select_build,
            options: &options_build,
            title_menu: &title_menu_build,
            title_notice: &title_notice_build,
            edit_runtime_text: &edit_runtime_text_build,
            bonus_menu: &bonus_menu_build,
            bonus_inventory: &bonus_inventory_build,
            bonus_inventory_action_labels: &bonus_inventory_action_labels_build,
            bonus_confirmation: &bonus_confirmation_build,
            bonus_inventory_stock_label: &bonus_inventory_stock_label_build,
            bonus_inventory_card_acquisition: &bonus_inventory_card_acquisition_build,
            bonus_inventory_memory_card_swap: &bonus_inventory_memory_card_swap_build,
            bonus_j_bank_return_label: &bonus_j_bank_return_label_build,
            bonus_page_indicator: &bonus_page_indicator_build,
            shop_ui: &shop_ui_build,
            bonus_shop_exit_confirmation: &bonus_shop_exit_confirmation_build,
            bonus_shop_text: &bonus_shop_text_build,
        },
        composition: DialogueDiscComposition {
            dialogue_bundles: &record_assembly.dialogue_bundles,
            development_menu: &record_assembly.development_menu,
            development_title_overlay: &record_assembly.development_title_overlay,
            main_executable: &main_executable_record,
            dialogue_name_runtime: &dialogue_name_runtime,
            bonus_inventory_surfaces: &bonus_inventory_surfaces,
            shop_surfaces: &shop_surfaces,
        },
        dialogue_name_runtime_build_manifest_sha256: &dialogue_name_runtime_build_manifest_sha256,
        shared_name_runtime_build_manifest_sha256: &shared_name_runtime_build_manifest_sha256,
        development_build_spec_sha256: &build_spec.sha256,
        surface_inventory: &surface_inventory_report,
        replacements: &replacements,
        outputs: &outputs,
    });
    if result.is_err() {
        let _ = outputs.remove_staged();
    }
    eprintln!(
        "build: finalization {:.2}s; total {:.2}s",
        finalization_started.elapsed().as_secs_f64(),
        build_started.elapsed().as_secs_f64()
    );
    result
}

fn require_snapshot_source(
    source_disc: &VerifiedSourceDisc,
    stage: &str,
    reported_sha256: &str,
) -> Result<()> {
    ensure!(
        reported_sha256 == source_disc.source_bin_sha256(),
        "{stage} build did not consume the verified source snapshot"
    );
    Ok(())
}
