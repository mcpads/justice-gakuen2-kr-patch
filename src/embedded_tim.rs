use anyhow::{Context, Result, bail, ensure};
use serde::Serialize;

use crate::pipeline::sha256_bytes;
use crate::tim::{
    Cell, RgbaImage, decode_4bpp_rgba_in_prefix, parse_4bpp_prefix, parse_4bpp_without_clut_prefix,
    parse_8bpp_prefix, read_8bpp_indexed_cell_in_prefix, read_8bpp_palette_words_in_prefix,
    read_indexed_cell_without_clut_in_prefix,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EmbeddedTimAudit {
    pub offset: usize,
    pub bits_per_pixel: u8,
    pub total_size: usize,
    pub source_tim_sha256: String,
    pub pixel_width: usize,
    pub pixel_height: usize,
    pub image_vram_word_x: u16,
    pub image_vram_y: u16,
    pub clut_vram_x: u16,
    pub clut_vram_y: u16,
    pub palette_count: usize,
    pub preview_file: String,
    pub preview_sha256: String,
}

pub(crate) fn parse_embedded_tim_at(decoded: &[u8], offset: usize) -> Result<EmbeddedTimAudit> {
    let prefix = decoded
        .get(offset..)
        .with_context(|| format!("embedded TIM offset {offset:#x} escaped its record"))?;
    let header = prefix
        .get(..8)
        .with_context(|| format!("embedded TIM header at {offset:#x} is truncated"))?;
    ensure!(
        header[..4] == 0x10u32.to_le_bytes(),
        "embedded TIM at {offset:#x} has invalid magic"
    );
    let flags = u32::from_le_bytes(header[4..8].try_into()?);
    let audit = match flags {
        0x00 => {
            let tim = parse_4bpp_without_clut_prefix(prefix)?;
            EmbeddedTimAudit {
                offset,
                bits_per_pixel: 4,
                total_size: tim.total_size,
                source_tim_sha256: source_tim_sha256(prefix, tim.total_size)?,
                pixel_width: tim.pixel_width(),
                pixel_height: tim.image_height,
                image_vram_word_x: tim.image_x,
                image_vram_y: tim.image_y,
                clut_vram_x: 0,
                clut_vram_y: 0,
                palette_count: 0,
                preview_file: String::new(),
                preview_sha256: String::new(),
            }
        }
        0x08 => {
            let tim = parse_4bpp_prefix(prefix)?;
            EmbeddedTimAudit {
                offset,
                bits_per_pixel: 4,
                total_size: tim.total_size,
                source_tim_sha256: source_tim_sha256(prefix, tim.total_size)?,
                pixel_width: tim.pixel_width(),
                pixel_height: tim.image_height,
                image_vram_word_x: tim.image_x,
                image_vram_y: tim.image_y,
                clut_vram_x: tim.clut_x,
                clut_vram_y: tim.clut_y,
                palette_count: tim.clut_width * tim.clut_height / 16,
                preview_file: String::new(),
                preview_sha256: String::new(),
            }
        }
        0x09 => {
            let tim = parse_8bpp_prefix(prefix)?;
            EmbeddedTimAudit {
                offset,
                bits_per_pixel: 8,
                total_size: tim.total_size,
                source_tim_sha256: source_tim_sha256(prefix, tim.total_size)?,
                pixel_width: tim.pixel_width(),
                pixel_height: tim.image_height,
                image_vram_word_x: tim.image_x,
                image_vram_y: tim.image_y,
                clut_vram_x: tim.clut_x,
                clut_vram_y: tim.clut_y,
                palette_count: tim.clut_width * tim.clut_height / 256,
                preview_file: String::new(),
                preview_sha256: String::new(),
            }
        }
        _ => bail!("embedded TIM at {offset:#x} has unsupported flags {flags:#x}"),
    };
    Ok(audit)
}

pub(crate) fn detect_embedded_tim_images(decoded: &[u8]) -> Vec<EmbeddedTimAudit> {
    let mut tims = Vec::new();
    let mut offset = 0usize;
    while offset + 8 <= decoded.len() {
        if decoded[offset..offset + 4] != 0x10u32.to_le_bytes() {
            offset += 1;
            continue;
        }
        let candidate = parse_embedded_tim_at(decoded, offset).ok();
        if let Some(tim) = candidate {
            offset += tim.total_size;
            tims.push(tim);
        } else {
            offset += 1;
        }
    }
    tims
}

fn source_tim_sha256(prefix: &[u8], total_size: usize) -> Result<String> {
    let bytes = prefix
        .get(..total_size)
        .context("embedded TIM parser returned an out-of-bounds size")?;
    Ok(sha256_bytes(bytes))
}

pub(crate) fn decode_embedded_tim_preview(
    decoded: &[u8],
    tim: &EmbeddedTimAudit,
) -> Result<RgbaImage> {
    match (tim.bits_per_pixel, tim.palette_count) {
        (4, 0) => decode_4bpp_index_preview_without_clut(decoded, tim),
        (4, _) => decode_4bpp_rgba_in_prefix(decoded, tim.offset, 0),
        (8, _) => decode_8bpp_rgba(decoded, tim),
        (bits, _) => bail!("unsupported embedded TIM depth {bits}"),
    }
}

fn decode_4bpp_index_preview_without_clut(
    decoded: &[u8],
    tim: &EmbeddedTimAudit,
) -> Result<RgbaImage> {
    let indexed = read_indexed_cell_without_clut_in_prefix(
        decoded,
        tim.offset,
        Cell {
            x: 0,
            y: 0,
            width: tim.pixel_width,
            height: tim.pixel_height,
        },
    )?;
    let mut pixels = Vec::with_capacity(indexed.len() * 4);
    for index in indexed {
        let value = index * 17;
        pixels.extend_from_slice(&[value, value, value, 255]);
    }
    Ok(RgbaImage {
        width: tim.pixel_width,
        height: tim.pixel_height,
        pixels,
    })
}

fn decode_8bpp_rgba(decoded: &[u8], tim: &EmbeddedTimAudit) -> Result<RgbaImage> {
    let palette = read_8bpp_palette_words_in_prefix(decoded, tim.offset, 0)?;
    let indexed = read_8bpp_indexed_cell_in_prefix(
        decoded,
        tim.offset,
        Cell {
            x: 0,
            y: 0,
            width: tim.pixel_width,
            height: tim.pixel_height,
        },
    )?;
    let mut pixels = Vec::with_capacity(indexed.len() * 4);
    for index in indexed {
        let value = palette[usize::from(index)];
        pixels.extend_from_slice(&[
            expand_five_bit_color((value & 0x1f) as u8),
            expand_five_bit_color(((value >> 5) & 0x1f) as u8),
            expand_five_bit_color(((value >> 10) & 0x1f) as u8),
            if value == 0 { 0 } else { 255 },
        ]);
    }
    Ok(RgbaImage {
        width: tim.pixel_width,
        height: tim.pixel_height,
        pixels,
    })
}

fn expand_five_bit_color(value: u8) -> u8 {
    (value << 3) | (value >> 2)
}

#[cfg(test)]
#[path = "embedded_tim_tests.rs"]
mod tests;
