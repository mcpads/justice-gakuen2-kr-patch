use std::path::Path;

use anyhow::{Context, Result, bail, ensure};

use crate::font::{IndexedTextRasterizer, RasterizedIndexedText};
use crate::pipeline::{difference_ranges, sha256_bytes, sha256_file, write_pretty_json_and_hash};
use crate::source_disc::SupportedSourceDisc;
use crate::tim::{
    parse_4bpp_prefix, read_indexed_cell_in_prefix, write_indexed_cell_in_prefix_with_report,
};

use super::assets::load_assets;
use super::consumer::{
    CATEGORY_HANDLER_RUNTIME_ADDRESS, CATEGORY_JUMP_TABLE_OFFSET, CATEGORY_STATE_TARGETS,
    CHANGED_INSTRUCTION_OFFSETS, NONCALLING_EXIT_STATE, OUTPUT_GLYPH_CODES, RENDERER_CALL_OFFSETS,
    RENDERER_CALLING_STATES, SCREEN_POSITIONS, SELECTOR_INSTRUCTION_OFFSETS, SOURCE_GLYPH_CODES,
    SPRITE_SIZE, STOCK_LABEL_RENDERER_RUNTIME_ADDRESS, TEXTURE_PAGE_INSTRUCTION_OFFSET,
};
use super::model::{
    BonusInventoryStockLabelBuild, BonusInventoryStockLabelBuildConfig,
    BonusInventoryStockLabelBuildReport, BonusInventoryStockLabelFontSource, DevelopmentStatus,
    ReleaseStatus, StockLabelGlyphBuildReport, StockLabelGlyphPixels,
};
use super::overlay::{apply_stock_label_overlay, patch_stock_label_overlay};
use super::source::{
    BONUS_INVENTORY_STOCK_LABEL_INVENTORY_PATH, BONUS_INVENTORY_STOCK_LABEL_OVERLAY_PATH,
    BonusInventoryStockLabelSource, GLYPH_TIM_OFFSET, GLYPH_TIM_SIZE, OVERLAY_SOURCE_SHA256,
    SOURCE_BLANK_GLYPH_INDEXED_SHA256, STOCK_LABEL_GLYPHS, load_source,
};

pub const BONUS_INVENTORY_STOCK_LABEL_OVERLAY_OUTPUT_FILE: &str =
    "bonus-inventory-stock-label-koubai2.bin";
pub const BONUS_INVENTORY_STOCK_LABEL_BUILD_MANIFEST_FILE: &str =
    "bonus-inventory-stock-label-build.json";

pub fn build_bonus_inventory_stock_label(
    config: &BonusInventoryStockLabelBuildConfig,
) -> Result<BonusInventoryStockLabelBuild> {
    let source = SupportedSourceDisc::open(&config.cue)?;
    build_bonus_inventory_stock_label_from_source(config, &source)
}

pub(crate) fn build_bonus_inventory_stock_label_from_source(
    config: &BonusInventoryStockLabelBuildConfig,
    source_disc: &SupportedSourceDisc,
) -> Result<BonusInventoryStockLabelBuild> {
    let source = load_source(source_disc)?;
    build_bonus_inventory_stock_label_from_inventory_source(config, &source)
}

pub(crate) fn build_bonus_inventory_stock_label_from_inventory_source(
    config: &BonusInventoryStockLabelBuildConfig,
    source: &BonusInventoryStockLabelSource,
) -> Result<BonusInventoryStockLabelBuild> {
    prepare_outputs(&config.output_dir, config.force)?;
    validate_font(&config.font)?;
    ensure!(
        config.build_spec_sha256.len() == 64
            && config
                .build_spec_sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit()),
        "bonus stock-label build-spec SHA-256 is invalid"
    );

    let assets = load_assets(&config.assets, source)?;
    let rasterizer = IndexedTextRasterizer::load(&config.font.path)?;
    let mut rasters = Vec::with_capacity(STOCK_LABEL_GLYPHS.len());
    let mut glyphs = Vec::with_capacity(STOCK_LABEL_GLYPHS.len());
    for (text, code, cell) in STOCK_LABEL_GLYPHS {
        let raster = rasterize_stock_label_glyph(&rasterizer, &config.font, text)?;
        ensure!(
            raster.pixels.len() == cell.width * cell.height,
            "bonus stock-label glyph raster dimensions changed"
        );
        glyphs.push(StockLabelGlyphPixels {
            code,
            cell,
            indexed_sha256: sha256_bytes(&raster.pixels),
            pixels: raster.pixels.clone(),
        });
        rasters.push(raster);
    }

    let mut inventory_probe = source.inventory_decoded.clone();
    let glyph_applications = apply_stock_label_glyphs(&mut inventory_probe, &glyphs)?;
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
        "bonus stock-label inventory changed bytes outside allocated glyph cells"
    );

    let overlay = patch_stock_label_overlay(&source.overlay)?;
    let font_name = rasters[0].font_name.clone();
    let font_sha256 = rasters[0].font_sha256.clone();
    ensure!(
        rasters
            .iter()
            .all(|raster| raster.font_name == font_name && raster.font_sha256 == font_sha256),
        "bonus stock-label glyphs were rasterized from different fonts"
    );
    let glyph_reports = STOCK_LABEL_GLYPHS
        .iter()
        .zip(&glyphs)
        .zip(&rasters)
        .zip(&glyph_applications)
        .map(
            |(((text_code_cell, glyph), raster), application)| StockLabelGlyphBuildReport {
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
    let report = BonusInventoryStockLabelBuildReport {
        kind: "Justice Gakuen 2 Korean bonus inventory stock-count label build".to_string(),
        source_bin_sha256: source.source_bin_sha256.clone(),
        build_spec_sha256: config.build_spec_sha256.clone(),
        translation_manifest_sha256: assets.manifest_sha256,
        translation_unit_sha256: assets.unit_sha256,
        source_inventory_path: BONUS_INVENTORY_STOCK_LABEL_INVENTORY_PATH.to_string(),
        source_inventory_stored_sha256: sha256_bytes(&source.inventory_stored),
        source_inventory_decoded_sha256: sha256_bytes(&source.inventory_decoded),
        output_inventory_decoded_sha256: sha256_bytes(&inventory_probe),
        source_overlay_path: BONUS_INVENTORY_STOCK_LABEL_OVERLAY_PATH.to_string(),
        source_overlay_sha256: OVERLAY_SOURCE_SHA256.to_string(),
        output_overlay_sha256: sha256_bytes(&overlay.bytes),
        runtime_consumer: assets.runtime_consumer,
        renderer_runtime_address: format!("0x{STOCK_LABEL_RENDERER_RUNTIME_ADDRESS:08x}"),
        category_handler_runtime_address: format!("0x{CATEGORY_HANDLER_RUNTIME_ADDRESS:08x}"),
        category_jump_table_offset: format!("0x{CATEGORY_JUMP_TABLE_OFFSET:05x}"),
        category_state_target_runtime_addresses: CATEGORY_STATE_TARGETS
            .map(|address| format!("0x{address:08x}"))
            .to_vec(),
        renderer_caller_runtime_addresses: RENDERER_CALL_OFFSETS
            .map(|offset| {
                format!(
                    "0x{:08x}",
                    super::consumer::OVERLAY_RUNTIME_BASE + offset as u32
                )
            })
            .to_vec(),
        renderer_calling_states: RENDERER_CALLING_STATES.to_vec(),
        noncalling_exit_state: NONCALLING_EXIT_STATE,
        exit_state_skips_renderer: true,
        source_glyph_codes: SOURCE_GLYPH_CODES
            .map(|code| format!("0x{code:04x}"))
            .to_vec(),
        output_glyph_codes: OUTPUT_GLYPH_CODES
            .map(|code| format!("0x{code:04x}"))
            .to_vec(),
        source_selector_instruction_offsets: SELECTOR_INSTRUCTION_OFFSETS
            .into_iter()
            .chain([TEXTURE_PAGE_INSTRUCTION_OFFSET])
            .map(|offset| format!("0x{offset:04x}"))
            .collect(),
        changed_selector_instruction_offsets: CHANGED_INSTRUCTION_OFFSETS
            .map(|offset| format!("0x{offset:04x}"))
            .to_vec(),
        screen_positions: SCREEN_POSITIONS.to_vec(),
        sprite_size: SPRITE_SIZE,
        glyphs: glyph_reports,
        inventory_expected_write_ranges,
        inventory_changed_byte_ranges,
        overlay_expected_write_ranges: overlay.expected_write_ranges,
        overlay_changed_byte_ranges: overlay.changed_byte_ranges,
        source_regions_match: true,
        allocated_glyphs_are_unreferenced_by_known_source_consumers: true,
        existing_bonus_glyph_allocations_are_disjoint: true,
        changes_confined_to_owned_regions: true,
        font_role: assets.font_role,
        font_name,
        font_sha256,
        font_px: config.font.font_px,
        tracking_px: config.font.tracking_px,
        vertical_shift_px: config.font.vertical_shift_px,
        development_input_available: assets.unit.development_status == DevelopmentStatus::Authored,
        release_candidate_input_eligible: assets.unit.release_status == ReleaseStatus::Approved,
        runtime_verification_required: true,
    };
    let build_manifest_sha256 = write_outputs(&config.output_dir, &overlay.bytes, &report)?;
    Ok(BonusInventoryStockLabelBuild {
        glyphs,
        overlay: overlay.bytes,
        build_manifest_sha256,
        report,
    })
}

impl BonusInventoryStockLabelBuild {
    pub fn apply_to_inventory_decoded(
        &self,
        inventory_decoded: &mut [u8],
    ) -> Result<Vec<[usize; 2]>> {
        let applications = apply_stock_label_glyphs(inventory_decoded, &self.glyphs)?;
        let ranges = applications
            .into_iter()
            .flat_map(|application| application.allowed_ranges)
            .collect::<Vec<_>>();
        ensure!(
            ranges == self.report.inventory_expected_write_ranges,
            "composed bonus stock-label glyph write ranges changed"
        );
        Ok(ranges)
    }

    pub fn apply_to_overlay(&self, overlay: &mut [u8]) -> Result<Vec<[usize; 2]>> {
        ensure!(
            overlay.len() == self.overlay.len(),
            "composed KOUBAI2 overlay length changed"
        );
        let ranges = apply_stock_label_overlay(overlay)?;
        for [start, end] in &ranges {
            ensure!(
                overlay[*start..*end] == self.overlay[*start..*end],
                "composed bonus stock-label overlay write differs from its plan"
            );
        }
        Ok(ranges)
    }
}

struct GlyphApplication {
    allowed_ranges: Vec<[usize; 2]>,
    changed_byte_count: usize,
}

fn apply_stock_label_glyphs(
    inventory_decoded: &mut [u8],
    glyphs: &[StockLabelGlyphPixels],
) -> Result<Vec<GlyphApplication>> {
    let glyph_tim =
        parse_4bpp_prefix(inventory_decoded.get(GLYPH_TIM_OFFSET..).ok_or_else(|| {
            anyhow::anyhow!("KOUBAI1 stock-label glyph TIM disappeared during composition")
        })?)?;
    ensure!(
        glyph_tim.total_size == GLYPH_TIM_SIZE
            && glyph_tim.pixel_width() == 1024
            && glyph_tim.image_height == 256
            && glyph_tim.image_x == 512
            && glyph_tim.image_y == 0,
        "KOUBAI1 stock-label glyph TIM geometry changed during composition"
    );
    ensure!(
        glyphs.len() == STOCK_LABEL_GLYPHS.len(),
        "bonus stock-label glyph plan changed"
    );
    let mut applications = Vec::with_capacity(glyphs.len());
    for (glyph, (_, expected_code, expected_cell)) in glyphs.iter().zip(STOCK_LABEL_GLYPHS) {
        ensure!(
            glyph.code == expected_code && glyph.cell == expected_cell,
            "bonus stock-label glyph identity changed"
        );
        let source_pixels =
            read_indexed_cell_in_prefix(inventory_decoded, GLYPH_TIM_OFFSET, glyph.cell)?;
        ensure!(
            sha256_bytes(&source_pixels) == SOURCE_BLANK_GLYPH_INDEXED_SHA256,
            "bonus stock-label glyph cell has another writer or changed source pixels"
        );
        ensure!(
            sha256_bytes(&glyph.pixels) == glyph.indexed_sha256,
            "bonus stock-label glyph pixels differ from their build plan"
        );
        let write = write_indexed_cell_in_prefix_with_report(
            inventory_decoded,
            GLYPH_TIM_OFFSET,
            glyph.cell,
            &glyph.pixels,
        )?;
        ensure!(
            write.changed_byte_count > 0,
            "bonus stock-label glyph changed no bytes"
        );
        let output_pixels =
            read_indexed_cell_in_prefix(inventory_decoded, GLYPH_TIM_OFFSET, glyph.cell)?;
        ensure!(
            sha256_bytes(&output_pixels) == glyph.indexed_sha256,
            "bonus stock-label glyph readback differs from its build plan"
        );
        applications.push(GlyphApplication {
            allowed_ranges: write.allowed_ranges,
            changed_byte_count: write.changed_byte_count,
        });
    }
    Ok(applications)
}

pub(super) fn rasterize_stock_label_glyph(
    rasterizer: &IndexedTextRasterizer,
    font: &BonusInventoryStockLabelFontSource,
    text: &str,
) -> Result<RasterizedIndexedText> {
    crate::bonus_inventory_source::rasterize_message_glyph(rasterizer, font, text)
}

fn validate_font(font: &BonusInventoryStockLabelFontSource) -> Result<()> {
    ensure!(
        font.font_px.is_finite()
            && font.font_px > 0.0
            && font.tracking_px.is_finite()
            && (-19..=19).contains(&font.vertical_shift_px),
        "bonus stock-label font settings are invalid"
    );
    Ok(())
}

fn prepare_outputs(output_dir: &Path, force: bool) -> Result<()> {
    std::fs::create_dir_all(output_dir)?;
    for file in [
        BONUS_INVENTORY_STOCK_LABEL_OVERLAY_OUTPUT_FILE,
        BONUS_INVENTORY_STOCK_LABEL_BUILD_MANIFEST_FILE,
    ] {
        let path = output_dir.join(file);
        if path.exists() && !force {
            bail!("bonus stock-label output exists; pass --force to replace it");
        }
    }
    Ok(())
}

fn write_outputs(
    output_dir: &Path,
    overlay: &[u8],
    report: &BonusInventoryStockLabelBuildReport,
) -> Result<String> {
    let overlay_path = output_dir.join(BONUS_INVENTORY_STOCK_LABEL_OVERLAY_OUTPUT_FILE);
    std::fs::write(&overlay_path, overlay)
        .with_context(|| format!("failed to write {}", overlay_path.display()))?;
    let manifest_path = output_dir.join(BONUS_INVENTORY_STOCK_LABEL_BUILD_MANIFEST_FILE);
    let build_manifest_sha256 = write_pretty_json_and_hash(&manifest_path, report, true)?;
    ensure!(
        sha256_file(&overlay_path)? == report.output_overlay_sha256,
        "stock-label overlay changed while writing"
    );
    Ok(build_manifest_sha256)
}
