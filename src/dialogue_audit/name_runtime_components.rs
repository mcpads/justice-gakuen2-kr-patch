//! Build the mutually dependent name consumers and enforce their shared storage
//! and program identities before other surfaces consume them. Disc composition
//! and publication stay with the disc builder.
use std::path::Path;

use anyhow::{Result, ensure};

use crate::development_build_spec::LoadedDevelopmentBuildSpec;
use crate::name_input::{
    NameGlyphConsumerLayout, load_name_input_keyboard, plan_name_glyph_consumer_layout,
    plan_nickname_hud_glyph_layout_for_crop,
};
use crate::pipeline::write_pretty_json_and_hash;
use crate::source_disc::{MAIN_EXECUTABLE_PATH, SupportedSourceDisc};

use super::dialogue_disc_build_model::DialogueDiscBuildConfig;
use super::dialogue_font_build_model::DialogueFontBuild;
use super::dialogue_name_runtime_build::{
    DialogueNameRuntimeBuild, build_dialogue_name_runtime_hook,
};
use super::name_entry_composed_build::{
    ComposedNameInputBuild, ComposedNameInputBuildConfig, build_composed_name_input_from_source,
};
use super::script_source::load_mgame_from_source;
use super::shared_name_runtime_build::{SharedNameRuntimeBuild, build_shared_name_runtime};

const NAME_ENTRY_OUTPUT_DIRECTORY: &str = "name-entry";
const DIALOGUE_NAME_RUNTIME_BUILD_MANIFEST_FILE: &str = "dialogue-name-runtime-build.json";
const SHARED_NAME_RUNTIME_BUILD_MANIFEST_FILE: &str = "shared-name-runtime-build.json";

pub(super) struct NameRuntimeComponents {
    pub name_glyph_layout: NameGlyphConsumerLayout,
    pub shared_name_runtime: SharedNameRuntimeBuild,
    pub dialogue_name_runtime: DialogueNameRuntimeBuild,
    pub name_entry_build: ComposedNameInputBuild,
    pub battle_names: super::battle_name_build::BattleNameBuild,
    pub dialogue_name_runtime_build_manifest_sha256: String,
    pub shared_name_runtime_build_manifest_sha256: String,
}

pub(super) fn build_name_runtime_components(
    config: &DialogueDiscBuildConfig,
    build_spec: &LoadedDevelopmentBuildSpec,
    source: &SupportedSourceDisc,
    source_cue_path: &Path,
    main_executable: &[u8],
    font_build: &DialogueFontBuild,
) -> Result<NameRuntimeComponents> {
    let font_report = &font_build.report;
    let name_input_keyboard = load_name_input_keyboard(
        &build_spec
            .assets
            .name_entry_candidates
            .join("keyboard.json"),
    )?;
    let name_glyph_layout = plan_name_glyph_consumer_layout(&name_input_keyboard)?;
    let nickname_hud_layout = plan_nickname_hud_glyph_layout_for_crop(
        font_report.name_glyph_pack.crop,
        build_spec.nickname_hud_glyph_style,
    )?;
    let shared_name_runtime = build_shared_name_runtime(
        main_executable,
        &name_glyph_layout,
        &font_build.name_glyph_materialization,
        &nickname_hud_layout,
    )?;
    ensure!(
        shared_name_runtime.report.outline_program.installed
            && shared_name_runtime
                .report
                .outline_program
                .fits_main_executable_region
            && shared_name_runtime.report.bootstrap_program.installed
            && shared_name_runtime
                .report
                .bootstrap_program
                .restores_entry_delay_slot
            && shared_name_runtime.report.entry_hook.source_verified
            && shared_name_runtime.report.entry_hook.installed
            && shared_name_runtime.report.dialogue_program.installed
            && shared_name_runtime
                .report
                .dialogue_program
                .mgame_runtime_repair
                .installed
            && shared_name_runtime
                .report
                .dialogue_program
                .mgame_runtime_repair
                .first_entry_copy_roundtrip_verified
            && shared_name_runtime
                .report
                .dialogue_program
                .mgame_runtime_repair
                .destination_repair_roundtrip_verified
            && shared_name_runtime
                .report
                .dialogue_program
                .fits_execution_region,
        "dialogue disc build requires the source-bound shared-name runtimes and bootstrap"
    );
    let dialogue_name_runtime = build_dialogue_name_runtime_hook(
        &load_mgame_from_source(source)?,
        &shared_name_runtime.dialogue_program,
        MAIN_EXECUTABLE_PATH,
        font_report.backup_slot_text,
        font_report.stat_result_layout,
    )?;
    ensure!(
        dialogue_name_runtime
            .report
            .name_consumer_hooks
            .iter()
            .all(|hook| hook.source_verified && hook.installed)
            && dialogue_name_runtime
                .report
                .relationship_name_field_capture
                .source_verified
            && dialogue_name_runtime
                .report
                .relationship_name_field_capture
                .installed
            && dialogue_name_runtime
                .report
                .mgame_reload_entry_hook
                .source_verified
            && dialogue_name_runtime
                .report
                .mgame_reload_entry_hook
                .installed
            && dialogue_name_runtime
                .report
                .mgame_reload_entry_hook
                .source_entry_pointer_preserved
            && dialogue_name_runtime
                .report
                .mgame_reload_entry_hook
                .selects_valid_source_copy_or_destination_repair_before_entry_continuation
            && dialogue_name_runtime
                .report
                .mgame_reload_entry_hook
                .displaced_instructions_preserved_by_wrapper
            && dialogue_name_runtime
                .report
                .mgame_reload_entry_hook
                .caller_return_address_preserved
            && dialogue_name_runtime
                .report
                .nickname_hud_lookup
                .source_entries_verified
            && dialogue_name_runtime.report.program.installed
            && dialogue_name_runtime
                .report
                .program
                .mgame_runtime_repair
                .installed
            && dialogue_name_runtime
                .report
                .program
                .mgame_runtime_repair
                .first_entry_copy_roundtrip_verified
            && dialogue_name_runtime
                .report
                .program
                .mgame_runtime_repair
                .destination_repair_roundtrip_verified
            && dialogue_name_runtime.report.program.fits_execution_region,
        "dialogue disc build requires a source-bound shared-name dialogue runtime"
    );
    let dialogue_name_runtime_build_manifest_sha256 = write_pretty_json_and_hash(
        &config
            .output_dir
            .join(DIALOGUE_NAME_RUNTIME_BUILD_MANIFEST_FILE),
        &dialogue_name_runtime.report,
        true,
    )?;
    let name_entry_output_dir = config.output_dir.join(NAME_ENTRY_OUTPUT_DIRECTORY);
    let name_entry_build = build_composed_name_input_from_source(
        &ComposedNameInputBuildConfig {
            cue: source_cue_path.to_owned(),
            keyboard: build_spec
                .assets
                .name_entry_candidates
                .join("keyboard.json"),
            graphics: build_spec.assets.name_entry_graphics.clone(),
            keyboard_font: build_spec.fonts.name_entry.keyboard_keys.path.clone(),
            keyboard_font_px: build_spec.fonts.name_entry.keyboard_keys.font_px,
            composed_glyph_font: build_spec.fonts.name_entry.composed_glyphs.path.clone(),
            composed_glyph_font_px: build_spec.fonts.name_entry.composed_glyphs.font_px,
            roster_font: build_spec.fonts.name_entry.roster_glyphs.path.clone(),
            roster_font_px: build_spec.fonts.name_entry.roster_glyphs.font_px,
            fixed_graphics_font: build_spec.fonts.name_entry.fixed_graphics.clone(),
            output_dir: name_entry_output_dir.clone(),
            force: config.force,
        },
        source,
    )?;
    let name_entry_report = &name_entry_build.report;
    ensure!(
        name_entry_report.font.glyph_pack == font_report.name_glyph_pack,
        "name-entry and shared dialogue glyph packs differ in font, format or payload"
    );
    ensure!(
        name_entry_report.static_asset_build_complete
            && name_entry_report.nickname_hangul_page.installed
            && name_entry_report
                .nickname_hangul_page
                .source_instructions_verified
            && name_entry_report.nickname_companion.installed
            && name_entry_report
                .nickname_companion
                .source_instructions_verified
            && name_entry_report
                .nickname_companion
                .tagged_slots_use_persistent_cache_codes
            && name_entry_report
                .nickname_companion
                .legacy_mapping_preserved
            && name_entry_report
                .nickname_companion
                .invalid_or_unmapped_values_terminate
            && name_entry_report.selection_hook.installed
            && name_entry_report
                .selection_hook
                .source_instructions_verified
            && name_entry_report
                .font
                .runtime_code_region
                .typed_code_installed
            && name_entry_report
                .font
                .redisplay_runtime_program
                .selection_hook_installed
            && name_entry_report.font.compressed_within_original_extent
            && name_entry_report.font.compression_roundtrip_verified,
        "dialogue disc build requires source-bound composed name-input assets"
    );
    ensure!(
        name_entry_report.shared_outline_runtime.sha256
            == shared_name_runtime.report.outline_program.sha256
            && name_entry_report
                .shared_outline_runtime
                .outline_pixel_address
                == dialogue_name_runtime
                    .report
                    .program
                    .shared_outline_pixel_address
            && name_entry_report
                .shared_outline_runtime
                .outline_cleanup_address
                == dialogue_name_runtime
                    .report
                    .program
                    .shared_outline_cleanup_address
            && shared_name_runtime
                .report
                .bootstrap_program
                .nickname_hud_store_address
                == name_entry_report
                    .font
                    .runtime_program
                    .nickname_hud_store_address
                    .as_deref()
                    .unwrap_or_default()
            && shared_name_runtime
                .report
                .bootstrap_program
                .nickname_hud_store_address
                == dialogue_name_runtime
                    .report
                    .program
                    .nickname_hud_store_address
            && shared_name_runtime
                .report
                .bootstrap_program
                .nickname_hud_scaler_address
                == dialogue_name_runtime
                    .report
                    .program
                    .nickname_hud_scaler_address
            && shared_name_runtime
                .report
                .bootstrap_program
                .nickname_hud_persistent_cell_addresses
                == dialogue_name_runtime
                    .report
                    .program
                    .nickname_hud_persistent_cell_addresses
            && shared_name_runtime
                .report
                .bootstrap_program
                .nickname_hud_reused_scene_cell_addresses
                == dialogue_name_runtime
                    .report
                    .program
                    .nickname_hud_reused_scene_cell_addresses
            && !shared_name_runtime
                .report
                .bootstrap_program
                .nickname_hud_writes_reused_scene_cells
            && !dialogue_name_runtime
                .report
                .program
                .nickname_hud_writes_reused_scene_cells
            && dialogue_name_runtime
                .report
                .program
                .shared_outline_runtime_sha256
                == shared_name_runtime.report.outline_program.sha256
            && dialogue_name_runtime.report.program.sha256
                == shared_name_runtime.report.dialogue_program.sha256,
        "name entry and dialogue did not bind the same persistent shared-name outline runtime"
    );
    let shared_name_runtime_build_manifest_sha256 = write_pretty_json_and_hash(
        &config
            .output_dir
            .join(SHARED_NAME_RUNTIME_BUILD_MANIFEST_FILE),
        &shared_name_runtime.report,
        true,
    )?;
    let battle_names = super::battle_name_build::BattleNameBuild::build(
        source,
        &name_entry_build,
        &build_spec
            .fonts
            .mode_descendants
            .password
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("battle names require the admitted ASCII font"))?
            .path,
    )?;
    write_pretty_json_and_hash(
        &name_entry_output_dir.join("battle-name-build.json"),
        &battle_names.report,
        true,
    )?;
    Ok(NameRuntimeComponents {
        battle_names,
        name_glyph_layout,
        shared_name_runtime,
        dialogue_name_runtime,
        name_entry_build,
        dialogue_name_runtime_build_manifest_sha256,
        shared_name_runtime_build_manifest_sha256,
    })
}
