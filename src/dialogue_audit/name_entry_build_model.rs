use std::path::PathBuf;

use serde::Serialize;

use super::name_entry_font_build_model::DialogueNameEntryFontBuildReport;

#[derive(Debug, Clone)]
pub struct DialogueNameEntryBuildConfig {
    pub cue: PathBuf,
    pub candidates: PathBuf,
    pub graphics: PathBuf,
    pub candidate_font: PathBuf,
    pub candidate_font_px: f32,
    pub fixed_graphics_font: PathBuf,
    pub output_dir: PathBuf,
    pub force: bool,
}

#[derive(Debug, Serialize)]
pub struct DialogueNameEntryBuildReport {
    pub kind: String,
    pub source_bin_sha256: String,
    pub source_overlay_path: String,
    pub source_overlay_sha256: String,
    pub candidate_manifest_sha256: String,
    pub development_mapping_complete: bool,
    pub release_candidate_mapping_eligible: bool,
    pub global_name_code_start: String,
    pub global_name_code_end: String,
    pub candidate_count: usize,
    pub localized_candidate_count: usize,
    pub preserved_source_candidate_count: usize,
    pub pages: Vec<DialogueNameEntryCandidatePageReport>,
    pub assignments: Vec<DialogueNameEntryCandidateAssignment>,
    pub patched_overlay_file: String,
    pub patched_overlay_sha256: String,
    pub page_and_sequence_codes_match: bool,
    pub source_padding_preserved: bool,
    pub source_routines_preserved: bool,
    pub font: DialogueNameEntryFontBuildReport,
}

#[derive(Debug, Clone, Serialize)]
pub struct DialogueNameEntryCandidatePageReport {
    pub candidate_file: String,
    pub source_page: String,
    pub selection_basis: String,
    pub candidate_count: usize,
    pub localized_candidate_count: usize,
    pub preserved_source_candidate_count: usize,
    pub first_code: String,
    pub last_code: String,
    pub characters: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DialogueNameEntryCandidateAssignment {
    pub source_page: String,
    pub page_position: usize,
    pub character: String,
    pub code: String,
    pub preserve_source_glyph: bool,
}
