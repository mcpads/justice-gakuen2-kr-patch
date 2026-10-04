use std::fs::File;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail, ensure};
use serde::Serialize;

use crate::pipeline::{sha256_bytes, sha256_file};
use crate::tim::{
    Cell, decode_4bpp_rgba_in_prefix, parse_4bpp_prefix, read_4bpp_indexed_image_in_prefix,
    read_4bpp_palette_words_in_prefix, read_indexed_cell_in_prefix,
};

use super::menu_textures::EMBEDDED_OPTIONS_SPRITE_TIM_OFFSET;
use super::source::{MENU_PATH, load_options_source};

const COLOR_PREVIEW_FILE: &str = "options-sprites-color.png";
const INDEX_PREVIEW_FILE: &str = "options-sprites-indices.png";
const REPORT_FILE: &str = "options-sprites.json";
const PREVIEW_SCALE: usize = 4;

#[derive(Debug, Clone)]
pub struct OptionsSpriteTextureSurveyConfig {
    pub cue: PathBuf,
    pub output_dir: PathBuf,
    pub force: bool,
}

#[derive(Debug, Serialize)]
pub struct OptionsSpriteTextureSurveyReport {
    pub kind: String,
    pub source_bin_sha256: String,
    pub source_path: String,
    pub source_stored_sha256: String,
    pub source_decoded_sha256: String,
    pub decoded_offset: String,
    pub decoded_size: usize,
    pub source_pixel_sha256: String,
    pub palette_index: usize,
    pub palette_words: Vec<String>,
    pub index_histogram: [usize; 16],
    pub localized_regions: Vec<OptionsSpriteRegionSurvey>,
    pub pixel_width: usize,
    pub pixel_height: usize,
    pub preview_scale: usize,
    pub image_vram_word_x: u16,
    pub image_vram_y: u16,
    pub clut_vram_x: u16,
    pub clut_vram_y: u16,
    pub color_preview_file: String,
    pub color_preview_sha256: String,
    pub index_preview_file: String,
    pub index_preview_sha256: String,
}

#[derive(Debug, Serialize)]
pub struct OptionsSpriteRegionSurvey {
    pub role: String,
    pub cell: Cell,
    pub indexed_pixel_sha256: String,
}

pub fn survey_options_sprite_texture(
    config: &OptionsSpriteTextureSurveyConfig,
) -> Result<OptionsSpriteTextureSurveyReport> {
    prepare_outputs(&config.output_dir, config.force)?;
    let source = load_options_source(&config.cue)?;
    let tim_data = source
        .menu_decoded
        .get(EMBEDDED_OPTIONS_SPRITE_TIM_OFFSET..)
        .context("options sprite TIM offset is outside MENU.BIZ")?;
    let tim = parse_4bpp_prefix(tim_data)?;
    ensure!(
        tim.clut_width * tim.clut_height == 16,
        "options sprite TIM no longer owns exactly one 4-bpp palette"
    );
    let rgba =
        decode_4bpp_rgba_in_prefix(&source.menu_decoded, EMBEDDED_OPTIONS_SPRITE_TIM_OFFSET, 0)?;
    let indexed = read_4bpp_indexed_image_in_prefix(
        &source.menu_decoded,
        EMBEDDED_OPTIONS_SPRITE_TIM_OFFSET,
    )?;
    let palette_words = read_4bpp_palette_words_in_prefix(
        &source.menu_decoded,
        EMBEDDED_OPTIONS_SPRITE_TIM_OFFSET,
        0,
    )?;
    let mut index_histogram = [0usize; 16];
    for index in &indexed.pixels {
        index_histogram[usize::from(*index)] += 1;
    }
    let localized_regions = [
        (
            "school_name_left",
            Cell {
                x: 0,
                y: 0,
                width: 16,
                height: 96,
            },
        ),
        (
            "school_name_right",
            Cell {
                x: 16,
                y: 0,
                width: 16,
                height: 96,
            },
        ),
        (
            "crest_mark",
            Cell {
                x: 84,
                y: 28,
                width: 34,
                height: 42,
            },
        ),
    ]
    .into_iter()
    .map(|(role, cell)| {
        Ok(OptionsSpriteRegionSurvey {
            role: role.to_string(),
            cell,
            indexed_pixel_sha256: sha256_bytes(&read_indexed_cell_in_prefix(
                &source.menu_decoded,
                EMBEDDED_OPTIONS_SPRITE_TIM_OFFSET,
                cell,
            )?),
        })
    })
    .collect::<Result<Vec<_>>>()?;
    ensure!(
        rgba.width == indexed.width && rgba.height == indexed.height,
        "options sprite decoded views disagree on geometry"
    );

    let color_preview_path = config.output_dir.join(COLOR_PREVIEW_FILE);
    let scaled_rgba = scale_pixels(&rgba.pixels, rgba.width, rgba.height, 4, PREVIEW_SCALE);
    write_rgba_png(
        &color_preview_path,
        rgba.width * PREVIEW_SCALE,
        rgba.height * PREVIEW_SCALE,
        &scaled_rgba,
    )?;
    let index_pixels = indexed
        .pixels
        .iter()
        .map(|index| index.saturating_mul(17))
        .collect::<Vec<_>>();
    let index_preview_path = config.output_dir.join(INDEX_PREVIEW_FILE);
    let scaled_indices = scale_pixels(
        &index_pixels,
        indexed.width,
        indexed.height,
        1,
        PREVIEW_SCALE,
    );
    write_grayscale_png(
        &index_preview_path,
        indexed.width * PREVIEW_SCALE,
        indexed.height * PREVIEW_SCALE,
        &scaled_indices,
    )?;

    let report = OptionsSpriteTextureSurveyReport {
        kind: "Justice Gakuen 2 source options sprite texture survey".to_string(),
        source_bin_sha256: source.source_bin_sha256,
        source_path: MENU_PATH.to_string(),
        source_stored_sha256: sha256_bytes(&source.menu_stored),
        source_decoded_sha256: sha256_bytes(&source.menu_decoded),
        decoded_offset: format!("0x{EMBEDDED_OPTIONS_SPRITE_TIM_OFFSET:05x}"),
        decoded_size: tim.total_size,
        source_pixel_sha256: sha256_bytes(
            &tim_data[tim.pixel_offset..tim.pixel_offset + tim.row_bytes() * tim.image_height],
        ),
        palette_index: 0,
        palette_words: palette_words
            .into_iter()
            .map(|word| format!("0x{word:04x}"))
            .collect(),
        index_histogram,
        localized_regions,
        pixel_width: rgba.width,
        pixel_height: rgba.height,
        preview_scale: PREVIEW_SCALE,
        image_vram_word_x: tim.image_x,
        image_vram_y: tim.image_y,
        clut_vram_x: tim.clut_x,
        clut_vram_y: tim.clut_y,
        color_preview_file: COLOR_PREVIEW_FILE.to_string(),
        color_preview_sha256: sha256_file(&color_preview_path)?,
        index_preview_file: INDEX_PREVIEW_FILE.to_string(),
        index_preview_sha256: sha256_file(&index_preview_path)?,
    };
    std::fs::write(
        config.output_dir.join(REPORT_FILE),
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    Ok(report)
}

fn scale_pixels(
    source: &[u8],
    width: usize,
    height: usize,
    channels: usize,
    scale: usize,
) -> Vec<u8> {
    let scaled_width = width * scale;
    let mut output = vec![0u8; scaled_width * height * scale * channels];
    for y in 0..height {
        for x in 0..width {
            let source_offset = (y * width + x) * channels;
            for scaled_y in 0..scale {
                for scaled_x in 0..scale {
                    let target_offset =
                        ((y * scale + scaled_y) * scaled_width + x * scale + scaled_x) * channels;
                    output[target_offset..target_offset + channels]
                        .copy_from_slice(&source[source_offset..source_offset + channels]);
                }
            }
        }
    }
    output
}

fn prepare_outputs(output_dir: &Path, force: bool) -> Result<()> {
    std::fs::create_dir_all(output_dir)?;
    let outputs =
        [COLOR_PREVIEW_FILE, INDEX_PREVIEW_FILE, REPORT_FILE].map(|file| output_dir.join(file));
    if !force && outputs.iter().any(|path| path.exists()) {
        bail!("options sprite survey output exists; pass --force to replace it");
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

fn write_rgba_png(path: &Path, width: usize, height: usize, pixels: &[u8]) -> Result<()> {
    ensure!(
        pixels.len() == width * height * 4,
        "RGBA PNG pixel count mismatch"
    );
    let file = File::create(path)?;
    let mut encoder = png::Encoder::new(file, u32::try_from(width)?, u32::try_from(height)?);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(pixels)?;
    Ok(())
}

fn write_grayscale_png(path: &Path, width: usize, height: usize, pixels: &[u8]) -> Result<()> {
    ensure!(
        pixels.len() == width * height,
        "grayscale PNG pixel count mismatch"
    );
    let file = File::create(path)?;
    let mut encoder = png::Encoder::new(file, u32::try_from(width)?, u32::try_from(height)?);
    encoder.set_color(png::ColorType::Grayscale);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(pixels)?;
    Ok(())
}
