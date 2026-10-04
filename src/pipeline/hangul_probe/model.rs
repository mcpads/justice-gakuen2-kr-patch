use std::path::PathBuf;

use serde::Serialize;

use crate::font::GlyphFit;
use crate::menu_audit::ProvisionalCodeEvidence;
use crate::text::AtlasPosition;
use crate::tim::GlyphInstallMetadata;

#[derive(Debug, Clone)]
pub struct HangulProbeConfig {
    pub cue: PathBuf,
    pub font: PathBuf,
    pub output_dir: PathBuf,
    pub force: bool,
}

#[derive(Debug, Serialize)]
pub struct InstalledHangulGlyph {
    pub character: char,
    pub provisional_code: String,
    pub atlas_position: AtlasPosition,
    pub font_fit: GlyphFit,
    pub install: GlyphInstallMetadata,
}

#[derive(Debug, Serialize)]
pub struct MenuAssetChange {
    pub path: String,
    pub extent_lba: u32,
    pub sector_count: usize,
    pub original_stored_sha256: String,
    pub output_stored_sha256: String,
    pub original_decoded_sha256: String,
    pub output_decoded_sha256: String,
    pub reencoded_size: usize,
    pub padding_size: usize,
    pub catalog_first_word: String,
    pub decoded_changed_byte_ranges: Vec<[usize; 2]>,
}

#[derive(Debug, Serialize)]
pub struct TextAssetChange {
    pub path: String,
    pub extent_lba: u32,
    pub sector_count: usize,
    pub original_sha256: String,
    pub output_sha256: String,
    pub label_offset: String,
    pub original_codes: Vec<String>,
    pub replacement_codes: Vec<String>,
    pub changed_byte_ranges: Vec<[usize; 2]>,
}

#[derive(Debug, Serialize)]
pub struct HangulProbeManifest {
    pub kind: String,
    pub implementation: String,
    pub source_bin_sha256: String,
    pub output_bin_sha256: String,
    pub output_cue: String,
    pub changed_lbas: Vec<u32>,
    pub edc_ecc_verified: bool,
    pub font_file: String,
    pub font_sha256: String,
    pub font_px: f32,
    pub outline_palette_index: u8,
    pub fill_palette_index: u8,
    pub original_label: String,
    pub replacement_label: String,
    pub allocation_status: String,
    pub provisional_code_evidence: ProvisionalCodeEvidence,
    pub glyphs: Vec<InstalledHangulGlyph>,
    pub menu_asset: MenuAssetChange,
    pub text_asset: TextAssetChange,
}
