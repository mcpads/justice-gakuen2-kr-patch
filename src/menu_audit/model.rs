use std::collections::BTreeSet;
use std::path::PathBuf;

use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MenuConsumerEvidence {
    KanriMenuLabelTable,
    KanriPlacementRecordTable,
    KanriStatusRatingTable,
    MgtitPlacementRecordTable,
    NewoptDirectStringWriterCall,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MenuNonTextEvidence {
    KanriCountedObjectRecordTable,
    Koubai2CommandByteSequence,
    MiniselPrimitiveRecordTable,
    Plsel5CountedParameterPointerTable,
}

#[derive(Debug, Clone)]
pub struct MenuCodeAuditConfig {
    pub cue: PathBuf,
    pub output: PathBuf,
    pub address_flow_state_budget: usize,
}

#[derive(Debug, Serialize)]
pub struct StringCandidate {
    pub target_offset: String,
    pub consumer_evidence: Vec<MenuConsumerEvidence>,
    pub consumer_reference_count: usize,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub non_text_evidence: Vec<MenuNonTextEvidence>,
    pub pointer_offsets: Vec<String>,
    pub address_materialization_offsets: Vec<String>,
    pub memory_access_offsets: Vec<String>,
    pub loaded_word_references: Vec<LoadedWordReferenceAudit>,
    pub entrypoint_reachable_address_materialization_references: Vec<AddressFlowReferenceAudit>,
    pub entrypoint_reachable_memory_access_references: Vec<AddressFlowReferenceAudit>,
    pub entrypoint_reachable_loaded_word_references: Vec<ReachableLoadedWordReferenceAudit>,
    pub entrypoint_reachable_direct_pointer_load_references: Vec<ReachableLoadedWordReferenceAudit>,
    pub shared_references: Vec<SharedStringReferences>,
    pub length: usize,
    pub raw_codes: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct SharedStringReferences {
    pub source_path: String,
    pub pointer_offsets: Vec<String>,
    pub address_materialization_offsets: Vec<String>,
    pub memory_access_offsets: Vec<String>,
    pub loaded_word_references: Vec<LoadedWordReferenceAudit>,
}

#[derive(Debug, Serialize)]
pub struct LoadedWordReferenceAudit {
    pub instruction_offset: String,
    pub load_instruction_offset: String,
    pub storage_address: String,
}

#[derive(Debug, Serialize)]
pub struct ReachableLoadedWordReferenceAudit {
    pub seed_offset: String,
    pub instruction_offset: String,
    pub load_instruction_offset: String,
    pub storage_address: String,
}

#[derive(Debug, Serialize)]
pub struct AddressFlowReferenceAudit {
    pub seed_offset: String,
    pub instruction_offset: String,
}

#[derive(Debug, Serialize)]
pub struct OverlayAudit {
    pub path: String,
    pub file_size: usize,
    pub consumer_confirmed_string_count: usize,
    pub consumer_confirmed_reference_count: usize,
    pub confirmed_non_text_candidate_count: usize,
    pub unclassified_entrypoint_reachable_direct_pointer_load_candidate_count: usize,
    pub address_flow_seed_count: usize,
    pub address_flow_instruction_state_count: usize,
    pub address_flow_budget_exhausted_seed_count: usize,
    pub candidate_pointer_reference_count: usize,
    pub candidate_address_materialization_reference_count: usize,
    pub candidate_memory_access_reference_count: usize,
    pub candidate_loaded_word_reference_count: usize,
    pub candidate_entrypoint_reachable_string_count: usize,
    pub candidate_entrypoint_reachable_address_materialization_reference_count: usize,
    pub candidate_entrypoint_reachable_memory_access_reference_count: usize,
    pub candidate_entrypoint_reachable_loaded_word_reference_count: usize,
    pub candidate_entrypoint_reachable_direct_pointer_load_string_count: usize,
    pub candidate_entrypoint_reachable_direct_pointer_load_reference_count: usize,
    pub candidate_shared_pointer_reference_count: usize,
    pub candidate_shared_address_materialization_reference_count: usize,
    pub candidate_shared_memory_access_reference_count: usize,
    pub candidate_shared_loaded_word_reference_count: usize,
    pub candidate_string_count: usize,
    pub code_occurrence_count: usize,
    pub skip_occurrence_count: usize,
    pub unique_normalized_code_count: usize,
    pub control_nibbles: Vec<u8>,
    pub strings: Vec<StringCandidate>,
}

#[derive(Debug, Serialize)]
pub struct AddressFlowSourceAudit {
    pub path: String,
    pub file_size: usize,
    pub seed_count: usize,
    pub instruction_state_count: usize,
    pub budget_exhausted_seed_count: usize,
    pub budget_exhausted_seed_offsets: Vec<String>,
    pub budget_exhausted_seeds: Vec<AddressFlowSeedExhaustionAudit>,
    pub code_reachability: CodeReachabilityAudit,
}

#[derive(Debug, Serialize)]
pub struct CodeReachabilityAudit {
    pub runtime_base: Option<String>,
    pub declared_entrypoints: Vec<String>,
    pub declared_entrypoints_in_loaded_image: bool,
    pub reachable_instruction_count: usize,
    pub reachable_lui_seed_count: usize,
    pub decode_failure_count: usize,
    pub unresolved_indirect_transfer_count: usize,
    pub unresolved_indirect_transfer_offsets: Vec<String>,
    pub unresolved_indirect_transfers: Vec<UnresolvedRegisterTransferAudit>,
    pub unresolved_indirect_jump_count: usize,
    pub unresolved_linked_indirect_call_count: usize,
    pub bounded_jump_table_count: usize,
    pub bounded_jump_table_entry_count: usize,
    pub bounded_jump_table_unique_target_count: usize,
    pub bounded_jump_tables: Vec<BoundedJumpTableAudit>,
    pub abi_return_count: usize,
    pub abi_return_offsets: Vec<String>,
    pub reachable_seed_resolved_register_transfer_count: usize,
    pub reachable_seed_resolved_register_transfer_instruction_count: usize,
    pub reachable_seed_resolved_register_transfers: Vec<ResolvedRegisterTransferAudit>,
    pub control_transfer_resolution_pass_count: usize,
    pub outside_image_transfer_target_count: usize,
    pub control_transfer_in_delay_slot_count: usize,
}

#[derive(Debug, Serialize)]
pub struct BoundedJumpTableAudit {
    pub bound_instruction_offset: String,
    pub transfer_instruction_offset: String,
    pub selector_register: String,
    pub table_address: String,
    pub table_offset: String,
    pub entry_count: usize,
    pub unique_target_count: usize,
    pub targets: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ResolvedRegisterTransferAudit {
    pub seed_offset: String,
    pub instruction_offset: String,
    pub target: String,
    pub target_in_loaded_image: bool,
}

#[derive(Debug, Serialize)]
pub struct UnresolvedRegisterTransferAudit {
    pub instruction_offset: String,
    pub source_register: String,
    pub transfer_kind: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AddressFlowSeedExhaustionAudit {
    pub seed_offset: String,
    pub processed_state_count: usize,
    pub discovered_state_count: usize,
    pub distinct_instruction_offset_count: usize,
    pub maximum_states_at_instruction_offset: usize,
    pub pending_state_count: usize,
    pub distinct_frontier_instruction_offset_count: usize,
}

#[derive(Debug, Serialize)]
pub struct UsedCodeAudit {
    pub code: String,
    pub occurrence_count: usize,
    pub overlays: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct PageAudit {
    pub page: u8,
    pub used_code_count: usize,
    pub unreferenced_candidate_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct SharedReferenceSourceAudit {
    pub path: String,
    pub file_size: usize,
    pub sha256: String,
    pub loaded_text_file_offset: String,
    pub loaded_text_runtime_base: String,
    pub loaded_text_size: usize,
    pub address_flow_seed_count: usize,
    pub address_flow_instruction_state_count: usize,
    pub address_flow_budget_exhausted_seed_count: usize,
    pub address_flow_budget_exhausted_seed_offsets: Vec<String>,
    pub address_flow_budget_exhausted_seeds: Vec<AddressFlowSeedExhaustionAudit>,
    pub overlay_window_pointer_reference_count: usize,
    pub overlay_window_address_materialization_reference_count: usize,
    pub overlay_window_memory_access_reference_count: usize,
    pub overlay_window_loaded_word_reference_count: usize,
}

#[derive(Debug, Serialize)]
pub struct MenuCodeAuditManifest {
    pub kind: String,
    pub implementation: String,
    pub source_bin_sha256: String,
    pub instruction_profile: String,
    pub overlay_runtime_base: String,
    pub pointer_alignment: usize,
    pub instruction_alignment: usize,
    pub address_flow_state_budget: usize,
    pub control_flow_policy: String,
    pub address_materialization_operations: Vec<String>,
    pub memory_access_operations: Vec<String>,
    pub loaded_word_policy: String,
    pub shared_reference_sources: Vec<SharedReferenceSourceAudit>,
    pub maximum_candidate_string_length: usize,
    pub code_normalization: String,
    pub accepted_normalized_codes: String,
    pub dat1_bin_files_scanned: usize,
    pub dat1_address_flow_seed_count: usize,
    pub dat1_address_flow_instruction_state_count: usize,
    pub dat1_address_flow_budget_exhausted_seed_count: usize,
    pub dat1_address_flow_sources: Vec<AddressFlowSourceAudit>,
    pub candidate_overlay_count: usize,
    pub consumer_confirmed_string_count: usize,
    pub consumer_confirmed_reference_count: usize,
    pub confirmed_non_text_candidate_count: usize,
    pub unclassified_entrypoint_reachable_direct_pointer_load_candidate_count: usize,
    pub candidate_pointer_reference_count: usize,
    pub candidate_address_materialization_reference_count: usize,
    pub candidate_memory_access_reference_count: usize,
    pub candidate_loaded_word_reference_count: usize,
    pub candidate_entrypoint_reachable_string_count: usize,
    pub candidate_entrypoint_reachable_address_materialization_reference_count: usize,
    pub candidate_entrypoint_reachable_memory_access_reference_count: usize,
    pub candidate_entrypoint_reachable_loaded_word_reference_count: usize,
    pub candidate_entrypoint_reachable_direct_pointer_load_string_count: usize,
    pub candidate_entrypoint_reachable_direct_pointer_load_reference_count: usize,
    pub candidate_shared_pointer_reference_count: usize,
    pub candidate_shared_address_materialization_reference_count: usize,
    pub candidate_shared_memory_access_reference_count: usize,
    pub candidate_shared_loaded_word_reference_count: usize,
    pub candidate_string_count: usize,
    pub code_occurrence_count: usize,
    pub skip_occurrence_count: usize,
    pub used_code_count: usize,
    pub unreferenced_code_candidate_count: usize,
    pub statically_unreferenced_nonoverlapping_code_count: usize,
    pub proven_reclaimable_code_count: usize,
    pub control_nibbles: Vec<u8>,
    pub pages: Vec<PageAudit>,
    pub overlays: Vec<OverlayAudit>,
    pub used_codes: Vec<UsedCodeAudit>,
    pub unreferenced_code_candidates: Vec<String>,
    pub statically_unreferenced_nonoverlapping_codes: Vec<String>,
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProvisionalCodeEvidence {
    pub dat1_bin_files_scanned: usize,
    pub dat1_address_flow_seed_count: usize,
    pub dat1_address_flow_instruction_state_count: usize,
    pub dat1_address_flow_budget_exhausted_seed_count: usize,
    pub shared_reference_sources: Vec<SharedReferenceSourceAudit>,
    pub candidate_overlay_count: usize,
    pub candidate_string_count: usize,
    pub candidate_pointer_reference_count: usize,
    pub candidate_address_materialization_reference_count: usize,
    pub candidate_memory_access_reference_count: usize,
    pub candidate_loaded_word_reference_count: usize,
    pub candidate_shared_pointer_reference_count: usize,
    pub candidate_shared_address_materialization_reference_count: usize,
    pub candidate_shared_memory_access_reference_count: usize,
    pub candidate_shared_loaded_word_reference_count: usize,
    pub checks: Vec<ProvisionalCodeCheck>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProvisionalCodeCheck {
    pub code: String,
    pub static_reference_overlays: Vec<String>,
    pub wrapped_cell_overlap_codes: Vec<String>,
    pub other_candidate_overlap_codes: Vec<String>,
}

#[derive(Debug, Default)]
pub(super) struct CodeAccumulation {
    pub(super) occurrence_count: usize,
    pub(super) overlays: BTreeSet<String>,
}
