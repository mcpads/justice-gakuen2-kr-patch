use std::path::Path;

use anyhow::{Context, Result, bail, ensure};

use crate::font::{IndexedTextRasterizer, RasterizedIndexedText};
use crate::pipeline::{difference_ranges, sha256_bytes, sha256_file, write_pretty_json_and_hash};
use crate::source_disc::SupportedSourceDisc;
use crate::tim::{
    parse_4bpp_prefix, read_indexed_cell_in_prefix, write_indexed_cell_in_prefix_with_report,
};

use super::assets::load_assets;
use super::model::{
    ActionGlyphBuildReport, ActionGlyphPixels, ActionLabelUnitBuildReport,
    BonusInventoryActionLabelBuild, BonusInventoryActionLabelBuildConfig,
    BonusInventoryActionLabelBuildReport, BonusInventoryActionLabelFontSource, DevelopmentStatus,
    ReleaseStatus,
};
use super::overlay::{
    ACTION_SEQUENCES, POINTER_TABLE_OFFSET, RUNTIME_OBSERVED_RETURN_ADDRESS,
    SECONDARY_PARSER_RUNTIME_ADDRESS, STATIC_CALLER_RUNTIME_ADDRESSES, patch_action_label_overlay,
};
use super::source::{
    ACTION_GLYPHS, BONUS_INVENTORY_ACTION_LABEL_INVENTORY_PATH,
    BONUS_INVENTORY_ACTION_LABEL_OVERLAY_PATH, BonusInventoryActionLabelSource, GLYPH_TIM_OFFSET,
    GLYPH_TIM_SIZE, OVERLAY_SOURCE_SHA256, SOURCE_BLANK_GLYPH_INDEXED_SHA256, load_source,
};

pub const BONUS_INVENTORY_ACTION_LABEL_OVERLAY_OUTPUT_FILE: &str =
    "bonus-inventory-action-labels-koubai2.bin";
pub const BONUS_INVENTORY_ACTION_LABEL_BUILD_MANIFEST_FILE: &str =
    "bonus-inventory-action-labels-build.json";

pub fn build_bonus_inventory_action_labels(
    config: &BonusInventoryActionLabelBuildConfig,
) -> Result<BonusInventoryActionLabelBuild> {
    let source = SupportedSourceDisc::open(&config.cue)?;
    build_bonus_inventory_action_labels_from_source(config, &source)
}

pub(crate) fn build_bonus_inventory_action_labels_from_source(
    config: &BonusInventoryActionLabelBuildConfig,
    source_disc: &SupportedSourceDisc,
) -> Result<BonusInventoryActionLabelBuild> {
    let source = load_source(source_disc)?;
    build_bonus_inventory_action_labels_from_inventory_source(config, &source)
}

pub(crate) fn build_bonus_inventory_action_labels_from_inventory_source(
    config: &BonusInventoryActionLabelBuildConfig,
    source: &BonusInventoryActionLabelSource,
) -> Result<BonusInventoryActionLabelBuild> {
    prepare_outputs(&config.output_dir, config.force)?;
    validate_font(&config.font)?;
    ensure!(
        config.build_spec_sha256.len() == 64
            && config
                .build_spec_sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit()),
        "bonus action-label build-spec SHA-256 is invalid"
    );

    let assets = load_assets(&config.assets, source)?;
    let rasterizer = IndexedTextRasterizer::load(&config.font.path)?;
    let mut rasters = Vec::with_capacity(ACTION_GLYPHS.len());
    let mut glyphs = Vec::with_capacity(ACTION_GLYPHS.len());
    for (text, code, cell) in ACTION_GLYPHS {
        let raster = rasterize_action_glyph(&rasterizer, &config.font, text)?;
        ensure!(
            raster.pixels.len() == cell.width * cell.height,
            "bonus action-label glyph raster dimensions changed"
        );
        glyphs.push(ActionGlyphPixels {
            code,
            cell,
            indexed_sha256: sha256_bytes(&raster.pixels),
            pixels: raster.pixels.clone(),
        });
        rasters.push(raster);
    }

    let mut inventory_probe = source.inventory_decoded.clone();
    let glyph_applications = apply_action_glyphs(&mut inventory_probe, &glyphs)?;
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
        "bonus action-label inventory changed bytes outside allocated glyph cells"
    );

    let overlay = patch_action_label_overlay(&source.overlay, &assets.units)?;
    let font_name = rasters[0].font_name.clone();
    let font_sha256 = rasters[0].font_sha256.clone();
    ensure!(
        rasters
            .iter()
            .all(|raster| raster.font_name == font_name && raster.font_sha256 == font_sha256),
        "bonus action-label glyphs were rasterized from different fonts"
    );

    let glyph_reports = ACTION_GLYPHS
        .iter()
        .zip(&glyphs)
        .zip(&rasters)
        .zip(&glyph_applications)
        .map(
            |(((text_code_cell, glyph), raster), application)| ActionGlyphBuildReport {
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
        .zip(ACTION_SEQUENCES)
        .zip(&overlay.output_sequence_sha256s)
        .map(|((unit, spec), output_sha256)| ActionLabelUnitBuildReport {
            id: unit.id.clone(),
            source_text: unit.source_text.clone(),
            korean_text: unit
                .korean_text
                .clone()
                .expect("validated authored action label"),
            source_sequence_offset: format!("0x{:04x}", spec.sequence_offset),
            source_terminator_offset: format!("0x{:04x}", spec.terminator_offset),
            source_pointer_storage_offset: format!("0x{:04x}", spec.pointer_storage_offset),
            source_encoded_sha256: unit.source.encoded_sha256.clone(),
            output_encoded_sha256: output_sha256.clone(),
            development_status: unit.development_status,
            release_status: unit.release_status,
        })
        .collect::<Vec<_>>();
    let development_input_available = assets
        .units
        .iter()
        .all(|unit| unit.development_status == DevelopmentStatus::Authored);
    let release_candidate_input_eligible = assets
        .units
        .iter()
        .all(|unit| unit.release_status == ReleaseStatus::Approved);
    let report = BonusInventoryActionLabelBuildReport {
        kind: "Justice Gakuen 2 Korean bonus inventory dynamic action-label build".to_string(),
        source_bin_sha256: source.source_bin_sha256.clone(),
        build_spec_sha256: config.build_spec_sha256.clone(),
        translation_manifest_sha256: assets.manifest_sha256,
        translation_unit_sha256s: assets.unit_sha256s,
        source_inventory_path: BONUS_INVENTORY_ACTION_LABEL_INVENTORY_PATH.to_string(),
        source_inventory_stored_sha256: sha256_bytes(&source.inventory_stored),
        source_inventory_decoded_sha256: sha256_bytes(&source.inventory_decoded),
        source_overlay_path: BONUS_INVENTORY_ACTION_LABEL_OVERLAY_PATH.to_string(),
        source_overlay_sha256: OVERLAY_SOURCE_SHA256.to_string(),
        output_overlay_sha256: sha256_bytes(&overlay.bytes),
        runtime_consumer: assets.runtime_consumer,
        secondary_parser_runtime_address: format!("0x{SECONDARY_PARSER_RUNTIME_ADDRESS:08x}"),
        statically_bound_caller_runtime_addresses: STATIC_CALLER_RUNTIME_ADDRESSES
            .iter()
            .map(|address| format!("0x{address:08x}"))
            .collect(),
        runtime_observed_return_address: format!("0x{RUNTIME_OBSERVED_RETURN_ADDRESS:08x}"),
        source_pointer_table_offset: format!("0x{POINTER_TABLE_OFFSET:04x}"),
        glyphs: glyph_reports,
        units: unit_reports,
        inventory_expected_write_ranges,
        inventory_changed_byte_ranges,
        overlay_expected_write_ranges: overlay.expected_write_ranges,
        overlay_changed_byte_ranges: overlay.changed_byte_ranges,
        source_regions_match: true,
        allocated_glyphs_are_unreferenced_by_source_consumers: true,
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
    Ok(BonusInventoryActionLabelBuild {
        glyphs,
        units: assets.units,
        overlay: overlay.bytes,
        build_manifest_sha256,
        report,
    })
}

impl BonusInventoryActionLabelBuild {
    pub fn apply_to_inventory_decoded(
        &self,
        inventory_decoded: &mut [u8],
    ) -> Result<Vec<[usize; 2]>> {
        let applications = apply_action_glyphs(inventory_decoded, &self.glyphs)?;
        let ranges = applications
            .into_iter()
            .flat_map(|application| application.allowed_ranges)
            .collect::<Vec<_>>();
        ensure!(
            ranges == self.report.inventory_expected_write_ranges,
            "composed bonus action-label glyph write ranges changed"
        );
        Ok(ranges)
    }

    pub fn apply_to_overlay(&self, overlay: &mut [u8]) -> Result<Vec<[usize; 2]>> {
        ensure!(
            overlay.len() == self.overlay.len(),
            "composed KOUBAI2 overlay length changed"
        );
        let patched = patch_action_label_overlay(overlay, &self.units)?;
        for [start, end] in &patched.expected_write_ranges {
            ensure!(
                patched.bytes[*start..*end] == self.overlay[*start..*end],
                "composed bonus action-label overlay write differs from its plan"
            );
        }
        overlay.copy_from_slice(&patched.bytes);
        Ok(patched.expected_write_ranges)
    }
}

struct GlyphApplication {
    allowed_ranges: Vec<[usize; 2]>,
    changed_byte_count: usize,
}

fn apply_action_glyphs(
    inventory_decoded: &mut [u8],
    glyphs: &[ActionGlyphPixels],
) -> Result<Vec<GlyphApplication>> {
    let glyph_tim = parse_4bpp_prefix(
        inventory_decoded
            .get(GLYPH_TIM_OFFSET..)
            .ok_or_else(|| anyhow::anyhow!("KOUBAI1 glyph TIM disappeared during composition"))?,
    )?;
    ensure!(
        glyph_tim.total_size == GLYPH_TIM_SIZE
            && glyph_tim.pixel_width() == 1024
            && glyph_tim.image_height == 256
            && glyph_tim.image_x == 512
            && glyph_tim.image_y == 0,
        "KOUBAI1 action-label glyph TIM geometry changed during composition"
    );
    ensure!(
        glyphs.len() == ACTION_GLYPHS.len(),
        "bonus action-label glyph plan changed"
    );
    let mut applications = Vec::with_capacity(glyphs.len());
    for (glyph, (_, expected_code, expected_cell)) in glyphs.iter().zip(ACTION_GLYPHS) {
        ensure!(
            glyph.code == expected_code && glyph.cell == expected_cell,
            "bonus action-label glyph identity changed"
        );
        let source_pixels =
            read_indexed_cell_in_prefix(inventory_decoded, GLYPH_TIM_OFFSET, glyph.cell)?;
        ensure!(
            sha256_bytes(&source_pixels) == SOURCE_BLANK_GLYPH_INDEXED_SHA256,
            "bonus action-label glyph cell has another writer or changed source pixels"
        );
        ensure!(
            sha256_bytes(&glyph.pixels) == glyph.indexed_sha256,
            "bonus action-label glyph pixels differ from their build plan"
        );
        let write = write_indexed_cell_in_prefix_with_report(
            inventory_decoded,
            GLYPH_TIM_OFFSET,
            glyph.cell,
            &glyph.pixels,
        )?;
        ensure!(
            write.changed_byte_count > 0,
            "bonus action-label glyph changed no bytes"
        );
        let output_pixels =
            read_indexed_cell_in_prefix(inventory_decoded, GLYPH_TIM_OFFSET, glyph.cell)?;
        ensure!(
            sha256_bytes(&output_pixels) == glyph.indexed_sha256,
            "bonus action-label glyph readback differs from its build plan"
        );
        applications.push(GlyphApplication {
            allowed_ranges: write.allowed_ranges,
            changed_byte_count: write.changed_byte_count,
        });
    }
    Ok(applications)
}

fn rasterize_action_glyph(
    rasterizer: &IndexedTextRasterizer,
    font: &BonusInventoryActionLabelFontSource,
    text: &str,
) -> Result<RasterizedIndexedText> {
    crate::bonus_inventory_source::rasterize_message_glyph(rasterizer, font, text)
}

fn validate_font(font: &BonusInventoryActionLabelFontSource) -> Result<()> {
    ensure!(
        font.font_px.is_finite()
            && font.font_px > 0.0
            && font.tracking_px.is_finite()
            && (-19..=19).contains(&font.vertical_shift_px),
        "bonus action-label font settings are invalid"
    );
    Ok(())
}

fn prepare_outputs(output_dir: &Path, force: bool) -> Result<()> {
    std::fs::create_dir_all(output_dir)?;
    for file in [
        BONUS_INVENTORY_ACTION_LABEL_OVERLAY_OUTPUT_FILE,
        BONUS_INVENTORY_ACTION_LABEL_BUILD_MANIFEST_FILE,
    ] {
        let path = output_dir.join(file);
        if path.exists() && !force {
            bail!("bonus action-label output exists; pass --force to replace it");
        }
    }
    Ok(())
}

fn write_outputs(
    output_dir: &Path,
    overlay: &[u8],
    report: &BonusInventoryActionLabelBuildReport,
) -> Result<String> {
    let overlay_path = output_dir.join(BONUS_INVENTORY_ACTION_LABEL_OVERLAY_OUTPUT_FILE);
    std::fs::write(&overlay_path, overlay)
        .with_context(|| format!("failed to write {}", overlay_path.display()))?;
    let manifest_path = output_dir.join(BONUS_INVENTORY_ACTION_LABEL_BUILD_MANIFEST_FILE);
    let build_manifest_sha256 = write_pretty_json_and_hash(&manifest_path, report, true)?;
    ensure!(
        sha256_file(&overlay_path)? == report.output_overlay_sha256,
        "bonus action-label overlay changed while writing"
    );
    Ok(build_manifest_sha256)
}
