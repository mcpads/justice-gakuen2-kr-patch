use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct MenuRuntimeProbeBuildConfig {
    pub spec: PathBuf,
    pub force: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct MenuRuntimeProbeBuildSpec {
    pub(super) kind: String,
    pub(super) probe_id: String,
    pub(super) base_cue: PathBuf,
    pub(super) base_bin_sha256: String,
    pub(super) replacement_menu: PathBuf,
    pub(super) replacement_menu_sha256: String,
    pub(super) output_dir: PathBuf,
    pub(super) output_stem: String,
}

#[derive(Debug, Serialize)]
pub struct MenuRuntimeProbeBuildReport {
    pub kind: String,
    pub probe_id: String,
    pub spec_path: String,
    pub spec_sha256: String,
    pub base_cue_path: String,
    pub base_bin_path: String,
    pub base_bin_sha256: String,
    pub replacement_record_path: String,
    pub replacement_menu_path: String,
    pub replacement_menu_sha256: String,
    pub replacement_menu_size: usize,
    pub changed_lbas: Vec<u32>,
    pub output_cue: String,
    pub output_bin: String,
    pub output_bin_sha256: String,
    pub readback_matches_replacement: bool,
    pub release_eligible: bool,
}
