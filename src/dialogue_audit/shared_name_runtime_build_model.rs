use serde::Serialize;

use crate::name_input::{
    NameDialogueRuntimeBootstrapProgramReport, NameDialogueRuntimeProgramReport,
    SharedNameOutlineRuntimeProgramReport,
};

use super::dialogue_runtime_entry_hook::DialogueRuntimeEntryHookReport;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SharedNameRuntimeBuildReport {
    pub source_path: String,
    pub source_sha256: String,
    pub source_file_size: usize,
    pub entry_hook: DialogueRuntimeEntryHookReport,
    pub bootstrap_file_byte_range: [usize; 2],
    pub bootstrap_address_range: [String; 2],
    pub bootstrap_source_region_sha256: String,
    pub outline_runtime_file_byte_range: [usize; 2],
    pub outline_runtime_address_range: [String; 2],
    pub outline_source_region_sha256: String,
    pub dialogue_storage_file_byte_range: [usize; 2],
    pub dialogue_storage_address_range: [String; 2],
    pub dialogue_storage_source_region_sha256: String,
    pub patched_sha256: String,
    pub outline_program: SharedNameOutlineRuntimeProgramReport,
    pub bootstrap_program: NameDialogueRuntimeBootstrapProgramReport,
    pub dialogue_program: NameDialogueRuntimeProgramReport,
    pub runtime_execution_verified: bool,
}
