use serde::Serialize;

use crate::tim::Cell;

pub(super) const DESCRIPTOR_BYTE_COUNT: usize =
    crate::contextual_texture_upload::TEXTURE_UPLOAD_DESCRIPTOR_BYTE_COUNT;

#[derive(Debug, Serialize)]
pub struct ContextualGlyphUploadReport {
    pub source_menu_graphics_preserved: bool,
    pub options_global_menu_write_count: usize,
    pub options_decompressor_input_windows_verified: bool,
    pub options_contextual_code_population_matches: bool,
    pub records_contextual_code_population_matches: bool,
    pub records_source_graphic_restore_population_matches: bool,
    pub options: GlyphUploadContextReport,
    pub options_exit_restore: OptionsAtlasRestoreReport,
    pub records: GlyphUploadContextReport,
    pub records_card_operation_refresh: RecordsCardOperationRefreshReport,
    pub records_exit_restore: GlyphUploadContextReport,
    pub boot_notice: BootNoticeGlyphReport,
}

#[derive(Debug, Serialize)]
pub struct BootNoticeGlyphReport {
    pub loaded_catalog_index: u16,
    pub loaded_menu_runtime_range: [String; 2],
    pub entry_hook_runtime_address: String,
    pub exit_hook_runtime_address: String,
    pub program_runtime_address: String,
    pub program_byte_count: usize,
    pub program_sha256: String,
    pub entry_count: usize,
    pub descriptor_runtime_address: String,
    pub restore_program_runtime_address: String,
    pub reuses_records_payloads: bool,
    pub runtime_execution_verified: bool,
}

#[derive(Debug, Serialize)]
pub struct RecordsCardOperationRefreshReport {
    pub context: String,
    pub entry_count: usize,
    pub hook_path: String,
    pub hook_offsets: Vec<String>,
    pub hook_runtime_addresses: Vec<String>,
    pub original_call_address: String,
    pub upload_routine_address: String,
    pub program_storage_path: String,
    pub program_offset: String,
    pub program_runtime_address: String,
    pub program_byte_count: usize,
    pub program_byte_capacity: usize,
    pub program_sha256: String,
    pub typed_instruction_count: usize,
    pub descriptor_offset: String,
    pub descriptor_runtime_address: String,
    pub descriptor_byte_count: usize,
    pub reused_payload_byte_count: usize,
    pub typed_program_verified: bool,
    pub runtime_execution_verified: bool,
}

#[derive(Debug, Serialize)]
pub struct GlyphUploadContextReport {
    pub context: String,
    pub entry_count: usize,
    pub storage_path: String,
    pub source_storage_padding_verified: bool,
    pub hook_path: String,
    pub hook_offset: String,
    pub hook_runtime_address: String,
    pub original_call_address: String,
    pub upload_routine_address: String,
    pub program_storage_path: String,
    pub program_offset: String,
    pub program_runtime_address: String,
    pub program_byte_count: usize,
    pub program_byte_capacity: usize,
    pub program_sha256: String,
    pub typed_instruction_count: usize,
    pub descriptor_offset: String,
    pub descriptor_runtime_address: String,
    pub descriptor_byte_count: usize,
    pub payload_byte_count: usize,
    pub stored_payload_byte_count: usize,
    pub payload_stream_count: usize,
    pub payload_ranges: Vec<[usize; 2]>,
    pub scratch_runtime_range: Option<[String; 2]>,
    pub typed_program_verified: bool,
    pub runtime_execution_verified: bool,
    pub entries: Vec<GlyphUploadEntry>,
}

#[derive(Debug, Serialize)]
pub struct OptionsAtlasRestoreReport {
    pub context: String,
    pub mechanism: String,
    pub consumer_path: String,
    pub menu_catalog_index: u16,
    pub menu_destination_address: String,
    pub menu_load_call_runtime_address: String,
    pub menu_tim_runtime_addresses: Vec<String>,
    pub menu_tim_parser_call_runtime_addresses: Vec<String>,
    pub menu_loader_address: String,
    pub menu_tim_parser_address: String,
    pub source_sequence_verified: bool,
    pub manual_overlay_write_count: usize,
    pub runtime_execution_verified: bool,
}

#[derive(Debug, Serialize)]
pub struct GlyphUploadEntry {
    pub role: String,
    pub character: Option<char>,
    pub source_preservation: bool,
    pub code: Option<String>,
    pub cell: Cell,
    pub vram_rect: [u16; 4],
    pub payload_offset: String,
    pub payload_runtime_address: String,
    pub payload_sha256: String,
}
