use std::fs::File;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};

use super::model::{
    BonusShopTextContactSheetIndex, BonusShopTextContactSheetPlacement,
    BonusShopTextContactSheetRef, BonusShopTextSourceTokenKind,
};
use super::parser::{ParsedShopTextRecord, ParsedShopTextTable, packed_glyph_pixels};
use super::writer::{path_string, write_json};
use crate::pipeline::sha256_file;

const GLYPH_WIDTH: usize = 20;
const GLYPH_HEIGHT: usize = 20;
const LINE_ADVANCE: usize = 26;
const SHEET_RECORD_LIMIT: usize = 12;
const SHEET_PADDING: usize = 6;
const RECORD_GUTTER: usize = 6;
const PREVIEW_SCALE: usize = 2;

pub(super) fn write_contact_sheets(
    root: &Path,
    table: &ParsedShopTextTable,
    shop_ui_decoded: &[u8],
) -> Result<Vec<BonusShopTextContactSheetRef>> {
    table
        .records
        .chunks(SHEET_RECORD_LIMIT)
        .enumerate()
        .map(|(sheet_index, records)| {
            write_contact_sheet(root, table, shop_ui_decoded, sheet_index, records)
        })
        .collect()
}

fn write_contact_sheet(
    root: &Path,
    table: &ParsedShopTextTable,
    shop_ui_decoded: &[u8],
    sheet_index: usize,
    records: &[ParsedShopTextRecord],
) -> Result<BonusShopTextContactSheetRef> {
    let layouts = records.iter().map(record_layout).collect::<Vec<_>>();
    let source_width = SHEET_PADDING * 2
        + layouts
            .iter()
            .map(|layout| layout.width)
            .max()
            .unwrap_or(GLYPH_WIDTH);
    let source_height = SHEET_PADDING * 2
        + layouts.iter().map(|layout| layout.height).sum::<usize>()
        + RECORD_GUTTER * records.len().saturating_sub(1);
    let mut pixels = vec![255u8; source_width * source_height];
    let mut placements = Vec::with_capacity(records.len());
    let mut origin_y = SHEET_PADDING;
    for (record, layout) in records.iter().zip(&layouts) {
        render_record(
            record,
            shop_ui_decoded,
            &mut pixels,
            source_width,
            SHEET_PADDING,
            origin_y,
        )?;
        placements.push(BonusShopTextContactSheetPlacement {
            unit_id: record.unit.unit_id.clone(),
            record_index: record.unit.record_index,
            source_offset: record.unit.source_offset.clone(),
            x: SHEET_PADDING * PREVIEW_SCALE,
            y: origin_y * PREVIEW_SCALE,
            width: layout.width * PREVIEW_SCALE,
            height: layout.height * PREVIEW_SCALE,
        });
        origin_y += layout.height + RECORD_GUTTER;
        if origin_y < source_height - SHEET_PADDING {
            let separator_y = origin_y - RECORD_GUTTER.div_ceil(2);
            pixels[separator_y * source_width..(separator_y + 1) * source_width].fill(208);
        }
    }

    let scaled = scale_nearest(&pixels, source_width, source_height, PREVIEW_SCALE);
    let sheet_id = format!("{}-sheet-{sheet_index:03}", table.spec.role.slug());
    let png_relative = PathBuf::from(format!(
        "{}/previews/sheet-{sheet_index:03}.png",
        table.spec.role.slug()
    ));
    let png_path = root.join(&png_relative);
    if let Some(parent) = png_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    write_grayscale_png(
        &png_path,
        source_width * PREVIEW_SCALE,
        source_height * PREVIEW_SCALE,
        &scaled,
    )?;
    let index = BonusShopTextContactSheetIndex {
        kind: "Justice Gakuen 2 bonus-shop source contact-sheet index".to_string(),
        sheet_id: sheet_id.clone(),
        role: table.spec.role,
        scale: PREVIEW_SCALE,
        width: source_width * PREVIEW_SCALE,
        height: source_height * PREVIEW_SCALE,
        records: placements,
    };
    let index_relative = PathBuf::from(format!(
        "{}/previews/sheet-{sheet_index:03}.json",
        table.spec.role.slug()
    ));
    let (index_sha256, _) = write_json(root, &index_relative, &index)?;
    Ok(BonusShopTextContactSheetRef {
        sheet_id,
        png_path: path_string(&png_relative),
        png_sha256: sha256_file(&png_path)?,
        index_path: path_string(&index_relative),
        index_sha256,
        record_count: records.len(),
    })
}

#[derive(Debug, Clone, Copy)]
struct RecordLayout {
    width: usize,
    height: usize,
}

fn record_layout(record: &ParsedShopTextRecord) -> RecordLayout {
    let mut x = 0usize;
    let mut y = 0usize;
    let mut width = 0usize;
    for token in &record.unit.tokens {
        match token.kind {
            BonusShopTextSourceTokenKind::Glyph | BonusShopTextSourceTokenKind::Blank => {
                x += GLYPH_WIDTH;
                width = width.max(x);
            }
            BonusShopTextSourceTokenKind::LineBreak => {
                x = 0;
                y += LINE_ADVANCE;
            }
            BonusShopTextSourceTokenKind::Terminator => {}
        }
    }
    RecordLayout {
        width: width.max(GLYPH_WIDTH),
        height: y + GLYPH_HEIGHT,
    }
}

fn render_record(
    record: &ParsedShopTextRecord,
    shop_ui_decoded: &[u8],
    output: &mut [u8],
    output_width: usize,
    origin_x: usize,
    origin_y: usize,
) -> Result<()> {
    let mut x = 0usize;
    let mut y = 0usize;
    for token in &record.unit.tokens {
        match token.kind {
            BonusShopTextSourceTokenKind::Glyph => {
                let code = token
                    .code
                    .as_deref()
                    .context("glyph token is missing its code")?;
                let code = u16::from_str_radix(
                    code.strip_prefix("0x")
                        .context("glyph code lost 0x prefix")?,
                    16,
                )?;
                let packed = packed_glyph_pixels(shop_ui_decoded, code)?;
                render_packed_glyph(&packed, output, output_width, origin_x + x, origin_y + y);
                x += GLYPH_WIDTH;
            }
            BonusShopTextSourceTokenKind::Blank => x += GLYPH_WIDTH,
            BonusShopTextSourceTokenKind::LineBreak => {
                x = 0;
                y += LINE_ADVANCE;
            }
            BonusShopTextSourceTokenKind::Terminator => {}
        }
    }
    Ok(())
}

fn render_packed_glyph(
    packed: &[u8],
    output: &mut [u8],
    output_width: usize,
    origin_x: usize,
    origin_y: usize,
) {
    for y in 0..GLYPH_HEIGHT {
        for x in 0..GLYPH_WIDTH {
            let byte = packed[y * (GLYPH_WIDTH / 2) + x / 2];
            let index = if x.is_multiple_of(2) {
                byte & 0x0f
            } else {
                byte >> 4
            };
            output[(origin_y + y) * output_width + origin_x + x] = 255 - index * 17;
        }
    }
}

fn scale_nearest(source: &[u8], width: usize, height: usize, scale: usize) -> Vec<u8> {
    let scaled_width = width * scale;
    let mut output = vec![0u8; scaled_width * height * scale];
    for y in 0..height {
        for x in 0..width {
            let value = source[y * width + x];
            for scaled_y in 0..scale {
                for scaled_x in 0..scale {
                    output[(y * scale + scaled_y) * scaled_width + x * scale + scaled_x] = value;
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
