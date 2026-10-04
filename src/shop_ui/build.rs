use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Result, bail, ensure};

use crate::bonus_shop_source::{BonusShopSource, load_source};
use crate::compression::decompress;
use crate::decoded_record_write_plan::DecodedDataClaim;
use crate::development_build_spec::SizedFontSource;
use crate::font::{HorizontalTextAlignment, IndexedTextRasterizers};
use crate::menu_compression::compress_menu_with_source_limits;
use crate::pipeline::{sha256_bytes, sha256_file, write_pretty_json_and_hash};
use crate::tim::{cells_overlap, write_indexed_cell_in_prefix_with_report};

use super::assets::{load_assets, parse_hex_offset};
use super::model::{
    ReleaseStatus, ShopUiBuild, ShopUiBuildConfig, ShopUiBuildReport, ShopUiFontBuild,
    ShopUiFontRole, ShopUiUnitBuild,
};
use super::source::{SHOP_UI_PATH, SOURCE_DECODED_SHA256, SOURCE_STORED_SHA256};

pub const SHOP_UI_OUTPUT_FILE: &str = "shop-ui-koubai.tiz";
pub const SHOP_UI_BUILD_MANIFEST_FILE: &str = "shop-ui-build.json";
const TRANSPARENT_INDEX: u8 = 0;
const FIRST_INK_INDEX: u8 = 1;
const LAST_INK_INDEX: u8 = 15;

pub fn build_shop_ui(config: &ShopUiBuildConfig) -> Result<ShopUiBuild> {
    let source = load_source(&config.cue)?;
    build_shop_ui_from_source(config, &source)
}

pub(crate) fn build_shop_ui_from_source(
    config: &ShopUiBuildConfig,
    source: &BonusShopSource,
) -> Result<ShopUiBuild> {
    prepare_outputs(&config.output_dir, config.force)?;
    let assets = load_assets(&config.assets, source)?;
    for (name, style) in [
        ("heading", &config.fonts.heading),
        ("current points", &config.fonts.current_points),
    ] {
        ensure!(
            style.font_px.is_finite() && style.font_px > 0.0,
            "shop UI {name} font size must be finite and positive"
        );
    }
    ensure!(
        config.fonts.heading_tracking_px.is_finite()
            && config.fonts.current_points_tracking_px.is_finite(),
        "shop UI text tracking must be finite"
    );

    let mut patched = source.shop_ui_decoded.clone();
    let mut allowed_ranges = Vec::new();
    let mut owned_cells = Vec::new();
    let mut font_builds = BTreeMap::new();
    let mut rasterizers = IndexedTextRasterizers::default();
    let mut unit_builds = Vec::with_capacity(assets.units.len());
    let mut approved_count = 0usize;
    for unit in assets.units {
        let (style, tracking_px) = font_for_role(&config.fonts, unit.font_role);
        let raster = rasterizers
            .for_font(&style.path)?
            .rasterize_shifted_with_coverage_ramp(
                &unit.korean_text,
                unit.cell.width,
                unit.cell.height,
                style.font_px,
                tracking_px,
                0,
                TRANSPARENT_INDEX,
                FIRST_INK_INDEX,
                LAST_INK_INDEX,
                HorizontalTextAlignment::Left,
            )?;
        let write = write_indexed_cell_in_prefix_with_report(
            &mut patched,
            parse_hex_offset(&unit.tim_offset)?,
            unit.cell,
            &raster.pixels,
        )?;
        let changed_decoded_byte_count = write.changed_byte_count;
        ensure!(
            changed_decoded_byte_count > 0,
            "shop UI unit {} changed no bytes",
            unit.id
        );
        allowed_ranges.extend(write.allowed_ranges);
        owned_cells.push(unit.cell);
        font_builds
            .entry(unit.font_role)
            .or_insert(ShopUiFontBuild {
                role: unit.font_role,
                font_name: raster.font_name,
                font_sha256: raster.font_sha256,
                font_px: style.font_px,
                tracking_px,
            });
        if unit.release_status == ReleaseStatus::Approved {
            approved_count += 1;
        }
        unit_builds.push(ShopUiUnitBuild {
            id: unit.id,
            source_text: unit.source_text,
            korean_text: unit.korean_text,
            font_role: unit.font_role,
            tim_offset: unit.tim_offset,
            cell: unit.cell,
            source_region_sha256: unit.source_region_sha256,
            development_status: unit.development_status,
            release_status: unit.release_status,
            measured_advance_px: raster.measured_advance_px,
            ink_bounds: raster.ink_bounds,
            changed_decoded_byte_count,
        });
    }
    ensure!(
        owned_cells
            .windows(2)
            .all(|pair| !cells_overlap(pair[0], pair[1])),
        "shop UI cells overlap"
    );
    ensure!(
        source
            .shop_ui_decoded
            .iter()
            .zip(&patched)
            .enumerate()
            .filter_map(|(offset, (before, after))| (before != after).then_some(offset))
            .all(|offset| allowed_ranges
                .iter()
                .any(|[start, end]| *start <= offset && offset < *end)),
        "shop UI build changed bytes outside owned cells"
    );
    let decoded_write_claims = DecodedDataClaim::from_effective_ranges(
        "shop:fixed-ui",
        "render the source-bound shop fixed UI",
        &source.shop_ui_decoded,
        &patched,
        allowed_ranges,
    )?;

    let (reencoded, source_compression, rebuilt_compression) =
        compress_menu_with_source_limits(&patched, &source.shop_ui_stored)?;
    ensure!(
        decompress(&reencoded, false)? == patched,
        "shop UI compression roundtrip changed decoded bytes"
    );
    let unpadded_stored_size = reencoded.len();
    let mut stored = reencoded;
    stored.resize(source.shop_ui_stored.len(), 0);
    ensure!(
        decompress(&stored, true)? == patched,
        "padded shop UI output changed decoded bytes"
    );
    let release_candidate_input_eligible = approved_count == unit_builds.len();
    let report = ShopUiBuildReport {
        kind: "Justice Gakuen 2 Korean shop fixed-UI build".to_string(),
        source_bin_sha256: source.source_bin_sha256.clone(),
        source_path: SHOP_UI_PATH.to_string(),
        source_stored_sha256: SOURCE_STORED_SHA256.to_string(),
        source_decoded_sha256: SOURCE_DECODED_SHA256.to_string(),
        patched_stored_sha256: sha256_bytes(&stored),
        patched_decoded_sha256: sha256_bytes(&patched),
        build_spec_sha256: config.build_spec_sha256.clone(),
        translation_manifest_sha256: assets.manifest_sha256,
        unit_count: unit_builds.len(),
        release_approved_unit_count: approved_count,
        source_regions_match: true,
        cells_are_unique_and_non_overlapping: true,
        changed_bytes_confined_to_owned_cells: true,
        development_input_available: unit_builds.len() == 2,
        release_candidate_input_eligible,
        unpadded_stored_size,
        source_record_size: source.shop_ui_stored.len(),
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
    Ok(ShopUiBuild {
        stored,
        decoded: patched,
        decoded_write_claims,
        build_manifest_sha256,
        report,
    })
}

fn font_for_role(
    fonts: &super::model::ShopUiFontSources,
    role: ShopUiFontRole,
) -> (&SizedFontSource, f32) {
    match role {
        ShopUiFontRole::Heading => (&fonts.heading, fonts.heading_tracking_px),
        ShopUiFontRole::CurrentPoints => (&fonts.current_points, fonts.current_points_tracking_px),
    }
}

fn prepare_outputs(output_dir: &Path, force: bool) -> Result<()> {
    std::fs::create_dir_all(output_dir)?;
    for file in [SHOP_UI_OUTPUT_FILE, SHOP_UI_BUILD_MANIFEST_FILE] {
        let path = output_dir.join(file);
        if path.exists() && !force {
            bail!("shop UI output exists; pass --force to replace it");
        }
    }
    Ok(())
}

fn write_outputs(output_dir: &Path, stored: &[u8], report: &ShopUiBuildReport) -> Result<String> {
    let output = output_dir.join(SHOP_UI_OUTPUT_FILE);
    std::fs::write(&output, stored)?;
    let build_manifest_sha256 =
        write_pretty_json_and_hash(&output_dir.join(SHOP_UI_BUILD_MANIFEST_FILE), report, true)?;
    ensure!(
        sha256_file(&output)? == report.patched_stored_sha256,
        "shop UI stored output changed while writing"
    );
    Ok(build_manifest_sha256)
}
