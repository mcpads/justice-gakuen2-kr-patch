//! Build independent menu/scene producers and validate their shared graphics
//! dependencies before the disc coordinator consumes them. Source inventory,
//! shared-record composition and publication remain outside this stage.
use super::bonus_inventory_development_build::{
    BonusInventoryDevelopmentBuild, build_bonus_inventory_development_surfaces,
};
use super::component_workers::join_component_builds;
use super::dialogue_disc_build_model::DialogueDiscBuildConfig;
use super::shop_development_build::{ShopDevelopmentBuild, build_shop_development_surfaces};
use crate::bonus_menu::BonusMenuBuild;
use crate::bonus_menu::{BonusMenuBuildConfig, build_bonus_menu_from_source};
use crate::character_select_graphics::CharacterSelectAtlasBuild;
use crate::character_select_graphics::{
    CharacterSelectAtlasBuildConfig, build_character_select_atlas_from_source,
};
use crate::development_build_spec::LoadedDevelopmentBuildSpec;
use crate::diary_header::DiaryHeaderBuild;
use crate::diary_header::{DiaryHeaderBuildConfig, build_diary_header_from_source};
use crate::diary_scene::DiarySceneBuild;
use crate::diary_scene::{DiarySceneBuildConfig, build_diary_scene_from_source};
use crate::loading_art::PlainLoadingBuild;
use crate::mode_descendant_graphics::ModeDescendantGraphicsBuild;
use crate::mode_descendant_graphics::{
    ModeDescendantGraphicsBuildConfig, build_mode_descendant_graphics_from_source,
};
use crate::mode_select::ModeSelectRecordBuild;
use crate::mode_select::{ModeSelectBuildConfig, build_mode_select_assets_from_source};
use crate::name_input::SelectorRuntimeInstallation;
use crate::options::OptionsRecordBuild;
use crate::options::{OptionsBuildConfig, build_options_records_with_plan};
use crate::practical_instruction_graphics::PracticalInstructionGraphicsBuild;
use crate::practical_instruction_graphics::{
    PracticalInstructionGraphicsBuildConfig, build_practical_instruction_graphics_from_source,
};
use crate::source_disc::VerifiedSourceDisc;
use crate::title_menu::TitleMenuRecordBuild;
use crate::title_menu::{TitleMenuBuildConfig, build_title_menu_assets_with_plan};
use anyhow::{Context, Result, ensure};

const CHARACTER_SELECT_OUTPUT_DIRECTORY: &str = "character-select";
const BONUS_MENU_OUTPUT_DIRECTORY: &str = "bonus-menu";
const DIARY_HEADER_OUTPUT_DIRECTORY: &str = "diary-header";
const DIARY_SCENE_OUTPUT_DIRECTORY: &str = "diary-scene";
const MODE_SELECT_OUTPUT_DIRECTORY: &str = "mode-select";
const MODE_DESCENDANT_OUTPUT_DIRECTORY: &str = "mode-descendants";
const PRACTICAL_INSTRUCTION_OUTPUT_DIRECTORY: &str = "practical-instructions";
const OPTIONS_OUTPUT_DIRECTORY: &str = "options";
const TITLE_MENU_OUTPUT_DIRECTORY: &str = "title-menu";

pub(super) struct MenuSceneComponents {
    pub menu_atlas_plan: crate::menu_atlas_plan::MenuAtlasPlan,
    pub shared_menu_atlas: crate::menu_atlas::presentation::SharedMenuAtlasBuild,
    pub diary_header_build: DiaryHeaderBuild,
    pub diary_scene_build: DiarySceneBuild,
    pub mode_select_build: ModeSelectRecordBuild,
    pub character_select_build: CharacterSelectAtlasBuild,
    pub mode_descendant_build: ModeDescendantGraphicsBuild,
    pub practical_instruction_build: PracticalInstructionGraphicsBuild,
    pub options_build: OptionsRecordBuild,
    pub title_menu_build: TitleMenuRecordBuild,
    pub bonus_menu_build: BonusMenuBuild,
    pub bonus_inventory_development: BonusInventoryDevelopmentBuild,
    pub shop_development: ShopDevelopmentBuild,
}

pub(super) fn build_menu_scene_components(
    config: &DialogueDiscBuildConfig,
    build_spec: &LoadedDevelopmentBuildSpec,
    source_disc: &VerifiedSourceDisc,
    plain_loading: Option<&PlainLoadingBuild>,
    selector_runtime: &SelectorRuntimeInstallation,
) -> Result<MenuSceneComponents> {
    let source_cue_path = source_disc.snapshot_cue_path().to_path_buf();
    let diary_header_output_dir = config.output_dir.join(DIARY_HEADER_OUTPUT_DIRECTORY);
    let diary_scene_output_dir = config.output_dir.join(DIARY_SCENE_OUTPUT_DIRECTORY);
    let mode_select_output_dir = config.output_dir.join(MODE_SELECT_OUTPUT_DIRECTORY);
    let character_select_output_dir = config.output_dir.join(CHARACTER_SELECT_OUTPUT_DIRECTORY);
    let mode_descendant_output_dir = config.output_dir.join(MODE_DESCENDANT_OUTPUT_DIRECTORY);
    let practical_instruction_output_dir = config
        .output_dir
        .join(PRACTICAL_INSTRUCTION_OUTPUT_DIRECTORY);
    let options_output_dir = config.output_dir.join(OPTIONS_OUTPUT_DIRECTORY);
    let title_menu_output_dir = config.output_dir.join(TITLE_MENU_OUTPUT_DIRECTORY);
    let bonus_menu_output_dir = config.output_dir.join(BONUS_MENU_OUTPUT_DIRECTORY);
    eprintln!("build: render and compress menu/scene components");
    let graphics_started = std::time::Instant::now();
    let source_context = source_disc.source();
    let options_config = OptionsBuildConfig {
        cue: source_cue_path.clone(),
        assets: build_spec.assets.options.clone(),
        fonts: build_spec.fonts.options.clone(),
        build_spec_sha256: build_spec.sha256.clone(),
        output_dir: options_output_dir.clone(),
        force: config.force,
    };
    let menu_atlas_plan =
        crate::menu_atlas_plan::prepare_development_plan(build_spec, source_context)?;
    crate::pipeline::write_pretty_json_and_hash(
        &config.output_dir.join("menu-atlas-plan.json"),
        &menu_atlas_plan,
        true,
    )?;

    let (
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
    ) = std::thread::scope(|scope| -> Result<_> {
        let diary_header = scope.spawn(|| {
            build_diary_header_from_source(
                &DiaryHeaderBuildConfig {
                    cue: source_cue_path.clone(),
                    assets: build_spec.assets.diary_header.clone(),
                    fonts: build_spec.fonts.diary_header.clone(),
                    build_spec_sha256: build_spec.sha256.clone(),
                    output_dir: diary_header_output_dir.clone(),
                    force: config.force,
                },
                source_context,
            )
        });
        let diary_scene = scope.spawn(|| {
            build_diary_scene_from_source(
                &DiarySceneBuildConfig {
                    cue: source_cue_path.clone(),
                    assets: build_spec.assets.diary_scenes.clone(),
                    fonts: build_spec.fonts.diary_scene.clone(),
                    build_spec_sha256: build_spec.sha256.clone(),
                    output_dir: diary_scene_output_dir.clone(),
                    force: config.force,
                },
                source_context,
            )
        });
        let mode_select = scope.spawn(|| {
            build_mode_select_assets_from_source(
                &ModeSelectBuildConfig {
                    cue: source_cue_path.clone(),
                    assets: build_spec.assets.mode_select.clone(),
                    fonts: build_spec.fonts.mode_select.clone(),
                    output_dir: mode_select_output_dir.clone(),
                    force: config.force,
                },
                source_context,
            )
        });
        let character_select = scope.spawn(|| {
            build_character_select_atlas_from_source(
                &CharacterSelectAtlasBuildConfig {
                    native_text_font: build_spec.fonts.shared_menu_numerals.clone(),
                    cue: source_cue_path.clone(),
                    assets: build_spec.assets.character_select.clone(),
                    fonts: build_spec.fonts.character_select.clone(),
                    build_spec_sha256: build_spec.sha256.clone(),
                    output_dir: character_select_output_dir.clone(),
                    force: config.force,
                },
                source_context,
                plain_loading,
                Some(selector_runtime),
            )
        });
        let mode_descendant = scope.spawn(|| {
            build_mode_descendant_graphics_from_source(
                &ModeDescendantGraphicsBuildConfig {
                    cue: source_cue_path.clone(),
                    assets: build_spec.assets.mode_descendants.clone(),
                    fonts: build_spec.fonts.mode_descendants.clone(),
                    build_spec_sha256: build_spec.sha256.clone(),
                    output_dir: mode_descendant_output_dir.clone(),
                    force: config.force,
                },
                source_context,
            )
        });
        let practical_instruction = scope.spawn(|| {
            build_practical_instruction_graphics_from_source(
                &PracticalInstructionGraphicsBuildConfig {
                    cue: source_cue_path.clone(),
                    assets: build_spec.assets.practical_instructions.clone(),
                    body_font: build_spec.fonts.practical_instruction.clone(),
                    build_spec_sha256: build_spec.sha256.clone(),
                    output_dir: practical_instruction_output_dir.clone(),
                    force: config.force,
                },
                source_context,
            )
        });
        let options = scope.spawn(|| {
            build_options_records_with_plan(&options_config, source_context, &menu_atlas_plan)
        });
        let title_menu = scope.spawn(|| {
            build_title_menu_assets_with_plan(
                &TitleMenuBuildConfig {
                    cue: source_cue_path.clone(),
                    assets: build_spec.assets.title_menu.clone(),
                    font: build_spec.fonts.title_menu.clone(),
                    build_spec_sha256: build_spec.sha256.clone(),
                    output_dir: title_menu_output_dir.clone(),
                    force: config.force,
                },
                source_context,
                &menu_atlas_plan,
            )
        });
        let bonus_menu = scope.spawn(|| {
            build_bonus_menu_from_source(
                &BonusMenuBuildConfig {
                    cue: source_cue_path.clone(),
                    assets: build_spec.assets.bonus_menu.clone(),
                    fonts: build_spec.fonts.bonus_menu.clone(),
                    build_spec_sha256: build_spec.sha256.clone(),
                    output_dir: bonus_menu_output_dir.clone(),
                    force: config.force,
                },
                source_context,
            )
        });
        let bonus_inventory = scope.spawn(|| {
            build_bonus_inventory_development_surfaces(
                source_disc,
                build_spec,
                &config.output_dir,
                config.force,
            )
        });
        let shop = scope.spawn(|| {
            build_shop_development_surfaces(
                source_disc,
                build_spec,
                &config.output_dir,
                config.force,
            )
        });
        join_component_builds!(
            diary_header => "diary-header",
            diary_scene => "diary-scene",
            mode_select => "mode-select",
            character_select => "character-select",
            mode_descendant => "mode-descendant",
            practical_instruction => "practical-instruction",
            options => "options",
            title_menu => "title-menu",
            bonus_menu => "bonus-menu",
            bonus_inventory => "bonus-inventory",
            shop => "shop",
        )
    })?;
    eprintln!(
        "build: menu/scene components completed in {:.2}s",
        graphics_started.elapsed().as_secs_f64()
    );
    ensure!(
        diary_header_build.report.development_input_available
            && diary_header_build.report.source_regions_match
            && diary_header_build.report.protected_regions_unchanged
            && diary_header_build
                .report
                .changed_bytes_confined_to_owned_cells,
        "dialogue disc build requires a verified diary header image"
    );
    ensure!(
        diary_scene_build.report.development_input_available
            && diary_scene_build.report.source_regions_match
            && diary_scene_build
                .report
                .changed_bytes_confined_to_owned_cells
            && diary_scene_build
                .report
                .archive_changes_confined_to_owned_members
            && diary_scene_build
                .report
                .fixed_presentation_source_regions_match
            && diary_scene_build
                .report
                .fixed_presentation_preserved_regions_unchanged
            && diary_scene_build
                .report
                .runtime_unclaimed_decoded_bytes_preserved
            && diary_scene_build
                .report
                .runtime_compression_requirements_satisfied,
        "dialogue disc build requires verified diary scene graphics"
    );
    ensure!(
        mode_select_build.report.development_input_available
            && mode_select_build.report.source_regions_match
            && mode_select_build.report.source_regions_are_disjoint,
        "dialogue disc build requires verified development MODE SELECT assets"
    );
    ensure!(
        character_select_build.report.record_count == 5
            && character_select_build
                .report
                .all_records_share_select_heading_allocations
            && character_select_build
                .report
                .changed_bytes_confined_to_allocated_cells
            && character_select_build.report.compression_roundtrip_verified
            && character_select_build.report.catalog_prefixes_preserved
            && character_select_build.report.development_input_available
            && character_select_build.records.len() == 5
            && !character_select_build.auxiliary_records.is_empty()
            && character_select_build.report.auxiliary_record_count
                == character_select_build.auxiliary_records.len()
            && character_select_build.overlays.len() == 5,
        "dialogue disc build requires complete guarded character-select graphics"
    );
    let practical_result_report = mode_descendant_build
        .report
        .practical_results
        .as_ref()
        .context("dialogue disc build lacks its practical-result graphics report")?;
    ensure!(
        mode_descendant_build.report.development_input_available
            && mode_descendant_build
                .report
                .record_owned_source_regions_match
            && mode_descendant_build
                .report
                .record_owned_cells_are_unique_and_non_overlapping
            && mode_descendant_build
                .report
                .record_owned_changes_confined_to_owned_cells
            && practical_result_report.unresolved_external_clut_binding_count == 0
            && practical_result_report.unresolved_external_clut_projection_count == 0
            && practical_result_report.external_clut_producer_family_count == 1
            && practical_result_report.external_clut_palette_family_sha256s
                == [practical_instruction_build
                    .report
                    .source_primary_clut_palette_family_sha256
                    .clone()]
            && mode_descendant_build.reported_records_match_stored_records(),
        "dialogue disc build requires verified mode-descendant graphics and its practical-result palette producer"
    );
    ensure!(
        practical_instruction_build.report.authored_member_count > 0
            && practical_instruction_build.report.all_source_members_bound
            && practical_instruction_build
                .report
                .all_unowned_members_preserved
            && practical_instruction_build.report.member_table_preserved
            && practical_instruction_build
                .report
                .compression_roundtrip_verified
            && practical_instruction_build.report.primary_clut_member_count
                == practical_instruction_build.report.member_count
            && practical_instruction_build.report.primary_clut_vram_x == 0
            && practical_instruction_build.report.primary_clut_vram_y == 503
            && practical_instruction_build
                .report
                .source_primary_clut_palette_family_sha256
                == practical_instruction_build
                    .report
                    .patched_primary_clut_palette_family_sha256
            && practical_instruction_build
                .report
                .all_primary_clut_palettes_preserved
            && practical_instruction_build
                .report
                .members
                .iter()
                .all(|member| {
                    member.primary_clut_vram_x == 0
                        && member.primary_clut_vram_y == 503
                        && member.source_primary_clut_palette_sha256
                            == member.patched_primary_clut_palette_sha256
                        && member.primary_clut_palette_preserved
                }),
        "dialogue disc build requires verified practical-instruction graphics"
    );
    ensure!(
        options_build.report.development_input_available,
        "development disc build requires verified options assets"
    );
    ensure!(
        title_menu_build.report.development_input_available
            && title_menu_build.report.source_records_match
            && title_menu_build.report.source_placements_match
            && title_menu_build
                .report
                .changed_bytes_confined_to_owned_ranges
            && title_menu_build.report.runtime_catalog_prefix_matches,
        "development disc build requires verified title-adjacent menu assets"
    );
    ensure!(
        bonus_menu_build.report.development_input_available
            && bonus_menu_build.report.source_regions_match
            && bonus_menu_build
                .report
                .authored_cells_are_unique_and_non_overlapping
            && bonus_menu_build.report.untranslated_regions_unchanged
            && bonus_menu_build
                .report
                .changed_bytes_confined_to_owned_cells,
        "development disc build requires verified bonus main-menu assets"
    );
    let shared_menu_atlas = crate::menu_atlas::presentation::prepare(
        source_context,
        &options_build.source_menu_decoded,
        &build_spec.fonts.options.help,
        &build_spec.fonts.shared_menu_numerals,
        &config.output_dir.join("viewer-panel"),
    )?;
    Ok(MenuSceneComponents {
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
    })
}
