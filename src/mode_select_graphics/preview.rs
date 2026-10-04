use std::path::Path;

use anyhow::{Result, bail};

use crate::pipeline::sha256_file;
use crate::tim::{
    Cell, RgbaImage, decode_4bpp_rgba_in_prefix, read_8bpp_indexed_cell_in_prefix,
    read_8bpp_palette_words_in_prefix, read_indexed_cell_without_clut_in_prefix,
};
use crate::tim_preview::write_tim_preview;

use super::model::ModeSelectTimAudit;
use super::tim16::parse_16bpp_prefix;

pub(super) fn write_tim_previews(
    decoded: &[u8],
    tims: &mut [ModeSelectTimAudit],
    output_dir: &Path,
) -> Result<()> {
    for (index, tim) in tims.iter_mut().enumerate() {
        let preview_file = format!(
            "menu-tim-{:02}-{:05x}-{}bpp.png",
            index, tim.offset, tim.bits_per_pixel
        );
        let preview_path = output_dir.join(&preview_file);
        let rgba = decode_tim_preview(decoded, tim)?;
        write_tim_preview(&preview_path, &rgba)?;
        tim.preview_sha256 = sha256_file(&preview_path)?;
        tim.preview_file = preview_file;
    }
    Ok(())
}

fn decode_tim_preview(decoded: &[u8], tim: &ModeSelectTimAudit) -> Result<RgbaImage> {
    match (tim.bits_per_pixel, tim.has_clut) {
        (4, true) => decode_4bpp_rgba_in_prefix(decoded, tim.offset, 0),
        (4, false) => decode_4bpp_without_clut_preview(decoded, tim),
        (8, true) => decode_8bpp_rgba(decoded, tim),
        (16, false) => decode_16bpp_rgba(decoded, tim),
        _ => bail!(
            "unsupported MODE SELECT TIM format: {} bpp, CLUT {}",
            tim.bits_per_pixel,
            tim.has_clut
        ),
    }
}

fn decode_4bpp_without_clut_preview(decoded: &[u8], tim: &ModeSelectTimAudit) -> Result<RgbaImage> {
    let indexed = read_indexed_cell_without_clut_in_prefix(decoded, tim.offset, full_cell(tim))?;
    let mut pixels = Vec::with_capacity(indexed.len() * 4);
    for index in indexed {
        let value = index.saturating_mul(17);
        pixels.extend_from_slice(&[value, value, value, if index == 0 { 0 } else { 255 }]);
    }
    Ok(RgbaImage {
        width: tim.pixel_width,
        height: tim.pixel_height,
        pixels,
    })
}

fn decode_8bpp_rgba(decoded: &[u8], tim: &ModeSelectTimAudit) -> Result<RgbaImage> {
    let palette = read_8bpp_palette_words_in_prefix(decoded, tim.offset, 0)?;
    let indexed = read_8bpp_indexed_cell_in_prefix(decoded, tim.offset, full_cell(tim))?;
    let mut pixels = Vec::with_capacity(indexed.len() * 4);
    for index in indexed {
        pixels.extend_from_slice(&ps1_color(palette[usize::from(index)]));
    }
    Ok(RgbaImage {
        width: tim.pixel_width,
        height: tim.pixel_height,
        pixels,
    })
}

fn decode_16bpp_rgba(decoded: &[u8], tim: &ModeSelectTimAudit) -> Result<RgbaImage> {
    let parsed = parse_16bpp_prefix(&decoded[tim.offset..])?;
    let pixel_bytes = &decoded[tim.offset + parsed.pixel_offset
        ..tim.offset + parsed.pixel_offset + parsed.pixel_width * parsed.pixel_height * 2];
    let mut pixels = Vec::with_capacity(parsed.pixel_width * parsed.pixel_height * 4);
    for bytes in pixel_bytes.as_chunks::<2>().0 {
        pixels.extend_from_slice(&ps1_color(u16::from_le_bytes([bytes[0], bytes[1]])));
    }
    Ok(RgbaImage {
        width: parsed.pixel_width,
        height: parsed.pixel_height,
        pixels,
    })
}

fn full_cell(tim: &ModeSelectTimAudit) -> Cell {
    Cell {
        x: 0,
        y: 0,
        width: tim.pixel_width,
        height: tim.pixel_height,
    }
}

fn ps1_color(value: u16) -> [u8; 4] {
    [
        expand_five_bit_color((value & 0x1f) as u8),
        expand_five_bit_color(((value >> 5) & 0x1f) as u8),
        expand_five_bit_color(((value >> 10) & 0x1f) as u8),
        if value == 0 { 0 } else { 255 },
    ]
}

fn expand_five_bit_color(value: u8) -> u8 {
    (value << 3) | (value >> 2)
}
