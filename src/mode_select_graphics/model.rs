use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct ModeSelectGraphicsAuditConfig {
    pub spec: PathBuf,
    pub force: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ModeSelectGraphicsAuditSpec {
    pub(super) kind: String,
    pub(super) source_cue: PathBuf,
    pub(super) observed_cue: Option<PathBuf>,
    pub(super) output_dir: PathBuf,
}

#[derive(Debug, Serialize)]
pub struct ModeSelectGraphicsAuditReport {
    pub kind: String,
    pub spec_path: String,
    pub spec_sha256: String,
    pub source_bin_sha256: String,
    pub source_menu_path: String,
    pub source_menu_stored_sha256: String,
    pub source_menu_decoded_sha256: String,
    pub source_menu_decoded_size: usize,
    pub source_overlay_path: String,
    pub source_overlay_sha256: String,
    pub observed_menu: Option<ModeSelectObservedMenuAudit>,
    pub embedded_tim_count: usize,
    pub classified_tim_count: usize,
    pub unclassified_tim_count: usize,
    pub mode_artwork_count: usize,
    pub mode_preview_count: usize,
    pub observed_matching_mode_preview_count: Option<usize>,
    pub tims: Vec<ModeSelectTimAudit>,
}

#[derive(Debug, Serialize)]
pub struct ModeSelectObservedMenuAudit {
    pub cue_path: String,
    pub bin_sha256: String,
    pub menu_stored_sha256: String,
    pub menu_decoded_sha256: String,
    pub menu_decoded_size: usize,
    pub exact_source_tim_match_count: usize,
    pub changed_source_tim_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModeSelectTimRole {
    SharedAtlas,
    DescriptionPanel {
        panel_index: usize,
        mode_index: usize,
    },
    ModeArtworkPanel {
        panel_index: usize,
        mode_index: usize,
    },
    ModePreviewPanel {
        panel_index: usize,
        mode_index: usize,
    },
    Unclassified,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ModeSelectTimAudit {
    pub offset: usize,
    pub flags: u32,
    pub bits_per_pixel: u8,
    pub has_clut: bool,
    pub total_size: usize,
    pub source_tim_sha256: String,
    pub pixel_width: usize,
    pub pixel_height: usize,
    pub image_vram_word_x: u16,
    pub image_vram_y: u16,
    pub palette_count: usize,
    pub role: ModeSelectTimRole,
    pub preview_file: String,
    pub preview_sha256: String,
    pub observed_tim_sha256: Option<String>,
    pub observed_matches_source: Option<bool>,
}
