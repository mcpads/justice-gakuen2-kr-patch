use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Cell {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

pub(crate) fn cells_overlap(left: Cell, right: Cell) -> bool {
    left.x < right.x + right.width
        && right.x < left.x + left.width
        && left.y < right.y + right.height
        && right.y < left.y + left.height
}

#[derive(Debug, Clone, Serialize)]
pub struct ImageMetadata {
    pub vram_x: u16,
    pub vram_y: u16,
    pub pixel_width: usize,
    pub height: usize,
    pub row_bytes: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClutMetadata {
    pub vram_x: u16,
    pub vram_y: u16,
    pub colors: usize,
    pub palette_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct EditMetadata {
    pub operation: String,
    pub target: String,
    pub cell: Cell,
    pub color_index: u8,
    pub thickness: usize,
    pub pixel_data_offset: usize,
    pub image: ImageMetadata,
    pub clut: ClutMetadata,
    pub allowed_decoded_byte_ranges: Vec<[usize; 2]>,
    pub changed_decoded_byte_count: usize,
    pub embedded_tim_prefix_size: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct GlyphInstallMetadata {
    pub operation: String,
    pub target: String,
    pub cell: Cell,
    pub palette_histogram: [usize; 16],
    pub pixel_data_offset: usize,
    pub image: ImageMetadata,
    pub clut: ClutMetadata,
    pub allowed_decoded_byte_ranges: Vec<[usize; 2]>,
    pub changed_decoded_byte_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IndexedImage {
    pub(crate) width: usize,
    pub(crate) height: usize,
    pub(crate) pixels: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RgbaImage {
    pub(crate) width: usize,
    pub(crate) height: usize,
    pub(crate) pixels: Vec<u8>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Tim4bpp {
    pub(crate) clut_x: u16,
    pub(crate) clut_y: u16,
    pub(crate) clut_width: usize,
    pub(crate) clut_height: usize,
    pub(crate) image_x: u16,
    pub(crate) image_y: u16,
    pub(crate) image_word_width: usize,
    pub(crate) image_height: usize,
    pub(crate) pixel_offset: usize,
    pub(crate) total_size: usize,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Tim4bppWithoutClut {
    pub(crate) image_x: u16,
    pub(crate) image_y: u16,
    pub(crate) image_word_width: usize,
    pub(crate) image_height: usize,
    pub(crate) pixel_offset: usize,
    pub(crate) total_size: usize,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Tim8bpp {
    pub(crate) clut_x: u16,
    pub(crate) clut_y: u16,
    pub(crate) clut_width: usize,
    pub(crate) clut_height: usize,
    pub(crate) image_x: u16,
    pub(crate) image_y: u16,
    pub(crate) image_word_width: usize,
    pub(crate) image_height: usize,
    pub(crate) pixel_offset: usize,
    pub(crate) total_size: usize,
}

impl Tim4bppWithoutClut {
    pub(crate) fn pixel_width(self) -> usize {
        self.image_word_width * 4
    }

    pub(crate) fn row_bytes(self) -> usize {
        self.image_word_width * 2
    }
}

impl Tim4bpp {
    pub(crate) fn pixel_width(self) -> usize {
        self.image_word_width * 4
    }

    pub(crate) fn row_bytes(self) -> usize {
        self.image_word_width * 2
    }
}

impl Tim8bpp {
    pub(crate) fn pixel_width(self) -> usize {
        self.image_word_width * 2
    }

    pub(crate) fn row_bytes(self) -> usize {
        self.image_word_width * 2
    }
}

pub fn overlay_options_heading_o(data: &mut [u8]) -> Result<EditMetadata> {
    overlay_x(
        data,
        Cell {
            x: 768,
            y: 0,
            width: 40,
            height: 40,
        },
        15,
        3,
        "large O katakana glyph used by the options screen heading",
    )
}

pub fn install_indexed_glyph(
    data: &mut [u8],
    cell: Cell,
    pixels: &[u8],
    target: &str,
) -> Result<GlyphInstallMetadata> {
    let tim = parse_4bpp(data)?;
    ensure!(cell.width > 0 && cell.height > 0, "empty glyph cell");
    ensure!(
        cell.x + cell.width <= tim.pixel_width() && cell.y + cell.height <= tim.image_height,
        "glyph cell is outside the TIM image"
    );
    ensure!(
        pixels.len() == cell.width * cell.height,
        "glyph pixel count does not match its cell"
    );
    ensure!(
        pixels.iter().all(|pixel| *pixel < 16),
        "glyph contains a non-4-bpp palette index"
    );

    let original = data.to_vec();
    let first_byte = cell.x / 2;
    let last_byte_exclusive = (cell.x + cell.width).div_ceil(2);
    let mut allowed = Vec::with_capacity(cell.height);
    for row in cell.y..cell.y + cell.height {
        let start = tim.pixel_offset + row * tim.row_bytes() + first_byte;
        allowed.push([start, start + last_byte_exclusive - first_byte]);
    }

    let mut palette_histogram = [0usize; 16];
    for local_y in 0..cell.height {
        for local_x in 0..cell.width {
            let color = pixels[local_y * cell.width + local_x];
            palette_histogram[usize::from(color)] += 1;
            set_pixel(data, tim, cell.x + local_x, cell.y + local_y, color);
        }
    }

    let changed_offsets: Vec<_> = original
        .iter()
        .zip(data.iter())
        .enumerate()
        .filter_map(|(offset, (before, after))| (before != after).then_some(offset))
        .collect();
    ensure!(
        !changed_offsets.is_empty(),
        "glyph install changed no bytes"
    );
    ensure!(
        changed_offsets.iter().all(|offset| allowed
            .iter()
            .any(|[start, end]| start <= offset && offset < end)),
        "glyph install changed a byte outside the declared cell"
    );

    Ok(GlyphInstallMetadata {
        operation: "replace one 20x20 atlas cell with an indexed glyph".to_string(),
        target: target.to_string(),
        cell,
        palette_histogram,
        pixel_data_offset: tim.pixel_offset,
        image: ImageMetadata {
            vram_x: tim.image_x,
            vram_y: tim.image_y,
            pixel_width: tim.pixel_width(),
            height: tim.image_height,
            row_bytes: tim.row_bytes(),
        },
        clut: ClutMetadata {
            vram_x: tim.clut_x,
            vram_y: tim.clut_y,
            colors: tim.clut_width * tim.clut_height,
            palette_count: tim.clut_width * tim.clut_height / 16,
        },
        allowed_decoded_byte_ranges: allowed,
        changed_decoded_byte_count: changed_offsets.len(),
    })
}

pub(crate) fn install_indexed_glyph_in_prefix(
    data: &mut [u8],
    tim_offset: usize,
    cell: Cell,
    pixels: &[u8],
    target: &str,
) -> Result<GlyphInstallMetadata> {
    let tim = parse_4bpp_prefix(
        data.get(tim_offset..)
            .ok_or_else(|| anyhow::anyhow!("TIM offset is outside the decoded asset"))?,
    )?;
    let tim_end = tim_offset + tim.total_size;
    let mut metadata = install_indexed_glyph(
        data.get_mut(tim_offset..tim_end)
            .ok_or_else(|| anyhow::anyhow!("embedded TIM is truncated"))?,
        cell,
        pixels,
        target,
    )?;
    metadata.pixel_data_offset += tim_offset;
    for range in &mut metadata.allowed_decoded_byte_ranges {
        range[0] += tim_offset;
        range[1] += tim_offset;
    }
    Ok(metadata)
}

pub(crate) fn read_indexed_cell_in_prefix(
    data: &[u8],
    tim_offset: usize,
    cell: Cell,
) -> Result<Vec<u8>> {
    let tim = parse_4bpp_prefix(
        data.get(tim_offset..)
            .ok_or_else(|| anyhow::anyhow!("TIM offset is outside the decoded asset"))?,
    )?;
    ensure!(cell.width > 0 && cell.height > 0, "empty indexed cell");
    ensure!(
        cell.x + cell.width <= tim.pixel_width() && cell.y + cell.height <= tim.image_height,
        "indexed cell is outside the TIM image"
    );
    let tim_data = &data[tim_offset..tim_offset + tim.total_size];
    let mut pixels = Vec::with_capacity(cell.width * cell.height);
    for y in cell.y..cell.y + cell.height {
        for x in cell.x..cell.x + cell.width {
            let offset = tim.pixel_offset + y * tim.row_bytes() + x / 2;
            let shift = 4 * (x & 1);
            pixels.push((tim_data[offset] >> shift) & 0x0f);
        }
    }
    Ok(pixels)
}

pub(crate) fn read_4bpp_indexed_image_in_prefix(
    data: &[u8],
    tim_offset: usize,
) -> Result<IndexedImage> {
    let tim_data = data
        .get(tim_offset..)
        .ok_or_else(|| anyhow::anyhow!("TIM offset is outside the decoded asset"))?;
    let tim = parse_4bpp_prefix(tim_data)?;
    let tim_data = &tim_data[..tim.total_size];
    let width = tim.pixel_width();
    let mut pixels = Vec::with_capacity(width * tim.image_height);
    for y in 0..tim.image_height {
        for x in 0..width {
            let offset = tim.pixel_offset + y * tim.row_bytes() + x / 2;
            let shift = 4 * (x & 1);
            pixels.push((tim_data[offset] >> shift) & 0x0f);
        }
    }
    Ok(IndexedImage {
        width,
        height: tim.image_height,
        pixels,
    })
}

pub(crate) fn read_4bpp_image_metadata_in_prefix(
    data: &[u8],
    tim_offset: usize,
) -> Result<ImageMetadata> {
    let tim = parse_4bpp_prefix(
        data.get(tim_offset..)
            .ok_or_else(|| anyhow::anyhow!("TIM offset is outside the decoded asset"))?,
    )?;
    Ok(ImageMetadata {
        vram_x: tim.image_x,
        vram_y: tim.image_y,
        pixel_width: tim.pixel_width(),
        height: tim.image_height,
        row_bytes: tim.row_bytes(),
    })
}

pub(crate) fn decode_4bpp_rgba_in_prefix(
    data: &[u8],
    tim_offset: usize,
    palette_index: usize,
) -> Result<RgbaImage> {
    let tim_data = data
        .get(tim_offset..)
        .ok_or_else(|| anyhow::anyhow!("TIM offset is outside the decoded asset"))?;
    let tim = parse_4bpp_prefix(tim_data)?;
    let palette_count = tim.clut_width * tim.clut_height / 16;
    ensure!(
        palette_index < palette_count,
        "4-bpp palette index is outside the TIM CLUT"
    );
    let palette_words = read_4bpp_palette_words_in_prefix(data, tim_offset, palette_index)?;
    let mut palette = [[0u8; 4]; 16];
    for (color, value) in palette.iter_mut().zip(palette_words) {
        color[0] = expand_five_bit_color((value & 0x1f) as u8);
        color[1] = expand_five_bit_color(((value >> 5) & 0x1f) as u8);
        color[2] = expand_five_bit_color(((value >> 10) & 0x1f) as u8);
        color[3] = if value == 0 { 0 } else { 255 };
    }

    let indexed = read_4bpp_indexed_image_in_prefix(data, tim_offset)?;
    let mut pixels = Vec::with_capacity(indexed.pixels.len() * 4);
    for index in indexed.pixels {
        pixels.extend_from_slice(&palette[usize::from(index)]);
    }
    Ok(RgbaImage {
        width: indexed.width,
        height: indexed.height,
        pixels,
    })
}

pub(crate) fn read_4bpp_palette_words_in_prefix(
    data: &[u8],
    tim_offset: usize,
    palette_index: usize,
) -> Result<[u16; 16]> {
    let tim_data = data
        .get(tim_offset..)
        .ok_or_else(|| anyhow::anyhow!("TIM offset is outside the decoded asset"))?;
    let tim = parse_4bpp_prefix(tim_data)?;
    let palette_count = tim.clut_width * tim.clut_height / 16;
    ensure!(
        palette_index < palette_count,
        "4-bpp palette index is outside the TIM CLUT"
    );
    let palette_offset = 20 + palette_index * 16 * 2;
    let mut words = [0u16; 16];
    for (index, word) in words.iter_mut().enumerate() {
        *word = u16_le(tim_data, palette_offset + index * 2)?;
    }
    Ok(words)
}

pub(crate) fn write_indexed_cell_in_prefix(
    data: &mut [u8],
    tim_offset: usize,
    cell: Cell,
    pixels: &[u8],
) -> Result<Vec<[usize; 2]>> {
    Ok(
        write_indexed_cell_in_prefix_internal(data, tim_offset, cell, pixels, false)?
            .allowed_ranges,
    )
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct IndexedCellWrite {
    pub(crate) allowed_ranges: Vec<[usize; 2]>,
    pub(crate) changed_byte_count: usize,
}

pub(crate) fn write_indexed_cell_in_prefix_with_report(
    data: &mut [u8],
    tim_offset: usize,
    cell: Cell,
    pixels: &[u8],
) -> Result<IndexedCellWrite> {
    write_indexed_cell_in_prefix_internal(data, tim_offset, cell, pixels, true)
}

fn write_indexed_cell_in_prefix_internal(
    data: &mut [u8],
    tim_offset: usize,
    cell: Cell,
    pixels: &[u8],
    count_changes: bool,
) -> Result<IndexedCellWrite> {
    let tim = parse_4bpp_prefix(
        data.get(tim_offset..)
            .ok_or_else(|| anyhow::anyhow!("TIM offset is outside the decoded asset"))?,
    )?;
    ensure_nonempty_cell_inside(cell, tim.pixel_width(), tim.image_height)?;
    ensure!(
        pixels.len() == cell.width * cell.height,
        "indexed pixel count does not match its cell"
    );
    ensure!(
        pixels.iter().all(|pixel| *pixel < 16),
        "indexed cell contains a non-4-bpp palette index"
    );
    let first_byte = cell.x / 2;
    let last_byte_exclusive = (cell.x + cell.width).div_ceil(2);
    let mut ranges = Vec::with_capacity(cell.height);
    let mut changed_byte_count = 0;
    for local_y in 0..cell.height {
        let row = cell.y + local_y;
        let start = tim_offset + tim.pixel_offset + row * tim.row_bytes() + first_byte;
        let end = start + last_byte_exclusive - first_byte;
        let before = count_changes.then(|| data[start..end].to_vec());
        for local_x in 0..cell.width {
            let x = cell.x + local_x;
            let offset = tim_offset + tim.pixel_offset + row * tim.row_bytes() + x / 2;
            let shift = 4 * (x & 1);
            let mask = 0x0f << shift;
            data[offset] =
                (data[offset] & !mask) | (pixels[local_y * cell.width + local_x] << shift);
        }
        if let Some(before) = before {
            changed_byte_count += before
                .iter()
                .zip(&data[start..end])
                .filter(|(left, right)| left != right)
                .count();
        }
        ranges.push([start, end]);
    }
    Ok(IndexedCellWrite {
        allowed_ranges: ranges,
        changed_byte_count,
    })
}

pub(crate) fn read_indexed_cell_without_clut_in_prefix(
    data: &[u8],
    tim_offset: usize,
    cell: Cell,
) -> Result<Vec<u8>> {
    let tim = parse_4bpp_without_clut_prefix(
        data.get(tim_offset..)
            .ok_or_else(|| anyhow::anyhow!("TIM offset is outside the decoded asset"))?,
    )?;
    read_indexed_cell_without_clut(data, tim_offset, tim, cell)
}

pub(crate) fn write_indexed_cell_without_clut_in_prefix(
    data: &mut [u8],
    tim_offset: usize,
    cell: Cell,
    pixels: &[u8],
) -> Result<Vec<[usize; 2]>> {
    Ok(
        write_indexed_cell_without_clut_in_prefix_internal(data, tim_offset, cell, pixels, false)?
            .allowed_ranges,
    )
}

pub(crate) fn write_indexed_cell_without_clut_in_prefix_with_report(
    data: &mut [u8],
    tim_offset: usize,
    cell: Cell,
    pixels: &[u8],
) -> Result<IndexedCellWrite> {
    write_indexed_cell_without_clut_in_prefix_internal(data, tim_offset, cell, pixels, true)
}

fn write_indexed_cell_without_clut_in_prefix_internal(
    data: &mut [u8],
    tim_offset: usize,
    cell: Cell,
    pixels: &[u8],
    count_changes: bool,
) -> Result<IndexedCellWrite> {
    let tim = parse_4bpp_without_clut_prefix(
        data.get(tim_offset..)
            .ok_or_else(|| anyhow::anyhow!("TIM offset is outside the decoded asset"))?,
    )?;
    ensure!(cell.width > 0 && cell.height > 0, "empty indexed cell");
    ensure!(
        cell.x + cell.width <= tim.pixel_width() && cell.y + cell.height <= tim.image_height,
        "indexed cell is outside the TIM image"
    );
    ensure!(
        pixels.len() == cell.width * cell.height,
        "indexed pixel count does not match its cell"
    );
    ensure!(
        pixels.iter().all(|pixel| *pixel < 16),
        "indexed cell contains a non-4-bpp palette index"
    );
    let first_byte = cell.x / 2;
    let last_byte_exclusive = (cell.x + cell.width).div_ceil(2);
    let mut ranges = Vec::with_capacity(cell.height);
    let mut changed_byte_count = 0;
    for local_y in 0..cell.height {
        let row = cell.y + local_y;
        let start = tim_offset + tim.pixel_offset + row * tim.row_bytes() + first_byte;
        let end = start + last_byte_exclusive - first_byte;
        let before = count_changes.then(|| data[start..end].to_vec());
        ranges.push([start, end]);
        for local_x in 0..cell.width {
            let offset =
                tim_offset + tim.pixel_offset + row * tim.row_bytes() + (cell.x + local_x) / 2;
            let shift = 4 * ((cell.x + local_x) & 1);
            let mask = 0x0f << shift;
            data[offset] =
                (data[offset] & !mask) | (pixels[local_y * cell.width + local_x] << shift);
        }
        if let Some(before) = before {
            changed_byte_count += before
                .iter()
                .zip(&data[start..end])
                .filter(|(left, right)| left != right)
                .count();
        }
    }
    Ok(IndexedCellWrite {
        allowed_ranges: ranges,
        changed_byte_count,
    })
}

pub(crate) fn read_8bpp_indexed_cell(data: &[u8], cell: Cell) -> Result<Vec<u8>> {
    let tim = parse_8bpp(data)?;
    read_8bpp_indexed_cell_with_tim(data, 0, tim, cell)
}

pub(crate) fn read_8bpp_indexed_cell_in_prefix(
    data: &[u8],
    tim_offset: usize,
    cell: Cell,
) -> Result<Vec<u8>> {
    let tim_data = data
        .get(tim_offset..)
        .ok_or_else(|| anyhow::anyhow!("8-bpp TIM offset is outside the decoded asset"))?;
    let tim = parse_8bpp_prefix(tim_data)?;
    read_8bpp_indexed_cell_with_tim(data, tim_offset, tim, cell)
}

pub(crate) fn read_8bpp_palette_words_in_prefix(
    data: &[u8],
    tim_offset: usize,
    palette_index: usize,
) -> Result<[u16; 256]> {
    let tim_data = data
        .get(tim_offset..)
        .ok_or_else(|| anyhow::anyhow!("8-bpp TIM offset is outside the decoded asset"))?;
    let tim = parse_8bpp_prefix(tim_data)?;
    let palette_count = tim.clut_width * tim.clut_height / 256;
    ensure!(
        palette_index < palette_count,
        "8-bpp palette index is outside the TIM CLUT"
    );
    let palette_offset = 20 + palette_index * 256 * 2;
    let mut words = [0u16; 256];
    for (index, word) in words.iter_mut().enumerate() {
        *word = u16_le(tim_data, palette_offset + index * 2)?;
    }
    Ok(words)
}

fn read_8bpp_indexed_cell_with_tim(
    data: &[u8],
    tim_offset: usize,
    tim: Tim8bpp,
    cell: Cell,
) -> Result<Vec<u8>> {
    ensure_nonempty_cell_inside(cell, tim.pixel_width(), tim.image_height)?;
    let mut pixels = Vec::with_capacity(cell.width * cell.height);
    for y in cell.y..cell.y + cell.height {
        let start = tim_offset + tim.pixel_offset + y * tim.row_bytes() + cell.x;
        pixels.extend_from_slice(&data[start..start + cell.width]);
    }
    Ok(pixels)
}

pub(crate) fn write_8bpp_indexed_cell(
    data: &mut [u8],
    cell: Cell,
    pixels: &[u8],
) -> Result<Vec<[usize; 2]>> {
    let tim = parse_8bpp(data)?;
    Ok(write_8bpp_indexed_cell_with_tim(data, 0, tim, cell, pixels, false)?.allowed_ranges)
}

#[cfg(test)]
pub(crate) fn write_8bpp_indexed_cell_in_prefix(
    data: &mut [u8],
    tim_offset: usize,
    cell: Cell,
    pixels: &[u8],
) -> Result<Vec<[usize; 2]>> {
    let tim_data = data
        .get(tim_offset..)
        .ok_or_else(|| anyhow::anyhow!("8-bpp TIM offset is outside the decoded asset"))?;
    let tim = parse_8bpp_prefix(tim_data)?;
    Ok(
        write_8bpp_indexed_cell_with_tim(data, tim_offset, tim, cell, pixels, false)?
            .allowed_ranges,
    )
}

pub(crate) fn write_8bpp_indexed_cell_in_prefix_with_report(
    data: &mut [u8],
    tim_offset: usize,
    cell: Cell,
    pixels: &[u8],
) -> Result<IndexedCellWrite> {
    let tim_data = data
        .get(tim_offset..)
        .ok_or_else(|| anyhow::anyhow!("8-bpp TIM offset is outside the decoded asset"))?;
    let tim = parse_8bpp_prefix(tim_data)?;
    write_8bpp_indexed_cell_with_tim(data, tim_offset, tim, cell, pixels, true)
}

fn write_8bpp_indexed_cell_with_tim(
    data: &mut [u8],
    tim_offset: usize,
    tim: Tim8bpp,
    cell: Cell,
    pixels: &[u8],
    count_changes: bool,
) -> Result<IndexedCellWrite> {
    ensure_nonempty_cell_inside(cell, tim.pixel_width(), tim.image_height)?;
    ensure!(
        pixels.len() == cell.width * cell.height,
        "8-bpp indexed pixel count does not match its cell"
    );
    let mut ranges = Vec::with_capacity(cell.height);
    let mut changed_byte_count = 0;
    for local_y in 0..cell.height {
        let start = tim_offset + tim.pixel_offset + (cell.y + local_y) * tim.row_bytes() + cell.x;
        let end = start + cell.width;
        if count_changes {
            changed_byte_count += data[start..end]
                .iter()
                .zip(&pixels[local_y * cell.width..(local_y + 1) * cell.width])
                .filter(|(left, right)| left != right)
                .count();
        }
        data[start..end].copy_from_slice(&pixels[local_y * cell.width..(local_y + 1) * cell.width]);
        ranges.push([start, end]);
    }
    Ok(IndexedCellWrite {
        allowed_ranges: ranges,
        changed_byte_count,
    })
}

fn overlay_x(
    data: &mut [u8],
    cell: Cell,
    color_index: u8,
    thickness: usize,
    target: &str,
) -> Result<EditMetadata> {
    let tim = parse_4bpp(data)?;
    ensure!(
        cell.width > 1 && cell.height > 1 && thickness > 0,
        "invalid probe size"
    );
    ensure!(
        cell.x + cell.width <= tim.pixel_width() && cell.y + cell.height <= tim.image_height,
        "probe cell is outside the TIM image"
    );
    ensure!(color_index < 16, "4-bpp color index is out of range");

    let original = data.to_vec();
    let first_byte = cell.x / 2;
    let last_byte_exclusive = (cell.x + cell.width).div_ceil(2);
    let mut allowed = Vec::with_capacity(cell.height);
    for row in cell.y..cell.y + cell.height {
        let start = tim.pixel_offset + row * tim.row_bytes() + first_byte;
        allowed.push([start, start + last_byte_exclusive - first_byte]);
    }

    for local_y in 0..cell.height {
        let numerator = local_y * (cell.width - 1);
        let denominator = cell.height - 1;
        let forward = (numerator + denominator / 2) / denominator;
        let backward = cell.width - 1 - forward;
        for base in [forward, backward] {
            for spread in 0..thickness {
                let local_x = base + spread;
                if local_x < cell.width {
                    set_pixel(data, tim, cell.x + local_x, cell.y + local_y, color_index);
                }
            }
        }
    }

    let changed_offsets: Vec<_> = original
        .iter()
        .zip(data.iter())
        .enumerate()
        .filter_map(|(offset, (before, after))| (before != after).then_some(offset))
        .collect();
    ensure!(
        !changed_offsets.is_empty(),
        "probe overlay changed no bytes"
    );
    ensure!(
        changed_offsets.iter().all(|offset| allowed
            .iter()
            .any(|[start, end]| start <= offset && offset < end)),
        "probe changed a byte outside the declared cell"
    );

    Ok(EditMetadata {
        operation: "overlay X in one atlas cell".to_string(),
        target: target.to_string(),
        cell,
        color_index,
        thickness,
        pixel_data_offset: tim.pixel_offset,
        image: ImageMetadata {
            vram_x: tim.image_x,
            vram_y: tim.image_y,
            pixel_width: tim.pixel_width(),
            height: tim.image_height,
            row_bytes: tim.row_bytes(),
        },
        clut: ClutMetadata {
            vram_x: tim.clut_x,
            vram_y: tim.clut_y,
            colors: tim.clut_width * tim.clut_height,
            palette_count: tim.clut_width * tim.clut_height / 16,
        },
        allowed_decoded_byte_ranges: allowed,
        changed_decoded_byte_count: changed_offsets.len(),
        embedded_tim_prefix_size: data.len(),
    })
}

fn set_pixel(data: &mut [u8], tim: Tim4bpp, x: usize, y: usize, color: u8) {
    let offset = tim.pixel_offset + y * tim.row_bytes() + x / 2;
    let shift = 4 * (x & 1);
    let mask = 0x0f << shift;
    data[offset] = (data[offset] & !mask) | (color << shift);
}

fn parse_4bpp(data: &[u8]) -> Result<Tim4bpp> {
    let tim = parse_4bpp_prefix(data)?;
    ensure!(
        tim.total_size == data.len(),
        "unexpected bytes after TIM image"
    );
    Ok(tim)
}

pub(crate) fn parse_8bpp(data: &[u8]) -> Result<Tim8bpp> {
    let tim = parse_8bpp_prefix(data)?;
    ensure!(
        tim.total_size == data.len(),
        "unexpected bytes after 8-bpp TIM image"
    );
    Ok(tim)
}

pub(crate) fn parse_8bpp_prefix(data: &[u8]) -> Result<Tim8bpp> {
    ensure!(data.len() >= 20, "truncated TIM header");
    ensure!(u32_le(data, 0)? == 0x10, "invalid TIM magic");
    ensure!(u32_le(data, 4)? == 0x09, "expected CLUT-bearing 8-bpp TIM");

    let (clut_size, clut_x, clut_y, clut_width, clut_height) = block(data, 8)?;
    ensure!(clut_width > 0 && clut_height > 0, "empty TIM CLUT");
    ensure!(
        clut_size == 12 + clut_width * clut_height * 2,
        "TIM CLUT dimensions do not match block size"
    );
    ensure!(
        (clut_width * clut_height).is_multiple_of(256),
        "8-bpp TIM CLUT does not contain whole palettes"
    );

    let image_offset = 8 + clut_size;
    let (image_size, image_x, image_y, image_word_width, image_height) = block(data, image_offset)?;
    ensure!(image_word_width > 0 && image_height > 0, "empty TIM image");
    ensure!(
        image_size == 12 + image_word_width * image_height * 2,
        "TIM image dimensions do not match block size"
    );

    Ok(Tim8bpp {
        clut_x,
        clut_y,
        clut_width,
        clut_height,
        image_x,
        image_y,
        image_word_width,
        image_height,
        pixel_offset: image_offset + 12,
        total_size: image_offset + image_size,
    })
}

fn ensure_nonempty_cell_inside(cell: Cell, width: usize, height: usize) -> Result<()> {
    ensure!(cell.width > 0 && cell.height > 0, "empty indexed cell");
    ensure!(
        cell.x + cell.width <= width && cell.y + cell.height <= height,
        "indexed cell is outside the TIM image"
    );
    Ok(())
}

pub(crate) fn parse_4bpp_prefix(data: &[u8]) -> Result<Tim4bpp> {
    ensure!(data.len() >= 20, "truncated TIM header");
    ensure!(u32_le(data, 0)? == 0x10, "invalid TIM magic");
    ensure!(u32_le(data, 4)? == 0x08, "expected CLUT-bearing 4-bpp TIM");

    let (clut_size, clut_x, clut_y, clut_width, clut_height) = block(data, 8)?;
    ensure!(clut_width > 0 && clut_height > 0, "empty TIM CLUT");
    ensure!(
        clut_size == 12 + clut_width * clut_height * 2,
        "TIM CLUT dimensions do not match block size"
    );
    ensure!(
        (clut_width * clut_height).is_multiple_of(16),
        "TIM CLUT does not contain whole palettes"
    );

    let image_offset = 8 + clut_size;
    let (image_size, image_x, image_y, image_word_width, image_height) = block(data, image_offset)?;
    ensure!(image_word_width > 0 && image_height > 0, "empty TIM image");
    ensure!(
        image_size == 12 + image_word_width * image_height * 2,
        "TIM image dimensions do not match block size"
    );
    let total_size = image_offset + image_size;

    Ok(Tim4bpp {
        clut_x,
        clut_y,
        clut_width,
        clut_height,
        image_x,
        image_y,
        image_word_width,
        image_height,
        pixel_offset: image_offset + 12,
        total_size,
    })
}

pub(crate) fn parse_4bpp_without_clut_prefix(data: &[u8]) -> Result<Tim4bppWithoutClut> {
    ensure!(data.len() >= 20, "truncated TIM header");
    ensure!(u32_le(data, 0)? == 0x10, "invalid TIM magic");
    ensure!(u32_le(data, 4)? == 0, "expected 4-bpp TIM without a CLUT");
    let (image_size, image_x, image_y, image_word_width, image_height) = block(data, 8)?;
    ensure!(image_word_width > 0 && image_height > 0, "empty TIM image");
    ensure!(
        image_size == 12 + image_word_width * image_height * 2,
        "TIM image dimensions do not match block size"
    );
    Ok(Tim4bppWithoutClut {
        image_x,
        image_y,
        image_word_width,
        image_height,
        pixel_offset: 20,
        total_size: 8 + image_size,
    })
}

fn read_indexed_cell_without_clut(
    data: &[u8],
    tim_offset: usize,
    tim: Tim4bppWithoutClut,
    cell: Cell,
) -> Result<Vec<u8>> {
    ensure!(cell.width > 0 && cell.height > 0, "empty indexed cell");
    ensure!(
        cell.x + cell.width <= tim.pixel_width() && cell.y + cell.height <= tim.image_height,
        "indexed cell is outside the TIM image"
    );
    ensure!(
        tim_offset + tim.total_size <= data.len(),
        "embedded TIM is truncated"
    );
    let mut pixels = Vec::with_capacity(cell.width * cell.height);
    for y in cell.y..cell.y + cell.height {
        for x in cell.x..cell.x + cell.width {
            let offset = tim_offset + tim.pixel_offset + y * tim.row_bytes() + x / 2;
            let shift = 4 * (x & 1);
            pixels.push((data[offset] >> shift) & 0x0f);
        }
    }
    Ok(pixels)
}

fn block(data: &[u8], offset: usize) -> Result<(usize, u16, u16, usize, usize)> {
    ensure!(offset + 12 <= data.len(), "truncated TIM block header");
    let size = u32_le(data, offset)? as usize;
    ensure!(
        size >= 12 && offset + size <= data.len(),
        "invalid TIM block size"
    );
    Ok((
        size,
        u16_le(data, offset + 4)?,
        u16_le(data, offset + 6)?,
        usize::from(u16_le(data, offset + 8)?),
        usize::from(u16_le(data, offset + 10)?),
    ))
}

fn u16_le(data: &[u8], offset: usize) -> Result<u16> {
    ensure!(offset + 2 <= data.len(), "truncated u16");
    Ok(u16::from_le_bytes(data[offset..offset + 2].try_into()?))
}

fn u32_le(data: &[u8], offset: usize) -> Result<u32> {
    ensure!(offset + 4 <= data.len(), "truncated u32");
    Ok(u32::from_le_bytes(data[offset..offset + 4].try_into()?))
}

fn expand_five_bit_color(value: u8) -> u8 {
    (value << 3) | (value >> 2)
}

#[cfg(test)]
#[path = "tim_tests.rs"]
mod tests;
