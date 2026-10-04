use anyhow::{Result, ensure};

use crate::pipeline::{difference_ranges, sha256_bytes};
use crate::source_disc::SupportedSourceDisc;
use crate::tim::read_indexed_cell_in_prefix;
use crate::write_scope::changed_ranges_are_within;

use super::assets::load_assets;
use super::command_sequences::CARD_ACQUISITION_SEQUENCES;
use super::consumer::{
    CALLER_RUNTIME_ADDRESSES, FOLLOWING_POINTER_STORAGE_OFFSET, SECONDARY_PARSER_RUNTIME_ADDRESS,
};
use super::glyph_atlas::{apply_card_acquisition_glyphs, rasterize_card_acquisition_glyphs};
use super::glyph_slots::{
    CARD_ACQUISITION_GLYPHS, REUSED_SOURCE_GLYPHS, SOURCE_BLANK_GLYPH_INDEXED_SHA256,
};
use super::model::{
    BonusInventoryCardAcquisitionBuild, BonusInventoryCardAcquisitionBuildConfig,
    BonusInventoryCardAcquisitionBuildReport, CardAcquisitionGlyphBuildReport,
    CardAcquisitionGlyphOwnershipEvidenceReport, CardAcquisitionUnitBuildReport, DevelopmentStatus,
    ReleaseStatus, ReusedSourceGlyphReport,
};
use super::output::{prepare_outputs, validate_font, write_outputs};
use super::overlay::{patch_card_acquisition_overlay, patch_composed_card_acquisition_overlay};
use super::source::{
    BONUS_INVENTORY_CARD_ACQUISITION_INVENTORY_PATH, BONUS_INVENTORY_CARD_ACQUISITION_OVERLAY_PATH,
    BonusInventoryCardAcquisitionSource, GLYPH_TIM_OFFSET, OVERLAY_SOURCE_SHA256, load_source,
};

pub use super::output::{
    BONUS_INVENTORY_CARD_ACQUISITION_BUILD_MANIFEST_FILE,
    BONUS_INVENTORY_CARD_ACQUISITION_OVERLAY_OUTPUT_FILE,
};

pub fn build_bonus_inventory_card_acquisition(
    config: &BonusInventoryCardAcquisitionBuildConfig,
) -> Result<BonusInventoryCardAcquisitionBuild> {
    let source = SupportedSourceDisc::open(&config.cue)?;
    build_bonus_inventory_card_acquisition_from_source(config, &source)
}

pub(crate) fn build_bonus_inventory_card_acquisition_from_source(
    config: &BonusInventoryCardAcquisitionBuildConfig,
    source_disc: &SupportedSourceDisc,
) -> Result<BonusInventoryCardAcquisitionBuild> {
    let source = load_source(source_disc)?;
    build_bonus_inventory_card_acquisition_from_inventory_source(config, &source)
}

pub(crate) fn build_bonus_inventory_card_acquisition_from_inventory_source(
    config: &BonusInventoryCardAcquisitionBuildConfig,
    source: &BonusInventoryCardAcquisitionSource,
) -> Result<BonusInventoryCardAcquisitionBuild> {
    prepare_outputs(&config.output_dir, config.force)?;
    validate_font(&config.font)?;
    ensure!(
        config.build_spec_sha256.len() == 64
            && config
                .build_spec_sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit()),
        "bonus card-acquisition build-spec SHA-256 is invalid"
    );

    let assets = load_assets(&config.assets, source)?;
    let (glyphs, rasters) = rasterize_card_acquisition_glyphs(&config.font)?;

    let mut inventory_probe = source.inventory_decoded.clone();
    let glyph_applications = apply_card_acquisition_glyphs(&mut inventory_probe, &glyphs)?;
    let inventory_expected_write_ranges = glyph_applications
        .iter()
        .flat_map(|application| application.allowed_ranges.iter().copied())
        .collect::<Vec<_>>();
    let inventory_changed_byte_ranges =
        difference_ranges(&source.inventory_decoded, &inventory_probe);
    let inventory_changes_confined_to_allocated_glyph_cells = !inventory_changed_byte_ranges
        .is_empty()
        && changed_ranges_are_within(
            &inventory_changed_byte_ranges,
            &inventory_expected_write_ranges,
        );
    ensure!(
        inventory_changes_confined_to_allocated_glyph_cells,
        "bonus card-acquisition inventory changed bytes outside allocated glyph cells"
    );

    let overlay = patch_card_acquisition_overlay(&source.overlay, &assets.units)?;
    let font_name = rasters[0].font_name.clone();
    let font_sha256 = rasters[0].font_sha256.clone();
    ensure!(
        rasters
            .iter()
            .all(|raster| raster.font_name == font_name && raster.font_sha256 == font_sha256),
        "bonus card-acquisition glyphs were rasterized from different fonts"
    );

    let glyph_reports = CARD_ACQUISITION_GLYPHS
        .iter()
        .zip(&glyphs)
        .zip(&rasters)
        .zip(&glyph_applications)
        .map(
            |(((text_code_cell, glyph), raster), application)| CardAcquisitionGlyphBuildReport {
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
    let reused_source_glyphs = REUSED_SOURCE_GLYPHS
        .into_iter()
        .map(|(text, code, cell, indexed_sha256)| {
            let source_pixels =
                read_indexed_cell_in_prefix(&source.inventory_decoded, GLYPH_TIM_OFFSET, cell)?;
            let source_indexed_pixels_match_declared_hash =
                sha256_bytes(&source_pixels) == indexed_sha256;
            ensure!(
                source_indexed_pixels_match_declared_hash,
                "reused source glyph pixels changed for {text}"
            );
            Ok(ReusedSourceGlyphReport {
                text: text.to_string(),
                code: format!("0x{code:04x}"),
                cell,
                indexed_pixel_sha256: indexed_sha256.to_string(),
                source_indexed_pixels_match_declared_hash,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let unit_reports = assets
        .units
        .iter()
        .zip(CARD_ACQUISITION_SEQUENCES)
        .zip(&overlay.output_sequence_sha256s)
        .map(
            |((unit, spec), output_sha256)| CardAcquisitionUnitBuildReport {
                id: unit.id.clone(),
                source_text: unit.source_text.clone(),
                korean_text: unit
                    .korean_text
                    .clone()
                    .expect("validated authored card-acquisition text"),
                source_sequence_offset: format!("0x{:04x}", spec.sequence_offset),
                source_storage_length: spec.storage_length,
                command_capacity: (spec.storage_length - 1) / 3,
                source_terminator_offset: format!("0x{:04x}", spec.terminator_offset),
                source_pointer_storage_offset: format!("0x{:04x}", spec.pointer_storage_offset),
                caller_runtime_addresses: spec
                    .caller_runtime_addresses
                    .iter()
                    .map(|address| format!("0x{address:08x}"))
                    .collect(),
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
    let glyph_ownership_evidence = CardAcquisitionGlyphOwnershipEvidenceReport {
        declared_physical_alias_set_matches: overlay
            .glyph_ownership
            .declared_physical_alias_set_matches,
        existing_bonus_component_allocations_disjoint: overlay
            .glyph_ownership
            .existing_bonus_component_allocations_disjoint,
        pointer_command_table_range: overlay.glyph_ownership.pointer_command_table_range,
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

    let report = BonusInventoryCardAcquisitionBuildReport {
        kind: "Justice Gakuen 2 Korean bonus inventory card-acquisition build".to_string(),
        source_bin_sha256: source.source_bin_sha256.clone(),
        build_spec_sha256: config.build_spec_sha256.clone(),
        translation_manifest_sha256: assets.manifest_sha256,
        translation_unit_sha256s: assets.unit_sha256s,
        source_inventory_path: BONUS_INVENTORY_CARD_ACQUISITION_INVENTORY_PATH.to_string(),
        source_inventory_stored_sha256: sha256_bytes(&source.inventory_stored),
        source_inventory_decoded_sha256: sha256_bytes(&source.inventory_decoded),
        output_inventory_decoded_sha256: sha256_bytes(&inventory_probe),
        source_overlay_path: BONUS_INVENTORY_CARD_ACQUISITION_OVERLAY_PATH.to_string(),
        source_overlay_sha256: OVERLAY_SOURCE_SHA256.to_string(),
        output_overlay_sha256: sha256_bytes(&overlay.bytes),
        runtime_consumer: assets.runtime_consumer,
        font_role: assets.font_role,
        secondary_parser_runtime_address: format!("0x{SECONDARY_PARSER_RUNTIME_ADDRESS:08x}"),
        caller_runtime_addresses: CALLER_RUNTIME_ADDRESSES
            .map(|address| format!("0x{address:08x}"))
            .to_vec(),
        pointer_storage_offsets: CARD_ACQUISITION_SEQUENCES
            .map(|spec| format!("0x{:04x}", spec.pointer_storage_offset))
            .to_vec(),
        following_pointer_storage_offset: format!("0x{FOLLOWING_POINTER_STORAGE_OFFSET:04x}"),
        glyphs: glyph_reports,
        reused_source_glyphs,
        units: unit_reports,
        inventory_expected_write_ranges,
        inventory_changed_byte_ranges,
        overlay_expected_write_ranges: overlay.expected_write_ranges,
        overlay_changed_byte_ranges: overlay.changed_byte_ranges,
        source_command_records_match: overlay.source_command_records_match,
        glyph_ownership_evidence,
        inventory_changes_confined_to_allocated_glyph_cells,
        overlay_changes_confined_to_fixed_command_records: overlay
            .changes_confined_to_fixed_command_records,
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
    Ok(BonusInventoryCardAcquisitionBuild {
        glyphs,
        units: assets.units,
        overlay: overlay.bytes,
        build_manifest_sha256,
        report,
    })
}

impl BonusInventoryCardAcquisitionBuild {
    pub fn apply_to_inventory_decoded(
        &self,
        inventory_decoded: &mut [u8],
    ) -> Result<Vec<[usize; 2]>> {
        let applications = apply_card_acquisition_glyphs(inventory_decoded, &self.glyphs)?;
        let ranges = applications
            .into_iter()
            .flat_map(|application| application.allowed_ranges)
            .collect::<Vec<_>>();
        ensure!(
            ranges == self.report.inventory_expected_write_ranges,
            "composed bonus card-acquisition glyph write ranges changed"
        );
        Ok(ranges)
    }

    pub fn apply_to_overlay(&self, overlay: &mut [u8]) -> Result<Vec<[usize; 2]>> {
        ensure!(
            overlay.len() == self.overlay.len(),
            "composed KOUBAI2 overlay length changed"
        );
        let patched = patch_composed_card_acquisition_overlay(overlay, &self.units)?;
        for [start, end] in &patched.expected_write_ranges {
            ensure!(
                patched.bytes[*start..*end] == self.overlay[*start..*end],
                "composed bonus card-acquisition overlay write differs from its plan"
            );
        }
        overlay.copy_from_slice(&patched.bytes);
        Ok(patched.expected_write_ranges)
    }
}
