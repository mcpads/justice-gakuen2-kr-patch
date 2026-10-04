use std::path::Path;

use anyhow::{Context, Result, bail, ensure};

use crate::pipeline::{difference_ranges, sha256_bytes, sha256_file, write_pretty_json_and_hash};
use crate::source_disc::SupportedSourceDisc;

use super::assets::load_assets;
use super::command_sequences::{
    SOURCE_MEMORY_CARD_COPY_PROMPT_SHA256, SOURCE_MEMORY_CARD_DESTINATION_SHA256,
};
use super::consumer::{
    COMMAND_RENDERER_RUNTIME_ADDRESS, COMPOSER_RUNTIME_ADDRESS, DIRECT_CALLER_RUNTIME_ADDRESSES,
    DIRECT_CALLER_SELECTOR_VALUES, EXIT_CALLER_RUNTIME_ADDRESS,
    MEMORY_CARD_COPY_PROMPT_POINTER_OFFSET, MEMORY_CARD_DESTINATION_POINTER_OFFSET,
};
use super::glyph_atlas::{
    apply_confirmation_glyphs, rasterize_confirmation_glyphs, validate_fixed_record_space_is_blank,
};
use super::glyph_slots::{
    CONFIRMATION_GLYPHS, FIXED_RECORD_SPACE_CELL, FIXED_RECORD_SPACE_CODE,
    SOURCE_BLANK_GLYPH_INDEXED_SHA256,
};
use super::model::{
    BonusConfirmationBuild, BonusConfirmationBuildConfig, BonusConfirmationBuildReport,
    BonusConfirmationFontSource, ConfirmationGlyphBuildReport, ConfirmationReservedBlankReport,
    ConfirmationUnitBuildReport, DevelopmentStatus, ReleaseStatus,
};
use super::overlay::{patch_composed_confirmation_overlay, patch_confirmation_overlay};
use super::records::{
    SOURCE_ALTERNATE_PROMPT_SHA256, SOURCE_EXIT_PROMPT_SHA256, SOURCE_SHARED_CHOICE_SHA256,
};
use super::source::{
    BONUS_CONFIRMATION_INVENTORY_PATH, BONUS_CONFIRMATION_OVERLAY_PATH, BonusConfirmationSource,
    OVERLAY_SOURCE_SHA256, load_source,
};
use super::text_units::TEXT_UNIT_SPECS;

pub const BONUS_CONFIRMATION_OVERLAY_OUTPUT_FILE: &str = "bonus-confirmation-koubai2.bin";
pub const BONUS_CONFIRMATION_BUILD_MANIFEST_FILE: &str = "bonus-confirmation-build.json";

pub fn build_bonus_confirmation(
    config: &BonusConfirmationBuildConfig,
) -> Result<BonusConfirmationBuild> {
    let source = SupportedSourceDisc::open(&config.cue)?;
    build_bonus_confirmation_from_source(config, &source)
}

pub(crate) fn build_bonus_confirmation_from_source(
    config: &BonusConfirmationBuildConfig,
    source_disc: &SupportedSourceDisc,
) -> Result<BonusConfirmationBuild> {
    let source = load_source(source_disc)?;
    build_bonus_confirmation_from_inventory_source(config, &source)
}

pub(crate) fn build_bonus_confirmation_from_inventory_source(
    config: &BonusConfirmationBuildConfig,
    source: &BonusConfirmationSource,
) -> Result<BonusConfirmationBuild> {
    prepare_outputs(&config.output_dir, config.force)?;
    validate_font(&config.font)?;
    ensure!(
        config.build_spec_sha256.len() == 64
            && config
                .build_spec_sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit()),
        "bonus confirmation build-spec SHA-256 is invalid"
    );

    let assets = load_assets(&config.assets, source)?;
    let (glyphs, rasters) = rasterize_confirmation_glyphs(&config.font)?;

    let mut inventory_probe = source.inventory_decoded.clone();
    let glyph_applications = apply_confirmation_glyphs(&mut inventory_probe, &glyphs)?;
    let inventory_expected_write_ranges = glyph_applications
        .iter()
        .flat_map(|application| application.allowed_ranges.iter().copied())
        .collect::<Vec<_>>();
    let inventory_changed_byte_ranges =
        difference_ranges(&source.inventory_decoded, &inventory_probe);
    ensure!(
        !inventory_changed_byte_ranges.is_empty()
            && inventory_changed_byte_ranges.iter().all(|[start, end]| {
                inventory_expected_write_ranges
                    .iter()
                    .any(|[allowed_start, allowed_end]| {
                        allowed_start <= start && end <= allowed_end
                    })
            }),
        "bonus confirmation inventory changed bytes outside allocated glyph cells"
    );
    let fixed_record_space_hash = validate_fixed_record_space_is_blank(&inventory_probe)?;

    let overlay = patch_confirmation_overlay(&source.overlay, &assets.units)?;
    let font_name = rasters[0].font_name.clone();
    let font_sha256 = rasters[0].font_sha256.clone();
    ensure!(
        rasters
            .iter()
            .all(|raster| raster.font_name == font_name && raster.font_sha256 == font_sha256),
        "bonus confirmation glyphs were rasterized from different fonts"
    );

    let glyph_reports = CONFIRMATION_GLYPHS
        .iter()
        .zip(&glyphs)
        .zip(&rasters)
        .zip(&glyph_applications)
        .map(
            |(((text_code_cell, glyph), raster), application)| ConfirmationGlyphBuildReport {
                text: text_code_cell.0.to_string(),
                code: format!("0x{:04x}", text_code_cell.1),
                cell: glyph.cell,
                source_indexed_pixel_sha256: SOURCE_BLANK_GLYPH_INDEXED_SHA256.to_string(),
                output_indexed_pixel_sha256: glyph.indexed_sha256.clone(),
                measured_advance_px: raster.measured_advance_px,
                ink_bounds: raster.ink_bounds,
                allowed_decoded_byte_ranges: application.allowed_ranges.clone(),
                changed_decoded_byte_count: application.changed_byte_count,
            },
        )
        .collect();
    let unit_reports = assets
        .units
        .iter()
        .zip(TEXT_UNIT_SPECS)
        .zip(&overlay.output_unit_sha256s)
        .map(
            |((unit, spec), output_sha256)| ConfirmationUnitBuildReport {
                id: unit.id.clone(),
                source_text: unit.source_text.clone(),
                korean_text: unit
                    .korean_text
                    .clone()
                    .expect("validated authored confirmation text"),
                source_record_offset: format!("0x{:04x}", spec.record_offset),
                source_record_length: spec.record_length,
                source_encoded_sha256: unit.source.encoded_sha256.clone(),
                output_encoded_sha256: output_sha256.clone(),
                development_status: unit.development_status,
                release_status: unit.release_status,
            },
        )
        .collect::<Vec<_>>();
    let development_input_available = assets
        .units
        .iter()
        .all(|unit| unit.development_status == DevelopmentStatus::Authored);
    let release_candidate_input_eligible = assets
        .units
        .iter()
        .all(|unit| unit.release_status == ReleaseStatus::Approved);
    let direct_caller_runtime_addresses = DIRECT_CALLER_RUNTIME_ADDRESSES
        .iter()
        .map(|address| format!("0x{address:08x}"))
        .collect::<Vec<_>>();
    let report = BonusConfirmationBuildReport {
        kind: "Justice Gakuen 2 Korean bonus confirmation overlay-renderer build".to_string(),
        source_bin_sha256: source.source_bin_sha256.clone(),
        build_spec_sha256: config.build_spec_sha256.clone(),
        translation_manifest_sha256: assets.manifest_sha256,
        translation_unit_sha256s: assets.unit_sha256s,
        source_inventory_path: BONUS_CONFIRMATION_INVENTORY_PATH.to_string(),
        source_inventory_stored_sha256: sha256_bytes(&source.inventory_stored),
        source_inventory_decoded_sha256: sha256_bytes(&source.inventory_decoded),
        source_overlay_path: BONUS_CONFIRMATION_OVERLAY_PATH.to_string(),
        source_overlay_sha256: OVERLAY_SOURCE_SHA256.to_string(),
        output_overlay_sha256: sha256_bytes(&overlay.bytes),
        runtime_consumer: assets.runtime_consumer,
        composer_runtime_address: format!("0x{COMPOSER_RUNTIME_ADDRESS:08x}"),
        command_renderer_runtime_address: format!("0x{COMMAND_RENDERER_RUNTIME_ADDRESS:08x}"),
        exit_caller_runtime_address: format!("0x{EXIT_CALLER_RUNTIME_ADDRESS:08x}"),
        direct_caller_runtime_addresses: direct_caller_runtime_addresses.clone(),
        direct_caller_selector_values: DIRECT_CALLER_SELECTOR_VALUES.to_vec(),
        memory_card_command_pointer_offsets: [
            MEMORY_CARD_DESTINATION_POINTER_OFFSET,
            MEMORY_CARD_COPY_PROMPT_POINTER_OFFSET,
        ]
        .map(|offset| format!("0x{offset:04x}"))
        .to_vec(),
        shared_choice_caller_runtime_addresses: direct_caller_runtime_addresses,
        shared_choice_scope: assets.shared_choice_scope,
        selected_card_prompt_translated: overlay.output_selected_card_prompt_sha256
            != SOURCE_ALTERNATE_PROMPT_SHA256,
        memory_card_destination_translated: overlay.output_memory_card_destination_sha256
            != SOURCE_MEMORY_CARD_DESTINATION_SHA256,
        memory_card_copy_prompt_translated: overlay.output_memory_card_copy_prompt_sha256
            != SOURCE_MEMORY_CARD_COPY_PROMPT_SHA256,
        fixed_record_space: ConfirmationReservedBlankReport {
            code: format!("0x{FIXED_RECORD_SPACE_CODE:04x}"),
            cell: FIXED_RECORD_SPACE_CELL,
            source_indexed_pixel_sha256: SOURCE_BLANK_GLYPH_INDEXED_SHA256.to_string(),
            output_indexed_pixel_sha256: fixed_record_space_hash,
            remains_blank: true,
            included_in_expected_write_ranges: false,
        },
        glyphs: glyph_reports,
        units: unit_reports,
        source_exit_prompt_record_sha256: SOURCE_EXIT_PROMPT_SHA256.to_string(),
        output_exit_prompt_record_sha256: overlay.output_exit_prompt_sha256,
        source_selected_card_prompt_record_sha256: SOURCE_ALTERNATE_PROMPT_SHA256.to_string(),
        output_selected_card_prompt_record_sha256: overlay.output_selected_card_prompt_sha256,
        source_memory_card_destination_sha256: SOURCE_MEMORY_CARD_DESTINATION_SHA256.to_string(),
        output_memory_card_destination_sha256: overlay.output_memory_card_destination_sha256,
        source_memory_card_copy_prompt_sha256: SOURCE_MEMORY_CARD_COPY_PROMPT_SHA256.to_string(),
        output_memory_card_copy_prompt_sha256: overlay.output_memory_card_copy_prompt_sha256,
        source_shared_choice_record_sha256: SOURCE_SHARED_CHOICE_SHA256.to_string(),
        output_shared_choice_record_sha256: overlay.output_shared_choice_sha256,
        inventory_expected_write_ranges,
        inventory_changed_byte_ranges,
        overlay_expected_write_ranges: overlay.expected_write_ranges,
        overlay_changed_byte_ranges: overlay.changed_byte_ranges,
        source_regions_match: true,
        allocated_glyphs_are_unreferenced_by_known_source_consumers: true,
        changes_confined_to_owned_regions: true,
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
    Ok(BonusConfirmationBuild {
        glyphs,
        units: assets.units,
        overlay: overlay.bytes,
        build_manifest_sha256,
        report,
    })
}

impl BonusConfirmationBuild {
    pub fn apply_to_inventory_decoded(
        &self,
        inventory_decoded: &mut [u8],
    ) -> Result<Vec<[usize; 2]>> {
        let applications = apply_confirmation_glyphs(inventory_decoded, &self.glyphs)?;
        let ranges = applications
            .into_iter()
            .flat_map(|application| application.allowed_ranges)
            .collect::<Vec<_>>();
        ensure!(
            ranges == self.report.inventory_expected_write_ranges,
            "composed bonus confirmation glyph write ranges changed"
        );
        Ok(ranges)
    }

    pub fn apply_to_overlay(&self, overlay: &mut [u8]) -> Result<Vec<[usize; 2]>> {
        ensure!(
            overlay.len() == self.overlay.len(),
            "composed KOUBAI2 overlay length changed"
        );
        let patched = patch_composed_confirmation_overlay(overlay, &self.units)?;
        for [start, end] in &patched.expected_write_ranges {
            ensure!(
                patched.bytes[*start..*end] == self.overlay[*start..*end],
                "composed bonus confirmation overlay write differs from its plan"
            );
        }
        overlay.copy_from_slice(&patched.bytes);
        Ok(patched.expected_write_ranges)
    }
}

fn validate_font(font: &BonusConfirmationFontSource) -> Result<()> {
    ensure!(
        font.font_px.is_finite()
            && font.font_px > 0.0
            && font.tracking_px.is_finite()
            && (-19..=19).contains(&font.vertical_shift_px),
        "bonus confirmation font settings are invalid"
    );
    Ok(())
}

fn prepare_outputs(output_dir: &Path, force: bool) -> Result<()> {
    std::fs::create_dir_all(output_dir)?;
    for file in [
        BONUS_CONFIRMATION_OVERLAY_OUTPUT_FILE,
        BONUS_CONFIRMATION_BUILD_MANIFEST_FILE,
    ] {
        let path = output_dir.join(file);
        if path.exists() && !force {
            bail!("bonus confirmation output exists; pass --force to replace it");
        }
    }
    Ok(())
}

fn write_outputs(
    output_dir: &Path,
    overlay: &[u8],
    report: &BonusConfirmationBuildReport,
) -> Result<String> {
    let overlay_path = output_dir.join(BONUS_CONFIRMATION_OVERLAY_OUTPUT_FILE);
    std::fs::write(&overlay_path, overlay)
        .with_context(|| format!("failed to write {}", overlay_path.display()))?;
    let manifest_path = output_dir.join(BONUS_CONFIRMATION_BUILD_MANIFEST_FILE);
    let build_manifest_sha256 = write_pretty_json_and_hash(&manifest_path, report, true)?;
    ensure!(
        sha256_file(&overlay_path)? == report.output_overlay_sha256,
        "bonus confirmation overlay changed while writing"
    );
    Ok(build_manifest_sha256)
}
