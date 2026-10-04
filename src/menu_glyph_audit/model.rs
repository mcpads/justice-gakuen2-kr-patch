use std::path::PathBuf;

use serde::Serialize;

#[derive(Debug, Clone)]
pub struct MenuGlyphAuditConfig {
    pub cue: PathBuf,
    pub dialogue_codebook: PathBuf,
    pub output: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
pub struct MenuGlyphCellAudit {
    pub code: String,
    pub page: u8,
    pub column: u8,
    pub row: u8,
    pub physical_x: usize,
    pub physical_y: usize,
    pub pixel_sha256: String,
    pub exact_dialogue_pixel_text: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MenuGlyphAuditManifest {
    pub kind: String,
    pub implementation: String,
    pub source_bin_sha256: String,
    pub dialogue_codebook_sha256: String,
    pub menu_path: String,
    pub menu_stored_sha256: String,
    pub menu_decoded_sha256: String,
    pub embedded_tim_size: usize,
    pub image_width: usize,
    pub image_height: usize,
    pub row_bytes: usize,
    pub addressable_code_count: usize,
    pub unique_physical_pixel_hash_count: usize,
    pub exact_dialogue_pixel_match_count: usize,
    pub unique_exact_dialogue_pixel_match_count: usize,
    pub unmatched_code_count: usize,
    pub cells: Vec<MenuGlyphCellAudit>,
    pub limitations: Vec<String>,
}
