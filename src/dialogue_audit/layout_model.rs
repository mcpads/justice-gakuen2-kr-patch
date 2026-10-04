use std::path::PathBuf;

use serde::Serialize;

#[derive(Debug, Clone)]
pub struct DialogueLayoutAuditConfig {
    pub cue: PathBuf,
    pub codebook: PathBuf,
    pub translation: PathBuf,
    pub translation_audit_output: PathBuf,
    pub require_approved: bool,
    pub output: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
pub struct DialogueRendererGeometry {
    pub maximum_cells_per_line: usize,
    pub maximum_lines_per_message: usize,
    pub cell_width_pixels: usize,
    pub line_advance_pixels: usize,
    pub text_origin_x_pixels: usize,
    pub text_origin_y_pixels: usize,
    pub evidence: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DialogueRuntimeInsertionWidth {
    pub semantic_name: String,
    pub minimum_cells: usize,
    pub maximum_cells: usize,
    pub basis: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DialogueLayoutIssueKind {
    TooManyLines,
    StaticTextTooWide,
    RuntimeInsertionMayOverflow,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DialogueLineMeasurement {
    pub line_index: usize,
    pub static_cells: usize,
    pub minimum_cells: usize,
    pub maximum_cells: usize,
    pub runtime_insertions: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DialogueLayoutIssue {
    pub source_path: String,
    pub translation_path: String,
    pub bank_selector: usize,
    pub variant_selector: usize,
    pub route_table_offset: String,
    pub semantic_source_sha256: String,
    pub referenced_coordinate_ids: Vec<String>,
    pub issue_kinds: Vec<DialogueLayoutIssueKind>,
    pub lines: Vec<DialogueLineMeasurement>,
}

#[derive(Debug, Serialize)]
pub struct DialogueLayoutAuditReport {
    pub kind: String,
    pub source_bin_sha256: String,
    pub codebook_sha256: String,
    pub source_corpus_sha256: String,
    pub script_inventory_sha256: String,
    pub independent_review_required: bool,
    pub independent_review_complete: bool,
    pub approved_translation_group_count: usize,
    pub pending_translation_review_group_count: usize,
    pub changes_requested_translation_group_count: usize,
    pub renderer_geometry: DialogueRendererGeometry,
    pub runtime_insertion_widths: Vec<DialogueRuntimeInsertionWidth>,
    pub semantic_group_count: usize,
    pub fitting_group_count: usize,
    pub overflow_group_count: usize,
    pub too_many_lines_group_count: usize,
    pub static_text_too_wide_group_count: usize,
    pub runtime_insertion_may_overflow_group_count: usize,
    pub largest_static_line_cells: usize,
    pub largest_maximum_line_cells: usize,
    pub source_maximum_static_line_cells: usize,
    pub source_maximum_lines_per_message: usize,
    pub layout_within_renderer_capacity: bool,
    pub issues: Vec<DialogueLayoutIssue>,
}
