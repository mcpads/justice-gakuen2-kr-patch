//! Source-only pointer runs and their reviewed consumer-association disposition.

use std::fmt;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(transparent)]
pub(super) struct PracticalResultOpaquePointerRunId(String);

impl PracticalResultOpaquePointerRunId {
    pub(super) fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PracticalResultOpaquePointerRunId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultOpaquePointerRunCatalog {
    pub(super) kind: PracticalResultOpaquePointerRunCatalogKind,
    pub(super) records: Vec<PracticalResultOpaquePointerRun>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
pub(super) enum PracticalResultOpaquePointerRunCatalogKind {
    #[serde(rename = "justice_gakuen2_practical_result_opaque_pointer_run_catalog")]
    JusticeGakuen2PracticalResultOpaquePointerRunCatalog,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultOpaquePointerRun {
    pub(super) id: PracticalResultOpaquePointerRunId,
    pub(super) overlay_path: String,
    pub(super) overlay_size: usize,
    pub(super) overlay_sha256: String,
    pub(super) runtime_base: String,
    pub(super) pointer_table_offset: String,
    pub(super) pointer_table_runtime_address: String,
    pub(super) pointer_count: usize,
    pub(super) pointer_table_size: usize,
    pub(super) pointer_table_sha256: String,
    pub(super) target_arena_offset: String,
    pub(super) target_arena_runtime_address: String,
    pub(super) target_arena_size: usize,
    pub(super) target_arena_sha256: String,
    pub(super) association_status: PracticalResultOpaquePointerRunAssociationStatus,
    pub(super) association_reason: String,
    #[serde(default)]
    pub(super) adopted_consumer_exclusion_evidence:
        Option<PracticalResultOpaquePointerRunConsumerExclusionEvidence>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum PracticalResultOpaquePointerRunAssociationStatus {
    Unresolved,
    ExcludedFromSourceAtlasConsumerDenominator,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultOpaquePointerRunConsumerExclusionEvidence {
    pub(super) analysis_report_path: String,
    pub(super) analysis_report_sha256: String,
    pub(super) source_bin_sha256: String,
    pub(super) loaded_image_sha256: String,
    pub(super) declared_entrypoints: Vec<PracticalResultOpaquePointerRunEntrypointEvidence>,
    pub(super) value_flow_state_budget: usize,
    pub(super) decoded_pointer_count: usize,
    pub(super) raw_address_reference_count: usize,
    pub(super) external_raw_address_reference_count: usize,
    pub(super) reachable_derived_reference_count: usize,
    pub(super) reachable_pointer_table_reference_count: usize,
    pub(super) reachable_target_arena_reference_count: usize,
    pub(super) profiled_non_address_exhausted_seed_ids: Vec<String>,
    pub(super) unprofiled_exhausted_seed_count: usize,
    pub(super) unresolved_indirect_jump_count: usize,
    pub(super) analysis_product_build_input: bool,
    pub(super) conclusion: PracticalResultOpaquePointerRunConsumerExclusionConclusion,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultOpaquePointerRunEntrypointEvidence {
    pub(super) source_reference_offset: String,
    pub(super) runtime_address: String,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum PracticalResultOpaquePointerRunConsumerExclusionConclusion {
    NoReachablePointerRunReferenceInReviewedDeclaredEntrypointFlow,
}
