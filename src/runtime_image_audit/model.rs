use std::path::PathBuf;

use serde::Serialize;

#[derive(Debug, Clone)]
pub struct RuntimeImageAuditConfig {
    pub cue: PathBuf,
    pub output: PathBuf,
}

#[derive(Debug, Serialize)]
pub struct RuntimeImageAuditReport {
    pub kind: String,
    pub source_bin_sha256: String,
    pub iso_file_count: usize,
    pub stored_biz_record_count: usize,
    pub decoded_biz_record_count: usize,
    pub selector_signature_record_count: usize,
    pub dialogue_runtime_image_record_count: usize,
    pub dialogue_runtime_image_path_manifest_sha256: String,
    pub records: Vec<RuntimeImageRecordAudit>,
}

#[derive(Debug, Serialize)]
pub struct RuntimeImageRecordAudit {
    pub path: String,
    pub extent_lba: u32,
    pub stored_size: usize,
    pub stored_sha256: String,
    pub decode_succeeded: bool,
    pub decode_error: Option<String>,
    pub decoded_size: Option<usize>,
    pub decoded_sha256: Option<String>,
    pub selector_table_prefix_offset: Option<String>,
    pub selector_table_prefix: Option<String>,
    pub matches_dialogue_runtime_image_size: bool,
}
