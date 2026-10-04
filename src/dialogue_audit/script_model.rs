use std::path::PathBuf;

use serde::Serialize;

#[derive(Debug, Clone)]
pub struct DialogueSceneAuditConfig {
    pub cue: PathBuf,
    pub codebook: PathBuf,
    pub runtime_ram: PathBuf,
    pub runtime_frame: PathBuf,
    pub output: PathBuf,
}

#[derive(Debug, Serialize)]
pub struct DialogueSceneAuditReport {
    pub kind: String,
    pub implementation: String,
    pub source_bin_sha256: String,
    pub codebook_sha256: String,
    pub source_corpus_sha256: String,
    pub script_runtime_base: String,
    pub script_interpreter: String,
    pub opcode_dispatch_table: String,
    pub primary_script_inventory_complete: bool,
    pub source_asset_count: usize,
    pub resolved_primary_script_asset_count: usize,
    pub unresolved_primary_script_asset_count: usize,
    pub bank_table_count: usize,
    pub route_binding_count: usize,
    pub unique_route_table_count: usize,
    pub route_entry_reference_count: usize,
    pub unique_route_entry_count: usize,
    pub route_slot_count: usize,
    pub unique_entrypoint_count: usize,
    pub reachable_command_count: usize,
    pub contextual_message_reference_count: usize,
    pub referenced_coordinate_count: usize,
    pub referenced_semantic_group_count: usize,
    pub unreferenced_coordinate_count: usize,
    pub unreferenced_semantic_group_count: usize,
    pub asset_shard_directory: String,
    pub resolved_primary_script_assets: Vec<DialogueScriptAssetSummary>,
    pub unresolved_primary_script_assets: Vec<DialogueUnresolvedPrimaryScriptAsset>,
    pub admitted_scene_count: usize,
    pub admitted_message_reference_count: usize,
    pub runtime_evidence: DialogueSceneRuntimeEvidence,
    pub scenes: Vec<DialogueSceneAudit>,
    pub limitations: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct DialogueUnresolvedPrimaryScriptAsset {
    pub source_path: String,
    pub last_established_boundary: String,
    pub first_unestablished_boundary: String,
    pub detail: String,
}

#[derive(Debug, Serialize)]
pub struct DialogueScriptAssetSummary {
    pub source_path: String,
    pub decoded_sha256: String,
    pub primary_region_end: String,
    pub bank_table_count: usize,
    pub route_binding_count: usize,
    pub unique_route_table_count: usize,
    pub route_entry_reference_count: usize,
    pub unique_route_entry_count: usize,
    pub route_slot_count: usize,
    pub unique_entrypoint_count: usize,
    pub reachable_command_count: usize,
    pub contextual_message_reference_count: usize,
    pub referenced_coordinate_count: usize,
    pub referenced_semantic_group_count: usize,
    pub shard_path: String,
    pub shard_sha256: String,
}

#[derive(Debug, Serialize)]
pub struct DialogueScriptAssetAudit {
    pub kind: String,
    pub source_path: String,
    pub decoded_sha256: String,
    pub primary_region_start: String,
    pub primary_region_end: String,
    pub bank_tables: Vec<DialogueScriptBankTableAudit>,
    pub unique_route_table_count: usize,
    pub route_entry_reference_count: usize,
    pub unique_route_entry_count: usize,
    pub route_slot_count: usize,
    pub unique_entrypoint_count: usize,
    pub reachable_command_count: usize,
    pub contextual_message_reference_count: usize,
    pub referenced_coordinate_count: usize,
    pub referenced_semantic_group_count: usize,
    pub commands: Vec<DialogueScriptCommandAudit>,
    pub message_bindings: Vec<DialogueScriptMessageBinding>,
}

#[derive(Debug, Serialize)]
pub struct DialogueScriptBankTableAudit {
    pub bank_selector: usize,
    pub decoded_offset: String,
    pub route_bindings: Vec<DialogueScriptRouteAudit>,
}

#[derive(Debug, Serialize)]
pub struct DialogueScriptRouteAudit {
    pub variant_selector: usize,
    pub decoded_offset: String,
    pub slot_count: usize,
    pub entrypoint_count: usize,
    pub entrypoints: Vec<Option<String>>,
    pub segments: Vec<DialogueScriptRouteSegmentAudit>,
}

#[derive(Debug, Serialize)]
pub struct DialogueScriptRouteSegmentAudit {
    pub slot_index: usize,
    pub entrypoint: Option<String>,
    pub command_count: usize,
    pub message_reference_count: usize,
    pub messages: Vec<DialogueScriptMessageBinding>,
}

#[derive(Debug, Serialize)]
pub struct DialogueScriptCommandAudit {
    pub decoded_offset: String,
    pub runtime_address: String,
    pub opcode: String,
    pub width: usize,
    pub handler_runtime_address: String,
    pub flow: String,
    pub command_kind: String,
    pub raw_bytes: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DialogueScriptMessageBinding {
    pub bank_selector: usize,
    pub command_offset: String,
    pub opcode: String,
    pub entry_index: usize,
    pub coordinate_id: String,
    pub semantic_source_sha256: String,
    pub source_markup: String,
}

#[derive(Debug, Serialize)]
pub struct DialogueSceneRuntimeEvidence {
    pub ram_path: String,
    pub ram_sha256: String,
    pub frame_path: String,
    pub frame_sha256: String,
    pub decoded_asset_ram_difference_count: usize,
    pub observed_bank_selector: usize,
    pub observed_variant_selector: usize,
    pub observed_route_table: String,
    pub observed_initial_command: String,
    pub observed_cursor_while_first_message_visible: String,
    pub observed_opcode: String,
}

#[derive(Debug, Serialize)]
pub struct DialogueSceneAudit {
    pub scene_id: String,
    pub source_path: String,
    pub bank_selector: usize,
    pub variant_selector: usize,
    pub top_level_table_offset: String,
    pub variant_table_offset: String,
    pub route_table_offset: String,
    pub initial_command_offset: String,
    pub speaker_identity: String,
    pub speaker_evidence: String,
    pub segment_count: usize,
    pub message_reference_count: usize,
    pub segments: Vec<DialogueSceneSegmentAudit>,
}

#[derive(Debug, Serialize)]
pub struct DialogueSceneSegmentAudit {
    pub segment_index: usize,
    pub decoded_offset: String,
    pub runtime_address: String,
    pub command_count: usize,
    pub message_reference_count: usize,
    pub commands: Vec<DialogueSceneCommandAudit>,
}

#[derive(Debug, Serialize)]
pub struct DialogueSceneCommandAudit {
    pub decoded_offset: String,
    pub runtime_address: String,
    pub opcode: String,
    pub handler_runtime_address: String,
    pub command_kind: String,
    pub raw_bytes: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_segment_index: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<DialogueSceneMessageReference>,
}

#[derive(Debug, Serialize)]
pub struct DialogueSceneMessageReference {
    pub bank_selector: usize,
    pub entry_index: usize,
    pub coordinate_id: String,
    pub semantic_source_sha256: String,
    pub source_markup: String,
}
