use std::path::PathBuf;

use serde::Serialize;

use crate::tim::Cell;

#[derive(Debug, Clone)]
pub struct MenuAtlasMigrationAuditConfig {
    pub cue: PathBuf,
    pub dialogue_codebook: PathBuf,
    pub candidate_build_dir: PathBuf,
    pub labels: PathBuf,
    pub output: PathBuf,
    pub force: bool,
    pub address_flow_state_budget: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MenuAtlasWriterComponent {
    Options,
    TitleMenu,
    TitleNotice,
}

impl MenuAtlasWriterComponent {
    pub(super) const fn key(self) -> &'static str {
        match self {
            Self::Options => "options",
            Self::TitleMenu => "title_menu",
            Self::TitleNotice => "title_notice",
        }
    }

    pub(super) const fn preview_color_rgb(self) -> [u8; 3] {
        match self {
            Self::Options => [255, 64, 64],
            Self::TitleMenu => [255, 176, 32],
            Self::TitleNotice => [224, 64, 255],
        }
    }
}

#[derive(Debug, Serialize)]
pub struct MenuAtlasWriterComponentAudit {
    pub component: MenuAtlasWriterComponent,
    pub report_file: String,
    pub report_sha256: String,
    pub report_kind: String,
    pub source_bin_sha256: String,
    pub allocation_proven_reclaimable: Option<bool>,
    pub glyph_entry_count: usize,
    pub selected_global_write_count: usize,
    pub selected_written_pixel_count: usize,
    pub selected_twenty_by_twenty_area_equivalent_count: usize,
    pub selection_policy: String,
    pub preview_color_rgb: [u8; 3],
}

#[derive(Debug, Serialize)]
pub struct CandidateMenuAtlasWrite {
    pub id: String,
    pub component: MenuAtlasWriterComponent,
    pub character: char,
    pub code: String,
    pub cell: Cell,
    pub written_pixel_count: usize,
    pub twenty_by_twenty_area_equivalent_count: usize,
    pub overlapping_candidate_write_ids: Vec<String>,
    pub source_code_obligation_count: usize,
    pub source_code_occurrence_count: usize,
    pub source_consumer_overlays: Vec<String>,
    pub exact_dialogue_pixel_texts: Vec<String>,
    pub overlapping_label_ids: Vec<String>,
    pub confirmed_label_ids: Vec<String>,
    pub confirmed_label_covered_pixel_count: usize,
    pub unclassified_written_pixel_count: usize,
}

#[derive(Debug, Serialize)]
pub struct CandidateMenuAtlasWriteOverlap {
    pub left_write_id: String,
    pub right_write_id: String,
    pub overlapping_pixel_count: usize,
}

#[derive(Debug, Serialize)]
pub struct SourceMenuCodeMigrationObligation {
    pub code: String,
    pub footprint_fragments: Vec<Cell>,
    pub footprint_pixel_count: usize,
    pub overwritten_pixel_count: usize,
    pub fully_overwritten: bool,
    pub occurrence_count: usize,
    pub overlays: Vec<String>,
    pub exact_dialogue_pixel_text: Option<String>,
    pub candidate_write_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct MenuAtlasMigrationAuditReport {
    pub kind: String,
    pub source_cue: String,
    pub source_bin_sha256: String,
    pub menu_path: String,
    pub menu_stored_sha256: String,
    pub menu_decoded_sha256: String,
    pub source_atlas_indexed_sha256: String,
    pub dialogue_codebook_sha256: String,
    pub candidate_build_directory: String,
    pub candidate_build_manifest_file: String,
    pub candidate_build_manifest_sha256: String,
    pub candidate_output_bin_file: String,
    pub candidate_output_bin_sha256: String,
    pub candidate_output_cue_file: String,
    pub candidate_output_cue_sha256: String,
    pub candidate_menu_stored_sha256: String,
    pub candidate_menu_decoded_sha256: String,
    pub candidate_atlas_indexed_sha256: String,
    pub candidate_menu_surface_writes_disjoint: bool,
    pub label_document_file: String,
    pub address_flow_state_budget: usize,
    pub dat1_address_flow_budget_exhausted_seed_count: usize,
    pub shared_reference_address_flow_budget_exhausted_seed_count: usize,
    pub statically_used_source_code_count: usize,
    pub proven_reclaimable_source_code_count: usize,
    pub candidate_components: Vec<MenuAtlasWriterComponentAudit>,
    pub candidate_write_count: usize,
    pub candidate_declared_written_pixel_count: usize,
    pub candidate_unique_written_pixel_count: usize,
    pub candidate_declared_twenty_by_twenty_area_equivalent_count: usize,
    pub candidate_atlas_changed_pixel_count: usize,
    pub candidate_atlas_changed_pixel_outside_declared_writes_count: usize,
    pub candidate_declared_write_unchanged_pixel_count: usize,
    pub candidate_atlas_changes_confined_to_declared_writes: bool,
    pub candidate_write_overlap_pair_count: usize,
    pub candidate_write_overlaps: Vec<CandidateMenuAtlasWriteOverlap>,
    pub source_code_migration_obligation_count: usize,
    pub fully_overwritten_source_code_obligation_count: usize,
    pub partially_overwritten_source_code_obligation_count: usize,
    pub source_code_occurrence_migration_obligation_count: usize,
    pub source_consumer_overlay_count: usize,
    pub source_consumer_overlays: Vec<String>,
    pub source_code_with_exact_dialogue_text_obligation_count: usize,
    pub exact_dialogue_texts: Vec<String>,
    pub label_count: usize,
    pub confirmed_label_count: usize,
    pub needs_review_label_count: usize,
    pub unreviewed_label_count: usize,
    pub candidate_written_pixels_fully_classified: bool,
    pub confirmed_label_covered_unique_written_pixel_count: usize,
    pub unclassified_unique_written_pixel_count: usize,
    pub candidate_writes: Vec<CandidateMenuAtlasWrite>,
    pub source_code_migration_obligations: Vec<SourceMenuCodeMigrationObligation>,
    pub migration_risk_overlay_preview_file: String,
    pub migration_risk_overlay_preview_sha256: String,
    pub migration_proof_complete: bool,
    pub local_evidence_only: bool,
    pub limitations: Vec<String>,
}
