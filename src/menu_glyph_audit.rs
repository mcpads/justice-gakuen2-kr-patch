use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};

#[path = "menu_glyph_audit/model.rs"]
mod model;

pub use model::{MenuGlyphAuditConfig, MenuGlyphAuditManifest, MenuGlyphCellAudit};

use crate::compression::decompress;
use crate::cue::CueSheet;
use crate::dialogue_audit::codebook::load_verified_dialogue_pixel_texts;
use crate::disc::rebuild;
use crate::menu_atlas::{
    MENU_ATLAS_HEIGHT, MENU_ATLAS_PAGE_WIDTH, MENU_GLYPH_CELL_HEIGHT, MENU_GLYPH_CELL_WIDTH,
    MENU_GLYPH_CODE_COUNT, logical_code_position,
};
use crate::pipeline::{
    BASELINE_BIN_SHA256, EMBEDDED_MOJI2_TIM_SIZE, ORIGINAL_MENU_DECODED_SHA256, sha256_bytes,
    sha256_file,
};
use crate::tim::parse_4bpp_prefix;

const MENU_PATH: &str = "DAT2/MENU.BIZ";

pub fn audit_menu_glyphs(config: &MenuGlyphAuditConfig) -> Result<MenuGlyphAuditManifest> {
    let manifest = collect_menu_glyph_audit(&config.cue, &config.dialogue_codebook)?;
    if let Some(parent) = config.output.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    std::fs::write(
        &config.output,
        format!("{}\n", serde_json::to_string_pretty(&manifest)?),
    )
    .with_context(|| format!("failed to write {}", config.output.display()))?;
    Ok(manifest)
}

pub fn collect_menu_glyph_audit(
    cue_path: &std::path::Path,
    dialogue_codebook_path: &std::path::Path,
) -> Result<MenuGlyphAuditManifest> {
    let cue = CueSheet::parse(cue_path)?;
    let source_bin_sha256 = sha256_file(&cue.image_path)?;
    ensure!(
        source_bin_sha256 == BASELINE_BIN_SHA256,
        "unsupported source BIN SHA-256: {source_bin_sha256}"
    );

    let (_, menu_stored) = rebuild::read_record(&cue.image_path, MENU_PATH)?;
    let menu_stored_sha256 = sha256_bytes(&menu_stored);
    let menu_decoded = decompress(&menu_stored, false)?;
    let menu_decoded_sha256 = sha256_bytes(&menu_decoded);
    ensure!(
        menu_decoded_sha256 == ORIGINAL_MENU_DECODED_SHA256,
        "MENU.BIZ decoded identity changed: {menu_decoded_sha256}"
    );
    let tim = parse_4bpp_prefix(&menu_decoded)?;
    ensure!(
        tim.total_size == EMBEDDED_MOJI2_TIM_SIZE,
        "MENU.BIZ embedded MOJI2 TIM size changed"
    );
    ensure!(
        tim.pixel_width() == 1024 && tim.image_height == 256,
        "MENU.BIZ embedded MOJI2 dimensions changed"
    );
    let image_pixels = &menu_decoded[tim.pixel_offset..tim.total_size];

    let (dialogue_codebook_sha256, dialogue_pixel_texts) = load_verified_dialogue_pixel_texts(
        &cue.image_path,
        dialogue_codebook_path,
        &source_bin_sha256,
    )?;
    let mut unique_physical_pixel_hashes = BTreeSet::new();
    let mut matched_physical_pixel_hashes = BTreeSet::new();
    let mut cells = Vec::with_capacity(MENU_GLYPH_CODE_COUNT);
    for code in 0..MENU_GLYPH_CODE_COUNT {
        let code = code as u16;
        let pixels = extract_wrapped_menu_cell(image_pixels, tim.row_bytes(), code)?;
        let pixel_sha256 = sha256_bytes(&pixels);
        unique_physical_pixel_hashes.insert(pixel_sha256.clone());
        let exact_dialogue_pixel_text = dialogue_pixel_texts.get(&pixel_sha256).cloned();
        if exact_dialogue_pixel_text.is_some() {
            matched_physical_pixel_hashes.insert(pixel_sha256.clone());
        }
        let (page, column, row, physical_x, physical_y) = physical_cell_position(code);
        cells.push(MenuGlyphCellAudit {
            code: format!("0x{code:04x}"),
            page,
            column,
            row,
            physical_x,
            physical_y,
            pixel_sha256,
            exact_dialogue_pixel_text,
        });
    }
    let exact_dialogue_pixel_match_count = cells
        .iter()
        .filter(|cell| cell.exact_dialogue_pixel_text.is_some())
        .count();
    let manifest = MenuGlyphAuditManifest {
        kind: "MENU.BIZ physical-cell exact dialogue-pixel cross-audit".to_string(),
        implementation: "Rust original-media packed-pixel comparison".to_string(),
        source_bin_sha256,
        dialogue_codebook_sha256,
        menu_path: MENU_PATH.to_string(),
        menu_stored_sha256,
        menu_decoded_sha256,
        embedded_tim_size: tim.total_size,
        image_width: tim.pixel_width(),
        image_height: tim.image_height,
        row_bytes: tim.row_bytes(),
        addressable_code_count: cells.len(),
        unique_physical_pixel_hash_count: unique_physical_pixel_hashes.len(),
        exact_dialogue_pixel_match_count,
        unique_exact_dialogue_pixel_match_count: matched_physical_pixel_hashes.len(),
        unmatched_code_count: cells.len() - exact_dialogue_pixel_match_count,
        cells,
        limitations: vec![
            "Only byte-exact packed 4-bpp pixel matches inherit dialogue-codebook text; visual similarity, palette remapping, and OCR are not admitted.".to_string(),
            "A decoded glyph does not prove that its code is referenced by a menu consumer; code usage remains owned by the separate menu-code audit.".to_string(),
            "Overlapping 20x20 cells use the renderer's byte-sized U/V wrapping inside each 256-pixel texture page; nominal grid coordinates are not treated as independent storage.".to_string(),
        ],
    };

    Ok(manifest)
}

fn extract_wrapped_menu_cell(image_pixels: &[u8], row_bytes: usize, code: u16) -> Result<Vec<u8>> {
    ensure!(
        usize::from(code) < MENU_GLYPH_CODE_COUNT,
        "menu code is outside 0x0000..0x03ff"
    );
    ensure!(
        image_pixels.len() == row_bytes * MENU_ATLAS_HEIGHT,
        "menu pixel plane dimensions changed"
    );
    let (page, _, _, start_x, start_y) = physical_cell_position(code);
    let page_x = usize::from(page) * MENU_ATLAS_PAGE_WIDTH;
    let mut packed = Vec::with_capacity(MENU_GLYPH_CELL_WIDTH * MENU_GLYPH_CELL_HEIGHT / 2);
    for local_y in 0..MENU_GLYPH_CELL_HEIGHT {
        let y = (start_y + local_y) & 0xff;
        for local_x in (0..MENU_GLYPH_CELL_WIDTH).step_by(2) {
            let left_x = page_x + ((start_x + local_x) & 0xff);
            let right_x = page_x + ((start_x + local_x + 1) & 0xff);
            let left = indexed_pixel(image_pixels, row_bytes, left_x, y);
            let right = indexed_pixel(image_pixels, row_bytes, right_x, y);
            packed.push(left | (right << 4));
        }
    }
    Ok(packed)
}

fn indexed_pixel(image_pixels: &[u8], row_bytes: usize, x: usize, y: usize) -> u8 {
    let byte = image_pixels[y * row_bytes + x / 2];
    (byte >> (4 * (x & 1))) & 0x0f
}

fn physical_cell_position(code: u16) -> (u8, u8, u8, usize, usize) {
    let position = logical_code_position(code).expect("audited MENU code is in range");
    (
        position.page,
        position.column,
        position.row,
        position.x % MENU_ATLAS_PAGE_WIDTH,
        position.y,
    )
}

#[cfg(test)]
#[path = "menu_glyph_audit_tests.rs"]
mod tests;
