use anyhow::{Result, ensure};

use crate::compression::decompress;
use crate::pipeline::sha256_bytes;
use crate::source_disc::{
    SupportedSourceDisc,
    profile::{MENU_RECORD, TITLE_MENU_OVERLAY_RECORD},
};

pub(super) const MENU_PATH: &str = MENU_RECORD.path;
pub(super) const MENU_STORED_SHA256: &str = MENU_RECORD.stored_sha256;
pub(super) const OVERLAY_PATH: &str = TITLE_MENU_OVERLAY_RECORD.path;
pub(super) const OVERLAY_STORED_SHA256: &str = TITLE_MENU_OVERLAY_RECORD.stored_sha256;
pub(super) const OVERLAY_DECODED_SHA256: &str = TITLE_MENU_OVERLAY_RECORD.decoded_sha256;
pub(super) const OVERLAY_RUNTIME_BASE: u32 = 0x800a_2000;

pub(super) struct TitleNoticeSource {
    pub(super) source_bin_sha256: String,
    pub(super) menu_decoded: Vec<u8>,
    pub(super) overlay_decoded: Vec<u8>,
}

pub(super) fn load_title_notice_source_from_disc(
    source: &SupportedSourceDisc,
) -> Result<TitleNoticeSource> {
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
    let (_, overlay_stored) = source.read_record(OVERLAY_PATH)?;
    ensure!(
        sha256_bytes(&overlay_stored) == OVERLAY_STORED_SHA256,
        "unsupported {OVERLAY_PATH} stored identity"
    );
    let overlay_decoded = decompress(&overlay_stored, false)?;
    ensure!(
        sha256_bytes(&overlay_decoded) == OVERLAY_DECODED_SHA256,
        "unsupported decoded {OVERLAY_PATH} identity"
    );
    Ok(TitleNoticeSource {
        source_bin_sha256: source.source_bin_sha256().to_string(),
        menu_decoded,
        overlay_decoded,
    })
}
