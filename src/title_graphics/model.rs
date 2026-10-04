use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::tim::Cell;

#[derive(Debug, Clone)]
pub struct TitleGraphicsAuditConfig {
    pub cue: PathBuf,
    pub assets: PathBuf,
    pub gpu_dump: PathBuf,
    pub runtime_frame: PathBuf,
    pub output_dir: PathBuf,
    pub force: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TitleGraphicsManifest {
    pub(super) kind: String,
    pub(super) source_bin_sha256: String,
    pub(super) source_path: String,
    pub(super) source_stored_sha256: String,
    pub(super) source_decoded_sha256: String,
    pub(super) tim_offset: usize,
    pub(super) tim_decoded_size: usize,
    pub(super) source_pixel_sha256: String,
    pub(super) palette_index: usize,
    pub(super) pixel_width: usize,
    pub(super) pixel_height: usize,
    pub(super) image_vram_word_x: u16,
    pub(super) image_vram_y: u16,
    pub(super) clut_vram_x: u16,
    pub(super) clut_vram_y: u16,
    pub(super) units: Vec<TitleGraphicsManifestEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TitleGraphicsManifestEntry {
    pub(super) id: String,
    pub(super) file: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TitleGraphicUnit {
    pub(super) kind: String,
    pub(super) id: String,
    pub(super) source_label: String,
    pub(super) korean_text: Option<String>,
    pub(super) runtime_role: String,
    pub(super) cell: Cell,
    pub(super) clear_index: u8,
    pub(super) source_indexed_pixel_sha256: String,
    pub(super) development_status: String,
    pub(super) release_status: String,
    pub(super) artwork: Option<TitleArtwork>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TitleArtwork {
    pub(super) indices_file: PathBuf,
    pub(super) indices_sha256: String,
    pub(super) palette_index: usize,
    pub(super) source_palette_sha256: String,
    pub(super) imagegen_sha256: String,
    pub(super) dotmend_art_id: String,
}

#[derive(Debug, Serialize)]
pub struct TitleGraphicsAuditReport {
    pub kind: String,
    pub source_bin_sha256: String,
    pub source_path: String,
    pub source_extent_lba: u32,
    pub source_stored_sha256: String,
    pub source_decoded_sha256: String,
    pub manifest_sha256: String,
    pub asset_set_sha256: String,
    pub gpu_dump_path: String,
    pub gpu_dump_sha256: String,
    pub runtime_frame_path: String,
    pub runtime_frame_sha256: String,
    pub source_texture_pixel_count: usize,
    pub matching_vram_pixel_count: usize,
    pub entire_source_texture_matches_vram: bool,
    pub storage_record_verified: bool,
    pub source_pixel_boundaries_verified: bool,
    pub runtime_residency_verified: bool,
    pub exact_draw_consumer_verified: bool,
    pub unit_count: usize,
    pub untranslated_unit_count: usize,
    pub units: Vec<TitleGraphicUnitAudit>,
    pub release_eligible: bool,
}

#[derive(Debug, Serialize)]
pub struct TitleGraphicUnitAudit {
    pub id: String,
    pub runtime_role: String,
    pub cell: Cell,
    pub source_indexed_pixel_sha256: String,
    pub resident_indexed_pixel_sha256: String,
    pub source_matches_vram: bool,
    pub source_preview_file: String,
    pub source_preview_palette_index: usize,
    pub source_preview_sha256: String,
    pub development_status: String,
    pub release_status: String,
}
