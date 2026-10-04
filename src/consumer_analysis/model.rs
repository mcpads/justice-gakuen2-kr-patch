use std::path::PathBuf;

use serde::Serialize;

pub const DEFAULT_STATIC_CONSUMER_VALUE_FLOW_STATE_BUDGET: usize = 8192;
pub const DEFAULT_POINTER_RUN_VALUE_FLOW_STATE_BUDGET: usize = 262144;

#[derive(Clone, Debug)]
pub struct StaticConsumerAuditConfig {
    pub cue: PathBuf,
    pub output: PathBuf,
    pub value_flow_state_budget: usize,
    pub pointer_run_value_flow_state_budget: usize,
}

#[derive(Debug, Serialize)]
pub struct StaticConsumerAuditReport {
    pub kind: String,
    pub source_bin_sha256: String,
    pub loaded_image_count: usize,
    pub analyzed_image_count: usize,
    pub unresolved_entrypoint_image_count: usize,
    pub image_without_runtime_base_count: usize,
    pub reachable_instruction_count: usize,
    pub value_flow_seed_count: usize,
    pub value_flow_instruction_state_count: usize,
    pub value_flow_budget_exhausted_seed_count: usize,
    pub unresolved_indirect_jump_count: usize,
    pub unresolved_indirect_call_count: usize,
    pub declared_indirect_jump_profile_count: usize,
    pub declared_indirect_jump_table_entry_count: usize,
    pub bounded_jump_table_count: usize,
    pub bounded_jump_table_entry_count: usize,
    pub declared_pointer_run_count: usize,
    pub pointer_run_decoded_pointer_count: usize,
    pub pointer_run_raw_address_reference_count: usize,
    pub pointer_run_external_raw_address_reference_count: usize,
    pub pointer_run_reachable_derived_reference_count: usize,
    pub pointer_run_reachable_pointer_table_reference_count: usize,
    pub pointer_run_reachable_target_arena_reference_count: usize,
    pub pointer_run_reachable_derived_instruction_site_count: usize,
    pub pointer_run_profiled_non_address_exhausted_seed_count: usize,
    pub pointer_run_unprofiled_exhausted_seed_count: usize,
    pub pointer_run_value_flow_state_budget: usize,
    pub declared_source_region_count: usize,
    pub consumer_bound_declared_source_region_count: usize,
    pub consumer_unbound_declared_source_region_count: usize,
    pub fully_entrypoint_reachable_consumer_bound_source_region_count: usize,
    pub source_region_scope_complete: bool,
    pub direct_disc_record_loader_call_candidate_count: usize,
    pub entrypoint_reachable_disc_record_loader_call_candidate_count: usize,
    pub entrypoint_unreachable_disc_record_loader_call_candidate_count: usize,
    pub catalog_index_resolved_disc_record_loader_call_candidate_count: usize,
    pub catalog_index_unresolved_disc_record_loader_call_candidate_count: usize,
    pub profiled_selector_table_load_count: usize,
    pub profiled_custom_record_table_load_count: usize,
    pub finite_profiled_selector_table_load_count: usize,
    pub dynamic_profiled_selector_table_load_count: usize,
    pub source_record_resolved_disc_record_loader_call_candidate_count: usize,
    pub source_record_unresolved_disc_record_loader_call_candidate_count: usize,
    pub catalog_or_static_owner_resolved_disc_record_loader_call_candidate_count: usize,
    pub catalog_and_static_owner_unresolved_disc_record_loader_call_candidate_count: usize,
    pub disc_record_loader_selector_state_access_candidate_count: usize,
    pub disc_record_loader_selector_state_unique_memory_access_site_count: usize,
    pub disc_record_loader_selector_state_unique_read_site_count: usize,
    pub disc_record_loader_selector_state_unique_write_site_count: usize,
    pub disc_record_loader_selector_state_classified_write_site_count: usize,
    pub disc_record_loader_selector_state_exact_value_write_site_count: usize,
    pub disc_record_loader_selector_state_bounded_value_write_site_count: usize,
    pub disc_record_loader_selector_state_runtime_source_write_site_count: usize,
    pub disc_record_loader_selector_state_unclassified_write_site_count: usize,
    pub disc_record_loader_selector_upstream_state_access_candidate_count: usize,
    pub disc_record_loader_selector_upstream_state_unique_memory_access_site_count: usize,
    pub disc_record_loader_selector_upstream_state_unique_read_site_count: usize,
    pub disc_record_loader_selector_upstream_state_unique_write_site_count: usize,
    pub disc_record_loader_selector_upstream_state_classified_write_site_count: usize,
    pub disc_record_loader_selector_upstream_state_exact_value_write_site_count: usize,
    pub disc_record_loader_selector_upstream_state_bounded_value_write_site_count: usize,
    pub disc_record_loader_selector_upstream_state_runtime_source_write_site_count: usize,
    pub disc_record_loader_selector_upstream_state_unclassified_write_site_count: usize,
    pub declared_active_consumer_sink_count: usize,
    pub reachable_declared_active_consumer_sink_count: usize,
    pub unreachable_declared_active_consumer_sink_count: usize,
    pub declared_dormant_candidate_sink_count: usize,
    pub reachable_declared_dormant_candidate_sink_count: usize,
    pub unreachable_declared_dormant_candidate_sink_count: usize,
    pub declared_consumer_edge_count: usize,
    pub entrypoint_reachable_declared_consumer_edge_count: usize,
    pub entrypoint_unresolved_declared_consumer_edge_count: usize,
    pub declared_dormant_candidate_edge_count: usize,
    pub declared_consumer_reachability_complete: bool,
    pub semantic_sink_classification_complete: bool,
    pub declared_renderer_call_census_count: usize,
    pub direct_renderer_call_site_count: usize,
    pub entrypoint_reachable_direct_renderer_call_site_count: usize,
    pub classified_direct_renderer_call_site_count: usize,
    pub finite_source_domain_direct_renderer_call_site_count: usize,
    pub dynamic_source_domain_direct_renderer_call_site_count: usize,
    pub closed_dynamic_source_boundary_direct_renderer_call_site_count: usize,
    pub renderer_call_site_classification_complete: bool,
    pub renderer_source_domain_analysis_complete: bool,
    pub product_build_input: bool,
    pub images: Vec<LoadedImageConsumerAudit>,
    pub declared_source_regions: Vec<StaticConsumerSourceRegionAudit>,
    pub declared_consumer_edges: Vec<StaticConsumerEdgeAudit>,
    pub declared_dormant_candidate_edges: Vec<StaticConsumerEdgeAudit>,
    pub limitations: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct StaticConsumerSourceRegionAudit {
    pub source_record_path: String,
    pub source_region_id: String,
    pub source_member_indices: Vec<u8>,
    pub source_catalog_index: u16,
    pub consumer_binding_status: String,
    pub declared_edge_ids: Vec<String>,
    pub dormant_candidate_edge_ids: Vec<String>,
    pub evidence: String,
}

#[derive(Debug, Serialize)]
pub struct LoadedImageConsumerAudit {
    pub path: String,
    pub loaded_bytes_sha256: String,
    pub loaded_byte_count: usize,
    pub runtime_base: Option<String>,
    pub declared_entrypoints: Vec<DeclaredEntrypointAudit>,
    pub analysis_status: String,
    pub executable_domain_source: Option<String>,
    pub executable_ranges: Vec<ExecutableRangeAudit>,
    pub reachability_refinement_pass_count: usize,
    pub reachable_instruction_count: usize,
    pub reachable_lui_instruction_count: usize,
    pub decode_failure_count: usize,
    pub decode_failure_offsets: Vec<String>,
    pub resolved_register_transfer_count: usize,
    pub resolved_direct_call_argument_count: usize,
    pub value_flow_seed_count: usize,
    pub value_flow_instruction_state_count: usize,
    pub value_flow_budget_exhausted_seed_count: usize,
    pub value_flow_budget_exhausted_seed_offsets: Vec<String>,
    pub unresolved_indirect_jump_count: usize,
    pub unresolved_indirect_jump_offsets: Vec<String>,
    pub unresolved_indirect_call_count: usize,
    pub unresolved_indirect_call_offsets: Vec<String>,
    pub declared_indirect_jumps: Vec<StaticDeclaredIndirectJumpAudit>,
    pub bounded_jump_table_count: usize,
    pub bounded_jump_table_entry_count: usize,
    pub outside_loaded_image_transfer_target_count: usize,
    pub outside_loaded_image_transfer_targets: Vec<String>,
    pub disc_record_load_call_candidates: Vec<StaticDiscRecordLoadAudit>,
    pub disc_record_loader_selector_state_accesses: Vec<StaticProfiledStateAccessAudit>,
    pub disc_record_loader_selector_upstream_state_accesses: Vec<StaticProfiledStateAccessAudit>,
    pub declared_pointer_runs: Vec<StaticPointerRunAudit>,
    pub declared_renderer_call_censuses: Vec<StaticRendererCallCensusAudit>,
    pub declared_semantic_sinks: Vec<StaticConsumerSinkAudit>,
    pub declared_dormant_candidate_sinks: Vec<StaticConsumerSinkAudit>,
}

#[derive(Clone, Debug, Serialize)]
pub struct StaticDeclaredIndirectJumpAudit {
    pub id: String,
    pub transfer_instruction_offset: String,
    pub transfer_runtime_address: String,
    pub table_offset: String,
    pub table_runtime_address: String,
    pub table_entry_count: usize,
    pub distinct_target_count: usize,
    pub targets: Vec<String>,
    pub evidence: String,
}

#[derive(Debug, Serialize)]
pub struct StaticRendererCallCensusAudit {
    pub id: String,
    pub renderer_runtime_address: String,
    pub direct_call_site_count: usize,
    pub entrypoint_reachable_call_site_count: usize,
    pub classified_call_site_count: usize,
    pub finite_source_domain_call_site_count: usize,
    pub dynamic_source_domain_call_site_count: usize,
    pub call_site_classification_complete: bool,
    pub source_domain_complete: bool,
    pub call_sites: Vec<StaticRendererCallSiteAudit>,
    pub limitations: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct StaticRendererCallSiteAudit {
    pub instruction_offset: String,
    pub instruction_runtime_address: String,
    pub entrypoint_reachable: bool,
    pub argument_source_class: String,
    pub source_domain_status: String,
    pub evidence: String,
    pub dynamic_source_boundary: Option<StaticRendererArgumentDynamicBoundaryAudit>,
}

#[derive(Debug, Serialize)]
pub struct StaticRendererArgumentDynamicBoundaryAudit {
    pub status: String,
    pub active_record_mask_runtime_address: String,
    pub selected_record_index_object_offset: String,
    pub active_record_count_object_offset: String,
    pub valid_state_selected_record_index_range: [i16; 2],
    pub valid_state_precondition: String,
    pub record_table_runtime_address: String,
    pub record_stride_bytes: usize,
    pub character_selector_record_offset: usize,
    pub character_selector_machine_value_range: [u16; 2],
    pub mapping_table_image_offset: String,
    pub mapping_table_runtime_address: String,
    pub mapping_result_machine_value_range: [u16; 2],
    pub placement_table_image_offset: String,
    pub placement_table_runtime_address: String,
    pub placement_record_stride_bytes: usize,
    pub native_save_restore_source: String,
    pub native_save_persistence_destination: String,
    pub evidence: Vec<String>,
    pub limitations: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct StaticPointerRunAudit {
    pub id: String,
    pub pointer_table_runtime_byte_range: [String; 2],
    pub pointer_count: usize,
    pub target_arena_runtime_byte_range: [String; 2],
    pub target_arena_size: usize,
    pub distinct_target_count: usize,
    pub decoded_targets: Vec<StaticPointerTargetAudit>,
    pub raw_address_references: Vec<StaticRawAddressReferenceAudit>,
    pub external_raw_address_reference_count: usize,
    pub reachable_derived_references: Vec<StaticDerivedAddressReferenceAudit>,
    pub reachable_pointer_table_reference_count: usize,
    pub reachable_target_arena_reference_count: usize,
    pub reachable_derived_instruction_site_count: usize,
    pub reachable_analysis_state_budget: usize,
    pub reachable_analysis_budget_exhausted_seed_count: usize,
    pub reachable_analysis_budget_exhaustions: Vec<StaticValueFlowSeedExhaustionAudit>,
    pub reachable_analysis_profiled_non_address_exhausted_seed_count: usize,
    pub reachable_analysis_profiled_non_address_seed_ids: Vec<String>,
    pub reachable_analysis_unprofiled_exhausted_seed_count: usize,
    pub reachable_analysis_unresolved_indirect_jump_count: usize,
    pub declared_entrypoint_flow_limits_remain: bool,
    pub reference_assessment: String,
}

#[derive(Debug, Serialize)]
pub struct StaticValueFlowSeedExhaustionAudit {
    pub seed_instruction_offset: String,
    pub seed_instruction_runtime_address: String,
    pub processed_state_count: usize,
    pub discovered_state_count: usize,
    pub distinct_instruction_offset_count: usize,
    pub maximum_states_at_instruction_offset: usize,
    pub pending_state_count: usize,
    pub distinct_frontier_instruction_offset_count: usize,
    pub frontier_instruction_offsets: Vec<String>,
    pub frontier_instruction_runtime_addresses: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct StaticPointerTargetAudit {
    pub index: usize,
    pub storage_offset: String,
    pub storage_runtime_address: String,
    pub target_runtime_address: String,
    pub target_loaded_image_offset: String,
    pub target_arena_offset: String,
}

#[derive(Debug, Serialize)]
pub struct StaticRawAddressReferenceAudit {
    pub source_offset: String,
    pub source_runtime_address: String,
    pub value_runtime_address: String,
    pub target_region: String,
    pub source_region: String,
}

#[derive(Debug, Serialize)]
pub struct StaticDerivedAddressReferenceAudit {
    pub seed_instruction_offset: String,
    pub seed_instruction_runtime_address: String,
    pub instruction_offset: String,
    pub instruction_runtime_address: String,
    pub decoded_instruction: String,
    pub derived_runtime_address: String,
    pub derived_target_region: String,
    pub derived_address_kind: String,
    pub loaded_value_storage_runtime_address: Option<String>,
    pub loaded_value_storage_region: Option<String>,
    pub loaded_value_instruction_offset: Option<String>,
    pub loaded_value_width_bytes: Option<u8>,
    pub loaded_value_sign_extended: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct StaticProfiledStateAccessAudit {
    pub profile_id: String,
    pub profile_runtime_byte_range: [String; 2],
    pub seed_instruction_offsets: Vec<String>,
    pub instruction_offset: String,
    pub instruction_runtime_address: String,
    pub decoded_instruction: String,
    pub access_runtime_addresses: Vec<String>,
    pub operation: String,
    pub width_bytes: usize,
    pub entrypoint_reachable: bool,
    pub writer_evidence: Option<StaticSelectorWriterEvidenceAudit>,
}

#[derive(Debug, Serialize)]
pub struct StaticSelectorWriterEvidenceAudit {
    pub classification: String,
    pub value_resolution: String,
    pub exact_value_candidates: Vec<u8>,
    pub bounded_value_range: Option<[u8; 2]>,
    pub source_runtime_byte_ranges: Vec<[String; 2]>,
    pub evidence: String,
}

#[derive(Debug, Serialize)]
pub struct StaticDiscRecordLoadAudit {
    pub call_runtime_address: String,
    pub instruction_offset: String,
    pub catalog_index_candidates: Vec<u16>,
    pub catalog_index_resolution: String,
    pub catalog_index_argument_producer: String,
    pub catalog_index_memory_load: Option<StaticCatalogIndexMemoryLoadAudit>,
    pub forwarding_call_runtime_addresses: Vec<String>,
    pub static_owner_id: Option<String>,
    pub static_owner_function_runtime_address: Option<String>,
    pub static_owner_destination_runtime_address: Option<String>,
    pub static_owner_direct_caller_runtime_addresses: Vec<String>,
    pub catalog_record_paths: Vec<String>,
    pub declared_source_region_paths: Vec<String>,
    pub entrypoint_reachable: bool,
    pub evidence: String,
}

#[derive(Debug, Serialize)]
pub struct StaticCatalogIndexMemoryLoadAudit {
    pub load_runtime_address: String,
    pub load_kind: String,
    pub address_resolution: String,
    pub effective_address_when_selector_zero: Option<String>,
    pub selector_register: Option<String>,
    pub selector_stride_bytes: Option<u32>,
    pub selector_origin_kind: Option<String>,
    pub selector_origin_runtime_address: Option<String>,
    pub declared_selector_domain: Option<StaticDeclaredSelectorDomainAudit>,
    pub custom_record_selector: Option<StaticCustomRecordSelectorAudit>,
}

#[derive(Debug, Serialize)]
pub struct StaticDeclaredSelectorDomainAudit {
    pub state_runtime_address: String,
    pub value_resolution: String,
    pub known_value_candidates: Vec<u8>,
    pub unresolved_runtime_sources: Vec<String>,
    pub effective_address_candidates: Vec<String>,
    pub table_value_candidates: Vec<u16>,
    pub evidence: String,
}

#[derive(Debug, Serialize)]
pub struct StaticCustomRecordSelectorAudit {
    pub profile_id: String,
    pub record_index_state_runtime_address: String,
    pub record_index_value_resolution: String,
    pub known_record_index_candidates: Vec<u8>,
    pub unresolved_record_index_sources: Vec<String>,
    pub record_base_runtime_address: String,
    pub record_stride_bytes: u32,
    pub selector_record_byte_offsets: Vec<u32>,
    pub selector_offset_condition_state_runtime_address: Option<String>,
    pub selector_runtime_address_candidates: Vec<String>,
    pub selector_value_resolution: String,
    pub known_selector_value_candidates: Vec<u8>,
    pub unresolved_selector_sources: Vec<String>,
    pub effective_address_candidates: Vec<String>,
    pub table_value_candidates: Vec<u16>,
    pub known_catalog_record_paths: Vec<String>,
    pub evidence: String,
}

#[derive(Debug, Serialize)]
pub struct ExecutableRangeAudit {
    pub start_offset: String,
    pub end_offset: String,
}

#[derive(Debug, Serialize)]
pub struct DeclaredEntrypointAudit {
    pub role: String,
    pub source_reference_kind: String,
    pub source_reference_offset: String,
    pub runtime_address: String,
}

#[derive(Debug, Serialize)]
pub struct StaticConsumerSinkAudit {
    pub id: String,
    pub kind: String,
    pub sites: Vec<StaticConsumerSinkSiteAudit>,
    pub entrypoint_reachable: bool,
    pub evidence: String,
}

#[derive(Debug, Serialize)]
pub struct StaticConsumerSinkSiteAudit {
    pub runtime_address: String,
    pub instruction_offset: Option<String>,
    pub entrypoint_reachable: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct StaticConsumerEdgeAudit {
    pub id: String,
    pub source_record_path: String,
    pub source_region_id: String,
    pub source_member_indices: Vec<u8>,
    pub relationship: String,
    pub sink_id: String,
    pub consumer_image_path: String,
    pub consumer_runtime_addresses: Vec<String>,
    pub sink_entrypoint_reachable: bool,
    pub evidence: String,
    pub runtime_confirmation_required: bool,
}
