use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::tim::Cell;

#[derive(Debug, Clone)]
pub struct DiaryHeaderFontStyle {
    pub path: PathBuf,
    pub font_px: f32,
    pub rendering: DiaryHeaderIndexedRendering,
    pub glyph_layout: DiaryHeaderGlyphLayout,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct DiaryHeaderGlyphLayout {
    pub slot_width_px: usize,
    pub vertical_shift_px: i32,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum DiaryHeaderIndexedRendering {
    CoverageRamp {
        first_ink_index: u8,
        last_ink_index: u8,
    },
    Outlined {
        outline_index: u8,
        fill_index: u8,
    },
}

#[derive(Debug, Clone)]
pub struct DiaryHeaderFontSources {
    pub calendar_text: DiaryHeaderFontStyle,
    pub status_label: DiaryHeaderFontStyle,
    pub club_label: DiaryHeaderFontStyle,
    pub action_label: DiaryHeaderFontStyle,
}

#[derive(Debug, Clone)]
pub struct DiaryHeaderBuildConfig {
    pub cue: PathBuf,
    pub assets: PathBuf,
    pub fonts: DiaryHeaderFontSources,
    pub build_spec_sha256: String,
    pub output_dir: PathBuf,
    pub force: bool,
}

#[derive(Debug, Clone)]
pub struct DiaryHeaderDiscBuildConfig {
    pub cue: PathBuf,
    pub assets: PathBuf,
    pub fonts: DiaryHeaderFontSources,
    pub build_spec_sha256: String,
    pub output_dir: PathBuf,
    pub force: bool,
}

pub struct DiaryHeaderBuild {
    pub stored: Vec<u8>,
    pub decoded: Vec<u8>,
    pub build_manifest_sha256: String,
    pub report: DiaryHeaderBuildReport,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Ord, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiaryHeaderFontRole {
    CalendarText,
    StatusLabel,
    ClubLabel,
    ProfileText,
    ActionLabel,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Serialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum DiaryHeaderTextLayout {
    GlyphSlots {
        #[serde(default)]
        slots: Vec<usize>,
    },
    Continuous,
}

impl Default for DiaryHeaderTextLayout {
    fn default() -> Self {
        Self::GlyphSlots { slots: Vec::new() }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DiaryHeaderManifest {
    pub(super) kind: String,
    pub(super) source: DiaryHeaderSourceBinding,
    pub(super) protected_regions: Vec<DiaryHeaderProtectedRegion>,
    pub(super) units: Vec<PathBuf>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DiaryHeaderSourceBinding {
    pub(super) path: String,
    pub(super) stored_sha256: String,
    pub(super) decoded_sha256: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DiaryHeaderProtectedRegion {
    pub(super) id: String,
    pub(super) cell: Cell,
    pub(super) source_region_sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DiaryHeaderUnit {
    pub(super) kind: String,
    pub(super) entries: Vec<DiaryHeaderEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DiaryHeaderIndexedArt {
    pub(super) file: PathBuf,
    pub(super) sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DiaryHeaderEntry {
    pub(super) id: String,
    pub(super) source_text: String,
    pub(super) korean_text: String,
    pub(super) font_role: DiaryHeaderFontRole,
    pub(super) cell: Cell,
    #[serde(default)]
    pub(super) layout: DiaryHeaderTextLayout,
    #[serde(default)]
    pub(super) font_px: Option<f32>,
    #[serde(default)]
    pub(super) vertical_shift_px: Option<i32>,
    pub(super) source_region_sha256: String,
    #[serde(default)]
    pub(super) indexed_art: Option<DiaryHeaderIndexedArt>,
}

#[derive(Debug, Serialize)]
pub struct DiaryHeaderBuildReport {
    pub kind: String,
    pub source_path: String,
    pub source_stored_sha256: String,
    pub source_decoded_sha256: String,
    pub patched_stored_sha256: String,
    pub patched_decoded_sha256: String,
    pub build_spec_sha256: String,
    pub source_tim_vram: [u16; 2],
    pub source_tim_pixel_size: [usize; 2],
    pub source_clut_vram: [u16; 2],
    pub source_palette_count: usize,
    pub entry_count: usize,
    pub protected_region_count: usize,
    pub source_regions_match: bool,
    pub protected_regions_unchanged: bool,
    pub cells_are_unique_and_non_overlapping: bool,
    pub changed_bytes_confined_to_owned_cells: bool,
    pub unpadded_stored_size: usize,
    pub source_compression_stream_byte_count: usize,
    pub source_compression_maximum_match_words: usize,
    pub rebuilt_compression_maximum_match_words: usize,
    pub source_compression_maximum_control_block_output_words: usize,
    pub rebuilt_compression_maximum_control_block_output_words: usize,
    pub source_compression_control_blocks_crossing_input_pages: usize,
    pub rebuilt_compression_control_blocks_crossing_input_pages: usize,
    pub source_record_size: usize,
    pub development_input_available: bool,
    pub release_candidate_input_eligible: bool,
    pub fonts: Vec<DiaryHeaderFontBuild>,
    pub protected_regions: Vec<DiaryHeaderProtectedRegionBuild>,
    pub entries: Vec<DiaryHeaderEntryBuild>,
}

#[derive(Debug, Serialize)]
pub struct DiaryHeaderDiscBuildReport {
    pub kind: String,
    pub output_bin_sha256: String,
    pub output_cue: String,
    pub diary_header: DiaryHeaderBuildReport,
    pub changed_lbas: Vec<u32>,
    pub changes_confined_to_diary_header: bool,
    pub readback_verified: bool,
    pub edc_ecc_verified: bool,
}

#[derive(Debug, Serialize)]
pub struct DiaryHeaderFontBuild {
    pub role: DiaryHeaderFontRole,
    pub font_name: String,
    pub font_sha256: String,
    pub font_px: f32,
    pub rendering: DiaryHeaderIndexedRendering,
    pub glyph_layout: DiaryHeaderGlyphLayout,
}

#[derive(Debug, Serialize)]
pub struct DiaryHeaderProtectedRegionBuild {
    pub id: String,
    pub cell: Cell,
    pub source_region_sha256: String,
}

#[derive(Debug, Serialize)]
pub struct DiaryHeaderEntryBuild {
    pub id: String,
    pub source_text: String,
    pub korean_text: String,
    pub font_role: DiaryHeaderFontRole,
    pub cell: Cell,
    pub layout: DiaryHeaderTextLayout,
    pub font_px: f32,
    pub vertical_shift_px: i32,
    pub source_region_sha256: String,
    pub measured_advance_px: f32,
    pub ink_bounds: [usize; 4],
    pub changed_decoded_byte_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indexed_art_sha256: Option<String>,
}
