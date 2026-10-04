use std::path::PathBuf;

use serde::Serialize;

use crate::tim::Cell;

#[derive(Debug, Clone)]
pub struct DialogueNameEntryAuditConfig {
    pub cue: PathBuf,
    pub output: PathBuf,
}

#[derive(Debug, Serialize)]
pub struct DialogueNameEntryAuditReport {
    pub kind: String,
    pub source_bin_sha256: String,
    pub overlay_path: String,
    pub overlay_sha256: String,
    pub overlay_size: usize,
    pub overlay_runtime_base: String,
    pub typed_isa_profile: String,
    pub page_pointer_table_file_offset: String,
    pub page_pointer_table_runtime_address: String,
    pub pages: Vec<DialogueNameEntryPageAudit>,
    pub source_selectable_cell_count: usize,
    pub source_unique_selectable_code_count: usize,
    pub source_duplicate_selectable_code_count: usize,
    pub selectable_code_sequence_file_offset: String,
    pub selectable_code_sequence_sha256: String,
    pub selectable_code_sequence_terminator_file_offset: String,
    pub fields: Vec<DialogueNameFieldAudit>,
    pub choice_surfaces: Vec<DialogueNameEntryChoiceSurfaceAudit>,
    pub sex_choice_surface: DialogueNameEntrySexChoiceSurfaceAudit,
    pub input: DialogueNameEntryInputAudit,
    pub renderer: DialogueNameEntryRendererAudit,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub save_transport: Option<DialogueNameSaveTransportAudit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_game_record_transport: Option<DialogueNameRecordTransportAudit>,
    pub message_end_code: String,
    pub copy_routine_runtime_address: String,
    pub nickname_companion_buffer_runtime_address: String,
}

#[derive(Debug, Serialize)]
pub struct DialogueNameEntryRendererAudit {
    pub redisplay_routine_file_offset: String,
    pub redisplay_routine_runtime_address: String,
    pub code_lookup_table_file_offset: String,
    pub code_lookup_table_runtime_address: String,
    pub layout_triplet_table_file_offset: String,
    pub layout_triplet_table_runtime_address: String,
    pub layout_triplet_size_bytes: usize,
    pub sprite_cell_width: usize,
    pub sprite_cell_height: usize,
    pub fields: Vec<DialogueNameEntryRendererFieldAudit>,
    pub atlas_initializer_file_offset: String,
    pub atlas_initializer_runtime_address: String,
    pub uploaded_source_buffers: Vec<String>,
    pub name_font_source_buffer_runtime_address: String,
    pub tagged_hangul_consumer_installed: bool,
}

#[derive(Debug, Serialize)]
pub struct DialogueNameEntryRendererFieldAudit {
    pub name: String,
    pub record_offset: usize,
    pub visible_slot_count: usize,
    pub sprite_buffer_runtime_address: String,
    pub origin_x: usize,
    pub origin_y: usize,
}

#[derive(Debug, Serialize)]
pub struct DialogueNameSaveTransportAudit {
    pub source_path: String,
    pub staging_runtime_address: String,
    pub copied_byte_count: usize,
    pub byte_copy_routine_runtime_address: String,
    pub byte_load_runtime_address: String,
    pub byte_store_runtime_address: String,
    pub save: DialogueNameSaveCopyEdgeAudit,
    pub load: DialogueNameSaveCopyEdgeAudit,
    pub preserves_all_slot_bits: bool,
    pub name_record_staging_offset: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DialogueNameSaveCopyEdgeAudit {
    pub direction: String,
    pub call_runtime_address: String,
    pub staging_argument_register: String,
    pub card_buffer_argument_register: String,
    pub card_buffer_offset: String,
}

#[derive(Debug, Serialize)]
pub struct DialogueNameRecordTransportAudit {
    pub source_path: String,
    pub source_sha256: String,
    pub runtime_base: String,
    pub workspace_pointer_runtime_address: String,
    pub live_record_runtime_address: String,
    pub record_byte_count: usize,
    pub slot_stride: usize,
    pub save_record_workspace_offset: String,
    pub card_record_workspace_offset: String,
    pub workspace_to_card_delta: String,
    pub workspace_clone_byte_count: usize,
    pub byte_copy_routine_runtime_address: String,
    pub byte_load_runtime_address: String,
    pub byte_store_runtime_address: String,
    pub save_call_runtime_address: String,
    pub workspace_clone_call_runtime_address: String,
    pub load_call_runtime_address: String,
    pub fields: Vec<DialogueNameRecordFieldAudit>,
    pub preserves_all_slot_bits: bool,
    pub memory_card_file_offset: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DialogueNameRecordFieldAudit {
    pub name: String,
    pub record_offset: String,
    pub visible_slot_count: usize,
    pub terminator_offset: String,
}

#[derive(Debug, Serialize)]
pub struct DialogueNameEntryInputAudit {
    pub input_routine_file_offset: String,
    pub input_routine_runtime_address: String,
    pub page_cycle_input_mask: String,
    pub physical_page_count: usize,
    pub page_index_object_offset: usize,
    pub record_slot_index_object_offset: usize,
    pub cursor_cell_object_offset: usize,
    pub field_index_object_offset: usize,
    pub nickname_skips_first_source_page: bool,
    pub selection_routine_file_offset: String,
    pub selection_routine_runtime_address: String,
    pub selected_code_store_file_offset: String,
    pub selected_code_store_runtime_address: String,
    pub selected_code_width_bytes: usize,
    pub family_name_record_offset: usize,
    pub given_name_record_offset: usize,
    pub nickname_record_offset: usize,
    pub navigation_dispatch_table_file_offset: String,
    pub navigation_dispatch_table_runtime_address: String,
    pub navigation_dispatch_table_sha256: String,
    pub navigation_position_count: usize,
    pub navigation_candidate_position_count: usize,
    pub navigation_action_position_count: usize,
    pub navigation_handler_count: usize,
    pub navigation_uses_computed_handler_dispatch: bool,
    pub navigation_producer_file_offset: String,
    pub navigation_producer_runtime_address: String,
    pub navigation_source_map_count: usize,
    pub navigation_source_map_sha256: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct DialogueNameEntrySexChoiceSurfaceAudit {
    pub id: String,
    pub producer_file_offset: String,
    pub producer_runtime_address: String,
    pub logical_choice_count: usize,
    pub texture_page: u8,
    pub source_cells: Vec<Cell>,
    pub producer_instruction_file_offsets: Vec<String>,
    pub all_producer_instructions_match: bool,
}

#[derive(Debug, Serialize)]
pub struct DialogueNameEntryChoiceSurfaceAudit {
    pub id: String,
    pub descriptor_table_file_offset: String,
    pub descriptor_table_runtime_address: String,
    pub descriptor_table_sha256: String,
    pub descriptor_size: usize,
    pub descriptor_count: usize,
    pub logical_choice_count: usize,
    pub text_descriptor_indexes: Vec<usize>,
    pub producer_address_materialization_file_offsets: Vec<String>,
    pub producer_loop_bound_file_offsets: Vec<String>,
    pub descriptors: Vec<DialogueNameEntryChoiceDescriptorAudit>,
}

#[derive(Debug, Serialize)]
pub struct DialogueNameEntryChoiceDescriptorAudit {
    pub index: usize,
    pub texture_page: u8,
    pub palette: u8,
    pub cell: Cell,
}

#[derive(Debug, Serialize)]
pub struct DialogueNameEntryPageAudit {
    pub name: String,
    pub file_offset: String,
    pub runtime_address: String,
    pub cell_count: usize,
    pub padding_cell_count: usize,
    pub selectable_cell_count: usize,
    pub unique_selectable_code_count: usize,
    pub source_cells_sha256: String,
    pub source_cells: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct DialogueNameFieldAudit {
    pub name: String,
    pub source_record_offset: String,
    pub visible_glyph_capacity: usize,
    pub destination_buffer_runtime_address: String,
    pub terminator_runtime_address: String,
}
