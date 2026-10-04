use anyhow::{Result, ensure};

use crate::compression::decompress;
use crate::pipeline::sha256_bytes;
use crate::source_disc::{SupportedSourceDisc, profile::BONUS_MENU_RECORD};
use crate::tim::{parse_4bpp_prefix, parse_8bpp_prefix};

pub const BONUS_MENU_PATH: &str = BONUS_MENU_RECORD.path;
pub(super) const SOURCE_STORED_SHA256: &str = BONUS_MENU_RECORD.stored_sha256;
pub(super) const SOURCE_DECODED_SHA256: &str = BONUS_MENU_RECORD.decoded_sha256;
pub(super) const ENTRY_TIM_OFFSET: usize = 0x3c800;

pub(super) struct BonusMenuSource {
    pub(super) source_bin_sha256: String,
    pub(super) stored: Vec<u8>,
    pub(super) decoded: Vec<u8>,
}

#[cfg(test)]
pub(super) fn load_source(cue_path: &std::path::Path) -> Result<BonusMenuSource> {
    let source = SupportedSourceDisc::open(cue_path)?;
    load_source_from_disc(&source)
}

pub(super) fn load_source_from_disc(source: &SupportedSourceDisc) -> Result<BonusMenuSource> {
    let (_, stored) = source.read_record(BONUS_MENU_PATH)?;
    ensure!(
        sha256_bytes(&stored) == SOURCE_STORED_SHA256,
        "KOUBAI0.TIZ stored source identity changed"
    );
    let decoded = decompress(&stored, false)?;
    ensure!(
        sha256_bytes(&decoded) == SOURCE_DECODED_SHA256,
        "KOUBAI0.TIZ decoded source identity changed"
    );
    ensure!(
        decoded.len() == BONUS_MENU_RECORD.decoded_size,
        "KOUBAI0.TIZ decoded length changed"
    );
    let heading = parse_8bpp_prefix(&decoded)?;
    ensure!(
        heading.total_size <= ENTRY_TIM_OFFSET
            && heading.pixel_width() == 512
            && heading.image_height == 480,
        "KOUBAI0 heading TIM geometry changed"
    );
    let entries = parse_4bpp_prefix(
        decoded
            .get(ENTRY_TIM_OFFSET..)
            .ok_or_else(|| anyhow::anyhow!("KOUBAI0 entry TIM disappeared"))?,
    )?;
    ensure!(
        entries.total_size <= decoded.len() - ENTRY_TIM_OFFSET
            && entries.pixel_width() == 256
            && entries.image_height == 256,
        "KOUBAI0 entry TIM geometry changed"
    );
    Ok(BonusMenuSource {
        source_bin_sha256: source.source_bin_sha256().to_string(),
        stored,
        decoded,
    })
}
