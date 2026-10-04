use std::path::Path;

use anyhow::{Result, ensure};

use crate::compression::decompress;
use crate::cue::CueSheet;
use crate::disc::rebuild;
use crate::pipeline::{BASELINE_BIN_SHA256, sha256_bytes, sha256_file};
use crate::tim::{Tim4bpp, parse_4bpp_prefix};

pub(super) const SOURCE_PATH: &str = "DAT2/MA_TIT.BIZ";
pub(super) const SOURCE_STORED_SHA256: &str =
    "136badbf4d61ee1ddf576e492b3826a4b9486ebd6fa6728af94f9481081c270b";
pub(super) const SOURCE_DECODED_SHA256: &str =
    "395c7b5ac3ac198c8cb7519f3860fefa145fa4177dd00be8a10a8907a8728fc8";
pub(super) const SOURCE_STORED_SIZE: usize = 113_696;
pub(super) const SOURCE_DECODED_SIZE: usize = 169_984;
pub(super) const TIM_OFFSET: usize = 0;
pub(super) const TIM_DECODED_SIZE: usize = 0x18_180;
pub(super) const SOURCE_PIXEL_SHA256: &str =
    "e15c03daf8015f82bc3c7a11903f97625b46e38958f183cabde9a1bb5cf16232";
pub(super) const PALETTE_INDEX: usize = 7;

pub(super) struct TitleGraphicsSource {
    pub(super) source_bin_sha256: String,
    pub(super) extent_lba: u32,
    pub(super) stored: Vec<u8>,
    pub(super) decoded: Vec<u8>,
    pub(super) tim: Tim4bpp,
}

pub(super) fn load_title_graphics_source(cue_path: &Path) -> Result<TitleGraphicsSource> {
    let cue = CueSheet::parse(cue_path)?;
    let source_bin_sha256 = sha256_file(&cue.image_path)?;
    ensure!(
        source_bin_sha256 == BASELINE_BIN_SHA256,
        "unsupported source BIN SHA-256: {source_bin_sha256}"
    );
    let (record, stored) = rebuild::read_record(&cue.image_path, SOURCE_PATH)?;
    ensure!(
        stored.len() == SOURCE_STORED_SIZE && sha256_bytes(&stored) == SOURCE_STORED_SHA256,
        "unsupported {SOURCE_PATH} stored identity"
    );
    let decoded = decompress(&stored, true)?;
    ensure!(
        decoded.len() == SOURCE_DECODED_SIZE && sha256_bytes(&decoded) == SOURCE_DECODED_SHA256,
        "unsupported decoded {SOURCE_PATH} identity"
    );
    let tim = parse_4bpp_prefix(&decoded[TIM_OFFSET..])?;
    ensure!(
        tim.total_size == TIM_DECODED_SIZE
            && tim.pixel_width() == 768
            && tim.image_height == 256
            && tim.image_x == 768
            && tim.image_y == 0
            && tim.clut_x == 0
            && tim.clut_y == 481
            && tim.clut_width == 176
            && tim.clut_height == 1,
        "{SOURCE_PATH} title texture geometry changed"
    );
    let pixel_bytes =
        &decoded[tim.pixel_offset..tim.pixel_offset + tim.row_bytes() * tim.image_height];
    ensure!(
        sha256_bytes(pixel_bytes) == SOURCE_PIXEL_SHA256,
        "{SOURCE_PATH} title texture pixels changed"
    );
    Ok(TitleGraphicsSource {
        source_bin_sha256,
        extent_lba: record.extent_lba,
        stored,
        decoded,
        tim,
    })
}
