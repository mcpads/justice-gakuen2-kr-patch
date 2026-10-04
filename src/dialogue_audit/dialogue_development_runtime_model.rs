use std::path::PathBuf;

use serde::Serialize;

#[derive(Debug, Clone)]
pub struct DialogueDevelopmentRuntimeAuditConfig {
    pub disc_report: PathBuf,
    pub disc_bin: PathBuf,
    pub runtime_ram: PathBuf,
    pub runtime_frame: PathBuf,
    pub source_path: String,
    pub launch_id: String,
    pub emulator_build: String,
    pub capability_revision: String,
    pub output: PathBuf,
}

#[derive(Debug, Serialize)]
pub struct DialogueDevelopmentRuntimeAuditReport {
    pub kind: String,
    pub source_path: String,
    pub launch_id: String,
    pub emulator_build: String,
    pub capability_revision: String,
    pub disc_report_path: String,
    pub disc_report_sha256: String,
    pub disc_bin_path: String,
    pub disc_bin_sha256: String,
    pub individual_record_extent_lba: u32,
    pub individual_record_stored_sha256: String,
    pub decoded_asset_sha256: String,
    pub bundle_path: String,
    pub bundle_extent_lba: u32,
    pub bundle_readback_sha256: String,
    pub bundle_member_offset: usize,
    pub bundle_member_byte_count: usize,
    pub bundle_member_readback_sha256: String,
    pub runtime_ram_path: String,
    pub runtime_ram_sha256: String,
    pub runtime_ram_byte_count: usize,
    pub resident_runtime_start: String,
    pub resident_runtime_end: String,
    pub resident_runtime_sha256: String,
    pub runtime_frame_path: String,
    pub runtime_frame_sha256: String,
    pub runtime_frame_png_verified: bool,
    pub disc_readback_chain_verified: bool,
    pub runtime_residency_verified: bool,
}
