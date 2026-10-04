use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};
use fontdue::Font;

use super::model::{HorizontalTextAlignment, RasterizedIndexedText};
use super::raster::{FILL_THRESHOLD, OUTLINE_RADIUS, dilate, load_supported_font};

pub struct IndexedTextRasterizer {
    font: Font,
    font_sha256: String,
    font_name: String,
}

#[derive(Default)]
pub struct IndexedTextRasterizers {
    by_path: BTreeMap<PathBuf, IndexedTextRasterizer>,
}

impl IndexedTextRasterizers {
    pub fn for_font(&mut self, font_path: &Path) -> Result<&IndexedTextRasterizer> {
        if !self.by_path.contains_key(font_path) {
            self.by_path.insert(
                font_path.to_path_buf(),
                IndexedTextRasterizer::load(font_path)?,
            );
        }
        self.by_path
            .get(font_path)
            .context("loaded indexed-text rasterizer disappeared")
    }
}

impl IndexedTextRasterizer {
    pub fn load(font_path: &Path) -> Result<Self> {
        let (font, font_sha256, identity) = load_supported_font(font_path)?;
        Ok(Self {
            font,
            font_sha256,
            font_name: identity.name.to_string(),
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn rasterize(
        &self,
        text: &str,
        width: usize,
        height: usize,
        font_px: f32,
        tracking_px: f32,
        clear_index: u8,
        outline_index: Option<u8>,
        fill_index: u8,
        alignment: HorizontalTextAlignment,
    ) -> Result<RasterizedIndexedText> {
        self.rasterize_shifted(
            text,
            width,
            height,
            font_px,
            tracking_px,
            0,
            clear_index,
            outline_index,
            fill_index,
            alignment,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn rasterize_shifted(
        &self,
        text: &str,
        width: usize,
        height: usize,
        font_px: f32,
        tracking_px: f32,
        vertical_shift_px: i32,
        clear_index: u8,
        outline_index: Option<u8>,
        fill_index: u8,
        alignment: HorizontalTextAlignment,
    ) -> Result<RasterizedIndexedText> {
        ensure!(!text.is_empty(), "indexed text is empty");
        ensure!(width > 0 && height > 0, "indexed text rectangle is empty");
        ensure!(font_px.is_finite() && font_px > 0.0, "invalid font size");
        ensure!(tracking_px.is_finite(), "invalid text tracking");
        ensure!(
            clear_index != fill_index,
            "clear and fill indices must differ"
        );
        if let Some(outline_index) = outline_index {
            ensure!(
                outline_index != clear_index,
                "outline and clear indices must differ"
            );
            ensure!(
                outline_index != fill_index,
                "outline and fill indices must differ"
            );
        }

        let raster = self.rasterize_coverage(
            text,
            width,
            height,
            font_px,
            tracking_px,
            vertical_shift_px,
            alignment,
        )?;
        let fill = raster
            .coverage
            .iter()
            .map(|coverage| *coverage >= FILL_THRESHOLD)
            .collect::<Vec<_>>();
        ensure!(fill.iter().any(|pixel| *pixel), "indexed text has no ink");
        let outline = outline_index.map(|_| dilate(&fill, width, height, OUTLINE_RADIUS));
        let mut pixels = vec![clear_index; width * height];
        for index in 0..pixels.len() {
            if outline.as_ref().is_some_and(|outline| outline[index]) {
                pixels[index] = outline_index.context("outline index disappeared")?;
            }
            if fill[index] {
                pixels[index] = fill_index;
            }
        }
        finish_indexed_raster(raster, pixels, width, height, clear_index)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn rasterize_shifted_with_coverage_ramp(
        &self,
        text: &str,
        width: usize,
        height: usize,
        font_px: f32,
        tracking_px: f32,
        vertical_shift_px: i32,
        clear_index: u8,
        first_ink_index: u8,
        last_ink_index: u8,
        alignment: HorizontalTextAlignment,
    ) -> Result<RasterizedIndexedText> {
        ensure!(
            first_ink_index > clear_index && first_ink_index <= last_ink_index,
            "invalid indexed coverage ramp"
        );
        let raster = self.rasterize_coverage(
            text,
            width,
            height,
            font_px,
            tracking_px,
            vertical_shift_px,
            alignment,
        )?;
        let span = u16::from(last_ink_index - first_ink_index);
        let pixels = raster
            .coverage
            .iter()
            .map(|coverage| {
                if *coverage == 0 {
                    clear_index
                } else {
                    let scaled = u16::from(*coverage - 1) * span / 254;
                    first_ink_index + scaled as u8
                }
            })
            .collect::<Vec<_>>();
        finish_indexed_raster(raster, pixels, width, height, clear_index)
    }

    #[allow(clippy::too_many_arguments)]
    fn rasterize_coverage(
        &self,
        text: &str,
        width: usize,
        height: usize,
        font_px: f32,
        tracking_px: f32,
        vertical_shift_px: i32,
        alignment: HorizontalTextAlignment,
    ) -> Result<RasterizedCoverage> {
        rasterize_coverage_with_font(
            &self.font,
            &self.font_name,
            &self.font_sha256,
            text,
            width,
            height,
            font_px,
            tracking_px,
            vertical_shift_px,
            alignment,
        )
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn centered_glyph_ink_spans(
    rasterizer: &IndexedTextRasterizer,
    text: &str,
    width: usize,
    height: usize,
    font_px: f32,
    tracking_px: f32,
    measured_advance_px: f32,
) -> Result<Vec<[usize; 2]>> {
    let mut cursor_x = (width as f32 - measured_advance_px) / 2.0;
    ensure!(
        cursor_x >= 0.0,
        "centered text advance exceeds its owned cell"
    );
    let characters = text.chars().collect::<Vec<_>>();
    let mut spans = Vec::with_capacity(characters.len());
    for (index, character) in characters.iter().enumerate() {
        let glyph = rasterizer.rasterize(
            &character.to_string(),
            width,
            height,
            font_px,
            0.0,
            0,
            Some(1),
            2,
            HorizontalTextAlignment::Left,
        )?;
        let shift = cursor_x.round() as usize;
        let span = [shift + glyph.ink_bounds[0], shift + glyph.ink_bounds[2]];
        ensure!(
            span[0] < span[1] && span[1] <= width,
            "centered glyph leaves its owned cell"
        );
        spans.push(span);
        cursor_x += glyph.measured_advance_px;
        if index + 1 < characters.len() {
            cursor_x += tracking_px;
        }
    }
    for pair in spans.windows(2) {
        ensure!(pair[0][1] <= pair[1][0], "centered glyph ink spans overlap");
    }
    for index in 0..spans.len().saturating_sub(1) {
        let boundary = (spans[index][1] + spans[index + 1][0]) / 2;
        spans[index][1] = boundary;
        spans[index + 1][0] = boundary;
    }
    Ok(spans)
}

#[allow(clippy::too_many_arguments)]
pub fn rasterize_indexed_text(
    font_path: &Path,
    text: &str,
    width: usize,
    height: usize,
    font_px: f32,
    tracking_px: f32,
    clear_index: u8,
    outline_index: Option<u8>,
    fill_index: u8,
    alignment: HorizontalTextAlignment,
) -> Result<RasterizedIndexedText> {
    let rasterizer = IndexedTextRasterizer::load(font_path)?;
    rasterizer.rasterize(
        text,
        width,
        height,
        font_px,
        tracking_px,
        clear_index,
        outline_index,
        fill_index,
        alignment,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn rasterize_indexed_text_with_coverage_ramp(
    font_path: &Path,
    text: &str,
    width: usize,
    height: usize,
    font_px: f32,
    tracking_px: f32,
    clear_index: u8,
    first_ink_index: u8,
    last_ink_index: u8,
    alignment: HorizontalTextAlignment,
) -> Result<RasterizedIndexedText> {
    rasterize_shifted_indexed_text_with_coverage_ramp(
        font_path,
        text,
        width,
        height,
        font_px,
        tracking_px,
        0,
        clear_index,
        first_ink_index,
        last_ink_index,
        alignment,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn rasterize_shifted_indexed_text_with_coverage_ramp(
    font_path: &Path,
    text: &str,
    width: usize,
    height: usize,
    font_px: f32,
    tracking_px: f32,
    vertical_shift_px: i32,
    clear_index: u8,
    first_ink_index: u8,
    last_ink_index: u8,
    alignment: HorizontalTextAlignment,
) -> Result<RasterizedIndexedText> {
    let rasterizer = IndexedTextRasterizer::load(font_path)?;
    rasterizer.rasterize_shifted_with_coverage_ramp(
        text,
        width,
        height,
        font_px,
        tracking_px,
        vertical_shift_px,
        clear_index,
        first_ink_index,
        last_ink_index,
        alignment,
    )
}

struct RasterizedCoverage {
    font_name: String,
    font_sha256: String,
    coverage: Vec<u8>,
    measured_advance_px: f32,
}

#[allow(clippy::too_many_arguments)]
fn rasterize_coverage_with_font(
    font: &Font,
    font_name: &str,
    font_sha256: &str,
    text: &str,
    width: usize,
    height: usize,
    font_px: f32,
    tracking_px: f32,
    vertical_shift_px: i32,
    alignment: HorizontalTextAlignment,
) -> Result<RasterizedCoverage> {
    ensure!(!text.is_empty(), "indexed text is empty");
    ensure!(width > 0 && height > 0, "indexed text rectangle is empty");
    ensure!(font_px.is_finite() && font_px > 0.0, "invalid font size");
    ensure!(tracking_px.is_finite(), "invalid text tracking");
    let glyphs = text
        .chars()
        .map(|character| {
            ensure!(
                font.has_glyph(character),
                "font lacks indexed-text character {character:?}"
            );
            Ok(font.rasterize(character, font_px))
        })
        .collect::<Result<Vec<_>>>()?;
    let measured_advance_px = glyphs
        .iter()
        .map(|(metrics, _)| metrics.advance_width)
        .sum::<f32>()
        + tracking_px * text.chars().count().saturating_sub(1) as f32;
    ensure!(
        measured_advance_px <= width as f32,
        "indexed text {text:?} needs {measured_advance_px:.2}px but owns {width}px"
    );
    let requested_origin = horizontal_origin(width, measured_advance_px, alignment);
    let mut cursor = 0.0f32;
    let mut left = f32::INFINITY;
    let mut right = f32::NEG_INFINITY;
    for (metrics, _) in &glyphs {
        if metrics.width > 0 && metrics.height > 0 {
            left = left.min(cursor.round() + metrics.xmin as f32);
            right = right.max(cursor.round() + metrics.xmin as f32 + metrics.width as f32);
        }
        cursor += metrics.advance_width + tracking_px;
    }
    let origin_x = if left.is_finite() {
        let minimum_origin = (-left).ceil();
        let maximum_origin = (width as f32 - right).floor();
        ensure!(
            minimum_origin <= maximum_origin,
            "indexed text {text:?} ink exceeds its {width}px rectangle"
        );
        requested_origin.clamp(minimum_origin, maximum_origin)
    } else {
        requested_origin
    };
    let line_metrics = font
        .horizontal_line_metrics(font_px)
        .context("font has no horizontal line metrics")?;
    let baseline = super::raster::font_baseline(
        font_sha256,
        height,
        line_metrics.ascent,
        line_metrics.descent,
    )? + vertical_shift_px;
    let mut coverage = vec![0u8; width * height];
    let mut cursor_x = origin_x;
    for (index, (metrics, raster)) in glyphs.iter().enumerate() {
        let x_offset = cursor_x.round() as i32 + metrics.xmin;
        let y_offset = baseline - metrics.ymin - metrics.height as i32;
        for row in 0..metrics.height {
            for column in 0..metrics.width {
                let value = raster[row * metrics.width + column];
                if value == 0 {
                    continue;
                }
                let x = x_offset + column as i32;
                let y = y_offset + row as i32;
                ensure!(
                    x >= 0 && y >= 0 && x < width as i32 && y < height as i32,
                    "indexed text {text:?} clips its {width}x{height} rectangle"
                );
                let offset = y as usize * width + x as usize;
                coverage[offset] = coverage[offset].max(value);
            }
        }
        cursor_x += metrics.advance_width;
        if index + 1 < glyphs.len() {
            cursor_x += tracking_px;
        }
    }

    Ok(RasterizedCoverage {
        font_name: font_name.to_string(),
        font_sha256: font_sha256.to_string(),
        coverage,
        measured_advance_px,
    })
}

fn finish_indexed_raster(
    raster: RasterizedCoverage,
    pixels: Vec<u8>,
    width: usize,
    height: usize,
    clear_index: u8,
) -> Result<RasterizedIndexedText> {
    let ink_bounds = indexed_ink_bounds(&pixels, width, height, clear_index)
        .context("indexed text raster has no non-clear pixels")?;
    Ok(RasterizedIndexedText {
        font_name: raster.font_name,
        font_sha256: raster.font_sha256,
        pixels,
        measured_advance_px: raster.measured_advance_px,
        ink_bounds,
    })
}

pub(super) fn horizontal_origin(
    width: usize,
    measured_advance_px: f32,
    alignment: HorizontalTextAlignment,
) -> f32 {
    match alignment {
        HorizontalTextAlignment::Left => 0.0,
        HorizontalTextAlignment::Center => (width as f32 - measured_advance_px) / 2.0,
        HorizontalTextAlignment::Right => width as f32 - measured_advance_px,
    }
}

fn indexed_ink_bounds(
    pixels: &[u8],
    width: usize,
    height: usize,
    clear_index: u8,
) -> Option<[usize; 4]> {
    let mut min_x = width;
    let mut min_y = height;
    let mut max_x = 0usize;
    let mut max_y = 0usize;
    let mut found = false;
    for y in 0..height {
        for x in 0..width {
            if pixels[y * width + x] == clear_index {
                continue;
            }
            found = true;
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x + 1);
            max_y = max_y.max(y + 1);
        }
    }
    found.then_some([min_x, min_y, max_x, max_y])
}
