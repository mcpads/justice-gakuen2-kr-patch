use anyhow::{Result, ensure};

use crate::pipeline::{difference_ranges, sha256_bytes};
use crate::source_disc::SupportedSourceDisc;
use crate::write_scope::changed_ranges_are_within;

use super::assets::load_assets;
use super::command_sequence::RETURN_LABEL_SEQUENCE;
use super::consumer::{CALLER_RUNTIME_ADDRESSES, COMMAND_RENDERER_RUNTIME_ADDRESS};
use super::glyph_atlas::{apply_return_label_glyphs, rasterize_return_label_glyphs};
use super::glyph_slots::SOURCE_BLANK_GLYPH_INDEXED_SHA256;
use super::model::{
    BonusJBankReturnLabelBuild, BonusJBankReturnLabelBuildConfig, BonusJBankReturnLabelBuildReport,
    DevelopmentStatus, JBankReturnLabelGlyphBuildReport,
    JBankReturnLabelGlyphOwnershipEvidenceReport, JBankReturnLabelUnitBuildReport, ReleaseStatus,
};
use super::output::{prepare_outputs, validate_font, write_outputs};
use super::overlay::{patch_composed_return_label_overlay, patch_return_label_overlay};
use super::source::{
    BONUS_J_BANK_RETURN_LABEL_INVENTORY_PATH, BONUS_J_BANK_RETURN_LABEL_OVERLAY_PATH,
    BonusJBankReturnLabelSource, OVERLAY_SOURCE_SHA256, load_source,
};

pub use super::output::{
    BONUS_J_BANK_RETURN_LABEL_BUILD_MANIFEST_FILE, BONUS_J_BANK_RETURN_LABEL_OVERLAY_OUTPUT_FILE,
};

pub fn build_bonus_j_bank_return_label(
    config: &BonusJBankReturnLabelBuildConfig,
) -> Result<BonusJBankReturnLabelBuild> {
    let source = SupportedSourceDisc::open(&config.cue)?;
    build_bonus_j_bank_return_label_from_source(config, &source)
}

pub(crate) fn build_bonus_j_bank_return_label_from_source(
    config: &BonusJBankReturnLabelBuildConfig,
    source_disc: &SupportedSourceDisc,
) -> Result<BonusJBankReturnLabelBuild> {
    let source = load_source(source_disc)?;
    build_bonus_j_bank_return_label_from_inventory_source(config, &source)
}

pub(crate) fn build_bonus_j_bank_return_label_from_inventory_source(
    config: &BonusJBankReturnLabelBuildConfig,
    source: &BonusJBankReturnLabelSource,
) -> Result<BonusJBankReturnLabelBuild> {
    prepare_outputs(&config.output_dir, config.force)?;
    validate_font(&config.font)?;
    ensure!(
        config.build_spec_sha256.len() == 64
            && config
                .build_spec_sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit()),
        "J-BANK return-label build-spec SHA-256 is invalid"
    );

    let assets = load_assets(&config.assets, source)?;
    let (glyphs, rasters) = rasterize_return_label_glyphs(&config.font, &assets.glyph_allocations)?;

    let mut inventory_probe = source.inventory_decoded.clone();
    let glyph_applications = apply_return_label_glyphs(&mut inventory_probe, &glyphs)?;
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
        "J-BANK return-label inventory changed outside allocated glyph cells"
    );

    let overlay =
        patch_return_label_overlay(&source.overlay, &assets.unit, &assets.glyph_allocations)?;
    let font_name = rasters[0].font_name.clone();
    let font_sha256 = rasters[0].font_sha256.clone();
    ensure!(
        rasters
            .iter()
            .all(|raster| raster.font_name == font_name && raster.font_sha256 == font_sha256),
        "J-BANK return-label glyphs were rasterized from different fonts"
    );

    let glyph_reports = assets
        .glyph_allocations
        .iter()
        .zip(&glyphs)
        .zip(&rasters)
        .zip(&glyph_applications)
        .map(
            |(((allocation, glyph), raster), application)| JBankReturnLabelGlyphBuildReport {
                text: allocation.text.to_string(),
                code: format!("0x{:04x}", glyph.code),
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
    let korean_text = assets
        .unit
        .korean_text
        .clone()
        .expect("validated authored J-BANK return-label text");
    let unit_report = JBankReturnLabelUnitBuildReport {
        id: assets.unit.id.clone(),
        source_text: assets.unit.source_text.clone(),
        korean_text,
        source_sequence_offset: format!("0x{:04x}", RETURN_LABEL_SEQUENCE.sequence_offset),
        source_storage_length: RETURN_LABEL_SEQUENCE.storage_length,
        source_terminator_offset: format!("0x{:04x}", RETURN_LABEL_SEQUENCE.terminator_offset),
        source_pointer_storage_offset: format!(
            "0x{:04x}",
            RETURN_LABEL_SEQUENCE.pointer_storage_offset
        ),
        caller_runtime_addresses: RETURN_LABEL_SEQUENCE
            .caller_runtime_addresses
            .iter()
            .map(|address| format!("0x{address:08x}"))
            .collect(),
        source_encoded_sha256: assets.unit.source.encoded_sha256.clone(),
        output_encoded_sha256: overlay.output_sequence_sha256.clone(),
        output_payload_byte_length: overlay.output_payload_byte_length,
        output_command_count: overlay.output_command_count,
        development_status: assets.unit.development_status,
        release_status: assets.unit.release_status,
    };
    let glyph_ownership_evidence = JBankReturnLabelGlyphOwnershipEvidenceReport {
        declared_physical_alias_set_matches: overlay
            .glyph_ownership
            .declared_physical_alias_set_matches,
        physical_alias_source_cells_match_blank_hash: assets
            .physical_alias_source_cells_match_blank_hash,
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
    let development_input_available = assets.unit.development_status == DevelopmentStatus::Authored;
    let release_candidate_input_eligible = assets.unit.release_status == ReleaseStatus::Approved;

    let report = BonusJBankReturnLabelBuildReport {
        kind: "Justice Gakuen 2 Korean bonus J-BANK return-label build".to_string(),
        source_bin_sha256: source.source_bin_sha256.clone(),
        build_spec_sha256: config.build_spec_sha256.clone(),
        translation_manifest_sha256: assets.manifest_sha256,
        translation_unit_sha256: assets.unit_sha256,
        source_inventory_path: BONUS_J_BANK_RETURN_LABEL_INVENTORY_PATH.to_string(),
        source_inventory_stored_sha256: sha256_bytes(&source.inventory_stored),
        source_inventory_decoded_sha256: sha256_bytes(&source.inventory_decoded),
        output_inventory_decoded_sha256: sha256_bytes(&inventory_probe),
        source_overlay_path: BONUS_J_BANK_RETURN_LABEL_OVERLAY_PATH.to_string(),
        source_overlay_sha256: OVERLAY_SOURCE_SHA256.to_string(),
        output_overlay_sha256: sha256_bytes(&overlay.bytes),
        runtime_consumer: assets.runtime_consumer,
        font_role: assets.font_role,
        command_renderer_runtime_address: format!("0x{COMMAND_RENDERER_RUNTIME_ADDRESS:08x}"),
        caller_runtime_addresses: CALLER_RUNTIME_ADDRESSES
            .map(|address| format!("0x{address:08x}"))
            .to_vec(),
        pointer_storage_offset: format!("0x{:04x}", RETURN_LABEL_SEQUENCE.pointer_storage_offset),
        glyphs: glyph_reports,
        unit: unit_report,
        inventory_expected_write_ranges,
        inventory_changed_byte_ranges,
        overlay_expected_write_ranges: overlay.expected_write_ranges,
        overlay_changed_byte_ranges: overlay.changed_byte_ranges,
        source_command_record_matches: overlay.source_command_record_matches,
        glyph_ownership_evidence,
        inventory_changes_confined_to_allocated_glyph_cells,
        overlay_changes_confined_to_fixed_command_record: overlay
            .changes_confined_to_fixed_command_record,
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
    Ok(BonusJBankReturnLabelBuild {
        glyphs,
        glyph_allocations: assets.glyph_allocations,
        unit: assets.unit,
        overlay: overlay.bytes,
        build_manifest_sha256,
        report,
    })
}

impl BonusJBankReturnLabelBuild {
    pub fn apply_to_inventory_decoded(
        &self,
        inventory_decoded: &mut [u8],
    ) -> Result<Vec<[usize; 2]>> {
        let applications = apply_return_label_glyphs(inventory_decoded, &self.glyphs)?;
        let ranges = applications
            .into_iter()
            .flat_map(|application| application.allowed_ranges)
            .collect::<Vec<_>>();
        ensure!(
            ranges == self.report.inventory_expected_write_ranges,
            "composed J-BANK return-label glyph write ranges changed"
        );
        Ok(ranges)
    }

    pub fn apply_to_overlay(&self, overlay: &mut [u8]) -> Result<Vec<[usize; 2]>> {
        ensure!(
            overlay.len() == self.overlay.len(),
            "composed KOUBAI2 overlay length changed"
        );
        let patched =
            patch_composed_return_label_overlay(overlay, &self.unit, &self.glyph_allocations)?;
        for [start, end] in &patched.expected_write_ranges {
            ensure!(
                patched.bytes[*start..*end] == self.overlay[*start..*end],
                "composed J-BANK return-label overlay write differs from its plan"
            );
        }
        overlay.copy_from_slice(&patched.bytes);
        Ok(patched.expected_write_ranges)
    }
}
