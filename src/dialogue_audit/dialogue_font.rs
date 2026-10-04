use anyhow::{Context, Result, ensure};
use serde::Serialize;

use crate::pipeline::sha256_bytes;

use super::atlas::{
    GLYPH_CELL_BYTE_COUNT, GLYPH_CELL_HEIGHT, GLYPH_CELL_WIDTH, ParsedDialogueAtlas,
    parse_dialogue_atlas,
};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DialogueExtensionGlyphInstall {
    pub code: u16,
    pub decoded_byte_range: [usize; 2],
    pub changed_decoded_byte_count: usize,
    pub cell_sha256: String,
}

pub fn install_dialogue_allocated_glyph(
    decoded: &mut [u8],
    code: u16,
    pixels: &[u8],
    expected_source_cell_sha256: Option<&str>,
) -> Result<DialogueExtensionGlyphInstall> {
    let atlas = parse_dialogue_atlas(decoded)?;
    install_dialogue_allocated_glyph_in_atlas(
        decoded,
        &atlas,
        code,
        pixels,
        expected_source_cell_sha256,
    )
}

pub(super) fn install_dialogue_allocated_glyph_in_atlas(
    decoded: &mut [u8],
    source_atlas: &ParsedDialogueAtlas,
    code: u16,
    pixels: &[u8],
    expected_source_cell_sha256: Option<&str>,
) -> Result<DialogueExtensionGlyphInstall> {
    validate_pixels(pixels)?;
    let mut packed = [0u8; GLYPH_CELL_BYTE_COUNT];
    for (target, pair) in packed.iter_mut().zip(pixels.as_chunks::<2>().0) {
        *target = pair[0] | (pair[1] << 4);
    }
    install_dialogue_cell_bytes_in_atlas(
        decoded,
        source_atlas,
        code,
        &packed,
        expected_source_cell_sha256,
    )
}

#[cfg(test)]
pub(super) fn install_dialogue_cell_bytes(
    decoded: &mut [u8],
    code: u16,
    bytes: &[u8],
    expected_source_cell_sha256: Option<&str>,
) -> Result<DialogueExtensionGlyphInstall> {
    let atlas = parse_dialogue_atlas(decoded)?;
    install_dialogue_cell_bytes_in_atlas(decoded, &atlas, code, bytes, expected_source_cell_sha256)
}

pub(super) fn install_dialogue_cell_bytes_in_atlas(
    decoded: &mut [u8],
    source_atlas: &ParsedDialogueAtlas,
    code: u16,
    bytes: &[u8],
    expected_source_cell_sha256: Option<&str>,
) -> Result<DialogueExtensionGlyphInstall> {
    ensure!(
        bytes.len() == GLYPH_CELL_BYTE_COUNT,
        "dialogue cell payload must contain exactly 200 bytes"
    );
    let code_index = usize::from(code);
    ensure!(
        code_index < source_atlas.addressable_slot_count,
        "dialogue cell code is outside the renderer-addressable atlas"
    );
    let start = source_atlas.pixel_data_offset + code_index * GLYPH_CELL_BYTE_COUNT;
    let end = start + GLYPH_CELL_BYTE_COUNT;
    ensure!(end <= decoded.len(), "dialogue cell is truncated");
    if code_index < source_atlas.fixed_cell_count {
        let expected = expected_source_cell_sha256
            .context("fixed dialogue cell install lacks its source-cell binding")?;
        ensure!(
            source_atlas.fixed_cell_sha256[code_index] == expected,
            "fixed dialogue cell source-cell binding changed"
        );
    } else {
        ensure!(
            expected_source_cell_sha256.is_none(),
            "empty dialogue extension cell unexpectedly has a source-cell binding"
        );
        ensure!(
            decoded[start..end].iter().all(|byte| *byte == 0),
            "dialogue extension cell is already occupied"
        );
    }
    decoded[start..end].copy_from_slice(bytes);
    Ok(DialogueExtensionGlyphInstall {
        code,
        decoded_byte_range: [start, end],
        changed_decoded_byte_count: bytes.iter().filter(|byte| **byte != 0).count(),
        cell_sha256: sha256_bytes(bytes),
    })
}

pub fn install_dialogue_extension_glyph(
    decoded: &mut [u8],
    code: u16,
    pixels: &[u8],
) -> Result<DialogueExtensionGlyphInstall> {
    validate_pixels(pixels)?;

    let atlas = parse_dialogue_atlas(decoded)?;
    let code_index = usize::from(code);
    ensure!(
        code_index >= atlas.fixed_cell_count,
        "dialogue glyph code belongs to the fixed source atlas"
    );
    install_dialogue_allocated_glyph_in_atlas(decoded, &atlas, code, pixels, None)
}

fn validate_pixels(pixels: &[u8]) -> Result<()> {
    ensure!(
        pixels.len() == GLYPH_CELL_WIDTH * GLYPH_CELL_HEIGHT,
        "dialogue glyph must contain exactly one 20x20 cell"
    );
    ensure!(
        pixels.iter().all(|pixel| *pixel < 16),
        "dialogue glyph contains a pixel outside 4-bpp"
    );
    ensure!(
        pixels.iter().any(|pixel| *pixel != 0),
        "dialogue glyph contains no visible pixels"
    );
    Ok(())
}
