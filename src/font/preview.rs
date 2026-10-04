use std::fs::File;
use std::io::Read;
use std::path::Path;

use anyhow::{Context, Result, ensure};
use sha2::{Digest, Sha256};

use super::model::{MenuFontPreviewConfig, MenuFontPreviewManifest, SizeFit};
use super::raster::{
    CELL_HEIGHT, CELL_WIDTH, FILL_THRESHOLD, OUTLINE_RADIUS, load_supported_font, render_glyph,
    try_render_glyph,
};

const PREVIEW_COLUMNS: usize = 14;
const PREVIEW_SCALE: usize = 4;
const HANGUL_SYLLABLE_START: u32 = 0xac00;
const HANGUL_SYLLABLE_END: u32 = 0xd7a3;
const FAILURE_EXAMPLE_LIMIT: usize = 32;

pub fn build_menu_font_preview(config: &MenuFontPreviewConfig) -> Result<MenuFontPreviewManifest> {
    ensure!(
        !config.sizes.is_empty(),
        "at least one font size is required"
    );
    ensure!(
        config
            .sizes
            .iter()
            .all(|size| size.is_finite() && *size > 0.0),
        "font sizes must be finite and positive"
    );
    let characters: Vec<char> = config.characters.chars().collect();
    ensure!(
        !characters.is_empty(),
        "at least one preview character is required"
    );

    let (font, font_sha256, font_identity) = load_supported_font(&config.font)?;
    let character_rows = characters.len().div_ceil(PREVIEW_COLUMNS);
    let source_width = PREVIEW_COLUMNS * CELL_WIDTH;
    let source_height = config.sizes.len() * character_rows * CELL_HEIGHT;
    let mut preview = vec![0u8; source_width * source_height];
    let mut size_reports = Vec::with_capacity(config.sizes.len());

    for (size_index, &font_px) in config.sizes.iter().enumerate() {
        let mut glyph_reports = Vec::with_capacity(characters.len());
        for (character_index, &character) in characters.iter().enumerate() {
            let rendered = render_glyph(&font, &font_sha256, character, font_px)?;
            let cell_x = (character_index % PREVIEW_COLUMNS) * CELL_WIDTH;
            let cell_y =
                (size_index * character_rows + character_index / PREVIEW_COLUMNS) * CELL_HEIGHT;
            for y in 0..CELL_HEIGHT {
                let source = &rendered.pixels[y * CELL_WIDTH..(y + 1) * CELL_WIDTH];
                let target_offset = (cell_y + y) * source_width + cell_x;
                preview[target_offset..target_offset + CELL_WIDTH].copy_from_slice(source);
            }
            glyph_reports.push(rendered.fit);
        }
        let all_preview_glyphs_fit_with_outline = glyph_reports
            .iter()
            .all(|glyph| glyph.fits_with_one_pixel_outline);
        let mut renderable_hangul_syllable_count = 0usize;
        let mut unrenderable_hangul_syllable_count = 0usize;
        let mut unrenderable_hangul_examples = Vec::new();
        let mut renderable_hangul_fit_failure_count = 0usize;
        let mut renderable_hangul_fit_failure_examples = Vec::new();
        for codepoint in HANGUL_SYLLABLE_START..=HANGUL_SYLLABLE_END {
            let character =
                char::from_u32(codepoint).expect("modern Hangul range is valid Unicode");
            let Some(rendered) = try_render_glyph(&font, &font_sha256, character, font_px)? else {
                unrenderable_hangul_syllable_count += 1;
                if unrenderable_hangul_examples.len() < FAILURE_EXAMPLE_LIMIT {
                    unrenderable_hangul_examples.push(character);
                }
                continue;
            };
            renderable_hangul_syllable_count += 1;
            let fit = rendered.fit;
            if !fit.fits_with_one_pixel_outline {
                renderable_hangul_fit_failure_count += 1;
                if renderable_hangul_fit_failure_examples.len() < FAILURE_EXAMPLE_LIMIT {
                    renderable_hangul_fit_failure_examples.push(fit);
                }
            }
        }
        let validated_hangul_syllable_count =
            usize::try_from(HANGUL_SYLLABLE_END - HANGUL_SYLLABLE_START + 1)?;
        let all_renderable_hangul_syllables_fit_with_outline =
            renderable_hangul_fit_failure_count == 0;
        let full_modern_hangul_gate_passes = unrenderable_hangul_syllable_count == 0
            && all_renderable_hangul_syllables_fit_with_outline;
        size_reports.push(SizeFit {
            font_px,
            all_preview_glyphs_fit_with_outline,
            validated_hangul_syllable_count,
            renderable_hangul_syllable_count,
            unrenderable_hangul_syllable_count,
            unrenderable_hangul_examples,
            renderable_hangul_fit_failure_count,
            renderable_hangul_fit_failure_examples,
            all_renderable_hangul_syllables_fit_with_outline,
            full_modern_hangul_gate_passes,
            preview_and_renderable_hangul_fit_with_outline: all_preview_glyphs_fit_with_outline
                && all_renderable_hangul_syllables_fit_with_outline,
            glyphs: glyph_reports,
        });
    }

    let scaled = scale_nearest(&preview, source_width, source_height, PREVIEW_SCALE);
    std::fs::create_dir_all(&config.output_dir)?;
    let png_file = format!("{}-menu-cells.png", font_identity.slug);
    let png_path = config.output_dir.join(&png_file);
    write_grayscale_png(
        &png_path,
        source_width * PREVIEW_SCALE,
        source_height * PREVIEW_SCALE,
        &scaled,
    )?;
    let preview_png_sha256 = sha256_file(&png_path)?;
    let largest_outline_safe_size_for_renderable_hangul = size_reports
        .iter()
        .filter(|report| report.preview_and_renderable_hangul_fit_with_outline)
        .map(|report| report.font_px)
        .max_by(f32::total_cmp);
    let font_file = config
        .font
        .file_name()
        .and_then(|name| name.to_str())
        .context("font file name is not UTF-8")?
        .to_string();
    let preview_png = png_path
        .file_name()
        .and_then(|name| name.to_str())
        .context("preview PNG file name is not UTF-8")?
        .to_string();
    let manifest = MenuFontPreviewManifest {
        kind: format!("{} 20x20 menu-cell fit preview", font_identity.name),
        implementation: "independent Rust fontdue raster pipeline".to_string(),
        font_name: font_identity.name.to_string(),
        font_file,
        font_sha256,
        characters: config.characters.clone(),
        cell_width: CELL_WIDTH,
        cell_height: CELL_HEIGHT,
        fill_threshold: FILL_THRESHOLD,
        outline_radius: OUTLINE_RADIUS,
        preview_scale: PREVIEW_SCALE,
        preview_png,
        preview_png_sha256,
        sizes: size_reports,
        largest_outline_safe_size_for_renderable_hangul,
    };
    std::fs::write(
        config
            .output_dir
            .join(format!("{}-menu-cells.json", font_identity.slug)),
        format!("{}\n", serde_json::to_string_pretty(&manifest)?),
    )?;
    Ok(manifest)
}

pub(super) fn scale_nearest(source: &[u8], width: usize, height: usize, scale: usize) -> Vec<u8> {
    let scaled_width = width * scale;
    let mut output = vec![0u8; scaled_width * height * scale];
    for y in 0..height {
        for x in 0..width {
            let value = source[y * width + x];
            for sy in 0..scale {
                for sx in 0..scale {
                    output[(y * scale + sy) * scaled_width + x * scale + sx] = value;
                }
            }
        }
    }
    output
}

fn write_grayscale_png(path: &Path, width: usize, height: usize, pixels: &[u8]) -> Result<()> {
    ensure!(pixels.len() == width * height, "PNG pixel count mismatch");
    let file = File::create(path)?;
    let mut encoder = png::Encoder::new(file, u32::try_from(width)?, u32::try_from(height)?);
    encoder.set_color(png::ColorType::Grayscale);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(pixels)?;
    Ok(())
}

fn sha256_file(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 1024 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(format!("{:x}", digest.finalize()))
}
