use anyhow::{Result, ensure};

use crate::compression::decompress;
use crate::dialogue_audit::{NAME_ENTRY_FONT_PATH, NAME_ENTRY_FONT_STORED_SHA256};
use crate::disc::iso9660::FileRecord;
use crate::pipeline::sha256_bytes;
use crate::source_disc::{
    MAIN_EXECUTABLE_PATH, MAIN_EXECUTABLE_SHA256, MAIN_TEXT_SIZE, PSX_EXE_HEADER_SIZE,
};
use crate::source_disc::{
    SupportedSourceDisc,
    profile::{EDIT_REGISTRATION_OVERLAY_RECORD, EDIT_REGISTRATION_UI_RECORD},
};

pub(crate) const OVERLAY_PATH: &str = EDIT_REGISTRATION_OVERLAY_RECORD.path;
pub(super) const OVERLAY_SHA256: &str = EDIT_REGISTRATION_OVERLAY_RECORD.sha256;
pub(super) const OVERLAY_RUNTIME_BASE: u32 = 0x800a_2000;
pub(super) const EDIT_SHARED_UI_PATH: &str = EDIT_REGISTRATION_UI_RECORD.path;
pub(super) const EDIT_SHARED_UI_STORED_SHA256: &str = EDIT_REGISTRATION_UI_RECORD.stored_sha256;
pub(super) const EDIT_SHARED_UI_DECODED_SHA256: &str = EDIT_REGISTRATION_UI_RECORD.decoded_sha256;
pub(crate) const PASS_PATH: &str = "DAT1/PASS.BIN";
pub(super) const PASS_SIZE: usize = 32_692;
pub(super) const PASS_SHA256: &str =
    "f160d227922e5928eda71a1d332fa6c463a4ba57826d35ca72d3c6e0581b90d4";

pub(super) struct EditRuntimeTextSource {
    pub(super) source_bin_sha256: String,
    pub(super) overlay: Vec<u8>,
    pub(super) pass: Vec<u8>,
    pub(super) edit_shared_ui_stored: Vec<u8>,
    pub(super) edit_shared_ui_decoded: Vec<u8>,
    pub(super) main_executable: Vec<u8>,
    pub(super) name_entry_font_record: FileRecord,
    pub(super) name_entry_font_source_stored: Vec<u8>,
}

pub(super) fn load_source(source: &SupportedSourceDisc) -> Result<EditRuntimeTextSource> {
    let (_, overlay) = source.read_record(OVERLAY_PATH)?;
    ensure!(
        overlay.len() == EDIT_REGISTRATION_OVERLAY_RECORD.size
            && sha256_bytes(&overlay) == OVERLAY_SHA256,
        "unsupported {OVERLAY_PATH} identity"
    );
    let (_, pass) = source.read_record(PASS_PATH)?;
    ensure!(
        pass.len() == PASS_SIZE && sha256_bytes(&pass) == PASS_SHA256,
        "unsupported {PASS_PATH} identity"
    );
    let (_, edit_shared_ui_stored) = source.read_record(EDIT_SHARED_UI_PATH)?;
    ensure!(
        sha256_bytes(&edit_shared_ui_stored) == EDIT_SHARED_UI_STORED_SHA256,
        "unsupported {EDIT_SHARED_UI_PATH} stored identity"
    );
    let edit_shared_ui_decoded = decompress(&edit_shared_ui_stored, false)?;
    ensure!(
        edit_shared_ui_decoded.len() == EDIT_REGISTRATION_UI_RECORD.decoded_size
            && sha256_bytes(&edit_shared_ui_decoded) == EDIT_SHARED_UI_DECODED_SHA256,
        "unsupported {EDIT_SHARED_UI_PATH} decoded identity"
    );
    let (_, main_executable) = source.read_record(MAIN_EXECUTABLE_PATH)?;
    ensure!(
        main_executable.len() == PSX_EXE_HEADER_SIZE + MAIN_TEXT_SIZE
            && sha256_bytes(&main_executable) == MAIN_EXECUTABLE_SHA256,
        "unsupported {MAIN_EXECUTABLE_PATH} identity"
    );
    let (name_entry_font_record, name_entry_font_source_stored) =
        source.read_record(NAME_ENTRY_FONT_PATH)?;
    ensure!(
        name_entry_font_record.size as usize == name_entry_font_source_stored.len()
            && sha256_bytes(&name_entry_font_source_stored) == NAME_ENTRY_FONT_STORED_SHA256,
        "unsupported {NAME_ENTRY_FONT_PATH} stored identity"
    );
    Ok(EditRuntimeTextSource {
        source_bin_sha256: source.source_bin_sha256().to_string(),
        overlay,
        pass,
        edit_shared_ui_stored,
        edit_shared_ui_decoded,
        main_executable,
        name_entry_font_record,
        name_entry_font_source_stored,
    })
}
