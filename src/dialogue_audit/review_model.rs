use std::path::PathBuf;

use serde::Serialize;

use super::codebook_model::DialogueGlyphSourceReference;

#[derive(Debug, Clone)]
pub struct DialogueCodebookReviewConfig {
    pub cue: PathBuf,
    pub codebook: PathBuf,
    pub output_dir: PathBuf,
}

#[derive(Debug, Serialize)]
pub struct DialogueCodebookReviewSlot {
    pub page_index: usize,
    pub slot_index: usize,
    pub pixel_sha256: String,
    pub occurrence_count: usize,
    pub source_references: Vec<DialogueGlyphSourceReference>,
    pub contexts: Vec<DialogueCodebookReviewContext>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DialogueCodebookReviewContext {
    pub coordinate_id: String,
    pub source_asset: String,
    pub bank_index: usize,
    pub entry_index: usize,
    pub decoded_offset: String,
    pub target_code: String,
    pub target_token_index: usize,
    pub target_occurrence_count: usize,
    pub context_markup: String,
}

#[derive(Debug, Serialize)]
pub struct DialogueCodebookReviewShard {
    pub kind: String,
    pub page_index: usize,
    pub slot_count: usize,
    pub slots: Vec<DialogueCodebookReviewSlot>,
}

#[derive(Debug, Serialize)]
pub struct DialogueCodebookReviewPage {
    pub page_index: usize,
    pub png: String,
    pub png_sha256: String,
    pub shard: String,
    pub shard_sha256: String,
    pub shard_byte_count: usize,
    pub slot_count: usize,
}

#[derive(Debug, Serialize)]
pub struct DialogueCodebookReviewManifest {
    pub kind: String,
    pub implementation: String,
    pub source_bin_sha256: String,
    pub codebook_sha256: String,
    pub unresolved_used_source_pixel_hash_count: usize,
    pub unresolved_glyph_occurrence_count: usize,
    pub cell_width: usize,
    pub cell_height: usize,
    pub scale: usize,
    pub gutter: usize,
    pub columns: usize,
    pub slots_per_page: usize,
    pub maximum_contexts_per_glyph: usize,
    pub context_token_radius: usize,
    pub maximum_shard_bytes: usize,
    pub coordinate_rule: String,
    pub ordering_rule: String,
    pub pages: Vec<DialogueCodebookReviewPage>,
}
