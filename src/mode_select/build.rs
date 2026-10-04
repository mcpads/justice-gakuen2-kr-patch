use std::path::Path;

use anyhow::{Context, Result, bail, ensure};

use crate::compression::decompress;
use crate::decoded_record_write_plan::DecodedDataClaim;
use crate::font::IndexedTextRasterizers;
use crate::menu_compression::compress_menu_with_source_limits;
use crate::pipeline::{sha256_bytes, write_pretty_json_and_hash};
use crate::source_disc::SupportedSourceDisc;
use crate::tim::{
    Cell, install_indexed_glyph_in_prefix, write_indexed_cell_without_clut_in_prefix,
};
use crate::write_scope::changed_ranges_are_within;
pub(super) use crate::write_scope::difference_ranges;

use super::assets::load_assets;
use super::model::{
    DESCRIPTION_ALIGNMENT, ModeSelectBuildConfig, ModeSelectBuildReport, ModeSelectFixedLabelBuild,
    ModeSelectFontBuild, ModeSelectModeBuild, ModeSelectRecordBuild, ReleaseStatus,
    TITLE_ALIGNMENT,
};
use super::source::{MENU_PATH, load_source_from_disc};

const OUTPUT_FILE: &str = "mode-select-menu.biz";
const OUTPUT_MANIFEST: &str = "mode-select-build.json";
const TITLE_CLEAR_INDEX: u8 = 0;
const TITLE_OUTLINE_INDEX: u8 = 3;
const TITLE_FILL_INDEX: u8 = 14;
const DESCRIPTION_CLEAR_INDEX: u8 = 0;
const DESCRIPTION_FILL_INDEX: u8 = 1;
const DESCRIPTION_HORIZONTAL_PADDING: usize = 4;

pub fn build_mode_select_assets(config: &ModeSelectBuildConfig) -> Result<ModeSelectBuildReport> {
    let source = SupportedSourceDisc::open(&config.cue)?;
    Ok(build_mode_select_assets_from_source(config, &source)?.report)
}

pub(crate) fn build_mode_select_assets_from_source(
    config: &ModeSelectBuildConfig,
    source_disc: &SupportedSourceDisc,
) -> Result<ModeSelectRecordBuild> {
    prepare_outputs(&config.output_dir, config.force)?;
    let source = load_source_from_disc(source_disc)?;
    let assets = load_assets(&config.assets, &source)?;
    let mut patched = source.menu_decoded.clone();
    let mut modes = Vec::with_capacity(assets.translations.len());
    let mut fixed_labels = Vec::with_capacity(assets.fixed_labels.len());
    let mut fixed_label_font = None;
    let mut list_label_font = None;
    let mut detail_title_font = None;
    let mut description_font = None;
    let mut menu_write_claims = Vec::new();
    let mut rasterizers = IndexedTextRasterizers::default();

    for translation in assets.fixed_labels {
        let rendered = rasterizers.for_font(&config.fonts.fixed_label)?.rasterize(
            &translation.korean_text,
            translation.region.cell.width,
            translation.region.cell.height,
            translation.font_px,
            0.0,
            TITLE_CLEAR_INDEX,
            Some(TITLE_OUTLINE_INDEX),
            TITLE_FILL_INDEX,
            TITLE_ALIGNMENT,
        )?;
        record_font_identity(
            &mut fixed_label_font,
            &rendered.font_name,
            &rendered.font_sha256,
            "fixed_label",
        )?;
        let install = install_indexed_glyph_in_prefix(
            &mut patched,
            translation.region.tim_offset,
            translation.region.cell,
            &rendered.pixels,
            "MODESEL fixed label",
        )?;
        menu_write_claims.extend(DecodedDataClaim::from_ranges(
            &format!("menu:mode-select:fixed:{}", translation.id),
            &format!("install MODE SELECT fixed label {}", translation.id),
            install.allowed_decoded_byte_ranges,
        ));
        fixed_labels.push(ModeSelectFixedLabelBuild {
            id: translation.id,
            source_text: translation.source_text,
            korean_text: translation.korean_text,
            development_status: translation.development_status,
            release_status: translation.release_status,
            font_px: translation.font_px,
            cell: translation.region.cell,
            measured_advance_px: rendered.measured_advance_px,
        });
    }

    for translation in assets.translations {
        let mut list_title = rasterizers.for_font(&config.fonts.list_label)?.rasterize(
            &translation.korean_title,
            translation.list_label.cell.width,
            translation.list_label.cell.height,
            translation.title_font_px,
            0.0,
            TITLE_CLEAR_INDEX,
            Some(TITLE_OUTLINE_INDEX),
            TITLE_FILL_INDEX,
            TITLE_ALIGNMENT,
        )?;
        // Yellow text area's native screen center is 88.5 (capture X=114.5).
        let selected_origin =
            super::assets::selected_title_origin_x(&source.overlay, translation.panel_index)?;
        align_mode_title_in_panel(
            &mut list_title.pixels,
            translation.list_label.cell.width,
            177 - i32::from(selected_origin) * 2,
        )?;
        record_font_identity(
            &mut list_label_font,
            &list_title.font_name,
            &list_title.font_sha256,
            "list_label",
        )?;
        let install = install_indexed_glyph_in_prefix(
            &mut patched,
            translation.list_label.tim_offset,
            translation.list_label.cell,
            &list_title.pixels,
            "MODESEL list label",
        )?;
        menu_write_claims.extend(DecodedDataClaim::from_ranges(
            &format!("menu:mode-select:list:{}", translation.id),
            &format!("install MODE SELECT list label {}", translation.id),
            install.allowed_decoded_byte_ranges,
        ));
        let mut detail_title = rasterizers
            .for_font(&config.fonts.detail_title)?
            .rasterize(
                &translation.korean_title,
                translation.detail_title.cell.width,
                translation.detail_title.cell.height,
                translation.title_font_px,
                0.0,
                TITLE_CLEAR_INDEX,
                Some(TITLE_OUTLINE_INDEX),
                TITLE_FILL_INDEX,
                TITLE_ALIGNMENT,
            )?;
        align_mode_title_in_panel(
            &mut detail_title.pixels,
            translation.detail_title.cell.width,
            95,
        )?;
        record_font_identity(
            &mut detail_title_font,
            &detail_title.font_name,
            &detail_title.font_sha256,
            "detail_title",
        )?;
        let install = install_indexed_glyph_in_prefix(
            &mut patched,
            translation.detail_title.tim_offset,
            translation.detail_title.cell,
            &detail_title.pixels,
            "MODESEL detail title",
        )?;
        menu_write_claims.extend(DecodedDataClaim::from_ranges(
            &format!("menu:mode-select:detail:{}", translation.id),
            &format!("install MODE SELECT detail title {}", translation.id),
            install.allowed_decoded_byte_ranges,
        ));

        let description_cell = translation.description.cell;
        let description_ranges = write_indexed_cell_without_clut_in_prefix(
            &mut patched,
            translation.description.tim_offset,
            description_cell,
            &vec![DESCRIPTION_CLEAR_INDEX; description_cell.width * description_cell.height],
        )?;
        menu_write_claims.extend(DecodedDataClaim::from_ranges(
            &format!("menu:mode-select:description:{}", translation.id),
            &format!("install MODE SELECT description {}", translation.id),
            description_ranges,
        ));
        let line_cells = description_line_cells(
            description_cell,
            translation.korean_description_lines.len(),
            translation.description_line_height,
        )?;
        let mut description_measured_advance_px = Vec::with_capacity(line_cells.len());
        for (line, cell) in translation.korean_description_lines.iter().zip(&line_cells) {
            let rendered = rasterizers.for_font(&config.fonts.description)?.rasterize(
                line,
                cell.width,
                cell.height,
                translation.description_font_px,
                0.0,
                DESCRIPTION_CLEAR_INDEX,
                None,
                DESCRIPTION_FILL_INDEX,
                DESCRIPTION_ALIGNMENT,
            )?;
            record_font_identity(
                &mut description_font,
                &rendered.font_name,
                &rendered.font_sha256,
                "description",
            )?;
            write_indexed_cell_without_clut_in_prefix(
                &mut patched,
                translation.description.tim_offset,
                *cell,
                &rendered.pixels,
            )?;
            description_measured_advance_px.push(rendered.measured_advance_px);
        }
        modes.push(ModeSelectModeBuild {
            mode_index: translation.mode_index,
            panel_index: translation.panel_index,
            id: translation.id,
            source_title: translation.source_title,
            korean_title: translation.korean_title,
            source_description_lines: translation.source_description_lines,
            korean_description_lines: translation.korean_description_lines,
            development_status: translation.development_status,
            release_status: translation.release_status,
            title_font_px: translation.title_font_px,
            description_font_px: translation.description_font_px,
            list_label_cell: translation.list_label.cell,
            detail_title_cell: translation.detail_title.cell,
            description_tim_offset: format!("0x{:05x}", translation.description.tim_offset),
            description_line_cells: line_cells,
            title_measured_advance_px: list_title.measured_advance_px,
            description_measured_advance_px,
        });
    }

    let images = super::images::install(
        &config.assets,
        &source.menu_decoded,
        &mut patched,
        &config.fonts.artwork,
        &config.output_dir,
        &mut menu_write_claims,
        &modes,
    )?;
    let logo = super::logo::apply(
        source_disc,
        &config.assets,
        &source.overlay,
        &source.menu_decoded,
        &mut patched,
        &mut menu_write_claims,
    )?;
    let changed_ranges = difference_ranges(&source.menu_decoded, &patched);
    let changed_count = changed_ranges.iter().map(|[start, end]| end - start).sum();
    ensure!(
        changed_count > 0,
        "mode-select build changed no decoded bytes"
    );
    let allowed_ranges = menu_write_claims
        .iter()
        .map(|claim| [claim.range.start, claim.range.end])
        .collect::<Vec<_>>();
    ensure!(
        changed_ranges_are_within(&changed_ranges, &allowed_ranges),
        "mode-select build escaped its semantic MENU claims"
    );
    menu_write_claims
        .retain(|claim| source.menu_decoded[claim.range.clone()] != patched[claim.range.clone()]);
    let (reencoded, source_compression, rebuilt_compression) =
        compress_menu_with_source_limits(&patched, &source.menu_stored)?;
    ensure!(
        decompress(&reencoded, false)? == patched,
        "mode-select compression roundtrip changed decoded bytes"
    );
    ensure!(
        reencoded.len() <= source.menu_stored.len(),
        "mode-select MENU.BIZ exceeds its original record"
    );
    ensure!(
        reencoded[..4] == source.menu_stored[..4],
        "mode-select compression changed the catalog prefix"
    );
    let mut padded = reencoded.clone();
    padded.resize(source.menu_stored.len(), 0);
    ensure!(
        decompress(&padded, true)? == patched,
        "padded mode-select MENU.BIZ changed decoded bytes"
    );
    let output_path = config.output_dir.join(OUTPUT_FILE);
    std::fs::write(&output_path, &padded)?;
    let release_approved_mode_count = modes
        .iter()
        .filter(|mode| mode.release_status == ReleaseStatus::Approved)
        .count();
    let release_approved_fixed_label_count = fixed_labels
        .iter()
        .filter(|label| label.release_status == ReleaseStatus::Approved)
        .count();
    let report = ModeSelectBuildReport {
        logo,
        kind: "Justice Gakuen 2 non-release development mode-select build".to_string(),
        source_bin_sha256: source.source_bin_sha256,
        source_menu_path: MENU_PATH.to_string(),
        source_menu_stored_sha256: sha256_bytes(&source.menu_stored),
        source_menu_decoded_sha256: sha256_bytes(&source.menu_decoded),
        source_overlay_path: super::source::MODE_SELECT_OVERLAY_PATH.to_string(),
        source_overlay_sha256: sha256_bytes(&source.overlay),
        manifest_sha256: assets.manifest_sha256,
        fonts: vec![
            required_font_identity("fixed_label", fixed_label_font)?,
            required_font_identity("list_label", list_label_font)?,
            required_font_identity("detail_title", detail_title_font)?,
            required_font_identity("description", description_font)?,
        ],
        mode_count: modes.len(),
        fixed_label_count: fixed_labels.len(),
        authored_mode_count: modes.len(),
        release_approved_mode_count,
        release_approved_fixed_label_count,
        development_input_available: modes.len() == super::source::MODE_COUNT
            && fixed_labels.len() == 1,
        release_candidate_input_eligible: images.release_input_eligible
            && release_approved_mode_count == modes.len()
            && release_approved_fixed_label_count == fixed_labels.len(),
        source_regions_match: true,
        source_regions_are_disjoint: true,
        decoded_changed_byte_count: changed_count,
        decoded_changed_byte_ranges: changed_ranges,
        compression_maximum_match_words: source_compression.maximum_match_words,
        compression_maximum_control_block_output_words: source_compression
            .maximum_control_block_output_words,
        compression_control_blocks_crossing_input_pages: source_compression
            .control_blocks_crossing_input_pages,
        compression_stream_byte_count: source_compression.stream_byte_count,
        rebuilt_compression_maximum_match_words: rebuilt_compression.maximum_match_words,
        rebuilt_compression_maximum_control_block_output_words: rebuilt_compression
            .maximum_control_block_output_words,
        rebuilt_compression_control_blocks_crossing_input_pages: rebuilt_compression
            .control_blocks_crossing_input_pages,
        rebuilt_compression_stream_byte_count: rebuilt_compression.stream_byte_count,
        rebuilt_decoded_sha256: sha256_bytes(&patched),
        rebuilt_stored_size: reencoded.len(),
        original_stored_size: source.menu_stored.len(),
        padding_size: source.menu_stored.len() - reencoded.len(),
        padded_stored_sha256: sha256_bytes(&padded),
        stored_output_file: OUTPUT_FILE.to_string(),
        modes,
        fixed_labels,
        images,
    };
    let build_manifest_sha256 =
        write_pretty_json_and_hash(&config.output_dir.join(OUTPUT_MANIFEST), &report, true)?;
    Ok(ModeSelectRecordBuild {
        menu_decoded: patched,
        menu_write_claims,
        build_manifest_sha256,
        report,
    })
}

fn prepare_outputs(output_dir: &Path, force: bool) -> Result<()> {
    std::fs::create_dir_all(output_dir)?;
    for name in [OUTPUT_FILE, OUTPUT_MANIFEST] {
        let path = output_dir.join(name);
        if path.exists() && !force {
            bail!("mode-select output exists; pass --force to replace it");
        }
        if path.exists() {
            std::fs::remove_file(&path)
                .with_context(|| format!("failed to remove {}", path.display()))?;
        }
    }
    Ok(())
}

pub(super) fn description_line_cells(
    description: Cell,
    line_count: usize,
    line_height: usize,
) -> Result<Vec<Cell>> {
    ensure!(line_count > 0, "mode-select description has no lines");
    let total_height = line_count
        .checked_mul(line_height)
        .context("mode-select description height overflow")?;
    ensure!(
        total_height <= description.height,
        "mode-select description lines exceed their TIM"
    );
    ensure!(
        DESCRIPTION_HORIZONTAL_PADDING * 2 < description.width,
        "mode-select description padding consumes its width"
    );
    let first_y = description.y + (description.height - total_height) / 2;
    Ok((0..line_count)
        .map(|index| Cell {
            x: description.x + DESCRIPTION_HORIZONTAL_PADDING,
            y: first_y + index * line_height,
            width: description.width - DESCRIPTION_HORIZONTAL_PADDING * 2,
            height: line_height,
        })
        .collect())
}

fn record_font_identity(
    identity: &mut Option<(String, String)>,
    rendered_name: &str,
    rendered_sha256: &str,
    role: &str,
) -> Result<()> {
    if let Some((name, sha256)) = identity.as_ref() {
        ensure!(
            name == rendered_name && sha256 == rendered_sha256,
            "mode-select {role} font identity changed"
        );
    } else {
        *identity = Some((rendered_name.to_string(), rendered_sha256.to_string()));
    }
    Ok(())
}

fn required_font_identity(
    role: &str,
    identity: Option<(String, String)>,
) -> Result<ModeSelectFontBuild> {
    let (font_name, font_sha256) =
        identity.ok_or_else(|| anyhow::anyhow!("mode-select {role} rendered no font"))?;
    Ok(ModeSelectFontBuild {
        role: role.to_string(),
        font_name,
        font_sha256,
    })
}

/// Center actual outlined ink on a consumer's panel center, in half pixels.
/// Selected title origins come from native descriptors (not all are equal).
/// Shared inactive titles start at capture X=66 and target X=113.5, hence
/// local center 47.5. Their right-tab consumer uses the same texture cells.
pub(super) fn align_mode_title_in_panel(
    pixels: &mut [u8],
    width: usize,
    center_twice: i32,
) -> Result<()> {
    ensure!(
        width > 0 && pixels.len().is_multiple_of(width),
        "invalid MODE title raster"
    );
    let mut columns = pixels
        .iter()
        .enumerate()
        .filter(|(_, p)| **p != TITLE_CLEAR_INDEX)
        .map(|(i, _)| i % width);
    let first = columns.next().context("MODE title contains no ink")?;
    let (left, right) = columns.fold((first, first), |(a, b), x| (a.min(x), b.max(x)));
    let shift = (center_twice - (left + right) as i32).div_euclid(2);
    ensure!(
        left as i32 + shift >= 0 && right as i32 + shift < width as i32,
        "MODE title would clip at its panel margin"
    );
    for row in pixels.chunks_exact_mut(width) {
        let original = row.to_vec();
        row.fill(TITLE_CLEAR_INDEX);
        for (x, pixel) in original.into_iter().enumerate() {
            if pixel != TITLE_CLEAR_INDEX {
                row[(x as i32 + shift) as usize] = pixel;
            }
        }
    }
    Ok(())
}
