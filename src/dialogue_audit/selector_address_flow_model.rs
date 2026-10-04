use std::path::PathBuf;

use serde::Serialize;

#[derive(Debug, Clone)]
pub struct DialogueSelectorAddressFlowAuditConfig {
    pub cue: PathBuf,
    pub output: PathBuf,
}

#[derive(Debug, Serialize)]
pub struct DialogueSelectorAddressFlowAuditReport {
    pub kind: String,
    pub source_mgame_sha256: String,
    pub typed_isa_profile: String,
    pub selector_table_runtime_start: String,
    pub selector_count: usize,
    pub selector_region_high_half_seed_count: usize,
    pub selector_region_high_half_seeds: Vec<DialogueSelectorRegionSeed>,
    pub table_base_materialization_count: usize,
    pub table_base_materializations: Vec<DialogueSelectorTableBaseMaterialization>,
    pub message_entry_constructor_runtime_address: String,
    pub direct_message_entry_constructor_call_count: usize,
    pub direct_message_entry_constructor_calls: Vec<DialogueMessageConstructorCall>,
    pub resolved_register_message_entry_constructor_call_count: usize,
    pub resolved_register_message_entry_constructor_calls:
        Vec<DialogueResolvedMessageConstructorCall>,
    pub message_bank_constructor_runtime_address: String,
    pub direct_message_bank_constructor_call_count: usize,
    pub direct_message_bank_constructor_calls: Vec<DialogueMessageConstructorCall>,
    pub resolved_register_message_bank_constructor_call_count: usize,
    pub resolved_register_message_bank_constructor_calls:
        Vec<DialogueResolvedMessageConstructorCall>,
    pub diary_save_slot_marker_flow: DiarySaveSlotMarkerFlow,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DiarySaveSlotMarkerFlow {
    pub kind: String,
    pub evidence_scope: String,
    pub selector_index: usize,
    pub clear_prompt_entry_index: usize,
    pub clear_prompt_action_row_index: usize,
    pub clear_prompt_action_row_runtime_address: String,
    pub clear_prompt_flag_runtime_address: String,
    pub alternate_save_path_flag_runtime_address: String,
    pub sentinel_state_runtime_address: String,
    pub sentinel_state_value: u8,
    pub required_alternate_state_flag_value: u8,
    pub selected_slot_runtime_address: String,
    pub save_buffer_pointer_storage_runtime_address: String,
    pub save_buffer_header_offset: String,
    pub save_buffer_state_marker_offset: String,
    pub save_buffer_slot_marker_start_offset: String,
    pub slot_marker_count: usize,
    pub mirror_buffer_offset: String,
    pub marker_value: u8,
    pub marker_producer_runtime_address: String,
    pub marker_producer_call_runtime_addresses: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DialogueSelectorRegionSeed {
    pub seed_instruction_runtime_address: String,
    pub diagnostic_use_window: Vec<DialogueTypedInstruction>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DialogueSelectorTableBaseMaterialization {
    pub table_base_runtime_address: String,
    pub seed_instruction_runtime_address: String,
    pub materialization_instruction_runtime_address: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DialogueMessageConstructorCall {
    pub call_instruction_runtime_address: String,
    pub diagnostic_setup_window: Vec<DialogueTypedInstruction>,
    pub enclosing_function_runtime_address: Option<String>,
    pub direct_enclosing_function_call_count: usize,
    pub direct_enclosing_function_calls: Vec<DialogueFunctionCaller>,
    pub enclosing_function_selector_pointer_load_count: usize,
    pub enclosing_function_selector_pointer_loads: Vec<DialogueSelectorPointerLoad>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct DialogueSelectorPointerLoad {
    pub selector_index: usize,
    pub pointer_storage_runtime_address: String,
    pub load_instruction_runtime_address: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DialogueFunctionCaller {
    pub call_instruction_runtime_address: String,
    pub diagnostic_setup_window: Vec<DialogueTypedInstruction>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DialogueResolvedMessageConstructorCall {
    pub seed_instruction_runtime_address: String,
    pub call_instruction_runtime_address: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DialogueTypedInstruction {
    pub runtime_address: String,
    pub instruction: String,
}
