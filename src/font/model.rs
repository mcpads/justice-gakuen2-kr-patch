use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct MenuFontPreviewConfig {
    pub font: PathBuf,
    pub output_dir: PathBuf,
    pub sizes: Vec<f32>,
    pub characters: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct GlyphFit {
    pub character: char,
    pub metrics_width: usize,
    pub metrics_height: usize,
    pub xmin: i32,
    pub ymin: i32,
    pub advance_width: f32,
    pub clipped_coverage_pixels: usize,
    pub fill_touches_cell_boundary: bool,
    pub fits_with_one_pixel_outline: bool,
}

#[derive(Debug, Serialize)]
pub struct SizeFit {
    pub font_px: f32,
    pub all_preview_glyphs_fit_with_outline: bool,
    pub validated_hangul_syllable_count: usize,
    pub renderable_hangul_syllable_count: usize,
    pub unrenderable_hangul_syllable_count: usize,
    pub unrenderable_hangul_examples: Vec<char>,
    pub renderable_hangul_fit_failure_count: usize,
    pub renderable_hangul_fit_failure_examples: Vec<GlyphFit>,
    pub all_renderable_hangul_syllables_fit_with_outline: bool,
    pub full_modern_hangul_gate_passes: bool,
    pub preview_and_renderable_hangul_fit_with_outline: bool,
    pub glyphs: Vec<GlyphFit>,
}

#[derive(Debug, Serialize)]
pub struct MenuFontPreviewManifest {
    pub kind: String,
    pub implementation: String,
    pub font_name: String,
    pub font_file: String,
    pub font_sha256: String,
    pub characters: String,
    pub cell_width: usize,
    pub cell_height: usize,
    pub fill_threshold: u8,
    pub outline_radius: usize,
    pub preview_scale: usize,
    pub preview_png: String,
    pub preview_png_sha256: String,
    pub sizes: Vec<SizeFit>,
    pub largest_outline_safe_size_for_renderable_hangul: Option<f32>,
}

pub(super) struct RenderedGlyph {
    pub(super) pixels: Vec<u8>,
    pub(super) fit: GlyphFit,
}

#[derive(Debug)]
pub struct RasterizedMenuGlyph {
    pub character: char,
    pub pixels: Vec<u8>,
    pub fit: GlyphFit,
}

#[derive(Debug)]
pub struct RasterizedMenuGlyphSet {
    pub font_name: String,
    pub font_sha256: String,
    pub glyphs: Vec<RasterizedMenuGlyph>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HorizontalTextAlignment {
    Left,
    Center,
    Right,
}

#[derive(Debug)]
pub struct RasterizedIndexedText {
    pub font_name: String,
    pub font_sha256: String,
    pub pixels: Vec<u8>,
    pub measured_advance_px: f32,
    pub ink_bounds: [usize; 4],
}
