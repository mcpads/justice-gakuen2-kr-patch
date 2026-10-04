use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct DialogueSelectorConsumerReport {
    pub typed_isa_profile: String,
    pub selector_table_runtime_start: String,
    pub selector_count: usize,
    pub direct_load_count: usize,
    pub direct_entry_load_count: usize,
    pub consumers: Vec<DialogueSelectorConsumerAudit>,
}

#[derive(Debug, Serialize)]
pub struct DialogueSelectorConsumerAudit {
    pub selector_index: usize,
    pub pointer_storage_runtime_address: String,
    pub load_instruction_runtime_address: String,
    pub entry_pointer_loads: Vec<DialogueSelectorEntryConsumerAudit>,
}

#[derive(Debug, Serialize)]
pub struct DialogueSelectorEntryConsumerAudit {
    pub entry_index: usize,
    pub load_instruction_runtime_address: String,
}
