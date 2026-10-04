use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result, bail, ensure};

use crate::bonus_inventory_source::BonusInventorySource;
use crate::compression::decompress;
use crate::decoded_record_write_plan::DecodedDataClaim;
use crate::font::IndexedTextRasterizers;
use crate::menu_compression::compress_menu_with_source_limits;
use crate::pipeline::{sha256_bytes, sha256_file, write_pretty_json_and_hash};
use crate::source_disc::SupportedSourceDisc;
use crate::tim::{read_indexed_cell_in_prefix, write_indexed_cell_in_prefix_with_report};

use super::assets::load_assets;
use super::model::{
    BonusInventoryBuild, BonusInventoryBuildConfig, BonusInventoryBuildReport,
    BonusInventoryFontBuild, BonusInventoryFontRole, BonusInventoryOccurrenceBuild,
    BonusInventoryUnitBuild, DevelopmentStatus, ReleaseStatus,
};
use super::raster::{font_for_role, overlay_text_preserving_background, rasterize_text};
use super::source::{
    BONUS_INVENTORY_PATH, FIXED_UI_TIM_OFFSET, SOURCE_DECODED_SHA256, SOURCE_STORED_SHA256,
    load_source,
};

pub const BONUS_INVENTORY_OUTPUT_FILE: &str = "bonus-inventory-koubai1.tiz";
pub const BONUS_INVENTORY_BUILD_MANIFEST_FILE: &str = "bonus-inventory-build.json";

pub fn build_bonus_inventory(config: &BonusInventoryBuildConfig) -> Result<BonusInventoryBuild> {
    let source = SupportedSourceDisc::open(&config.cue)?;
    build_bonus_inventory_from_source(config, &source)
}

pub(crate) fn build_bonus_inventory_from_source(
    config: &BonusInventoryBuildConfig,
    source_disc: &SupportedSourceDisc,
) -> Result<BonusInventoryBuild> {
    let source = load_source(source_disc)?;
    build_bonus_inventory_from_inventory_source(config, &source)
}

pub(crate) fn build_bonus_inventory_from_inventory_source(
    config: &BonusInventoryBuildConfig,
    source: &BonusInventorySource,
) -> Result<BonusInventoryBuild> {
    prepare_outputs(&config.output_dir, config.force)?;
    let assets = load_assets(&config.assets, source)?;
    validate_fonts(config)?;

    let mut patched = source.inventory_decoded.clone();
    let mut allowed_ranges = Vec::new();
    let mut font_builds = BTreeMap::new();
    let mut rasterizers = IndexedTextRasterizers::default();
    let mut unit_builds = Vec::with_capacity(assets.units.len());
    let mut authored_unit_count = 0usize;
    let mut untranslated_unit_count = 0usize;
    let mut authored_occurrence_count = 0usize;
    let mut approved_count = 0usize;
    for unit in assets.units {
        let text = unit.korean_text.as_deref();
        let mut occurrence_builds = Vec::with_capacity(unit.occurrences.len());
        match unit.development_status {
            DevelopmentStatus::Authored => authored_unit_count += 1,
            DevelopmentStatus::Untranslated => untranslated_unit_count += 1,
        }
        if unit.release_status == ReleaseStatus::Approved {
            approved_count += 1;
        }
        for occurrence in unit.occurrences {
            let before_pixels =
                read_indexed_cell_in_prefix(&patched, FIXED_UI_TIM_OFFSET, occurrence.cell)?;
            let (measured_advance_px, ink_bounds, changed_decoded_byte_count) =
                if unit.development_status == DevelopmentStatus::Authored {
                    authored_occurrence_count += 1;
                    let role = occurrence
                        .font_role
                        .context("authored bonus-inventory occurrence lost its font role")?;
                    let style = font_for_role(&config.fonts, role);
                    let raster = rasterize_text(
                        rasterizers.for_font(&style.path)?,
                        style,
                        role,
                        text.context("authored bonus-inventory unit lost Korean text")?,
                        occurrence.cell.width,
                        occurrence.cell.height,
                    )?;
                    let output_pixels = if role == BonusInventoryFontRole::ViewerCardPlaceholder {
                        overlay_text_preserving_background(
                            &before_pixels,
                            &raster.pixels,
                            occurrence.cell.width,
                            occurrence.cell.height,
                            8,
                        )?
                    } else {
                        raster.pixels.clone()
                    };
                    let write = write_indexed_cell_in_prefix_with_report(
                        &mut patched,
                        FIXED_UI_TIM_OFFSET,
                        occurrence.cell,
                        &output_pixels,
                    )?;
                    let changed = write.changed_byte_count;
                    ensure!(
                        changed > 0,
                        "bonus-inventory occurrence {}:{} changed no bytes",
                        unit.id,
                        occurrence.id
                    );
                    allowed_ranges.extend(write.allowed_ranges);
                    font_builds.entry(role).or_insert(BonusInventoryFontBuild {
                        role,
                        font_name: raster.font_name,
                        font_sha256: raster.font_sha256,
                        font_px: style.font_px,
                        tracking_px: style.tracking_px,
                        vertical_shift_px: style.vertical_shift_px,
                    });
                    (
                        Some(raster.measured_advance_px),
                        Some(raster.ink_bounds),
                        changed,
                    )
                } else {
                    (None, None, 0)
                };
            let output_pixels =
                read_indexed_cell_in_prefix(&patched, FIXED_UI_TIM_OFFSET, occurrence.cell)?;
            if unit.development_status == DevelopmentStatus::Untranslated {
                ensure!(
                    output_pixels == before_pixels,
                    "untranslated bonus-inventory occurrence changed"
                );
            }
            occurrence_builds.push(BonusInventoryOccurrenceBuild {
                id: occurrence.id,
                font_role: occurrence.font_role,
                cell: occurrence.cell,
                source_indexed_pixel_sha256: occurrence.source_indexed_pixel_sha256,
                output_indexed_pixel_sha256: sha256_bytes(&output_pixels),
                measured_advance_px,
                ink_bounds,
                changed_decoded_byte_count,
            });
        }
        unit_builds.push(BonusInventoryUnitBuild {
            id: unit.id,
            source_text: unit.source_text,
            korean_text: unit.korean_text,
            development_status: unit.development_status,
            release_status: unit.release_status,
            occurrences: occurrence_builds,
        });
    }
    let (card_back, card_back_ranges) = super::card_back::apply(
        &config.assets,
        &source.inventory_decoded,
        &mut patched,
        &config.fonts.item_label,
        &mut rasterizers,
    )?;
    allowed_ranges.extend(card_back_ranges);
    let (item_labels, item_label_report, item_ranges) =
        super::item_labels::build(config, source, &mut patched)?;
    allowed_ranges.extend(item_ranges);
    let (device_text, device_text_report, device_ranges) =
        super::device_text::build(config, source, &mut patched)?;
    allowed_ranges.extend(device_ranges);
    ensure_changes_are_owned(&source.inventory_decoded, &patched, &allowed_ranges)?;
    let decoded_write_claims = DecodedDataClaim::from_effective_ranges(
        "bonus-inventory:fixed-ui",
        "render the source-bound bonus-inventory fixed UI",
        &source.inventory_decoded,
        &patched,
        allowed_ranges,
    )?;

    let (reencoded, source_compression, rebuilt_compression) =
        compress_menu_with_source_limits(&patched, &source.inventory_stored)?;
    ensure!(
        decompress(&reencoded, false)? == patched,
        "KOUBAI1 compression roundtrip changed decoded bytes"
    );
    ensure!(
        reencoded.len() <= source.inventory_stored.len(),
        "rebuilt KOUBAI1.TIZ exceeds its source record"
    );
    let unpadded_stored_size = reencoded.len();
    let mut stored = reencoded;
    stored.resize(source.inventory_stored.len(), 0);
    ensure!(
        decompress(&stored, true)? == patched,
        "padded KOUBAI1.TIZ changed decoded bytes"
    );
    let complete_fixed_ui_localized = untranslated_unit_count == 0;
    let release_candidate_input_eligible = complete_fixed_ui_localized
        && approved_count == unit_builds.len()
        && item_label_report.release_status == "approved"
        && card_back["release_status"] == "approved";
    let report = BonusInventoryBuildReport {
        card_back,
        device_text: device_text_report,
        item_labels: item_label_report,
        kind: "Justice Gakuen 2 Korean bonus-inventory UI and item-label build".to_string(),
        source_bin_sha256: source.source_bin_sha256.clone(),
        source_path: BONUS_INVENTORY_PATH.to_string(),
        source_stored_sha256: SOURCE_STORED_SHA256.to_string(),
        source_decoded_sha256: SOURCE_DECODED_SHA256.to_string(),
        patched_stored_sha256: sha256_bytes(&stored),
        patched_decoded_sha256: sha256_bytes(&patched),
        build_spec_sha256: config.build_spec_sha256.clone(),
        translation_manifest_sha256: assets.manifest_sha256,
        fixed_unit_count: unit_builds.len(),
        authored_unit_count,
        untranslated_unit_count,
        occurrence_count: unit_builds.iter().map(|unit| unit.occurrences.len()).sum(),
        authored_occurrence_count,
        release_approved_unit_count: approved_count,
        source_regions_match: true,
        occurrence_cells_are_unique_and_non_overlapping: true,
        untranslated_regions_unchanged: true,
        changes_confined_to_owned_cells: true,
        complete_fixed_ui_localized,
        development_input_available: authored_unit_count > 0,
        release_candidate_input_eligible,
        unpadded_stored_size,
        source_record_size: source.inventory_stored.len(),
        source_compression_maximum_match_words: source_compression.maximum_match_words,
        source_compression_maximum_control_block_output_words: source_compression
            .maximum_control_block_output_words,
        rebuilt_compression_maximum_match_words: rebuilt_compression.maximum_match_words,
        rebuilt_compression_maximum_control_block_output_words: rebuilt_compression
            .maximum_control_block_output_words,
        fonts: font_builds.into_values().collect(),
        units: unit_builds,
    };
    let build_manifest_sha256 = write_outputs(&config.output_dir, &stored, &report)?;
    Ok(BonusInventoryBuild {
        device_text,
        item_labels,
        stored,
        decoded: patched,
        decoded_write_claims,
        build_manifest_sha256,
        report,
    })
}

fn validate_fonts(config: &BonusInventoryBuildConfig) -> Result<()> {
    for style in [
        &config.fonts.item_label,
        &config.fonts.device_text,
        &config.fonts.title,
        &config.fonts.compact_label,
        &config.fonts.large_label,
        &config.fonts.compact_action,
        &config.fonts.large_action,
        &config.fonts.help,
        &config.fonts.viewer_navigation,
        &config.fonts.viewer_card_placeholder,
    ] {
        ensure!(
            style.font_px.is_finite() && style.font_px > 0.0 && style.tracking_px.is_finite(),
            "bonus-inventory font settings must be finite and positive"
        );
    }
    Ok(())
}

fn ensure_changes_are_owned(
    source: &[u8],
    patched: &[u8],
    allowed_ranges: &[[usize; 2]],
) -> Result<()> {
    let changed = source
        .iter()
        .zip(patched)
        .enumerate()
        .filter_map(|(offset, (before, after))| (before != after).then_some(offset))
        .collect::<Vec<_>>();
    ensure!(
        !changed.is_empty()
            && changed.iter().all(|offset| allowed_ranges
                .iter()
                .any(|[start, end]| *start <= *offset && *offset < *end)),
        "bonus-inventory build changed bytes outside authored cells"
    );
    Ok(())
}

fn prepare_outputs(output_dir: &Path, force: bool) -> Result<()> {
    std::fs::create_dir_all(output_dir)?;
    for file in [
        BONUS_INVENTORY_OUTPUT_FILE,
        BONUS_INVENTORY_BUILD_MANIFEST_FILE,
    ] {
        let path = output_dir.join(file);
        if path.exists() && !force {
            bail!("bonus-inventory output exists; pass --force to replace it");
        }
    }
    Ok(())
}

fn write_outputs(
    output_dir: &Path,
    stored: &[u8],
    report: &BonusInventoryBuildReport,
) -> Result<String> {
    let output = output_dir.join(BONUS_INVENTORY_OUTPUT_FILE);
    std::fs::write(&output, stored)?;
    let build_manifest_sha256 = write_pretty_json_and_hash(
        &output_dir.join(BONUS_INVENTORY_BUILD_MANIFEST_FILE),
        report,
        true,
    )?;
    ensure!(
        sha256_file(&output)? == report.patched_stored_sha256,
        "bonus-inventory stored output changed while writing"
    );
    Ok(build_manifest_sha256)
}
