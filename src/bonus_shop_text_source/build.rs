use std::collections::BTreeMap;

use anyhow::{Context, Result, ensure};

use super::asset_sync::load_existing_assets;
use super::card_results::BonusShopCardResultText;
use super::delegated_records::validate_exit_confirmation_delegations;
use super::glyph_allocation::allocate_bonus_shop_glyphs;
use super::glyph_atlas::{apply_bonus_shop_glyphs, rasterize_bonus_shop_glyphs};
use super::model::{
    BonusShopDevelopmentStatus, BonusShopFontRole, BonusShopReleaseStatus, BonusShopTextBuild,
    BonusShopTextBuildConfig, BonusShopTextBuildReport, BonusShopTextFontBuildReport,
    BonusShopTextGlyphBuildReport, BonusShopTextUnitBuildReport,
};
use super::output::{prepare_outputs, write_outputs};
use super::overlay_plan::plan_bonus_shop_text_overlay;
use super::parser::parse_shop_text_tables;
use super::validate_complete_table_set;
use crate::bonus_shop_exit_confirmation::{
    BonusShopExitConfirmationBuild, bonus_shop_exit_reserved_glyph_codes,
};
use crate::bonus_shop_source::{
    BonusShopSource, OVERLAY_PATH, OVERLAY_SOURCE_SHA256, SHOP_UI_PATH,
    SHOP_UI_SOURCE_DECODED_SHA256, load_source,
};
use crate::pipeline::{difference_ranges, sha256_bytes, sha256_file};

pub use super::output::{BONUS_SHOP_TEXT_BUILD_MANIFEST_FILE, BONUS_SHOP_TEXT_OVERLAY_OUTPUT_FILE};

pub fn build_bonus_shop_text(
    config: &BonusShopTextBuildConfig,
    exit_confirmation: &BonusShopExitConfirmationBuild,
) -> Result<BonusShopTextBuild> {
    let source = load_source(&config.cue)?;
    build_bonus_shop_text_from_source(config, &source, exit_confirmation)
}

pub(crate) fn build_bonus_shop_text_from_source(
    config: &BonusShopTextBuildConfig,
    source: &BonusShopSource,
    exit_confirmation: &BonusShopExitConfirmationBuild,
) -> Result<BonusShopTextBuild> {
    prepare_outputs(&config.output_dir, config.force)?;
    validate_build_spec_sha256(&config.build_spec_sha256)?;
    validate_fonts(config)?;

    let tables =
        parse_shop_text_tables(&source.overlay, &source.shop_ui_decoded, &BTreeMap::new())?;
    validate_complete_table_set(&tables)?;
    let expected_by_id = tables
        .iter()
        .flat_map(|table| table.records.iter().map(|record| record.unit.clone()))
        .map(|unit| (unit.unit_id.clone(), unit))
        .collect::<BTreeMap<_, _>>();
    ensure!(
        expected_by_id.len() == 186,
        "bonus-shop build source-record population changed"
    );
    let assets = load_existing_assets(&config.assets, &expected_by_id)?;
    let complete_source_population = assets.units.len() + assets.delegated_records.len()
        == expected_by_id.len()
        && expected_by_id
            .keys()
            .all(|id| assets.units.contains_key(id) || assets.delegated_records.contains_key(id));
    ensure!(
        complete_source_population,
        "bonus-shop build assets do not cover the complete source-record population"
    );
    validate_exit_confirmation_delegations(&assets.delegated_records, exit_confirmation)?;

    let card_results = BonusShopCardResultText::load(&config.assets)?;
    let allocation = allocate_bonus_shop_glyphs(
        &source.overlay,
        &source.shop_ui_decoded,
        &assets.units,
        &assets.delegated_records,
        bonus_shop_exit_reserved_glyph_codes(),
        card_results.required_characters(),
    )?;
    let (glyphs, rasters) = rasterize_bonus_shop_glyphs(&config.fonts, &allocation.allocations)?;
    let mut shop_ui_probe = source.shop_ui_decoded.clone();
    let glyph_applications = apply_bonus_shop_glyphs(&mut shop_ui_probe, &glyphs)?;
    let shop_ui_expected_write_ranges = glyph_applications
        .iter()
        .flat_map(|application| application.allowed_ranges.iter().copied())
        .collect::<Vec<_>>();
    let shop_ui_changed_byte_ranges = difference_ranges(&source.shop_ui_decoded, &shop_ui_probe);
    let shop_ui_changes_confined_to_allocated_glyph_cells = shop_ui_changed_byte_ranges
        .iter()
        .all(|range| range_is_covered(*range, &shop_ui_expected_write_ranges));
    ensure!(
        shop_ui_changes_confined_to_allocated_glyph_cells
            && (glyphs.is_empty() == shop_ui_changed_byte_ranges.is_empty()),
        "bonus-shop text glyph writer changed bytes outside allocated cells"
    );

    let mut overlay =
        plan_bonus_shop_text_overlay(&source.overlay, &assets.units, &allocation.glyph_codes)?;
    card_results.append_to_plan(&source.overlay, &mut overlay, &allocation.glyph_codes)?;
    let overlay_changes_confined_to_authored_records = overlay
        .changed_byte_ranges
        .iter()
        .all(|range| range_is_covered(*range, &overlay.expected_write_ranges));
    ensure!(
        overlay_changes_confined_to_authored_records && !overlay.changed_byte_ranges.is_empty(),
        "bonus-shop text overlay changed bytes outside authored records"
    );

    let authored_unit_count = assets
        .units
        .values()
        .filter(|unit| unit.development_status == BonusShopDevelopmentStatus::Authored)
        .count();
    let untranslated_unit_count = assets.units.len() - authored_unit_count;
    let release_approved_unit_count = assets
        .units
        .values()
        .filter(|unit| unit.release_status == BonusShopReleaseStatus::Approved)
        .count();
    ensure!(
        authored_unit_count == overlay.authored_unit_count
            && untranslated_unit_count == overlay.untranslated_unit_count,
        "bonus-shop text asset and overlay-plan states disagree"
    );

    let glyph_reports = allocation
        .allocations
        .iter()
        .zip(&glyphs)
        .zip(&rasters)
        .zip(&glyph_applications)
        .map(
            |(((allocation, glyph), raster), application)| BonusShopTextGlyphBuildReport {
                role: allocation.font_role,
                text: allocation.character.to_string(),
                code: format!("0x{:04x}", allocation.code),
                cell: allocation.cell,
                source_indexed_pixel_sha256: allocation.source_indexed_sha256.clone(),
                output_indexed_pixel_sha256: glyph.indexed_sha256.clone(),
                measured_advance_px: raster.measured_advance_px,
                ink_bounds: raster.ink_bounds,
                allowed_decoded_byte_ranges: application.allowed_ranges.clone(),
                changed_decoded_byte_count: application.changed_byte_count,
            },
        )
        .collect::<Vec<_>>();
    let unit_reports = overlay
        .units
        .iter()
        .map(|plan| {
            let unit = assets
                .units
                .get(&plan.id)
                .expect("overlay plan unit comes from validated assets");
            BonusShopTextUnitBuildReport {
                id: plan.id.clone(),
                role: unit.source.role,
                source_offset: unit.source.source_offset.clone(),
                source_record_byte_count: plan.source_record_byte_count,
                output_record_byte_count: plan.output_record_byte_count,
                changed: plan.changed,
                development_status: unit.development_status,
                release_status: unit.release_status,
            }
        })
        .collect::<Vec<_>>();
    let fonts = font_reports(config)?;
    let translation_manifest_sha256 = sha256_file(&config.assets.join("manifest.json"))?;
    let translation_unit_sha256s = assets
        .references
        .iter()
        .map(|reference| sha256_file(&config.assets.join(&reference.file)))
        .collect::<Result<Vec<_>>>()?;
    let delegated_record_sha256s = assets
        .delegated_references
        .iter()
        .map(|reference| sha256_file(&config.assets.join(&reference.file)))
        .collect::<Result<Vec<_>>>()?;
    let development_translation_input_available =
        authored_unit_count > 0 || exit_confirmation.report.development_input_available;
    let release_candidate_input_eligible = complete_source_population
        && authored_unit_count == assets.units.len()
        && release_approved_unit_count == assets.units.len()
        && exit_confirmation.report.release_candidate_input_eligible
        && card_results.release_status == BonusShopReleaseStatus::Approved;
    let report = BonusShopTextBuildReport {
        kind: "Justice Gakuen 2 Korean bonus-shop text build".to_string(),
        source_bin_sha256: source.source_bin_sha256.clone(),
        build_spec_sha256: config.build_spec_sha256.clone(),
        translation_manifest_sha256,
        translation_unit_sha256s,
        delegated_record_sha256s,
        card_result_text_sha256: sha256_file(&config.assets.join("card-results.json"))?,
        card_result_record_count: 2,
        source_shop_ui_path: SHOP_UI_PATH.to_string(),
        source_shop_ui_decoded_sha256: SHOP_UI_SOURCE_DECODED_SHA256.to_string(),
        output_shop_ui_decoded_sha256: sha256_bytes(&shop_ui_probe),
        source_overlay_path: OVERLAY_PATH.to_string(),
        source_overlay_sha256: OVERLAY_SOURCE_SHA256.to_string(),
        output_overlay_sha256: sha256_bytes(&overlay.bytes),
        source_record_count: expected_by_id.len(),
        tracked_unit_count: assets.units.len(),
        delegated_record_count: assets.delegated_records.len(),
        untranslated_unit_count,
        authored_unit_count,
        release_approved_unit_count,
        delegated_records_match_existing_writer: true,
        protected_source_code_count: allocation.protected_source_code_count,
        direct_selector_code_count: allocation.direct_selector_code_count,
        runtime_glyph_code_count: allocation.runtime_glyph_code_count,
        available_glyph_cell_count: allocation.available_glyph_cell_count,
        required_glyph_count: allocation.required_glyph_count,
        fonts,
        glyphs: glyph_reports,
        units: unit_reports,
        shop_ui_expected_write_ranges,
        shop_ui_changed_byte_ranges,
        overlay_expected_write_ranges: overlay.expected_write_ranges,
        overlay_changed_byte_ranges: overlay.changed_byte_ranges,
        source_records_match: true,
        complete_source_population,
        shop_ui_changes_confined_to_allocated_glyph_cells,
        overlay_changes_confined_to_authored_records,
        development_can_continue: complete_source_population,
        development_translation_input_available,
        release_candidate_input_eligible,
        runtime_verification_required: true,
    };
    let build_manifest_sha256 = write_outputs(&config.output_dir, &overlay.bytes, &report)?;
    Ok(BonusShopTextBuild {
        glyphs,
        glyph_codes: allocation.glyph_codes,
        units: assets.units,
        card_results,
        overlay: overlay.bytes,
        build_manifest_sha256,
        report,
    })
}

impl BonusShopTextBuild {
    pub fn apply_to_shop_ui_decoded(&self, shop_ui_decoded: &mut [u8]) -> Result<Vec<[usize; 2]>> {
        let applications = apply_bonus_shop_glyphs(shop_ui_decoded, &self.glyphs)?;
        let ranges = applications
            .into_iter()
            .flat_map(|application| application.allowed_ranges)
            .collect::<Vec<_>>();
        ensure!(
            ranges == self.report.shop_ui_expected_write_ranges,
            "composed bonus-shop glyph write ranges changed"
        );
        Ok(ranges)
    }

    pub fn apply_to_overlay(&self, overlay: &mut [u8]) -> Result<Vec<[usize; 2]>> {
        ensure!(
            overlay.len() == self.overlay.len(),
            "composed bonus-shop overlay length changed"
        );
        let mut planned = plan_bonus_shop_text_overlay(overlay, &self.units, &self.glyph_codes)?;
        self.card_results
            .append_to_plan(overlay, &mut planned, &self.glyph_codes)?;
        ensure!(
            planned.expected_write_ranges == self.report.overlay_expected_write_ranges,
            "composed bonus-shop overlay write ranges changed"
        );
        for [start, end] in &planned.expected_write_ranges {
            ensure!(
                planned.bytes[*start..*end] == self.overlay[*start..*end],
                "composed bonus-shop overlay write differs from its plan"
            );
        }
        overlay.copy_from_slice(&planned.bytes);
        Ok(planned.expected_write_ranges)
    }
}

fn font_reports(config: &BonusShopTextBuildConfig) -> Result<Vec<BonusShopTextFontBuildReport>> {
    [
        BonusShopFontRole::ProductDescription,
        BonusShopFontRole::ProductLabel,
        BonusShopFontRole::ClerkDialogue,
    ]
    .into_iter()
    .map(|role| {
        let font = config.fonts.for_role(role);
        Ok(BonusShopTextFontBuildReport {
            role,
            font_name: font
                .path
                .file_name()
                .context("bonus-shop font path has no file name")?
                .to_string_lossy()
                .into_owned(),
            font_sha256: sha256_file(&font.path)?,
            font_px: font.font_px,
            tracking_px: font.tracking_px,
            vertical_shift_px: font.vertical_shift_px,
        })
    })
    .collect()
}

fn validate_fonts(config: &BonusShopTextBuildConfig) -> Result<()> {
    for role in [
        BonusShopFontRole::ProductDescription,
        BonusShopFontRole::ProductLabel,
        BonusShopFontRole::ClerkDialogue,
    ] {
        let font = config.fonts.for_role(role);
        ensure!(
            font.path.is_file()
                && font.font_px.is_finite()
                && font.font_px > 0.0
                && font.tracking_px.is_finite()
                && (-19..=19).contains(&font.vertical_shift_px),
            "bonus-shop {role:?} font settings are invalid"
        );
    }
    Ok(())
}

fn validate_build_spec_sha256(value: &str) -> Result<()> {
    ensure!(
        value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "bonus-shop build-spec SHA-256 is invalid"
    );
    Ok(())
}

pub(super) fn range_is_covered(range: [usize; 2], allowed: &[[usize; 2]]) -> bool {
    (range[0]..range[1]).all(|offset| {
        allowed
            .iter()
            .any(|[start, end]| *start <= offset && offset < *end)
    })
}
