use super::component_workers::join_component_builds;

use std::path::Path;

use anyhow::{Result, ensure};

use crate::bonus_shop_exit_confirmation::{
    BonusShopExitConfirmationBuild, BonusShopExitConfirmationBuildConfig,
    build_bonus_shop_exit_confirmation_from_source,
};
use crate::bonus_shop_source::load_source_from_disc;
use crate::bonus_shop_text_source::{
    BonusShopTextBuild, BonusShopTextBuildConfig, build_bonus_shop_text_from_source,
};
use crate::development_build_spec::LoadedDevelopmentBuildSpec;
use crate::shop_ui::{SHOP_UI_PATH, ShopUiBuild, ShopUiBuildConfig, build_shop_ui_from_source};
use crate::source_disc::VerifiedSourceDisc;

use super::shop_surface_composition::{ShopSurfaceBuild, load_shop_surface_composition};

const FIXED_UI_OUTPUT_DIRECTORY: &str = "shop-ui";
const EXIT_CONFIRMATION_OUTPUT_DIRECTORY: &str = "bonus-shop-exit-confirmation";
const POINTER_TEXT_OUTPUT_DIRECTORY: &str = "bonus-shop-text";

pub(super) struct ShopDevelopmentBuild {
    pub fixed_ui: ShopUiBuild,
    pub exit_confirmation: BonusShopExitConfirmationBuild,
    pub pointer_text: BonusShopTextBuild,
    pub surfaces: ShopSurfaceBuild,
}

pub(super) fn build_shop_development_surfaces(
    source_disc: &VerifiedSourceDisc,
    build_spec: &LoadedDevelopmentBuildSpec,
    output_directory: &Path,
    force: bool,
) -> Result<ShopDevelopmentBuild> {
    let source = load_source_from_disc(source_disc.source())?;
    let source_cue = source_disc.snapshot_cue_path().to_path_buf();
    let fixed_ui_output_directory = output_directory.join(FIXED_UI_OUTPUT_DIRECTORY);
    let exit_confirmation_output_directory =
        output_directory.join(EXIT_CONFIRMATION_OUTPUT_DIRECTORY);
    let (fixed_ui, exit_confirmation) = std::thread::scope(|scope| -> Result<_> {
        let fixed_ui = scope.spawn(|| {
            build_shop_ui_from_source(
                &ShopUiBuildConfig {
                    cue: source_cue.clone(),
                    assets: build_spec.assets.shop_ui.clone(),
                    fonts: build_spec.fonts.shop_ui.clone(),
                    build_spec_sha256: build_spec.sha256.clone(),
                    output_dir: fixed_ui_output_directory.clone(),
                    force,
                },
                &source,
            )
        });
        let exit_confirmation = scope.spawn(|| {
            build_bonus_shop_exit_confirmation_from_source(
                &BonusShopExitConfirmationBuildConfig {
                    cue: source_cue.clone(),
                    assets: build_spec.assets.bonus_shop_exit_confirmation.clone(),
                    font: build_spec.fonts.bonus_shop_exit_confirmation.clone(),
                    build_spec_sha256: build_spec.sha256.clone(),
                    output_dir: exit_confirmation_output_directory.clone(),
                    force,
                },
                &source,
            )
        });
        join_component_builds!(
            fixed_ui => "shop fixed-UI",
            exit_confirmation => "shop exit-confirmation",
        )
    })?;
    ensure!(
        fixed_ui.report.development_input_available
            && fixed_ui.report.source_regions_match
            && fixed_ui.report.cells_are_unique_and_non_overlapping
            && fixed_ui.report.changed_bytes_confined_to_owned_cells,
        "development disc build requires verified shop fixed-UI assets"
    );

    let glyph_ownership = &exit_confirmation.report.glyph_ownership_evidence;
    ensure!(
        exit_confirmation.report.development_input_available
            && exit_confirmation.report.source_command_records_match
            && glyph_ownership.declared_physical_alias_set_matches
            && glyph_ownership.physical_alias_source_cells_match_blank_hash
            && glyph_ownership.fixed_ui_tim_disjoint
            && glyph_ownership.pointer_command_record_count > 0
            && glyph_ownership.pointer_command_unique_glyph_count > 0
            && glyph_ownership.pointer_command_table_parsed_glyphs_disjoint
            && glyph_ownership.declared_direct_selector_byte_region_scan_disjoint
            && exit_confirmation
                .report
                .shop_ui_changes_confined_to_allocated_glyph_cells
            && exit_confirmation
                .report
                .overlay_changes_confined_to_fixed_record
            && exit_confirmation
                .report
                .reused_source_glyphs
                .iter()
                .all(|glyph| glyph.source_indexed_pixels_match_declared_hash),
        "development disc build requires a verified bonus shop exit confirmation"
    );

    let pointer_text_output_directory = output_directory.join(POINTER_TEXT_OUTPUT_DIRECTORY);
    let pointer_text = build_bonus_shop_text_from_source(
        &BonusShopTextBuildConfig {
            cue: source_cue,
            assets: build_spec.assets.bonus_shop_text.clone(),
            fonts: build_spec.fonts.bonus_shop_text.clone(),
            build_spec_sha256: build_spec.sha256.clone(),
            output_dir: pointer_text_output_directory.clone(),
            force,
        },
        &source,
        &exit_confirmation,
    )?;
    ensure!(
        pointer_text.report.source_records_match
            && pointer_text.report.complete_source_population
            && pointer_text.report.delegated_records_match_existing_writer
            && pointer_text
                .report
                .shop_ui_changes_confined_to_allocated_glyph_cells
            && pointer_text
                .report
                .overlay_changes_confined_to_authored_records
            && pointer_text.report.development_can_continue,
        "development disc build requires verified bonus shop pointer-text assets"
    );

    for (stage, reported_sha256) in [
        ("shop fixed UI", fixed_ui.report.source_bin_sha256.as_str()),
        (
            "bonus shop exit confirmation",
            exit_confirmation.report.source_bin_sha256.as_str(),
        ),
        (
            "bonus shop pointer text",
            pointer_text.report.source_bin_sha256.as_str(),
        ),
    ] {
        ensure!(
            reported_sha256 == source_disc.source_bin_sha256(),
            "{stage} build did not consume the verified source snapshot"
        );
    }
    ensure!(
        exit_confirmation.report.source_shop_ui_path == SHOP_UI_PATH
            && exit_confirmation.report.source_shop_ui_stored_sha256
                == fixed_ui.report.source_stored_sha256
            && exit_confirmation.report.source_shop_ui_decoded_sha256
                == fixed_ui.report.source_decoded_sha256,
        "shop surfaces do not share one verified KOUBAI.TIZ source"
    );
    ensure!(
        pointer_text.report.source_shop_ui_path == SHOP_UI_PATH
            && pointer_text.report.source_shop_ui_decoded_sha256
                == fixed_ui.report.source_decoded_sha256
            && pointer_text.report.source_overlay_path
                == exit_confirmation.report.source_overlay_path
            && pointer_text.report.source_overlay_sha256
                == exit_confirmation.report.source_overlay_sha256,
        "shop pointer text does not share the verified KOUBAI sources"
    );

    let mut surfaces = load_shop_surface_composition(
        &source,
        &fixed_ui,
        &exit_confirmation.report.source_overlay_path,
        &exit_confirmation.report.source_overlay_sha256,
    )?;
    surfaces.apply_surface_writer(
        "bonus shop exit confirmation",
        |shop_ui| exit_confirmation.apply_to_shop_ui_decoded(shop_ui),
        |overlay| exit_confirmation.apply_to_overlay(overlay),
    )?;
    if pointer_text.report.authored_unit_count > 0 {
        surfaces.apply_surface_writer(
            "bonus shop pointer text",
            |shop_ui| pointer_text.apply_to_shop_ui_decoded(shop_ui),
            |overlay| pointer_text.apply_to_overlay(overlay),
        )?;
    }
    let surfaces = surfaces.finish()?;

    Ok(ShopDevelopmentBuild {
        fixed_ui,
        exit_confirmation,
        pointer_text,
        surfaces,
    })
}
