use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result, bail, ensure};

use crate::compression::decompress;
use crate::decoded_record_write_plan::{DecodedDataClaim, DecodedRecordWritePlan};
use crate::font::IndexedTextRasterizers;
use crate::menu_compression::compress_menu_with_source_limits;
use crate::pipeline::{sha256_bytes, sha256_file, write_pretty_json_and_hash};
use crate::source_disc::SupportedSourceDisc;
use crate::tim::{
    parse_8bpp_prefix, read_8bpp_indexed_cell_in_prefix, read_8bpp_palette_words_in_prefix,
    write_8bpp_indexed_cell_in_prefix_with_report, write_indexed_cell_in_prefix_with_report,
};

use super::assets::{load_assets, parse_hex_offset};
use super::heading::compose_heading_cell;
use super::model::{
    BonusMenuBuild, BonusMenuBuildConfig, BonusMenuBuildReport, BonusMenuFontBuild,
    BonusMenuFontRole, BonusMenuUnitBuild, DevelopmentStatus, ReleaseStatus,
};
use super::ownership::{OwnedCell, cells_are_unique_and_non_overlapping};
use super::raster::{TRANSPARENT_INDEX, font_for_role, rasterize_unit_text};
use super::source::{
    BONUS_MENU_PATH, SOURCE_DECODED_SHA256, SOURCE_STORED_SHA256, load_source_from_disc,
};

pub const BONUS_MENU_OUTPUT_FILE: &str = "bonus-main-koubai0.tiz";
pub const BONUS_MENU_BUILD_MANIFEST_FILE: &str = "bonus-main-build.json";

pub fn build_bonus_menu(config: &BonusMenuBuildConfig) -> Result<BonusMenuBuild> {
    let source = SupportedSourceDisc::open(&config.cue)?;
    build_bonus_menu_from_source(config, &source)
}

pub(crate) fn build_bonus_menu_from_source(
    config: &BonusMenuBuildConfig,
    source_disc: &SupportedSourceDisc,
) -> Result<BonusMenuBuild> {
    prepare_outputs(&config.output_dir, config.force)?;
    let source = load_source_from_disc(source_disc)?;
    let assets = load_assets(&config.assets, &source)?;
    ensure!(
        config.fonts.heading.font_px.is_finite() && config.fonts.heading.font_px > 0.0,
        "bonus heading font size must be finite and positive"
    );
    ensure!(
        config.fonts.entry.font_px.is_finite() && config.fonts.entry.font_px > 0.0,
        "bonus entry font size must be finite and positive"
    );
    ensure!(
        config.fonts.compact_entry.font_px.is_finite() && config.fonts.compact_entry.font_px > 0.0,
        "bonus compact-entry font size must be finite and positive"
    );

    let heading_tim = parse_8bpp_prefix(&source.decoded)?;
    let mut patched_decoded = source.decoded.clone();
    let mut decoded_write_claims = Vec::new();
    let mut unit_builds = Vec::with_capacity(assets.units.len());
    let mut font_builds = BTreeMap::new();
    let mut rasterizers = IndexedTextRasterizers::default();
    let mut owned_cells = Vec::new();
    let mut heading_background_preserved_outside_cleanup = false;
    let mut authored_count = 0usize;
    let mut suppressed_count = 0usize;
    let mut untranslated_count = 0usize;
    let mut approved_count = 0usize;

    for unit in assets.units {
        let mut measured_advance_px = None;
        let mut ink_bounds = None;
        let mut changed_decoded_byte_count = 0usize;
        match unit.development_status {
            DevelopmentStatus::Untranslated => untranslated_count += 1,
            DevelopmentStatus::Authored => {
                authored_count += 1;
                let style = font_for_role(&config.fonts, unit.font_role);
                let text = unit
                    .korean_text
                    .as_deref()
                    .context("authored bonus-main unit lost Korean text")?;
                let raster = rasterize_unit_text(
                    rasterizers.for_font(&style.path)?,
                    style,
                    unit.font_role,
                    unit.cell,
                    text,
                )?;
                let tim_offset = parse_hex_offset(&unit.tim_offset)?;
                let write = match unit.bits_per_pixel {
                    4 => write_indexed_cell_in_prefix_with_report(
                        &mut patched_decoded,
                        tim_offset,
                        unit.cell,
                        &raster.pixels,
                    )?,
                    8 => {
                        ensure!(
                            unit.font_role == BonusMenuFontRole::Heading && tim_offset == 0,
                            "authored 8-bpp bonus-main unit is not the source-bound heading"
                        );
                        let source_pixels = read_8bpp_indexed_cell_in_prefix(
                            &source.decoded,
                            tim_offset,
                            unit.cell,
                        )?;
                        let palette =
                            read_8bpp_palette_words_in_prefix(&source.decoded, tim_offset, 0)?;
                        let composition = compose_heading_cell(&source_pixels, &palette, &raster)?;
                        heading_background_preserved_outside_cleanup =
                            composition.source_background_preserved_outside_cleanup;
                        write_8bpp_indexed_cell_in_prefix_with_report(
                            &mut patched_decoded,
                            tim_offset,
                            unit.cell,
                            &composition.pixels,
                        )?
                    }
                    bits => bail!("unsupported authored bonus-main TIM depth {bits}"),
                };
                changed_decoded_byte_count = write.changed_byte_count;
                ensure!(
                    changed_decoded_byte_count > 0,
                    "bonus-main unit {} changed no bytes",
                    unit.id
                );
                extend_unit_write_claims(
                    &mut decoded_write_claims,
                    &unit.id,
                    &source.decoded,
                    &patched_decoded,
                    write.allowed_ranges,
                )?;
                owned_cells.push(OwnedCell {
                    tim_offset,
                    bits_per_pixel: unit.bits_per_pixel,
                    cell: unit.cell,
                });
                measured_advance_px = Some(raster.measured_advance_px);
                ink_bounds = Some(raster.ink_bounds);
                font_builds
                    .entry(unit.font_role)
                    .or_insert(BonusMenuFontBuild {
                        role: unit.font_role,
                        font_name: raster.font_name,
                        font_sha256: raster.font_sha256,
                        font_px: style.font_px,
                    });
            }
            DevelopmentStatus::SuppressedDuplicate => {
                suppressed_count += 1;
                ensure!(
                    unit.bits_per_pixel == 4,
                    "suppressed bonus-main duplicate {} is not in the entry atlas",
                    unit.id
                );
                let write = write_indexed_cell_in_prefix_with_report(
                    &mut patched_decoded,
                    parse_hex_offset(&unit.tim_offset)?,
                    unit.cell,
                    &vec![TRANSPARENT_INDEX; unit.cell.width * unit.cell.height],
                )?;
                changed_decoded_byte_count = write.changed_byte_count;
                ensure!(
                    changed_decoded_byte_count > 0,
                    "suppressed bonus-main duplicate {} changed no bytes",
                    unit.id
                );
                extend_unit_write_claims(
                    &mut decoded_write_claims,
                    &unit.id,
                    &source.decoded,
                    &patched_decoded,
                    write.allowed_ranges,
                )?;
                owned_cells.push(OwnedCell {
                    tim_offset: parse_hex_offset(&unit.tim_offset)?,
                    bits_per_pixel: unit.bits_per_pixel,
                    cell: unit.cell,
                });
            }
        }
        if unit.release_status == ReleaseStatus::Approved {
            approved_count += 1;
        }
        unit_builds.push(BonusMenuUnitBuild {
            id: unit.id,
            source_text: unit.source_text,
            korean_text: unit.korean_text,
            font_role: unit.font_role,
            bits_per_pixel: unit.bits_per_pixel,
            tim_offset: unit.tim_offset,
            cell: unit.cell,
            source_region_sha256: unit.source_region_sha256,
            development_status: unit.development_status,
            release_status: unit.release_status,
            measured_advance_px,
            ink_bounds,
            changed_decoded_byte_count,
        });
    }

    ensure!(
        authored_count == 4 && suppressed_count == 1 && untranslated_count == 0,
        "bonus-main development scope changed"
    );
    ensure!(
        cells_are_unique_and_non_overlapping(&owned_cells),
        "bonus-main owned cells overlap"
    );
    let source_decoded_sha256 = sha256_bytes(&source.decoded);
    let mut write_plan =
        DecodedRecordWritePlan::new(BONUS_MENU_PATH, &source.decoded, &source_decoded_sha256)?;
    write_plan.register_data_candidate(
        "bonus menu units",
        &source_decoded_sha256,
        &patched_decoded,
        &decoded_write_claims,
    )?;
    let planned_decoded = write_plan.apply(None)?;
    ensure!(
        planned_decoded == patched_decoded,
        "bonus-main decoded plan omitted an authored unit"
    );
    let patched_decoded = planned_decoded;
    let source_palette_unchanged =
        source.decoded[..heading_tim.pixel_offset] == patched_decoded[..heading_tim.pixel_offset];
    ensure!(
        source_palette_unchanged,
        "bonus-main build changed the source 8-bpp palette or TIM header"
    );
    ensure!(
        heading_background_preserved_outside_cleanup,
        "bonus-main heading did not preserve its protected source background"
    );

    let (reencoded, source_compression, rebuilt_compression) =
        compress_menu_with_source_limits(&patched_decoded, &source.stored)?;
    ensure!(
        decompress(&reencoded, false)? == patched_decoded,
        "KOUBAI0 compression roundtrip changed decoded bytes"
    );
    ensure!(
        reencoded.len() <= source.stored.len(),
        "rebuilt KOUBAI0.TIZ exceeds its source record"
    );
    let unpadded_stored_size = reencoded.len();
    let mut stored = reencoded;
    stored.resize(source.stored.len(), 0);
    ensure!(
        decompress(&stored, true)? == patched_decoded,
        "padded KOUBAI0.TIZ changed decoded bytes"
    );

    let complete_surface_localized = untranslated_count == 0;
    let report = BonusMenuBuildReport {
        kind: "Justice Gakuen 2 Korean bonus main-menu build".to_string(),
        source_bin_sha256: source.source_bin_sha256,
        source_path: BONUS_MENU_PATH.to_string(),
        source_stored_sha256: SOURCE_STORED_SHA256.to_string(),
        source_decoded_sha256: SOURCE_DECODED_SHA256.to_string(),
        patched_stored_sha256: sha256_bytes(&stored),
        patched_decoded_sha256: sha256_bytes(&patched_decoded),
        build_spec_sha256: config.build_spec_sha256.clone(),
        translation_manifest_sha256: assets.manifest_sha256,
        source_unit_count: unit_builds.len(),
        authored_unit_count: authored_count,
        suppressed_duplicate_unit_count: suppressed_count,
        untranslated_unit_count: untranslated_count,
        release_approved_unit_count: approved_count,
        source_regions_match: true,
        authored_cells_are_unique_and_non_overlapping: true,
        untranslated_regions_unchanged: true,
        source_palette_unchanged,
        heading_background_preserved_outside_cleanup,
        changed_bytes_confined_to_owned_cells: true,
        complete_surface_localized,
        development_input_available: authored_count > 0 && suppressed_count == 1,
        release_candidate_input_eligible: complete_surface_localized
            && unit_builds.iter().all(|unit| {
                matches!(
                    unit.release_status,
                    ReleaseStatus::Approved | ReleaseStatus::NotApplicable
                )
            }),
        unpadded_stored_size,
        source_record_size: source.stored.len(),
        source_compression_maximum_match_words: source_compression.maximum_match_words,
        source_compression_maximum_control_block_output_words: source_compression
            .maximum_control_block_output_words,
        rebuilt_compression_maximum_match_words: rebuilt_compression.maximum_match_words,
        rebuilt_compression_maximum_control_block_output_words: rebuilt_compression
            .maximum_control_block_output_words,
        fonts: font_builds.into_values().collect(),
        units: unit_builds,
    };
    std::fs::write(config.output_dir.join(BONUS_MENU_OUTPUT_FILE), &stored)?;
    let build_manifest_sha256 = write_pretty_json_and_hash(
        &config.output_dir.join(BONUS_MENU_BUILD_MANIFEST_FILE),
        &report,
        true,
    )?;
    ensure!(
        sha256_file(&config.output_dir.join(BONUS_MENU_OUTPUT_FILE))?
            == report.patched_stored_sha256,
        "bonus-main stored output changed while writing"
    );
    Ok(BonusMenuBuild {
        stored,
        decoded: patched_decoded,
        build_manifest_sha256,
        report,
    })
}

fn extend_unit_write_claims(
    claims: &mut Vec<DecodedDataClaim>,
    unit_id: &str,
    source: &[u8],
    candidate: &[u8],
    ranges: Vec<[usize; 2]>,
) -> Result<()> {
    let unit_claims = DecodedDataClaim::from_effective_ranges(
        &format!("bonus-menu:{unit_id}"),
        &format!("render bonus menu unit {unit_id}"),
        source,
        candidate,
        ranges,
    )?;
    claims.extend(unit_claims);
    Ok(())
}

fn prepare_outputs(output_dir: &Path, force: bool) -> Result<()> {
    std::fs::create_dir_all(output_dir)?;
    for file in [BONUS_MENU_OUTPUT_FILE, BONUS_MENU_BUILD_MANIFEST_FILE] {
        let path = output_dir.join(file);
        if path.exists() && !force {
            bail!("bonus-main output exists; pass --force to replace it");
        }
        if path.exists() {
            std::fs::remove_file(&path)
                .with_context(|| format!("failed to remove {}", path.display()))?;
        }
    }
    Ok(())
}
