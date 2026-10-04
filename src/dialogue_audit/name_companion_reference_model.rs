use std::path::PathBuf;

use serde::Serialize;

#[derive(Debug, Clone)]
pub struct NameCompanionReferenceAuditConfig {
    pub cue: PathBuf,
    pub output: PathBuf,
    pub address_flow_state_budget: usize,
}

#[derive(Debug, Serialize)]
pub struct NameCompanionReferenceAuditReport {
    pub kind: String,
    pub source_bin_sha256: String,
    pub target_runtime_byte_range: [String; 2],
    pub scanned_loaded_image_count: usize,
    pub skipped_loaded_image_paths: Vec<String>,
    pub address_flow_state_budget: usize,
    pub address_flow_seed_count: usize,
    pub address_flow_instruction_state_count: usize,
    pub address_flow_budget_exhausted_seed_count: usize,
    pub all_address_flows_completed_within_budget: bool,
    pub static_memory_access_site_count: usize,
    pub static_read_site_count: usize,
    pub static_write_site_count: usize,
    pub runtime_confirmation_required: bool,
    pub candidates: Vec<NameCompanionStaticReference>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct NameCompanionStaticReference {
    pub source_path: String,
    pub loaded_bytes_sha256: String,
    pub loaded_runtime_base: String,
    pub seed_loaded_offsets: Vec<String>,
    pub instruction_loaded_offset: String,
    pub instruction_runtime_address: String,
    pub access_runtime_addresses: Vec<String>,
    pub operation: String,
    pub width_bytes: usize,
}
