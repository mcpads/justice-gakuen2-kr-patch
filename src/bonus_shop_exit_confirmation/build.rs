use anyhow::{Result, ensure};

use crate::pipeline::{difference_ranges, sha256_bytes};
use crate::tim::read_indexed_cell_in_prefix;
use crate::write_scope::changed_ranges_are_within;

use super::assets::load_assets;
use super::command_sequence::RECORD_SPECS;
use super::consumer::{CALLER_RUNTIME_ADDRESS, COMMAND_RENDERER_RUNTIME_ADDRESS};
use super::glyph_atlas::{
    apply_exit_confirmation_glyphs, rasterize_exit_confirmation_glyphs, source_glyph_sha256,
};
use super::glyph_slots::{QUESTION_MARK_CODE, QUESTION_MARK_SOURCE_INDEXED_SHA256, glyph_cell};
use super::model::{
    BonusShopExitConfirmationBuild, BonusShopExitConfirmationBuildConfig,
    BonusShopExitConfirmationBuildReport, DevelopmentStatus, ReleaseStatus,
    ReusedSourceGlyphReport, ShopExitGlyphAllocation, ShopExitGlyphBuildReport,
    ShopExitGlyphOwnershipEvidenceReport, ShopExitRecordBuildReport, ShopExitUnitBuildReport,
};
use super::output::{prepare_outputs, validate_font, write_outputs};
use super::overlay::{patch_composed_exit_confirmation_overlay, patch_exit_confirmation_overlay};
use super::source::{
    BonusShopExitConfirmationSource, GLYPH_TIM_OFFSET, OVERLAY_PATH, OVERLAY_SOURCE_SHA256,
    SHOP_UI_PATH, load_source,
};

pub use super::output::{
    BONUS_SHOP_EXIT_CONFIRMATION_BUILD_MANIFEST_FILE,
    BONUS_SHOP_EXIT_CONFIRMATION_OVERLAY_OUTPUT_FILE,
};

pub fn build_bonus_shop_exit_confirmation(
    config: &BonusShopExitConfirmationBuildConfig,
) -> Result<BonusShopExitConfirmationBuild> {
    let source = load_source(&config.cue)?;
    build_bonus_shop_exit_confirmation_from_source(config, &source)
}

pub(crate) fn build_bonus_shop_exit_confirmation_from_source(
    config: &BonusShopExitConfirmationBuildConfig,
    source: &BonusShopExitConfirmationSource,
) -> Result<BonusShopExitConfirmationBuild> {
    prepare_outputs(&config.output_dir, config.force)?;
    validate_font(&config.font)?;
    ensure!(
        config.build_spec_sha256.len() == 64
            && config
                .build_spec_sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit()),
        "shop exit-confirmation build-spec SHA-256 is invalid"
    );

    let assets = load_assets(&config.assets, source)?;
    // Keep the native code and all command bindings; replace only its font pixels.
    let mut rendered_glyphs = assets.glyph_allocations.clone();
    rendered_glyphs.push(ShopExitGlyphAllocation {
        text: '?',
        code: QUESTION_MARK_CODE,
        cell: glyph_cell(QUESTION_MARK_CODE),
    });
    let (glyphs, rasters) = rasterize_exit_confirmation_glyphs(&config.font, &rendered_glyphs)?;

    let mut shop_ui_probe = source.shop_ui_decoded.clone();
    let glyph_applications = apply_exit_confirmation_glyphs(&mut shop_ui_probe, &glyphs)?;
    let shop_ui_expected_write_ranges = glyph_applications
        .iter()
        .flat_map(|application| application.allowed_ranges.iter().copied())
        .collect::<Vec<_>>();
    let shop_ui_changed_byte_ranges = difference_ranges(&source.shop_ui_decoded, &shop_ui_probe);
    let shop_ui_changes_confined_to_allocated_glyph_cells = !shop_ui_changed_byte_ranges.is_empty()
        && changed_ranges_are_within(&shop_ui_changed_byte_ranges, &shop_ui_expected_write_ranges);
    ensure!(
        shop_ui_changes_confined_to_allocated_glyph_cells,
        "shop exit-confirmation UI changed outside allocated glyph cells"
    );

    let overlay =
        patch_exit_confirmation_overlay(&source.overlay, &assets.units, &assets.glyph_allocations)?;
    let first_raster = rasters
        .first()
        .expect("validated shop exit-confirmation glyph population");
    let font_name = first_raster.font_name.clone();
    let font_sha256 = first_raster.font_sha256.clone();
    ensure!(
        rasters
            .iter()
            .all(|raster| raster.font_name == font_name && raster.font_sha256 == font_sha256),
        "shop exit-confirmation glyphs were rasterized from different fonts"
    );

    let glyph_reports = rendered_glyphs
        .iter()
        .zip(&glyphs)
        .zip(&rasters)
        .zip(&glyph_applications)
        .map(
            |(((allocation, glyph), raster), application)| ShopExitGlyphBuildReport {
                text: allocation.text.to_string(),
                code: format!("0x{:04x}", glyph.code),
                cell: glyph.cell,
                source_indexed_pixel_sha256: source_glyph_sha256(glyph.code).to_string(),
                output_indexed_pixel_sha256: glyph.indexed_sha256.clone(),
                measured_advance_px: raster.measured_advance_px,
                ink_bounds: raster.ink_bounds,
                allowed_decoded_byte_ranges: application.allowed_ranges.clone(),
                changed_decoded_byte_count: application.changed_byte_count,
            },
        )
        .collect();
    let question_mark_cell = glyph_cell(QUESTION_MARK_CODE);
    let question_mark_pixels = read_indexed_cell_in_prefix(
        &source.shop_ui_decoded,
        GLYPH_TIM_OFFSET,
        question_mark_cell,
    )?;
    let question_mark_matches =
        sha256_bytes(&question_mark_pixels) == QUESTION_MARK_SOURCE_INDEXED_SHA256;
    ensure!(
        question_mark_matches,
        "reused shop exit-confirmation question-mark pixels changed"
    );
    let reused_source_glyphs = vec![ReusedSourceGlyphReport {
        text: "?".to_string(),
        code: format!("0x{QUESTION_MARK_CODE:04x}"),
        cell: question_mark_cell,
        indexed_pixel_sha256: QUESTION_MARK_SOURCE_INDEXED_SHA256.to_string(),
        source_indexed_pixels_match_declared_hash: question_mark_matches,
    }];
    let unit_reports = assets
        .units
        .iter()
        .map(|unit| ShopExitUnitBuildReport {
            id: unit.id.clone(),
            source_text: unit.source_text.clone(),
            korean_text: unit
                .korean_text
                .clone()
                .expect("validated authored shop exit-confirmation text"),
            source_encoded_offset: unit.source.encoded_offset.clone(),
            source_encoded_length: unit.source.encoded_length,
            source_encoded_sha256: unit.source.encoded_sha256.clone(),
            output_command_count: overlay.unit_command_counts[&unit.id],
            development_status: unit.development_status,
            release_status: unit.release_status,
        })
        .collect();
    let record_reports = RECORD_SPECS
        .iter()
        .zip(&overlay.records)
        .map(|(spec, record)| ShopExitRecordBuildReport {
            variant_id: record.variant_id.to_string(),
            clerk_index: spec.clerk_index,
            pointer_storage_offset: format!("0x{:04x}", spec.pointer_storage_offset),
            record_offset: format!("0x{:04x}", spec.record_offset),
            source_record_sha256: spec.source_record_sha256.to_string(),
            output_record_sha256: record.output_record_sha256.clone(),
            output_record_byte_length: record.output_record_byte_length,
            output_line_count: record.output_line_count,
            output_translation_command_counts: record.output_translation_command_counts,
        })
        .collect();
    let glyph_ownership_evidence = ShopExitGlyphOwnershipEvidenceReport {
        declared_physical_alias_set_matches: overlay
            .glyph_ownership
            .declared_physical_alias_set_matches,
        physical_alias_source_cells_match_blank_hash: assets
            .physical_alias_source_cells_match_blank_hash,
        fixed_ui_tim_disjoint: overlay.glyph_ownership.fixed_ui_tim_disjoint,
        pointer_command_record_count: overlay.glyph_ownership.pointer_command_record_count,
        pointer_command_unique_glyph_count: overlay
            .glyph_ownership
            .pointer_command_unique_glyph_count,
        pointer_command_table_parsed_glyphs_disjoint: overlay
            .glyph_ownership
            .pointer_command_table_parsed_glyphs_disjoint,
        declared_direct_selector_byte_region: overlay
            .glyph_ownership
            .declared_direct_selector_byte_region,
        declared_direct_selector_byte_region_scan_disjoint: overlay
            .glyph_ownership
            .declared_direct_selector_byte_region_scan_disjoint,
    };
    let development_input_available = assets
        .units
        .iter()
        .all(|unit| unit.development_status == DevelopmentStatus::Authored);
    let release_candidate_input_eligible = assets
        .units
        .iter()
        .all(|unit| unit.release_status == ReleaseStatus::Approved);

    let report = BonusShopExitConfirmationBuildReport {
        kind: "Justice Gakuen 2 Korean bonus shop exit-confirmation build".to_string(),
        source_bin_sha256: source.source_bin_sha256.clone(),
        build_spec_sha256: config.build_spec_sha256.clone(),
        translation_manifest_sha256: assets.manifest_sha256,
        translation_unit_sha256s: assets.unit_sha256s,
        source_shop_ui_path: SHOP_UI_PATH.to_string(),
        source_shop_ui_stored_sha256: sha256_bytes(&source.shop_ui_stored),
        source_shop_ui_decoded_sha256: sha256_bytes(&source.shop_ui_decoded),
        output_shop_ui_decoded_sha256: sha256_bytes(&shop_ui_probe),
        source_overlay_path: OVERLAY_PATH.to_string(),
        source_overlay_sha256: OVERLAY_SOURCE_SHA256.to_string(),
        output_overlay_sha256: sha256_bytes(&overlay.bytes),
        runtime_consumer: assets.runtime_consumer,
        font_role: assets.font_role,
        command_renderer_runtime_address: format!("0x{COMMAND_RENDERER_RUNTIME_ADDRESS:08x}"),
        caller_runtime_address: format!("0x{CALLER_RUNTIME_ADDRESS:08x}"),
        records: record_reports,
        glyphs: glyph_reports,
        reused_source_glyphs,
        units: unit_reports,
        shop_ui_expected_write_ranges,
        shop_ui_changed_byte_ranges,
        overlay_expected_write_ranges: overlay.expected_write_ranges,
        overlay_changed_byte_ranges: overlay.changed_byte_ranges,
        source_command_records_match: overlay.source_command_records_match,
        glyph_ownership_evidence,
        shop_ui_changes_confined_to_allocated_glyph_cells,
        overlay_changes_confined_to_fixed_record: overlay.changes_confined_to_fixed_record,
        font_name,
        font_sha256,
        font_px: config.font.font_px,
        tracking_px: config.font.tracking_px,
        vertical_shift_px: config.font.vertical_shift_px,
        development_input_available,
        release_candidate_input_eligible,
        runtime_verification_required: true,
    };
    let build_manifest_sha256 = write_outputs(&config.output_dir, &overlay.bytes, &report)?;
    Ok(BonusShopExitConfirmationBuild {
        glyphs,
        glyph_allocations: assets.glyph_allocations,
        units: assets.units,
        overlay: overlay.bytes,
        build_manifest_sha256,
        report,
    })
}

impl BonusShopExitConfirmationBuild {
    pub fn apply_to_shop_ui_decoded(&self, shop_ui_decoded: &mut [u8]) -> Result<Vec<[usize; 2]>> {
        let applications = apply_exit_confirmation_glyphs(shop_ui_decoded, &self.glyphs)?;
        let ranges = applications
            .into_iter()
            .flat_map(|application| application.allowed_ranges)
            .collect::<Vec<_>>();
        ensure!(
            ranges == self.report.shop_ui_expected_write_ranges,
            "composed shop exit-confirmation glyph write ranges changed"
        );
        Ok(ranges)
    }

    pub fn apply_to_overlay(&self, overlay: &mut [u8]) -> Result<Vec<[usize; 2]>> {
        ensure!(
            overlay.len() == self.overlay.len(),
            "composed KOUBAI overlay length changed"
        );
        let patched = patch_composed_exit_confirmation_overlay(
            overlay,
            &self.units,
            &self.glyph_allocations,
        )?;
        for [start, end] in &patched.expected_write_ranges {
            ensure!(
                patched.bytes[*start..*end] == self.overlay[*start..*end],
                "composed shop exit-confirmation overlay write differs from its plan"
            );
        }
        overlay.copy_from_slice(&patched.bytes);
        Ok(patched.expected_write_ranges)
    }
}
