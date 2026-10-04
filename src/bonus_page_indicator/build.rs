use std::path::Path;

use anyhow::{Context, Result, bail, ensure};

use crate::font::{IndexedTextRasterizer, RasterizedIndexedText};
use crate::pipeline::{sha256_bytes, sha256_file, write_pretty_json_and_hash};
use crate::source_disc::SupportedSourceDisc;
use crate::tim::{
    parse_4bpp_prefix, read_indexed_cell_in_prefix, write_indexed_cell_in_prefix_with_report,
};

use super::assets::load_asset;
use super::model::{
    BonusPageIndicatorBuild, BonusPageIndicatorBuildConfig, BonusPageIndicatorBuildReport,
    BonusPageIndicatorFontSource, DevelopmentStatus, ReleaseStatus,
};
use super::overlay::{
    CONSUMER_RUNTIME_ADDRESS, OUTPUT_SUFFIX_SPRITE_COUNT, SOURCE_SUFFIX_SPRITE_COUNT,
    patch_page_indicator_overlay, suffix_glyph_reference_offsets,
};
use super::source::{
    BONUS_PAGE_INDICATOR_INVENTORY_PATH, BONUS_PAGE_INDICATOR_OVERLAY_PATH,
    BonusPageIndicatorSource, GLYPH_TIM_OFFSET, GLYPH_TIM_SIZE, OVERLAY_SOURCE_SHA256,
    SOURCE_SUFFIX_GLYPH_CODE, SOURCE_SUFFIX_GLYPH_INDEXED_SHA256,
    SOURCE_SUFFIX_GLYPH_PACKED_SHA256, SUFFIX_GLYPH_CELL, load_source, pack_indexed_pixels,
};

pub const BONUS_PAGE_INDICATOR_OVERLAY_OUTPUT_FILE: &str = "bonus-page-indicator-koubai2.bin";
pub const BONUS_PAGE_INDICATOR_BUILD_MANIFEST_FILE: &str = "bonus-page-indicator-build.json";

pub fn build_bonus_page_indicator(
    config: &BonusPageIndicatorBuildConfig,
) -> Result<BonusPageIndicatorBuild> {
    let source = SupportedSourceDisc::open(&config.cue)?;
    build_bonus_page_indicator_from_source(config, &source)
}

pub(crate) fn build_bonus_page_indicator_from_source(
    config: &BonusPageIndicatorBuildConfig,
    source_disc: &SupportedSourceDisc,
) -> Result<BonusPageIndicatorBuild> {
    let source = load_source(source_disc)?;
    build_bonus_page_indicator_from_inventory_source(config, &source)
}

pub(crate) fn build_bonus_page_indicator_from_inventory_source(
    config: &BonusPageIndicatorBuildConfig,
    source: &BonusPageIndicatorSource,
) -> Result<BonusPageIndicatorBuild> {
    prepare_outputs(&config.output_dir, config.force)?;
    validate_font(&config.font)?;
    ensure!(
        config.build_spec_sha256.len() == 64
            && config
                .build_spec_sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit()),
        "bonus page-indicator build-spec SHA-256 is invalid"
    );

    let loaded_asset = load_asset(&config.asset)?;
    let korean_text = loaded_asset
        .asset
        .korean_text
        .as_deref()
        .context("authored page-indicator suffix lost Korean text")?;
    let raster = rasterize_suffix(&config.font, korean_text)?;
    let output_suffix_glyph_packed_sha256 = sha256_bytes(&pack_indexed_pixels(&raster.pixels)?);
    let output_suffix_glyph_indexed_sha256 = sha256_bytes(&raster.pixels);

    let mut inventory_probe = source.inventory_decoded.clone();
    let glyph_application = apply_suffix_pixels(
        &mut inventory_probe,
        &raster.pixels,
        &output_suffix_glyph_packed_sha256,
        &output_suffix_glyph_indexed_sha256,
    )?;
    let overlay = patch_page_indicator_overlay(&source.overlay)?;
    let (source_suffix_sprite_selector_offsets, source_suffix_string_selector_offsets) =
        suffix_glyph_reference_offsets(&source.overlay);

    let report = BonusPageIndicatorBuildReport {
        kind: "Justice Gakuen 2 Korean bonus card-viewer page-indicator build".to_string(),
        source_bin_sha256: source.source_bin_sha256.clone(),
        build_spec_sha256: config.build_spec_sha256.clone(),
        translation_asset_sha256: loaded_asset.sha256,
        source_inventory_path: BONUS_PAGE_INDICATOR_INVENTORY_PATH.to_string(),
        source_inventory_stored_sha256: sha256_bytes(&source.inventory_stored),
        source_inventory_decoded_sha256: sha256_bytes(&source.inventory_decoded),
        source_overlay_path: BONUS_PAGE_INDICATOR_OVERLAY_PATH.to_string(),
        source_overlay_sha256: OVERLAY_SOURCE_SHA256.to_string(),
        output_overlay_sha256: sha256_bytes(&overlay.bytes),
        runtime_consumer: loaded_asset.asset.runtime_consumer,
        consumer_runtime_address: format!("0x{CONSUMER_RUNTIME_ADDRESS:08x}"),
        source_text: loaded_asset.asset.source_text,
        korean_text: korean_text.to_string(),
        source_suffix_sprite_count: SOURCE_SUFFIX_SPRITE_COUNT,
        output_suffix_sprite_count: OUTPUT_SUFFIX_SPRITE_COUNT,
        dynamic_current_page_lookup_preserved: true,
        dynamic_total_page_lookup_preserved: true,
        source_suffix_glyph_code: format!("0x{SOURCE_SUFFIX_GLYPH_CODE:04x}"),
        source_suffix_sprite_selector_offsets: source_suffix_sprite_selector_offsets
            .into_iter()
            .map(|offset| format!("0x{offset:04x}"))
            .collect(),
        source_suffix_string_selector_offsets: source_suffix_string_selector_offsets
            .into_iter()
            .map(|offset| format!("0x{offset:04x}"))
            .collect(),
        suffix_glyph_cell: SUFFIX_GLYPH_CELL,
        source_suffix_glyph_packed_sha256: SOURCE_SUFFIX_GLYPH_PACKED_SHA256.to_string(),
        output_suffix_glyph_packed_sha256,
        output_suffix_glyph_indexed_sha256,
        suffix_glyph_allowed_decoded_byte_ranges: glyph_application.allowed_ranges,
        suffix_glyph_changed_decoded_byte_count: glyph_application.changed_byte_count,
        overlay_expected_write_ranges: overlay.expected_write_ranges,
        overlay_changed_byte_ranges: overlay.changed_byte_ranges,
        source_regions_match: true,
        changes_confined_to_owned_regions: true,
        font_name: raster.font_name,
        font_sha256: raster.font_sha256,
        font_px: config.font.font_px,
        tracking_px: config.font.tracking_px,
        vertical_shift_px: config.font.vertical_shift_px,
        measured_advance_px: raster.measured_advance_px,
        ink_bounds: raster.ink_bounds,
        development_input_available: loaded_asset.asset.development_status
            == DevelopmentStatus::Authored,
        release_candidate_input_eligible: loaded_asset.asset.release_status
            == ReleaseStatus::Approved,
        runtime_verification_required: true,
    };
    let build_manifest_sha256 = write_outputs(&config.output_dir, &overlay.bytes, &report)?;
    Ok(BonusPageIndicatorBuild {
        suffix_pixels: raster.pixels,
        overlay: overlay.bytes,
        build_manifest_sha256,
        report,
    })
}

impl BonusPageIndicatorBuild {
    pub fn apply_to_inventory_decoded(
        &self,
        inventory_decoded: &mut [u8],
    ) -> Result<Vec<[usize; 2]>> {
        let application = apply_suffix_pixels(
            inventory_decoded,
            &self.suffix_pixels,
            &self.report.output_suffix_glyph_packed_sha256,
            &self.report.output_suffix_glyph_indexed_sha256,
        )?;
        ensure!(
            application.allowed_ranges == self.report.suffix_glyph_allowed_decoded_byte_ranges,
            "composed page-indicator glyph write ranges changed"
        );
        Ok(application.allowed_ranges)
    }

    pub fn apply_to_overlay(&self, overlay: &mut [u8]) -> Result<Vec<[usize; 2]>> {
        ensure!(
            overlay.len() == self.overlay.len(),
            "composed KOUBAI2 overlay length changed"
        );
        let expected_ranges = apply_overlay_patch(overlay)?;
        for [start, end] in &expected_ranges {
            ensure!(
                overlay[*start..*end] == self.overlay[*start..*end],
                "composed page-indicator overlay write differs from its plan"
            );
        }
        Ok(expected_ranges)
    }
}

pub(super) fn apply_overlay_patch(overlay: &mut [u8]) -> Result<Vec<[usize; 2]>> {
    let patched = patch_page_indicator_overlay(overlay)?;
    ensure!(
        patched.bytes.len() == overlay.len(),
        "page-indicator overlay patch changed record length"
    );
    overlay.copy_from_slice(&patched.bytes);
    Ok(patched.expected_write_ranges)
}

struct SuffixGlyphApplication {
    allowed_ranges: Vec<[usize; 2]>,
    changed_byte_count: usize,
}

fn apply_suffix_pixels(
    inventory_decoded: &mut [u8],
    suffix_pixels: &[u8],
    expected_packed_sha256: &str,
    expected_indexed_sha256: &str,
) -> Result<SuffixGlyphApplication> {
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
        "KOUBAI1 page-indicator glyph TIM geometry changed during composition"
    );
    let source_pixels =
        read_indexed_cell_in_prefix(inventory_decoded, GLYPH_TIM_OFFSET, SUFFIX_GLYPH_CELL)?;
    ensure!(
        sha256_bytes(&source_pixels) == SOURCE_SUFFIX_GLYPH_INDEXED_SHA256
            && sha256_bytes(&pack_indexed_pixels(&source_pixels)?)
                == SOURCE_SUFFIX_GLYPH_PACKED_SHA256,
        "page-indicator suffix cell has another writer or changed source pixels"
    );
    ensure!(
        sha256_bytes(suffix_pixels) == expected_indexed_sha256
            && sha256_bytes(&pack_indexed_pixels(suffix_pixels)?) == expected_packed_sha256,
        "page-indicator suffix pixels do not match the planned font raster"
    );

    let write = write_indexed_cell_in_prefix_with_report(
        inventory_decoded,
        GLYPH_TIM_OFFSET,
        SUFFIX_GLYPH_CELL,
        suffix_pixels,
    )?;
    ensure!(
        write.changed_byte_count > 0,
        "page-indicator suffix changed no bytes"
    );
    let output_pixels =
        read_indexed_cell_in_prefix(inventory_decoded, GLYPH_TIM_OFFSET, SUFFIX_GLYPH_CELL)?;
    ensure!(
        sha256_bytes(&output_pixels) == expected_indexed_sha256
            && sha256_bytes(&pack_indexed_pixels(&output_pixels)?) == expected_packed_sha256,
        "page-indicator suffix readback differs from its planned font raster"
    );
    Ok(SuffixGlyphApplication {
        allowed_ranges: write.allowed_ranges,
        changed_byte_count: write.changed_byte_count,
    })
}

fn rasterize_suffix(
    font: &BonusPageIndicatorFontSource,
    text: &str,
) -> Result<RasterizedIndexedText> {
    let rasterizer = IndexedTextRasterizer::load(&font.path)?;
    crate::bonus_inventory_source::rasterize_message_glyph(&rasterizer, font, text)
}

fn validate_font(font: &BonusPageIndicatorFontSource) -> Result<()> {
    ensure!(
        font.font_px.is_finite()
            && font.font_px > 0.0
            && font.tracking_px.is_finite()
            && (-19..=19).contains(&font.vertical_shift_px),
        "bonus page-indicator font settings are invalid"
    );
    Ok(())
}

fn prepare_outputs(output_dir: &Path, force: bool) -> Result<()> {
    std::fs::create_dir_all(output_dir)?;
    for file in [
        BONUS_PAGE_INDICATOR_OVERLAY_OUTPUT_FILE,
        BONUS_PAGE_INDICATOR_BUILD_MANIFEST_FILE,
    ] {
        let path = output_dir.join(file);
        if path.exists() && !force {
            bail!("bonus page-indicator output exists; pass --force to replace it");
        }
    }
    Ok(())
}

fn write_outputs(
    output_dir: &Path,
    overlay: &[u8],
    report: &BonusPageIndicatorBuildReport,
) -> Result<String> {
    let overlay_path = output_dir.join(BONUS_PAGE_INDICATOR_OVERLAY_OUTPUT_FILE);
    std::fs::write(&overlay_path, overlay)
        .with_context(|| format!("failed to write {}", overlay_path.display()))?;
    let manifest_path = output_dir.join(BONUS_PAGE_INDICATOR_BUILD_MANIFEST_FILE);
    let build_manifest_sha256 = write_pretty_json_and_hash(&manifest_path, report, true)?;
    ensure!(
        sha256_file(&overlay_path)? == report.output_overlay_sha256,
        "page-indicator overlay changed while writing"
    );
    Ok(build_manifest_sha256)
}
