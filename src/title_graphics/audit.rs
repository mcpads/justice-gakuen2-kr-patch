use std::path::Path;

use anyhow::{Context, Result, bail, ensure};

use crate::pipeline::{sha256_bytes, sha256_file};
use crate::tim::{decode_4bpp_rgba_in_prefix, read_4bpp_indexed_image_in_prefix};

use super::assets::load_title_graphics_assets;
use super::model::{TitleGraphicUnitAudit, TitleGraphicsAuditConfig, TitleGraphicsAuditReport};
use super::preview::write_cell_preview;
use super::source::{PALETTE_INDEX, SOURCE_PATH, TIM_OFFSET, load_title_graphics_source};

const REPORT_FILE: &str = "title-graphics-runtime-audit.json";
const VRAM_WIDTH_WORDS: usize = 1024;
const VRAM_HEIGHT: usize = 512;
const VRAM_BYTE_COUNT: usize = VRAM_WIDTH_WORDS * VRAM_HEIGHT * 2;

pub fn audit_title_graphics(config: &TitleGraphicsAuditConfig) -> Result<TitleGraphicsAuditReport> {
    let source = load_title_graphics_source(&config.cue)?;
    let assets = load_title_graphics_assets(&config.assets, &source)?;
    prepare_outputs(
        &config.output_dir,
        assets.units.iter().map(|unit| unit.id.as_str()),
        config.force,
    )?;

    let gpu = std::fs::read(&config.gpu_dump)
        .with_context(|| format!("failed to read {}", config.gpu_dump.display()))?;
    ensure!(
        gpu.len() == VRAM_BYTE_COUNT,
        "runtime GPU dump is not a complete 1024x512 16-bit PS1 VRAM image"
    );
    let frame = std::fs::read(&config.runtime_frame)
        .with_context(|| format!("failed to read {}", config.runtime_frame.display()))?;
    ensure!(
        frame.starts_with(b"\x89PNG\r\n\x1a\n"),
        "paired runtime frame is not a PNG"
    );

    let source_indexed = read_4bpp_indexed_image_in_prefix(&source.decoded, TIM_OFFSET)?;
    let resident_pixels = read_resident_4bpp_pixels(
        &gpu,
        source.tim.image_x,
        source.tim.image_y,
        source_indexed.width,
        source_indexed.height,
    )?;
    let matching_vram_pixel_count =
        verify_exact_texture_residency(&source_indexed.pixels, &resident_pixels)?;

    let mut unit_reports = Vec::with_capacity(assets.units.len());
    for unit in &assets.units {
        let preview_palette_index = unit
            .artwork
            .as_ref()
            .map_or(PALETTE_INDEX, |art| art.palette_index);
        let rgba = decode_4bpp_rgba_in_prefix(&source.decoded, TIM_OFFSET, preview_palette_index)?;
        let resident = read_resident_cell(&resident_pixels, source_indexed.width, unit.cell)?;
        let resident_sha256 = sha256_bytes(&resident);
        ensure!(
            resident_sha256 == unit.source_indexed_pixel_sha256,
            "runtime VRAM changed title graphic {}",
            unit.id
        );
        let preview_file = format!("{}.png", unit.id.replace('_', "-"));
        let preview_path = config.output_dir.join(&preview_file);
        write_cell_preview(&preview_path, &rgba, unit.cell)?;
        unit_reports.push(TitleGraphicUnitAudit {
            id: unit.id.clone(),
            runtime_role: unit.runtime_role.clone(),
            cell: unit.cell,
            source_indexed_pixel_sha256: unit.source_indexed_pixel_sha256.clone(),
            resident_indexed_pixel_sha256: resident_sha256,
            source_matches_vram: true,
            source_preview_file: preview_file,
            source_preview_palette_index: preview_palette_index,
            source_preview_sha256: sha256_file(&preview_path)?,
            development_status: unit.development_status.clone(),
            release_status: unit.release_status.clone(),
        });
    }

    let report = TitleGraphicsAuditReport {
        kind: "justice_gakuen2_title_graphics_runtime_audit".to_string(),
        source_bin_sha256: source.source_bin_sha256,
        source_path: SOURCE_PATH.to_string(),
        source_extent_lba: source.extent_lba,
        source_stored_sha256: sha256_bytes(&source.stored),
        source_decoded_sha256: sha256_bytes(&source.decoded),
        manifest_sha256: assets.manifest_sha256,
        asset_set_sha256: assets.asset_set_sha256,
        gpu_dump_path: path_string(&config.gpu_dump),
        gpu_dump_sha256: sha256_bytes(&gpu),
        runtime_frame_path: path_string(&config.runtime_frame),
        runtime_frame_sha256: sha256_bytes(&frame),
        source_texture_pixel_count: source_indexed.pixels.len(),
        matching_vram_pixel_count,
        entire_source_texture_matches_vram: true,
        storage_record_verified: true,
        source_pixel_boundaries_verified: true,
        runtime_residency_verified: true,
        exact_draw_consumer_verified: false,
        unit_count: assets.units.len(),
        untranslated_unit_count: assets
            .units
            .iter()
            .filter(|unit| unit.artwork.is_none())
            .count(),
        units: unit_reports,
        release_eligible: false,
    };
    write_report(&config.output_dir.join(REPORT_FILE), &report)?;
    Ok(report)
}

pub(super) fn read_resident_4bpp_pixels(
    gpu: &[u8],
    image_word_x: u16,
    image_y: u16,
    pixel_width: usize,
    pixel_height: usize,
) -> Result<Vec<u8>> {
    ensure!(
        gpu.len() == VRAM_BYTE_COUNT,
        "runtime GPU dump has the wrong byte count"
    );
    ensure!(
        usize::from(image_word_x) + pixel_width.div_ceil(4) <= VRAM_WIDTH_WORDS
            && usize::from(image_y) + pixel_height <= VRAM_HEIGHT,
        "4-bpp texture is outside PS1 VRAM"
    );
    let mut pixels = Vec::with_capacity(pixel_width * pixel_height);
    for y in 0..pixel_height {
        for x in 0..pixel_width {
            let word_offset =
                (usize::from(image_y) + y) * VRAM_WIDTH_WORDS + usize::from(image_word_x) + x / 4;
            let byte_offset = word_offset * 2;
            let word = u16::from_le_bytes(gpu[byte_offset..byte_offset + 2].try_into()?);
            pixels.push(((word >> (4 * (x & 3))) & 0x0f) as u8);
        }
    }
    Ok(pixels)
}

pub(super) fn verify_exact_texture_residency(source: &[u8], resident: &[u8]) -> Result<usize> {
    ensure!(
        source.len() == resident.len(),
        "source and resident title textures have different pixel counts"
    );
    let matching = source
        .iter()
        .zip(resident)
        .filter(|(source, resident)| source == resident)
        .count();
    ensure!(
        matching == source.len(),
        "the paired runtime state does not contain the exact MA_TIT title texture"
    );
    Ok(matching)
}

fn read_resident_cell(
    resident: &[u8],
    texture_width: usize,
    cell: crate::tim::Cell,
) -> Result<Vec<u8>> {
    let texture_height = resident.len() / texture_width;
    ensure!(
        cell.x + cell.width <= texture_width && cell.y + cell.height <= texture_height,
        "title graphic cell is outside resident texture"
    );
    let mut pixels = Vec::with_capacity(cell.width * cell.height);
    for y in cell.y..cell.y + cell.height {
        let start = y * texture_width + cell.x;
        pixels.extend_from_slice(&resident[start..start + cell.width]);
    }
    Ok(pixels)
}

fn prepare_outputs<'a>(
    output_dir: &Path,
    unit_ids: impl Iterator<Item = &'a str>,
    force: bool,
) -> Result<()> {
    std::fs::create_dir_all(output_dir)?;
    let mut outputs = vec![output_dir.join(REPORT_FILE)];
    outputs.extend(unit_ids.map(|id| output_dir.join(format!("{}.png", id.replace('_', "-")))));
    if !force && outputs.iter().any(|path| path.exists()) {
        bail!("title graphics audit output exists; pass --force to replace owned files");
    }
    if force {
        for path in outputs {
            if path.exists() {
                std::fs::remove_file(&path)
                    .with_context(|| format!("failed to remove {}", path.display()))?;
            }
        }
    }
    Ok(())
}

fn write_report(path: &Path, report: &TitleGraphicsAuditReport) -> Result<()> {
    let mut bytes = serde_json::to_vec_pretty(report)?;
    bytes.push(b'\n');
    std::fs::write(path, bytes).with_context(|| format!("failed to write {}", path.display()))
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}
