use std::fs::File;
use std::path::Path;

use anyhow::{Context, Result, ensure};

use crate::compression::decompress;
use crate::cue::CueSheet;
use crate::pipeline::{BASELINE_BIN_SHA256, sha256_file};

use super::atlas::{
    GLYPH_CELL_BYTE_COUNT, GLYPH_CELL_HEIGHT, GLYPH_CELL_WIDTH, parse_dialogue_atlas,
};
use super::model::{
    DialogueFontPreviewAsset, DialogueFontPreviewConfig, DialogueFontPreviewManifest,
};
use super::sources::load_dialogue_runtime_image_sources;

const PREVIEW_COLUMNS: usize = 32;
const PREVIEW_SCALE: usize = 3;
const PREVIEW_GUTTER: usize = 16;

pub fn build_dialogue_font_previews(
    config: &DialogueFontPreviewConfig,
) -> Result<DialogueFontPreviewManifest> {
    let cue = CueSheet::parse(&config.cue)?;
    let source_bin_sha256 = sha256_file(&cue.image_path)?;
    ensure!(
        source_bin_sha256 == BASELINE_BIN_SHA256,
        "unsupported source BIN SHA-256: {source_bin_sha256}"
    );
    std::fs::create_dir_all(&config.output_dir)
        .with_context(|| format!("failed to create {}", config.output_dir.display()))?;

    let sources = load_dialogue_runtime_image_sources(&cue.image_path)?;
    let mut assets = Vec::with_capacity(sources.len());
    for source in sources {
        let decoded = decompress(&source.data, true)
            .with_context(|| format!("failed to decode {}", source.path))?;
        let atlas = parse_dialogue_atlas(&decoded)
            .with_context(|| format!("failed to parse {} dialogue atlas", source.path))?;
        let rows = atlas.fixed_cell_count.div_ceil(PREVIEW_COLUMNS);
        let tile_width = GLYPH_CELL_WIDTH * PREVIEW_SCALE;
        let tile_height = GLYPH_CELL_HEIGHT * PREVIEW_SCALE;
        let width = PREVIEW_GUTTER + PREVIEW_COLUMNS * (tile_width + PREVIEW_GUTTER);
        let height = PREVIEW_GUTTER + rows * (tile_height + PREVIEW_GUTTER);
        let mut pixels = vec![0xff; width * height];
        for code in 0..atlas.fixed_cell_count {
            let cell = &decoded[atlas.pixel_data_offset + code * GLYPH_CELL_BYTE_COUNT
                ..atlas.pixel_data_offset + (code + 1) * GLYPH_CELL_BYTE_COUNT];
            let origin_x =
                PREVIEW_GUTTER + (code % PREVIEW_COLUMNS) * (tile_width + PREVIEW_GUTTER);
            let origin_y =
                PREVIEW_GUTTER + (code / PREVIEW_COLUMNS) * (tile_height + PREVIEW_GUTTER);
            render_cell(cell, &mut pixels, width, origin_x, origin_y, PREVIEW_SCALE);
        }

        let stem = source
            .path
            .rsplit_once('/')
            .map_or(source.path.as_str(), |(_, name)| name)
            .trim_end_matches(".BIZ")
            .to_ascii_lowercase();
        let filename = format!("{stem}-dialogue-font.png");
        let png_path = config.output_dir.join(&filename);
        write_grayscale_png(&png_path, width, height, &pixels)?;
        assets.push(DialogueFontPreviewAsset {
            source_path: source.path,
            fixed_cell_count: atlas.fixed_cell_count,
            columns: PREVIEW_COLUMNS,
            rows,
            first_code: "0x0000".to_string(),
            last_code: format!("0x{:04x}", atlas.fixed_cell_count - 1),
            png: filename,
            png_sha256: sha256_file(&png_path)?,
        });
    }

    let manifest = DialogueFontPreviewManifest {
        kind: "Dialogue runtime-image-scoped font contact sheets".to_string(),
        implementation: "independent Rust 4-bpp TIM preview".to_string(),
        source_bin_sha256,
        cell_width: GLYPH_CELL_WIDTH,
        cell_height: GLYPH_CELL_HEIGHT,
        scale: PREVIEW_SCALE,
        gutter: PREVIEW_GUTTER,
        columns: PREVIEW_COLUMNS,
        coordinate_rule: "code = row * columns + column; rows and columns are zero-based"
            .to_string(),
        assets,
    };
    std::fs::write(
        config.output_dir.join("dialogue-font-previews.json"),
        format!("{}\n", serde_json::to_string_pretty(&manifest)?),
    )?;
    Ok(manifest)
}

pub(super) fn render_cell(
    cell: &[u8],
    output: &mut [u8],
    output_width: usize,
    origin_x: usize,
    origin_y: usize,
    scale: usize,
) {
    for y in 0..GLYPH_CELL_HEIGHT {
        for x in 0..GLYPH_CELL_WIDTH {
            let packed = cell[y * (GLYPH_CELL_WIDTH / 2) + x / 2];
            let index = if x.is_multiple_of(2) {
                packed & 0x0f
            } else {
                packed >> 4
            };
            let value = 255 - index * 17;
            for scaled_y in 0..scale {
                let row = origin_y + y * scale + scaled_y;
                for scaled_x in 0..scale {
                    let column = origin_x + x * scale + scaled_x;
                    output[row * output_width + column] = value;
                }
            }
        }
    }
}

pub(super) fn write_grayscale_png(
    path: &Path,
    width: usize,
    height: usize,
    pixels: &[u8],
) -> Result<()> {
    ensure!(pixels.len() == width * height, "PNG pixel count mismatch");
    let file = File::create(path)?;
    let mut encoder = png::Encoder::new(file, u32::try_from(width)?, u32::try_from(height)?);
    encoder.set_color(png::ColorType::Grayscale);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(pixels)?;
    Ok(())
}
