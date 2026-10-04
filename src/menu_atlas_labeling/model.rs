use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::tim::Cell;

#[derive(Debug, Clone)]
pub struct MenuAtlasLabelingConfig {
    pub cue: PathBuf,
    pub dialogue_codebook: PathBuf,
    pub labels: Option<PathBuf>,
    pub migration_risk: Option<PathBuf>,
    pub output_dir: PathBuf,
    pub force: bool,
    pub address_flow_state_budget: usize,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MenuAtlasLabelKind {
    FontGlyph,
    MultiCellGlyph,
    ControllerIcon,
    InterfaceGraphic,
    BakedText,
    Unknown,
}

impl MenuAtlasLabelKind {
    pub(super) const ALL: [Self; 6] = [
        Self::FontGlyph,
        Self::MultiCellGlyph,
        Self::ControllerIcon,
        Self::InterfaceGraphic,
        Self::BakedText,
        Self::Unknown,
    ];

    pub(super) const fn key(self) -> &'static str {
        match self {
            Self::FontGlyph => "font_glyph",
            Self::MultiCellGlyph => "multi_cell_glyph",
            Self::ControllerIcon => "controller_icon",
            Self::InterfaceGraphic => "interface_graphic",
            Self::BakedText => "baked_text",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MenuAtlasLabelReviewStatus {
    Unreviewed,
    NeedsReview,
    Confirmed,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MenuAtlasLabel {
    pub id: String,
    pub kind: MenuAtlasLabelKind,
    pub review_status: MenuAtlasLabelReviewStatus,
    pub bounds: Cell,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_role: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_text: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub logical_codes: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MenuAtlasLabelDocument {
    pub kind: String,
    pub source_bin_sha256: String,
    pub menu_stored_sha256: String,
    pub menu_decoded_sha256: String,
    pub source_atlas_indexed_sha256: String,
    pub atlas_width: usize,
    pub atlas_height: usize,
    pub labels: Vec<MenuAtlasLabel>,
}

#[derive(Debug, Serialize)]
pub struct MenuAtlasLogicalCodeReference {
    pub code: String,
    pub page: u8,
    pub column: u8,
    pub row: u8,
    pub physical_x: usize,
    pub physical_y: usize,
    pub footprint_fragments: Vec<Cell>,
    pub pixel_sha256: String,
    pub exact_dialogue_pixel_text: Option<String>,
    pub occurrence_count: usize,
    pub overlays: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct MenuAtlasLogicalCodeMap {
    pub kind: String,
    pub source_bin_sha256: String,
    pub menu_decoded_sha256: String,
    pub address_flow_state_budget: usize,
    pub code_count: usize,
    pub used_code_count: usize,
    pub exact_dialogue_pixel_match_count: usize,
    pub references: Vec<MenuAtlasLogicalCodeReference>,
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(super) struct MenuAtlasMigrationReviewWrite {
    pub id: String,
    pub component: String,
    pub character: char,
    pub code: String,
    pub cell: Cell,
    pub source_code_occurrence_count: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(super) struct MenuAtlasMigrationReviewObligation {
    pub code: String,
    pub footprint_fragments: Vec<Cell>,
    pub overwritten_pixel_count: usize,
    pub fully_overwritten: bool,
    pub occurrence_count: usize,
    pub overlays: Vec<String>,
    pub exact_dialogue_pixel_text: Option<String>,
    pub candidate_write_ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(super) struct MenuAtlasMigrationReviewContext {
    pub kind: String,
    pub source_bin_sha256: String,
    pub menu_stored_sha256: String,
    pub menu_decoded_sha256: String,
    pub source_atlas_indexed_sha256: String,
    pub address_flow_state_budget: usize,
    pub statically_used_source_code_count: usize,
    pub candidate_write_count: usize,
    pub source_code_migration_obligation_count: usize,
    pub migration_proof_complete: bool,
    pub candidate_writes: Vec<MenuAtlasMigrationReviewWrite>,
    pub source_code_migration_obligations: Vec<MenuAtlasMigrationReviewObligation>,
}

#[derive(Debug, Serialize)]
pub struct MenuAtlasLabelEvidence {
    pub label_id: String,
    pub bounds: Cell,
    pub indexed_pixel_sha256: String,
    pub palette_histogram: [usize; 16],
}

#[derive(Debug, Serialize)]
pub struct MenuAtlasLabelingReport {
    pub kind: String,
    pub source_cue: String,
    pub address_flow_state_budget: usize,
    pub source_bin_sha256: String,
    pub menu_path: String,
    pub menu_stored_sha256: String,
    pub menu_decoded_sha256: String,
    pub source_atlas_indexed_sha256: String,
    pub atlas_width: usize,
    pub atlas_height: usize,
    pub label_count: usize,
    pub confirmed_label_count: usize,
    pub needs_review_label_count: usize,
    pub unreviewed_label_count: usize,
    pub overlapping_label_pairs: Vec<[String; 2]>,
    pub label_kinds: Vec<String>,
    pub label_evidence: Vec<MenuAtlasLabelEvidence>,
    pub exact_dialogue_pixel_match_count: usize,
    pub statically_used_code_count: usize,
    pub proven_reclaimable_code_count: usize,
    pub atlas_preview_file: String,
    pub label_document_file: String,
    pub logical_code_map_file: String,
    pub editor_file: String,
    pub report_file: String,
    pub migration_risk_file: Option<String>,
    pub migration_risk_sha256: Option<String>,
    pub migration_candidate_write_count: usize,
    pub migration_affected_code_count: usize,
    pub local_workspace_only: bool,
    pub limitations: Vec<String>,
}
