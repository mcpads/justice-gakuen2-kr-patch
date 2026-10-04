use std::path::Path;

use anyhow::{Result, ensure};

use crate::compression::decompress;
use crate::cue::CueSheet;
use crate::disc::rebuild;
use crate::pipeline::{
    BASELINE_BIN_SHA256, ORIGINAL_MENU_DECODED_SHA256, sha256_bytes, sha256_file,
};
use crate::tim::{IndexedImage, read_4bpp_indexed_image_in_prefix};

use super::{MENU_ATLAS_HEIGHT, MENU_ATLAS_WIDTH};

pub(crate) const MENU_ATLAS_RECORD_PATH: &str = "DAT2/MENU.BIZ";

pub(crate) struct SourceMenuAtlas {
    pub(crate) source_bin_sha256: String,
    pub(crate) menu_stored_sha256: String,
    pub(crate) menu_decoded_sha256: String,
    pub(crate) menu_decoded: Vec<u8>,
    pub(crate) indexed: IndexedImage,
}

pub(crate) fn load_source_menu_atlas(cue_path: &Path) -> Result<SourceMenuAtlas> {
    let cue = CueSheet::parse(cue_path)?;
    let source_bin_sha256 = sha256_file(&cue.image_path)?;
    ensure!(
        source_bin_sha256 == BASELINE_BIN_SHA256,
        "unsupported source BIN SHA-256: {source_bin_sha256}"
    );
    let (_, menu_stored) = rebuild::read_record(&cue.image_path, MENU_ATLAS_RECORD_PATH)?;
    let menu_stored_sha256 = sha256_bytes(&menu_stored);
    let menu_decoded = decompress(&menu_stored, false)?;
    let menu_decoded_sha256 = sha256_bytes(&menu_decoded);
    ensure!(
        menu_decoded_sha256 == ORIGINAL_MENU_DECODED_SHA256,
        "MENU.BIZ decoded identity changed: {menu_decoded_sha256}"
    );
    let indexed = read_4bpp_indexed_image_in_prefix(&menu_decoded, 0)?;
    ensure!(
        indexed.width == MENU_ATLAS_WIDTH && indexed.height == MENU_ATLAS_HEIGHT,
        "MENU.BIZ shared atlas dimensions changed"
    );
    Ok(SourceMenuAtlas {
        source_bin_sha256,
        menu_stored_sha256,
        menu_decoded_sha256,
        menu_decoded,
        indexed,
    })
}
