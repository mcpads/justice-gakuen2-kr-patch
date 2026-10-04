use anyhow::{Result, ensure};

use crate::compression::decompress;
use crate::cue::CueSheet;
use crate::pipeline::sha256_bytes;
use crate::source_disc::{MAIN_EXECUTABLE_PATH, MAIN_EXECUTABLE_SHA256};
use crate::source_disc::{
    SupportedSourceDisc,
    profile::{MAIN_EXECUTABLE_RECORD, MENU_RECORD, OPTIONS_INFO_RECORD, OPTIONS_OVERLAY_RECORD},
};

use super::description_source::{
    OPTINFO_DECODED_SHA256, OPTINFO_PATH, OPTINFO_STORED_SHA256, inspect_optinfo_slots,
};

pub(crate) const OVERLAY_PATH: &str = OPTIONS_OVERLAY_RECORD.path;
pub(super) const OVERLAY_SHA256: &str = OPTIONS_OVERLAY_RECORD.sha256;
pub(super) const OVERLAY_RUNTIME_BASE: u32 = 0x800a_2000;
pub(super) const MENU_PATH: &str = MENU_RECORD.path;
pub(super) const MENU_STORED_SHA256: &str = MENU_RECORD.stored_sha256;

pub(super) struct OptionsSource {
    pub(super) cue: CueSheet,
    pub(super) source_bin_sha256: String,
    pub(super) menu_stored: Vec<u8>,
    pub(super) menu_decoded: Vec<u8>,
    pub(super) optinfo_stored: Vec<u8>,
    pub(super) optinfo_decoded: Vec<u8>,
    pub(super) overlay: Vec<u8>,
    pub(super) main_executable: Vec<u8>,
}

pub(super) fn load_options_source(cue_path: &std::path::Path) -> Result<OptionsSource> {
    let source = SupportedSourceDisc::open(cue_path)?;
    load_options_source_from_disc(&source)
}

pub(super) fn load_options_source_from_disc(source: &SupportedSourceDisc) -> Result<OptionsSource> {
    let (_, overlay) = source.read_record(OVERLAY_PATH)?;
    ensure!(
        overlay.len() == OPTIONS_OVERLAY_RECORD.size && sha256_bytes(&overlay) == OVERLAY_SHA256,
        "unsupported {OVERLAY_PATH} source identity"
    );
    let (_, menu_stored) = source.read_record(MENU_PATH)?;
    ensure!(
        sha256_bytes(&menu_stored) == MENU_STORED_SHA256,
        "unsupported {MENU_PATH} stored identity"
    );
    let menu_decoded = decompress(&menu_stored, false)?;
    ensure!(
        menu_decoded.len() == MENU_RECORD.decoded_size
            && sha256_bytes(&menu_decoded) == MENU_RECORD.decoded_sha256,
        "unsupported {MENU_PATH} decoded identity"
    );
    let (_, optinfo_stored) = source.read_record(OPTINFO_PATH)?;
    ensure!(
        sha256_bytes(&optinfo_stored) == OPTINFO_STORED_SHA256,
        "unsupported {OPTINFO_PATH} stored identity"
    );
    let optinfo_decoded = decompress(&optinfo_stored, false)?;
    ensure!(
        optinfo_decoded.len() == OPTIONS_INFO_RECORD.decoded_size
            && sha256_bytes(&optinfo_decoded) == OPTINFO_DECODED_SHA256,
        "unsupported {OPTINFO_PATH} decoded identity"
    );
    inspect_optinfo_slots(&optinfo_decoded)?;
    let (_, main_executable) = source.read_record(MAIN_EXECUTABLE_PATH)?;
    ensure!(
        main_executable.len() == MAIN_EXECUTABLE_RECORD.size
            && sha256_bytes(&main_executable) == MAIN_EXECUTABLE_SHA256,
        "unsupported {MAIN_EXECUTABLE_PATH} identity"
    );
    Ok(OptionsSource {
        cue: source.cue().clone(),
        source_bin_sha256: source.source_bin_sha256().to_string(),
        menu_stored,
        menu_decoded,
        optinfo_stored,
        optinfo_decoded,
        overlay,
        main_executable,
    })
}
