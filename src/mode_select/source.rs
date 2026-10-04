use anyhow::{Result, ensure};

use crate::compression::decompress;
use crate::pipeline::sha256_bytes;
use crate::source_disc::{
    SupportedSourceDisc,
    profile::{MENU_RECORD, MODE_SELECT_OVERLAY_RECORD},
};

pub(crate) const MENU_PATH: &str = MENU_RECORD.path;
pub(crate) const MENU_STORED_SHA256: &str = MENU_RECORD.stored_sha256;
pub(crate) const MODE_SELECT_OVERLAY_PATH: &str = MODE_SELECT_OVERLAY_RECORD.path;
pub(crate) const MODE_SELECT_OVERLAY_SHA256: &str = MODE_SELECT_OVERLAY_RECORD.sha256;
pub(crate) const MODE_SELECT_OVERLAY_SIZE: usize = MODE_SELECT_OVERLAY_RECORD.size;
pub(crate) const ATLAS_TIM_OFFSET: usize = 0x22800;
pub(crate) const DESCRIPTION_TIM_FIRST_OFFSET: usize = 0x4b800;
pub(crate) const DESCRIPTION_TIM_STRIDE: usize = 0x2800;
pub(crate) const MODE_ARTWORK_FIRST_OFFSET: usize = 0x6e800;
pub(crate) const MODE_ARTWORK_STRIDE: usize = 0x1800;
pub(crate) const MODE_PREVIEW_FIRST_OFFSET: usize = 0x83800;
pub(crate) const MODE_PREVIEW_STRIDE: usize = 0x2000;
pub(crate) const MODE_COUNT: usize = 14;
pub(crate) const PANEL_INDEX_BY_MODE_INDEX: [usize; MODE_COUNT] =
    [6, 0, 1, 10, 8, 9, 7, 13, 2, 11, 12, 5, 4, 3];

/// The MODESEL +0x80/+0x98 background descriptors draw two atlas slices.
/// The first holds twelve pictures by columns of three; the second holds two.
/// Shared atlas CLUT (64,482) provides the original sepia palette.
pub(crate) fn thumbnail_cell(mode_index: usize) -> crate::tim::Cell {
    let (x, y) = if mode_index < 12 {
        (512 + (mode_index / 3) * 57, (mode_index % 3) * 45)
    } else {
        (512, 136 + (mode_index - 12) * 45)
    };
    crate::tim::Cell {
        x,
        y,
        width: 52,
        height: 40,
    }
}

pub(crate) struct ModeSelectSource {
    pub(crate) source_bin_sha256: String,
    pub(crate) menu_stored: Vec<u8>,
    pub(crate) menu_decoded: Vec<u8>,
    pub(crate) overlay: Vec<u8>,
}

pub(crate) fn load_source(cue_path: &std::path::Path) -> Result<ModeSelectSource> {
    let source = SupportedSourceDisc::open(cue_path)?;
    load_source_from_disc(&source)
}

pub(crate) fn load_source_from_disc(source: &SupportedSourceDisc) -> Result<ModeSelectSource> {
    let (_, menu_stored) = source.read_record(MENU_PATH)?;
    ensure!(
        sha256_bytes(&menu_stored) == MENU_STORED_SHA256,
        "unsupported {MENU_PATH} stored SHA-256"
    );
    let menu_decoded = decompress(&menu_stored, false)?;
    ensure!(
        menu_decoded.len() == MENU_RECORD.decoded_size
            && sha256_bytes(&menu_decoded) == MENU_RECORD.decoded_sha256,
        "unsupported {MENU_PATH} decoded SHA-256"
    );
    let (_, overlay) = source.read_record(MODE_SELECT_OVERLAY_PATH)?;
    ensure!(
        overlay.len() == MODE_SELECT_OVERLAY_SIZE
            && sha256_bytes(&overlay) == MODE_SELECT_OVERLAY_SHA256,
        "unsupported {MODE_SELECT_OVERLAY_PATH} source identity"
    );
    Ok(ModeSelectSource {
        source_bin_sha256: source.source_bin_sha256().to_string(),
        menu_stored,
        menu_decoded,
        overlay,
    })
}
