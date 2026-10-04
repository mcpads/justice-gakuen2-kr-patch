use serde::{Deserialize, Serialize};

use crate::font::HorizontalTextAlignment;
use crate::tim::Cell;

#[derive(Debug, Clone, Copy)]
pub(super) struct NameEntryFixedGraphicSurfaceSpec {
    pub(super) id: &'static str,
    pub(super) translation_file: &'static str,
    pub(super) translation_kind: &'static str,
    pub(super) build_kind: &'static str,
    pub(super) target: &'static str,
    pub(super) tim_offset: usize,
    pub(super) image_x: u16,
    pub(super) image_y: u16,
    pub(super) image_width: usize,
    pub(super) image_height: usize,
    pub(super) clut_width: usize,
    pub(super) clut_height: usize,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct NameEntryFixedGraphicsSource<'a> {
    pub(super) path: &'a str,
    pub(super) decoded_sha256: &'a str,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct NameEntryFixedGraphicTranslations {
    pub(super) kind: String,
    pub(super) source_path: String,
    pub(super) source_decoded_sha256: String,
    pub(super) tim_offset: usize,
    pub(super) entries: Vec<NameEntryFixedGraphicTranslation>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct NameEntryFixedGraphicTranslation {
    pub(super) id: String,
    pub(super) source_text: String,
    pub(super) korean_text: String,
    pub(super) source_region_sha256: String,
    pub(super) cell: Cell,
    pub(super) font_px: f32,
    pub(super) tracking_px: f32,
    pub(super) clear_index: u8,
    pub(super) outline_index: Option<u8>,
    pub(super) fill_index: u8,
    pub(super) alignment: HorizontalTextAlignment,
}

#[derive(Debug, Serialize)]
pub struct DialogueNameEntryFixedGraphicBuildReport {
    pub kind: String,
    pub surface_id: String,
    pub translation_file: String,
    pub translation_sha256: String,
    pub source_decoded_sha256: String,
    pub tim_offset: String,
    pub tim_image_width: usize,
    pub tim_image_height: usize,
    pub entry_count: usize,
    pub all_source_regions_match: bool,
    pub all_cells_unique_and_non_overlapping: bool,
    pub all_cells_disjoint_from_protected_cells: bool,
    pub entries: Vec<DialogueNameEntryFixedGraphicInstall>,
}

#[derive(Debug, Serialize)]
pub struct DialogueNameEntryFixedGraphicInstall {
    pub id: String,
    pub source_text: String,
    pub korean_text: String,
    pub source_region_sha256: String,
    pub cell: Cell,
    pub font_sha256: String,
    pub font_px: f32,
    pub tracking_px: f32,
    pub alignment: HorizontalTextAlignment,
    pub measured_advance_px: f32,
    pub ink_bounds: [usize; 4],
    pub indexed_pixels_sha256: String,
    pub changed_decoded_byte_count: usize,
}
