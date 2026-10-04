use anyhow::{Result, ensure};

use crate::pipeline::sha256_bytes;
use crate::tim::parse_4bpp_prefix;

use super::parser::SELECTOR_TABLE_OFFSET;

pub(super) const GLYPH_CELL_WIDTH: usize = 20;
pub(super) const GLYPH_CELL_HEIGHT: usize = 20;
pub(super) const GLYPH_CELL_BYTE_COUNT: usize = GLYPH_CELL_WIDTH * GLYPH_CELL_HEIGHT / 2;

#[derive(Debug, PartialEq, Eq)]
pub(super) struct ParsedDialogueAtlas {
    pub(super) tim_size: usize,
    pub(super) clut_vram_x: u16,
    pub(super) clut_vram_y: u16,
    pub(super) clut_color_count: usize,
    pub(super) image_vram_x: u16,
    pub(super) image_vram_y: u16,
    pub(super) pixel_data_offset: usize,
    pub(super) fixed_cell_count: usize,
    pub(super) addressable_slot_count: usize,
    pub(super) source_extension_start: usize,
    pub(super) source_extension_end: usize,
    pub(super) source_extension_nonzero_byte_count: usize,
    pub(super) source_extension_sha256: String,
    pub(super) fixed_atlas_sha256: String,
    pub(super) fixed_cell_sha256: Vec<String>,
}

pub(super) fn parse_dialogue_atlas(decoded: &[u8]) -> Result<ParsedDialogueAtlas> {
    let tim = parse_4bpp_prefix(decoded)?;
    ensure!(
        tim.pixel_width() == GLYPH_CELL_WIDTH,
        "dialogue atlas is not one 20-pixel-wide glyph column"
    );
    ensure!(
        tim.image_height.is_multiple_of(GLYPH_CELL_HEIGHT),
        "dialogue atlas height does not contain whole 20-pixel glyph cells"
    );
    ensure!(
        tim.row_bytes() * GLYPH_CELL_HEIGHT == GLYPH_CELL_BYTE_COUNT,
        "dialogue atlas cell byte size is not 200 bytes"
    );
    ensure!(
        tim.total_size <= SELECTOR_TABLE_OFFSET,
        "dialogue atlas overlaps the selector table"
    );

    let fixed_cell_count = tim.image_height / GLYPH_CELL_HEIGHT;
    let addressable_slot_count = (SELECTOR_TABLE_OFFSET - tim.pixel_offset) / GLYPH_CELL_BYTE_COUNT;
    ensure!(
        addressable_slot_count >= fixed_cell_count,
        "dialogue atlas has fewer addressable slots than fixed TIM cells"
    );

    let fixed_pixel_end = tim.pixel_offset + fixed_cell_count * GLYPH_CELL_BYTE_COUNT;
    ensure!(
        fixed_pixel_end == tim.total_size,
        "dialogue atlas pixels are not contiguous 20x20 cells"
    );
    let fixed_pixels = &decoded[tim.pixel_offset..fixed_pixel_end];
    let fixed_cell_sha256 = fixed_pixels
        .as_chunks::<GLYPH_CELL_BYTE_COUNT>()
        .0
        .iter()
        .map(|bytes| sha256_bytes(bytes))
        .collect();
    let source_extension = &decoded[tim.total_size..SELECTOR_TABLE_OFFSET];

    Ok(ParsedDialogueAtlas {
        tim_size: tim.total_size,
        clut_vram_x: tim.clut_x,
        clut_vram_y: tim.clut_y,
        clut_color_count: tim.clut_width * tim.clut_height,
        image_vram_x: tim.image_x,
        image_vram_y: tim.image_y,
        pixel_data_offset: tim.pixel_offset,
        fixed_cell_count,
        addressable_slot_count,
        source_extension_start: tim.total_size,
        source_extension_end: SELECTOR_TABLE_OFFSET,
        source_extension_nonzero_byte_count: source_extension
            .iter()
            .filter(|byte| **byte != 0)
            .count(),
        source_extension_sha256: sha256_bytes(source_extension),
        fixed_atlas_sha256: sha256_bytes(fixed_pixels),
        fixed_cell_sha256,
    })
}
