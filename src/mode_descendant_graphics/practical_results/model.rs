use std::fmt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::tim::Cell;

use super::super::model::{ModeDescendantFontRole, ModeDescendantStorageKind, TextAlignment};
use super::opaque_pointer_run_model::PracticalResultOpaquePointerRunCatalog;
use super::projection_model::{
    PracticalResultActionConsumerOccurrenceCatalog, PracticalResultClutBindingCatalog,
    PracticalResultConsumerProjectionCatalog, PracticalResultConsumerProjectionId,
    PracticalResultProtectedRegionId, PracticalResultSemanticEntryId,
    PracticalResultVramResidencyCatalog,
};
use super::source_atlas_domain_model::{
    PracticalResultSourceAtlasConsumerProjectionDenominatorStatus,
    PracticalResultSourceAtlasDomainCatalog, SourceAtlasDomainId,
};

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(transparent)]
pub(super) struct PhysicalRegionId(String);

impl PhysicalRegionId {
    pub(super) fn as_str(&self) -> &str {
        &self.0
    }

    pub(super) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Display for PhysicalRegionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(transparent)]
pub(super) struct SourceReferenceId(String);

impl SourceReferenceId {
    pub(super) fn as_str(&self) -> &str {
        &self.0
    }

    pub(super) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Display for SourceReferenceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

macro_rules! fixed_text_string_id {
    ($name:ident) => {
        #[derive(Debug, Clone, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
        #[serde(transparent)]
        pub(super) struct $name(String);

        impl $name {
            pub(super) fn as_str(&self) -> &str {
                &self.0
            }

            pub(super) fn is_empty(&self) -> bool {
                self.0.is_empty()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(formatter)
            }
        }
    };
}

fixed_text_string_id!(PracticalResultFixedTextTargetId);
fixed_text_string_id!(PracticalResultFixedTextLayoutCandidateId);
fixed_text_string_id!(PracticalResultFixedTextConsumerOccurrenceId);
fixed_text_string_id!(PracticalResultJudgmentStampTargetId);
fixed_text_string_id!(PracticalResultJudgmentStampLayoutCandidateId);
fixed_text_string_id!(PracticalResultJudgmentStampConsumerOccurrenceId);
fixed_text_string_id!(PracticalResultActionCellTargetId);
fixed_text_string_id!(PracticalResultDecorativeBackgroundTargetId);

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultActionCellTargetCatalog {
    pub(super) kind: PracticalResultActionCellTargetCatalogKind,
    pub(super) targets: Vec<PracticalResultActionCellTarget>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
pub(super) enum PracticalResultActionCellTargetCatalogKind {
    #[serde(rename = "justice_gakuen2_practical_result_action_cell_target_catalog")]
    JusticeGakuen2PracticalResultActionCellTargetCatalog,
}

/// A transparent source-atlas allocation whose existing readers have been
/// exhaustively checked before action descriptors are redirected to it.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultActionCellTarget {
    pub(super) target_id: PracticalResultActionCellTargetId,
    pub(super) semantic_entry_id: PracticalResultSemanticEntryId,
    pub(super) target_record_path: String,
    pub(super) target_tim_offset: String,
    pub(super) target_bpp: u8,
    pub(super) target_cell: Cell,
    pub(super) palette_index: usize,
    pub(super) font_px: f32,
    pub(super) vertical_shift_px: i32,
    pub(super) expected_preimage_indexed_sha256: String,
    pub(super) consumer_occurrence_ids:
        Vec<super::projection_model::PracticalResultActionConsumerOccurrenceId>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultFixedTextConsumerOccurrenceCatalog {
    pub(super) kind: PracticalResultFixedTextConsumerOccurrenceCatalogKind,
    pub(super) occurrences: Vec<PracticalResultFixedTextConsumerOccurrence>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
pub(super) enum PracticalResultFixedTextConsumerOccurrenceCatalogKind {
    #[serde(rename = "justice_gakuen2_practical_result_fixed_text_consumer_occurrence_catalog")]
    JusticeGakuen2PracticalResultFixedTextConsumerOccurrenceCatalog,
}

/// Evidence that a source TIM, or one exact cell within it, reaches a validated
/// consumer. Fixed-text targets may cite cell evidence only inside that cell.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultFixedTextConsumerOccurrence {
    pub(super) occurrence_id: PracticalResultFixedTextConsumerOccurrenceId,
    pub(super) target_record_path: String,
    pub(super) target_tim_offset: String,
    pub(super) target_bpp: u8,
    pub(super) source_tim_sha256: String,
    pub(super) evidence: PracticalResultTextureConsumerEvidence,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum PracticalResultTextureConsumerEvidence {
    #[serde(rename = "exact_source_texture_runtime_observed")]
    RuntimeObservedTexture {
        runtime_artifact_bin_sha256: String,
        runtime_frame_path: String,
        runtime_frame_sha256: String,
    },
    #[serde(rename = "exact_source_texture_static_consumer_bound")]
    StaticBoundTexture {
        consumer_record_path: String,
        consumer_source_sha256: String,
        source_catalog_index: u16,
        renderer_entry_offset: String,
        full_texture_table_offset: String,
        full_texture_table_entry_count: usize,
        full_texture_table_sha256: String,
    },
    #[serde(rename = "exact_indexed_member_texture_static_consumer_bound")]
    StaticBoundIndexedMemberTexture {
        consumer_record_path: String,
        consumer_source_sha256: String,
        source_catalog_index: u16,
        selected_member_index: usize,
        selected_when_state_byte_equals_two: bool,
        loader_span_offset: String,
        loader_span_size: usize,
        loader_span_sha256: String,
        indexed_load_call_offset: String,
        tim_upload_call_offsets: Vec<String>,
    },
    #[serde(rename = "exact_source_cell_static_consumer_bound")]
    StaticBoundCell {
        source_atlas_domain_id: SourceAtlasDomainId,
        source_region: PracticalResultFixedTextStaticSourceRegion,
        consumer_source_cell: Cell,
        consumer_projection_ids: Vec<PracticalResultConsumerProjectionId>,
    },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum PracticalResultFixedTextStaticSourceRegion {
    Physical {
        region_id: PhysicalRegionId,
    },
    Protected {
        region_id: PracticalResultProtectedRegionId,
    },
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultJudgmentStampConsumerOccurrenceCatalog {
    pub(super) kind: PracticalResultJudgmentStampConsumerOccurrenceCatalogKind,
    pub(super) occurrences: Vec<PracticalResultJudgmentStampConsumerOccurrence>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
pub(super) enum PracticalResultJudgmentStampConsumerOccurrenceCatalogKind {
    #[serde(rename = "justice_gakuen2_practical_result_judgment_stamp_consumer_occurrence_catalog")]
    JusticeGakuen2PracticalResultJudgmentStampConsumerOccurrenceCatalog,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultJudgmentStampConsumerOccurrence {
    pub(super) occurrence_id: PracticalResultJudgmentStampConsumerOccurrenceId,
    pub(super) target_record_path: String,
    pub(super) target_tim_offset: String,
    pub(super) target_bpp: u8,
    pub(super) source_tim_sha256: String,
    pub(super) evidence: PracticalResultTextureConsumerEvidence,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultManifest {
    pub(super) kind: String,
    pub(super) source_catalog: Vec<PracticalResultSourceBinding>,
    pub(super) physical_region_catalog_file: PathBuf,
    pub(super) source_atlas_domain_catalog_file: PathBuf,
    pub(super) opaque_pointer_run_catalog_file: PathBuf,
    pub(super) source_glyph_catalog_file: PathBuf,
    pub(super) fixed_text_target_catalog_file: PathBuf,
    pub(super) fixed_text_consumer_occurrence_catalog_file: PathBuf,
    pub(super) judgment_stamp_target_catalog_file: PathBuf,
    pub(super) judgment_stamp_consumer_occurrence_catalog_file: PathBuf,
    pub(super) action_cell_target_catalog_file: PathBuf,
    pub(super) decorative_background_target_catalog_file: PathBuf,
    pub(super) vram_residency_catalog_file: PathBuf,
    pub(super) clut_binding_catalog_file: PathBuf,
    pub(super) action_consumer_occurrence_catalog_file: PathBuf,
    pub(super) consumer_projection_catalog_file: PathBuf,
    pub(super) physical_region_hash_contract: PracticalResultRegionHashContract,
    pub(super) visual_region_hash_contract: PracticalResultRegionHashContract,
    pub(super) glyph_sequence_hash_contract: PracticalResultGlyphSequenceHashContract,
    pub(super) coverage_boundary: PracticalResultCoverageBoundary,
    pub(super) protected_content: PracticalResultProtectedContent,
    pub(super) shards: Vec<PracticalResultShardReference>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultPhysicalRegionCatalog {
    pub(super) kind: String,
    pub(super) regions: Vec<PracticalResultPhysicalRegion>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultPhysicalRegion {
    pub(super) region_id: PhysicalRegionId,
    #[serde(default)]
    pub(super) source_atlas_domain_id: Option<SourceAtlasDomainId>,
    pub(super) source_path: String,
    pub(super) tim_offset: String,
    pub(super) bpp: u8,
    pub(super) cell: Cell,
    pub(super) source_indexed_sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultSourceGlyphCatalog {
    pub(super) kind: String,
    pub(super) banks: Vec<PracticalResultSourceGlyphBank>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultSourceGlyphBank {
    pub(super) id: String,
    pub(super) source_atlas_domain_id: SourceAtlasDomainId,
    pub(super) source_path: String,
    pub(super) tim_offset: String,
    pub(super) bpp: u8,
    pub(super) palette_count: usize,
    pub(super) rows: Vec<PracticalResultSourceGlyphRow>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultSourceGlyphRow {
    pub(super) id: String,
    pub(super) role: String,
    pub(super) start_x: usize,
    pub(super) y: usize,
    pub(super) cell_width: usize,
    pub(super) cell_height: usize,
    pub(super) glyphs: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultFixedTextTargetCatalog {
    pub(super) kind: PracticalResultFixedTextTargetCatalogKind,
    pub(super) targets: Vec<PracticalResultFixedTextTarget>,
    pub(super) layout_candidates: Vec<PracticalResultFixedTextLayoutCandidate>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultDecorativeBackgroundTargetCatalog {
    pub(super) kind: PracticalResultDecorativeBackgroundTargetCatalogKind,
    pub(super) targets: Vec<PracticalResultDecorativeBackgroundTarget>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
pub(super) enum PracticalResultDecorativeBackgroundTargetCatalogKind {
    #[serde(rename = "justice_gakuen2_practical_result_decorative_background_target_catalog")]
    JusticeGakuen2PracticalResultDecorativeBackgroundTargetCatalog,
}

/// One explicit full-texture backdrop whose consumer and immutable source
/// identity are already closed. The clear cells deliberately form a finite
/// border around the preserved result sheet; they are not inferred from source
/// colors during the product build.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultDecorativeBackgroundTarget {
    pub(super) target_id: PracticalResultDecorativeBackgroundTargetId,
    pub(super) target_record_path: String,
    pub(super) target_tim_offset: String,
    pub(super) target_bpp: u8,
    pub(super) source_cell: Cell,
    pub(super) replaces_source_region_id: PhysicalRegionId,
    pub(super) expected_preimage_indexed_sha256: String,
    pub(super) expected_consumer_occurrence_ids: Vec<PracticalResultFixedTextConsumerOccurrenceId>,
    pub(super) background_palette_index: u8,
    pub(super) background_palette_word: String,
    pub(super) composition_stage: PracticalResultDecorativeCompositionStage,
    #[serde(default)]
    pub(super) clear_cells: Vec<Cell>,
    #[serde(default)]
    pub(super) preserve_regions: Vec<PracticalResultDecorativePreserveRegion>,
    #[serde(default)]
    pub(super) preserve_cells: Vec<Cell>,
    pub(super) text_placements: Vec<PracticalResultDecorativeTextPlacement>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum PracticalResultDecorativeCompositionStage {
    BeforeFixedText,
    AfterFixedText,
}

/// A reviewed non-text silhouette retained from a mixed source texture. The
/// spans are explicit authored data; the product build never derives them from
/// source colors, OCR, connected components, or neighboring assets.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultDecorativePreserveRegion {
    pub(super) region_id: String,
    pub(super) content_kind: PracticalResultDecorativePreserveContentKind,
    pub(super) spans: Vec<PracticalResultDecorativePreserveSpan>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub(super) enum PracticalResultDecorativePreserveContentKind {
    Paper,
    Glasses,
    AttendanceBook,
    Sword,
    ButtonIcon,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultDecorativePreserveSpan {
    pub(super) y: usize,
    pub(super) x: usize,
    pub(super) width: usize,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultDecorativeTextPlacement {
    pub(super) placement_id: String,
    pub(super) semantic_entry_id: PracticalResultSemanticEntryId,
    pub(super) cell: Cell,
    pub(super) alignment: TextAlignment,
    pub(super) font_px: f32,
    pub(super) clockwise_rotation_degrees: f64,
    pub(super) outline_palette_index: u8,
    pub(super) outline_palette_word: String,
    pub(super) fill_palette_index: u8,
    pub(super) fill_palette_word: String,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
pub(super) enum PracticalResultFixedTextTargetCatalogKind {
    #[serde(rename = "justice_gakuen2_practical_result_fixed_text_target_catalog")]
    JusticeGakuen2PracticalResultFixedTextTargetCatalog,
}

/// A write destination whose consumer occurrences and exact indexed preimage
/// have both been independently closed. Source observations are deliberately
/// absent: they cannot create destination authority.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultFixedTextTarget {
    #[serde(default)]
    pub(super) indexed_artwork: Option<PracticalResultIndexedArtwork>,
    pub(super) target_id: PracticalResultFixedTextTargetId,
    pub(super) semantic_entry_id: PracticalResultSemanticEntryId,
    pub(super) target_record_path: String,
    pub(super) target_tim_offset: String,
    pub(super) target_bpp: u8,
    pub(super) target_cell: Cell,
    pub(super) alignment: TextAlignment,
    pub(super) replaces_source_region_id: PhysicalRegionId,
    pub(super) expected_preimage_indexed_sha256: String,
    pub(super) expected_consumer_occurrence_ids: Vec<PracticalResultFixedTextConsumerOccurrenceId>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultIndexedArtwork {
    pub(super) indices_file: PathBuf,
    pub(super) indices_sha256: String,
    pub(super) palette_sha256: String,
    pub(super) imagegen_sha256: String,
    pub(super) dotmend_art_id: String,
    pub(super) dotmend_bundle_id: String,
}

/// JP source evidence that may help investigate a future KR destination, but
/// never authorizes a write by itself.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultFixedTextLayoutCandidate {
    pub(super) candidate_id: PracticalResultFixedTextLayoutCandidateId,
    pub(super) semantic_entry_id: PracticalResultSemanticEntryId,
    pub(super) source_reference_id: SourceReferenceId,
    pub(super) source_path: String,
    pub(super) source_tim_offset: String,
    pub(super) source_bpp: u8,
    pub(super) candidate_cell: Cell,
    pub(super) alignment: TextAlignment,
    pub(super) replaces_source_region_id: PhysicalRegionId,
    pub(super) source_indexed_sha256: String,
    pub(super) evidence_status: PracticalResultFixedTextCandidateEvidenceStatus,
    pub(super) authority_status: PracticalResultFixedTextCandidateAuthorityStatus,
    pub(super) unresolved_reason: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultJudgmentStampTargetCatalog {
    pub(super) kind: PracticalResultJudgmentStampTargetCatalogKind,
    pub(super) targets: Vec<PracticalResultJudgmentStampTarget>,
    pub(super) layout_candidates: Vec<PracticalResultJudgmentStampLayoutCandidate>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
pub(super) enum PracticalResultJudgmentStampTargetCatalogKind {
    #[serde(rename = "justice_gakuen2_practical_result_judgment_stamp_target_catalog")]
    JusticeGakuen2PracticalResultJudgmentStampTargetCatalog,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultJudgmentStampTarget {
    pub(super) target_id: PracticalResultJudgmentStampTargetId,
    pub(super) semantic_entry_id: PracticalResultSemanticEntryId,
    pub(super) target_record_path: String,
    pub(super) target_tim_offset: String,
    pub(super) target_bpp: u8,
    pub(super) target_cell: Cell,
    pub(super) replaces_source_region_id: PhysicalRegionId,
    pub(super) expected_preimage_indexed_sha256: String,
    pub(super) expected_consumer_occurrence_ids:
        Vec<PracticalResultJudgmentStampConsumerOccurrenceId>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultJudgmentStampLayoutCandidate {
    pub(super) candidate_id: PracticalResultJudgmentStampLayoutCandidateId,
    pub(super) semantic_entry_id: PracticalResultSemanticEntryId,
    pub(super) source_reference_id: SourceReferenceId,
    pub(super) source_path: String,
    pub(super) source_tim_offset: String,
    pub(super) source_bpp: u8,
    pub(super) candidate_cell: Cell,
    pub(super) replaces_source_region_id: PhysicalRegionId,
    pub(super) source_indexed_sha256: String,
    pub(super) evidence_status: PracticalResultFixedTextCandidateEvidenceStatus,
    pub(super) authority_status: PracticalResultFixedTextCandidateAuthorityStatus,
    pub(super) unresolved_reason: String,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum PracticalResultFixedTextCandidateEvidenceStatus {
    SourceObservationOnly,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum PracticalResultFixedTextCandidateAuthorityStatus {
    NonAuthoritativeLayoutCandidate,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultCoverageBoundary {
    pub(super) existing_shared_ui_owner: PracticalResultExistingOwner,
    pub(super) excluded_tim_surfaces: Vec<PracticalResultExcludedTim>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultExistingOwner {
    pub(super) source_paths: Vec<String>,
    pub(super) tim_offset: String,
    pub(super) bpp: u8,
    pub(super) owned_entry_count: usize,
    pub(super) disposition: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultExcludedTim {
    pub(super) source_path: String,
    pub(super) tim_offset: String,
    pub(super) bpp: u8,
    pub(super) disposition: String,
    pub(super) reason: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultProtectedContent {
    pub(super) regions: Vec<PracticalResultProtectedRegion>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultProtectedRegion {
    pub(super) id: PracticalResultProtectedRegionId,
    pub(super) protection_role: PracticalResultSourceProtectionRole,
    pub(super) source_atlas_domain_id: SourceAtlasDomainId,
    pub(super) source_path: String,
    pub(super) tim_offset: String,
    pub(super) bpp: u8,
    pub(super) cell: Cell,
    pub(super) source_indexed_sha256: String,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PracticalResultSourceProtectionRole {
    DynamicNumericGlyphBank,
    PercentGlyph,
    FullwidthTermDigitBank,
    UnresolvedExamGlyphs,
    SelectionMarker,
    RatingGlyphBank,
    RemainingTimeComposite,
    OutcomeBadgeComposite,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultGlyphSequenceHashContract {
    pub(super) algorithm: String,
    pub(super) composition: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultSourceBinding {
    pub(super) source_path: String,
    pub(super) storage_kind: ModeDescendantStorageKind,
    pub(super) source_stored_size: usize,
    pub(super) source_stored_sha256: String,
    pub(super) source_decoded_size: usize,
    pub(super) source_decoded_sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultRegionHashContract {
    pub(super) algorithm: String,
    pub(super) pixel_format: String,
    pub(super) pixel_order: String,
    pub(super) row_order: String,
    pub(super) coordinate_space: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultShardReference {
    pub(super) responsibility: String,
    pub(super) file: PathBuf,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultShard {
    pub(super) kind: String,
    pub(super) responsibility: String,
    pub(super) entries: Vec<PracticalResultEntry>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PracticalResultStrategy {
    FixedText,
    GlyphSequence,
    JudgmentStamp,
    DecorativeMask,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PracticalResultDevelopmentStatus {
    Authored,
    Unresolved,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PracticalResultReleaseStatus {
    NeedsHumanReview,
    NotApplicable,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PracticalResultSourceUsageStatus {
    RuntimeObserved,
    StaticObserved,
    Unresolved,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultEntry {
    pub(super) id: String,
    pub(super) strategy: PracticalResultStrategy,
    pub(super) source_text: String,
    pub(super) korean_text: Option<String>,
    pub(super) font_role: ModeDescendantFontRole,
    pub(super) development_status: PracticalResultDevelopmentStatus,
    pub(super) release_status: PracticalResultReleaseStatus,
    #[serde(default)]
    pub(super) unresolved_reason: Option<String>,
    #[serde(default)]
    pub(super) source_cell_sequence_indexed_sha256: Option<String>,
    #[serde(default)]
    pub(super) source_references: Vec<PracticalResultSourceReference>,
    #[serde(default)]
    pub(super) unresolved_source_references: Vec<PracticalResultUnresolvedSourceReference>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultSourceReference {
    pub(super) reference_id: SourceReferenceId,
    pub(super) physical_region_id: PhysicalRegionId,
    pub(super) source_usage_status: PracticalResultSourceUsageStatus,
    #[serde(default)]
    pub(super) sequence_index: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultUnresolvedSourceReference {
    pub(super) reference_id: SourceReferenceId,
    pub(super) source_path: String,
    pub(super) tim_offset: String,
    pub(super) bpp: u8,
    #[serde(default)]
    pub(super) cell: Option<Cell>,
    #[serde(default)]
    pub(super) source_visual_rgba_sha256: Option<String>,
    pub(super) source_usage_status: PracticalResultSourceUsageStatus,
    pub(super) reason: String,
}

pub(super) struct PracticalResultAssets {
    pub(super) manifest_sha256: String,
    pub(super) physical_region_catalog_sha256: String,
    pub(super) source_atlas_domain_catalog_sha256: String,
    pub(super) opaque_pointer_run_catalog_sha256: String,
    pub(super) source_glyph_catalog_sha256: String,
    pub(super) fixed_text_target_catalog_sha256: String,
    pub(super) fixed_text_consumer_occurrence_catalog_sha256: String,
    pub(super) judgment_stamp_target_catalog_sha256: String,
    pub(super) judgment_stamp_consumer_occurrence_catalog_sha256: String,
    pub(super) action_cell_target_catalog_sha256: String,
    pub(super) decorative_background_target_catalog_sha256: String,
    pub(super) vram_residency_catalog_sha256: String,
    pub(super) clut_binding_catalog_sha256: String,
    pub(super) action_consumer_occurrence_catalog_sha256: String,
    pub(super) consumer_projection_catalog_sha256: String,
    pub(super) sources: Vec<PracticalResultSourceBinding>,
    pub(super) physical_region_catalog: PracticalResultPhysicalRegionCatalog,
    pub(super) source_atlas_domain_catalog: PracticalResultSourceAtlasDomainCatalog,
    pub(super) opaque_pointer_run_catalog: PracticalResultOpaquePointerRunCatalog,
    pub(super) source_glyph_catalog: PracticalResultSourceGlyphCatalog,
    pub(super) fixed_text_target_catalog: PracticalResultFixedTextTargetCatalog,
    pub(super) fixed_text_consumer_occurrence_catalog:
        PracticalResultFixedTextConsumerOccurrenceCatalog,
    pub(super) judgment_stamp_target_catalog: PracticalResultJudgmentStampTargetCatalog,
    pub(super) judgment_stamp_consumer_occurrence_catalog:
        PracticalResultJudgmentStampConsumerOccurrenceCatalog,
    pub(super) action_cell_target_catalog: PracticalResultActionCellTargetCatalog,
    pub(super) decorative_background_target_catalog:
        PracticalResultDecorativeBackgroundTargetCatalog,
    pub(super) vram_residency_catalog: PracticalResultVramResidencyCatalog,
    pub(super) clut_binding_catalog: PracticalResultClutBindingCatalog,
    pub(super) action_consumer_occurrence_catalog: PracticalResultActionConsumerOccurrenceCatalog,
    pub(super) consumer_projection_catalog: PracticalResultConsumerProjectionCatalog,
    pub(super) coverage_boundary: PracticalResultCoverageBoundary,
    pub(super) protected_content: PracticalResultProtectedContent,
    pub(super) entries: Vec<PracticalResultEntry>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PracticalResultSourceReadFootprintDerivation {
    CatalogDescriptorFragments,
    CatalogDigitDescriptorTable,
    DirectNumericSpriteSites,
    DirectSpriteSelectorTable,
    CatalogDescriptorSelectorTable,
    NumericLookupTable,
    StaticTileStreams,
    DynamicConfigGraph,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PracticalResultSourceReadSetAssessment {
    InputSetUnassessed,
    DeclaredReadSetComplete,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PracticalResultSourceConsumerReachability {
    Unassessed,
    RuntimeRootSelectionUnresolved,
    DormantOutsideDeclaredEntrypoints,
    Closed,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PracticalResultSourceReadPopulationBuild {
    DeclaredRectangles {
        declared_rectangle_count: usize,
        unique_rectangle_count: usize,
    },
    StaticDecodedStreams {
        decoded_stream_count: usize,
    },
    DynamicRootArrayTargets {
        pointer_target_count: usize,
        unique_pointer_target_count: usize,
        selector_referenced_unique_pointer_target_count: usize,
    },
}

#[derive(Debug, Serialize)]
pub struct PracticalResultDynamicStreamStateHeaderBuild {
    pub source_offset: usize,
    pub value: u8,
    pub selector_referenced: bool,
}

#[derive(Debug, Serialize)]
pub struct PracticalResultSourceRegionIntersectionBuild {
    pub region_id: String,
    pub region_cell: Cell,
    pub decoded_overlapping_tile_ids: Option<Vec<usize>>,
    pub selector_referenced_overlapping_tile_ids: Option<Vec<usize>>,
    pub source_read_intersection_cells: Vec<Cell>,
    pub selector_referenced_intersection_cells: Option<Vec<Cell>>,
}

#[derive(Debug, Serialize)]
pub struct PracticalResultProtectedSourceRegionIntersectionBuild {
    pub region_id: String,
    pub protection_role: PracticalResultSourceProtectionRole,
    pub region_cell: Cell,
    pub decoded_overlapping_tile_ids: Option<Vec<usize>>,
    pub selector_referenced_overlapping_tile_ids: Option<Vec<usize>>,
    pub source_read_intersection_cells: Vec<Cell>,
    pub selector_referenced_intersection_cells: Option<Vec<Cell>>,
}

#[derive(Debug, Serialize)]
pub struct PracticalResultSourceTileGeometryBuild {
    pub atlas_width: usize,
    pub atlas_height: usize,
    pub tile_width: usize,
    pub tile_height: usize,
}

#[derive(Debug, Serialize)]
pub struct PracticalResultSourceReadFootprintBuild {
    pub projection_id: String,
    pub source_atlas_domain_id: String,
    pub derivation: PracticalResultSourceReadFootprintDerivation,
    pub read_population: PracticalResultSourceReadPopulationBuild,
    pub tile_geometry: Option<PracticalResultSourceTileGeometryBuild>,
    pub decoded_tile_ids: Option<Vec<usize>>,
    pub selector_referenced_tile_ids: Option<Vec<usize>>,
    pub structural_stream_source_offsets: Option<Vec<usize>>,
    pub selector_referenced_stream_source_offsets: Option<Vec<usize>>,
    pub dynamic_stream_state_headers: Option<Vec<PracticalResultDynamicStreamStateHeaderBuild>>,
    pub source_read_rectangles: Vec<Cell>,
    pub selector_referenced_rectangles: Option<Vec<Cell>>,
    pub source_read_set_assessment: PracticalResultSourceReadSetAssessment,
    pub consumer_reachability: PracticalResultSourceConsumerReachability,
    pub physical_source_region_intersections: Vec<PracticalResultSourceRegionIntersectionBuild>,
    pub protected_source_region_intersections:
        Vec<PracticalResultProtectedSourceRegionIntersectionBuild>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum PracticalResultSourceAtlasRewriteGateBlocker {
    NoBoundConsumerProjection,
    ConsumerProjectionDenominatorIncomplete,
    UnscopedSourceReadBlocker,
    UnassessedSourceProjectionRead,
    IncompleteSourceReadSet,
    ConsumerReachabilityUnassessed,
    ConsumerReachabilityUnresolved,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PracticalResultSourceAtlasRewriteGateBuild {
    pub source_atlas_domain_id: String,
    pub consumer_projection_denominator_status:
        PracticalResultSourceAtlasConsumerProjectionDenominatorStatus,
    pub consumer_projection_denominator_incomplete_reason: Option<String>,
    pub bound_consumer_projection_count: usize,
    pub assessed_source_projection_read_count: usize,
    pub declared_read_set_complete_projection_count: usize,
    pub rewrite_eligible_projection_count: usize,
    pub unscoped_source_read_blocker_count: usize,
    pub gate_open: bool,
    pub blockers: Vec<PracticalResultSourceAtlasRewriteGateBlocker>,
}

#[derive(Debug, Serialize)]
pub struct PracticalResultBuildReport {
    pub kind: String,
    pub translation_manifest_sha256: String,
    pub physical_region_catalog_sha256: String,
    pub source_atlas_domain_catalog_sha256: String,
    pub declared_source_atlas_domain_count: usize,
    pub domain_bound_physical_region_count: usize,
    pub physical_region_without_declared_source_atlas_domain_count: usize,
    pub domain_bound_protected_region_count: usize,
    pub domain_bound_source_glyph_bank_count: usize,
    pub domain_bound_vram_residency_count: usize,
    pub domain_bound_clut_binding_count: usize,
    pub external_clut_producer_family_count: usize,
    pub external_clut_palette_family_sha256s: Vec<String>,
    pub unresolved_external_clut_binding_count: usize,
    pub domain_bound_consumer_descriptor_count: usize,
    pub domain_bound_consumer_projection_count: usize,
    pub alternative_consumer_source_binding_count: usize,
    pub assessed_source_projection_read_count: usize,
    pub unassessed_source_projection_read_count: usize,
    pub declared_read_set_complete_source_projection_count: usize,
    pub consumer_reachability_closed_source_projection_count: usize,
    pub consumer_reachability_unassessed_source_projection_count: usize,
    pub consumer_reachability_unresolved_source_projection_count: usize,
    pub consumer_reachability_dormant_source_projection_count: usize,
    pub source_projection_read_assessment_coverage_complete: bool,
    pub source_atlas_in_place_rewrite_gate_open_domain_count: usize,
    pub source_atlas_in_place_rewrite_gate_blocked_domain_count: usize,
    pub source_atlas_in_place_rewrite_gates: Vec<PracticalResultSourceAtlasRewriteGateBuild>,
    pub derived_source_read_footprint_count: usize,
    pub source_read_footprints: Vec<PracticalResultSourceReadFootprintBuild>,
    pub source_atlas_evidence_joins_complete: bool,
    pub indexed_result_selected_member_count: usize,
    pub indexed_result_known_post_upload_descriptor_aliases: Vec<u16>,
    pub indexed_result_direct_descriptor_render_call_offsets: Vec<String>,
    pub indexed_result_known_post_upload_draw_path_validated: bool,
    pub indexed_result_delegated_overlay_callback_index: usize,
    pub indexed_result_delegated_overlay_dispatch_offset: String,
    pub indexed_result_tim_upload_call_offsets: Vec<String>,
    pub indexed_result_validated_result_title_member_count: usize,
    pub indexed_result_texture_lifetime_validated: bool,
    pub opaque_pointer_run_catalog_sha256: String,
    pub declared_opaque_pointer_run_count: usize,
    pub source_atlas_excluded_opaque_pointer_run_count: usize,
    pub unresolved_opaque_pointer_run_count: usize,
    pub source_glyph_catalog_sha256: String,
    pub cataloged_source_glyph_cell_count: usize,
    pub fixed_text_target_catalog_sha256: String,
    pub fixed_text_consumer_occurrence_catalog_sha256: String,
    pub validated_fixed_text_consumer_occurrence_count: usize,
    pub validated_fixed_text_target_count: usize,
    pub fixed_text_layout_candidate_count: usize,
    pub completed_fixed_text_expected_write_count: usize,
    pub judgment_stamp_target_catalog_sha256: String,
    pub judgment_stamp_consumer_occurrence_catalog_sha256: String,
    pub action_cell_target_catalog_sha256: String,
    pub validated_action_cell_target_count: usize,
    pub decorative_background_target_catalog_sha256: String,
    pub validated_decorative_background_target_count: usize,
    pub rendered_decorative_background_target_count: usize,
    pub completed_decorative_background_expected_write_count: usize,
    pub rendered_decorative_background_text_placement_count: usize,
    pub decorative_background_palettes_preserved: bool,
    pub decorative_background_changes_confined_to_owned_cells: bool,
    pub validated_judgment_stamp_consumer_occurrence_count: usize,
    pub validated_judgment_stamp_target_count: usize,
    pub judgment_stamp_layout_candidate_count: usize,
    pub completed_judgment_stamp_expected_write_count: usize,
    pub vram_residency_catalog_sha256: String,
    pub clut_binding_catalog_sha256: String,
    pub action_consumer_occurrence_catalog_sha256: String,
    pub consumer_projection_catalog_sha256: String,
    pub declared_vram_residency_count: usize,
    pub declared_clut_binding_count: usize,
    pub resolved_clut_binding_count: usize,
    pub declared_consumer_descriptor_count: usize,
    pub declared_consumer_projection_count: usize,
    pub declared_action_consumer_occurrence_count: usize,
    pub validated_action_consumer_occurrence_count: usize,
    pub action_consumer_occurrence_coverage_complete: bool,
    pub source_unit_count: usize,
    pub authored_unit_count: usize,
    pub unresolved_unit_count: usize,
    pub fixed_text_unit_count: usize,
    pub rendered_fixed_text_unit_count: usize,
    pub deferred_fixed_text_unit_count: usize,
    pub deferred_glyph_sequence_unit_count: usize,
    pub authored_glyph_sequence_unit_count: usize,
    pub rendered_glyph_sequence_unit_count: usize,
    pub completed_action_cell_expected_write_count: usize,
    pub shared_action_suffix_cell_count: usize,
    pub resident_result_provider_cell_count: usize,
    pub resident_result_provider_expected_write_count: usize,
    pub resident_result_provider_write_contract_complete: bool,
    pub indexed_result_member_count: usize,
    pub indexed_result_mirrored_cell_count: usize,
    pub indexed_result_expected_write_count: usize,
    pub indexed_result_write_contract_complete: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_term_titles: Option<PracticalResultTermTitleBuildReport>,
    pub korean_sequence_token_count: usize,
    pub unique_korean_glyph_demand_count: usize,
    pub korean_atlas_slot_assignment_count: usize,
    pub consumer_projection_rewrite_count: usize,
    pub dynamic_atlas_plan_complete: bool,
    pub deferred_judgment_unit_count: usize,
    pub rendered_judgment_unit_count: usize,
    pub deferred_decorative_mask_unit_count: usize,
    pub source_reference_count: usize,
    pub located_source_reference_count: usize,
    pub unresolved_source_reference_count: usize,
    pub physical_region_count: usize,
    pub shared_physical_region_count: usize,
    pub overlapping_physical_region_count: usize,
    pub overlapping_physical_region_pair_count: usize,
    pub observed_source_usage_group_count: usize,
    pub semantic_bound_consumer_projection_count: usize,
    pub matched_authored_glyph_sequence_unit_count: usize,
    pub authored_glyph_sequence_projection_coverage_complete: bool,
    pub residency_bound_consumer_projection_count: usize,
    pub resolved_clut_bound_consumer_projection_count: usize,
    pub unresolved_external_clut_projection_count: usize,
    pub validated_projection_hashed_span_count: usize,
    pub validated_projection_runtime_address_count: usize,
    pub validated_projection_pointer_alias_count: usize,
    pub consumer_projection_binding_complete: bool,
    pub indexed_region_hash_matched_reference_count: usize,
    pub source_reference_without_indexed_region_count: usize,
    pub runtime_observed_source_reference_count: usize,
    pub static_observed_source_reference_count: usize,
    pub unresolved_source_usage_reference_count: usize,
    pub rendered_source_reference_count: usize,
    pub all_source_references_have_hash_matched_indexed_regions: bool,
    pub source_records_match: bool,
    pub rendered_fixed_cells_are_unique_and_non_overlapping: bool,
    pub rendered_fixed_changes_confined_to_owned_cells: bool,
    pub rendered_judgment_cells_are_unique_and_non_overlapping: bool,
    pub rendered_judgment_changes_confined_to_owned_cells: bool,
    pub rendered_action_cell_writes_are_unique_and_non_overlapping: bool,
    pub rendered_action_changes_confined_to_owned_cells: bool,
    pub fixed_text_expected_writes_complete: bool,
    pub judgment_stamp_expected_writes_complete: bool,
    pub development_input_available: bool,
    pub static_patch_insertion_complete: bool,
    pub release_candidate_input_eligible: bool,
    pub units: Vec<PracticalResultUnitBuild>,
}

#[derive(Debug, Serialize)]
pub struct PracticalResultTermTitleBuildReport {
    pub source_path: String,
    pub source_catalog_index: u16,
    pub archive_member_count: usize,
    pub localized_member_count: usize,
    pub producer_selector_count: usize,
    pub renderer_consumer_count: usize,
    pub full_124_by_28_sprite_extent_preserved: bool,
    pub changes_confined_to_complete_member_tim_pixels: bool,
    pub producer_selectors: Vec<PracticalResultTermTitleSelectorReport>,
    pub renderer_consumers: Vec<PracticalResultTermTitleRendererReport>,
    pub members: Vec<PracticalResultTermTitleMemberReport>,
}

#[derive(Debug, Serialize)]
pub struct PracticalResultTermTitleSelectorReport {
    pub path: String,
    pub source_sha256: String,
    pub catalog_index_setup_offset: String,
    pub member_selector: String,
}

#[derive(Debug, Serialize)]
pub struct PracticalResultTermTitleRendererReport {
    pub path: String,
    pub source_sha256: String,
    pub runtime_base: String,
    pub renderer_offset: String,
    pub screen_x: i16,
    pub screen_y: i16,
    pub sprite_width: i16,
    pub sprite_height: i16,
}

#[derive(Debug, Serialize)]
pub struct PracticalResultTermTitleMemberReport {
    pub member_index: usize,
    pub semantic_id: String,
    pub source_text: String,
    pub korean_text: String,
    pub rendered_text: String,
    pub source_tim_sha256: String,
    pub source_indexed_sha256: String,
    pub patched_indexed_sha256: String,
    pub transparent_palette_index: u8,
    pub coverage_palette_indices: Vec<u8>,
    pub font_px: f32,
    pub tracking_px: f32,
    pub measured_advance_px: f32,
    pub changed_decoded_byte_count: usize,
    pub source_preview_file: String,
    pub source_preview_sha256: String,
    pub preview_file: String,
    pub preview_sha256: String,
}

#[derive(Debug, Serialize)]
pub struct PracticalResultUnitBuild {
    pub id: String,
    pub strategy: PracticalResultStrategy,
    pub source_text: String,
    pub korean_text: Option<String>,
    pub font_role: ModeDescendantFontRole,
    pub development_status: PracticalResultDevelopmentStatus,
    pub release_status: PracticalResultReleaseStatus,
    pub source_reference_count: usize,
    pub located_source_reference_count: usize,
    pub unresolved_source_reference_count: usize,
    pub indexed_region_hash_matched_reference_count: usize,
    pub runtime_observed_source_reference_count: usize,
    pub static_observed_source_reference_count: usize,
    pub unresolved_source_usage_reference_count: usize,
    pub rendered_source_reference_count: usize,
    pub changed_decoded_byte_count: usize,
}
