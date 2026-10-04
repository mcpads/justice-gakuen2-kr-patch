use std::path::{Path, PathBuf};

use anyhow::{Result, ensure};

use crate::compression::decompress;
use crate::pipeline::sha256_bytes;
use crate::source_disc::{SupportedSourceDisc, profile::BONUS_MENU_OVERLAY_RECORD};
pub(crate) use crate::tim::cells_overlap;
use crate::tim::{Cell, parse_4bpp_prefix};

pub(crate) const SHOP_UI_PATH: &str = "DAT2/KOUBAI.TIZ";
pub(crate) const SHOP_UI_SOURCE_STORED_SHA256: &str =
    "73df3b61189cdd3e4bc85fe659fa4c4e267e58045b835b0f18372b35c877951f";
pub(crate) const SHOP_UI_SOURCE_DECODED_SHA256: &str =
    "ca40831475ff124e5b83aa5b1e594e3eb79872a6afcf7e6480b3b3e44a283dd2";
pub(crate) const FIXED_UI_TIM_OFFSET: usize = 0x1e000;
pub(crate) const FIXED_UI_TIM_SIZE: usize = 0x101a0;
pub(crate) const GLYPH_TIM_OFFSET: usize = 0x2e800;
pub(crate) const GLYPH_TIM_SIZE: usize = 0x201c0;
pub(crate) const GLYPH_TIM_SHA256: &str =
    "9f9c8d4f896ed285403bfcc984b31dc6804148a68a1da0f3d323740cf0a1cffe";
pub(crate) const OVERLAY_PATH: &str = BONUS_MENU_OVERLAY_RECORD.path;
pub(crate) const OVERLAY_SOURCE_SHA256: &str = BONUS_MENU_OVERLAY_RECORD.sha256;
pub(crate) const OVERLAY_RUNTIME_BASE: u32 = 0x800a_2000;
pub(crate) const POINTER_COMMAND_TABLE_RANGES: [[usize; 2]; 3] =
    [[0x2458, 0x25a4], [0x2cbc, 0x2e08], [0x371c, 0x376c]];
pub(crate) const POINTER_COMMAND_RECORD_RANGES: [[usize; 2]; 3] =
    [[0x0000, 0x2458], [0x25a4, 0x2cbc], [0x2e08, 0x371c]];
pub(crate) const DIRECT_SELECTOR_REGION: [usize; 2] = [0x376c, 0x90c0];

pub(crate) const fn glyph_cell(code: u16) -> Cell {
    let page = (code >> 8) as usize;
    let column = (code & 0x000f) as usize;
    let row = ((code >> 4) & 0x000f) as usize;
    Cell {
        x: page * 256 + ((column * 20) & 0xff),
        y: (row * 20) & 0xff,
        width: 20,
        height: 20,
    }
}

pub(crate) const fn sprite_selector(code: u16) -> [u8; 3] {
    [
        9 + (code >> 8) as u8,
        (code & 0x000f) as u8,
        ((code >> 4) & 0x000f) as u8,
    ]
}

pub(crate) struct BonusShopSource {
    pub(crate) source_image_path: PathBuf,
    pub(crate) source_bin_sha256: String,
    pub(crate) shop_ui_stored: Vec<u8>,
    pub(crate) shop_ui_decoded: Vec<u8>,
    pub(crate) overlay: Vec<u8>,
}

pub(crate) fn load_source(cue: &Path) -> Result<BonusShopSource> {
    let source = SupportedSourceDisc::open(cue)?;
    load_source_from_disc(&source)
}

pub(crate) fn load_source_from_disc(source: &SupportedSourceDisc) -> Result<BonusShopSource> {
    let (_, shop_ui_stored) = source.read_record(SHOP_UI_PATH)?;
    ensure!(
        sha256_bytes(&shop_ui_stored) == SHOP_UI_SOURCE_STORED_SHA256,
        "KOUBAI.TIZ stored source identity changed"
    );
    let shop_ui_decoded = decompress(&shop_ui_stored, false)?;
    ensure!(
        shop_ui_decoded.len() == 0x51000
            && sha256_bytes(&shop_ui_decoded) == SHOP_UI_SOURCE_DECODED_SHA256,
        "KOUBAI.TIZ decoded source identity changed"
    );
    validate_source_tim_identity(&shop_ui_decoded)?;

    let (_, overlay) = source.read_record(OVERLAY_PATH)?;
    ensure!(
        overlay.len() == BONUS_MENU_OVERLAY_RECORD.size
            && sha256_bytes(&overlay) == OVERLAY_SOURCE_SHA256,
        "KOUBAI.BIN source identity changed"
    );
    Ok(BonusShopSource {
        source_image_path: source.image_path().to_path_buf(),
        source_bin_sha256: source.source_bin_sha256().to_string(),
        shop_ui_stored,
        shop_ui_decoded,
        overlay,
    })
}

pub(crate) fn validate_tim_geometry(decoded: &[u8]) -> Result<()> {
    let fixed = parse_4bpp_prefix(
        decoded
            .get(FIXED_UI_TIM_OFFSET..)
            .ok_or_else(|| anyhow::anyhow!("KOUBAI fixed-UI TIM disappeared"))?,
    )?;
    ensure!(
        fixed.total_size == FIXED_UI_TIM_SIZE
            && fixed.pixel_width() == 512
            && fixed.image_height == 256,
        "KOUBAI fixed-UI TIM geometry changed"
    );
    ensure!(
        FIXED_UI_TIM_OFFSET + fixed.total_size <= GLYPH_TIM_OFFSET,
        "KOUBAI fixed-UI TIM overlaps the dynamic glyph atlas"
    );

    let glyph = parse_4bpp_prefix(
        decoded
            .get(GLYPH_TIM_OFFSET..)
            .ok_or_else(|| anyhow::anyhow!("KOUBAI dynamic glyph TIM disappeared"))?,
    )?;
    ensure!(
        glyph.total_size == GLYPH_TIM_SIZE
            && glyph.pixel_width() == 1024
            && glyph.image_height == 256
            && glyph.clut_width == 208
            && glyph.clut_height == 1
            && glyph.image_x == 576
            && glyph.image_y == 0,
        "KOUBAI dynamic glyph TIM geometry changed"
    );
    Ok(())
}

pub(crate) fn validate_source_tim_identity(decoded: &[u8]) -> Result<()> {
    validate_tim_geometry(decoded)?;
    ensure!(
        sha256_bytes(&decoded[GLYPH_TIM_OFFSET..GLYPH_TIM_OFFSET + GLYPH_TIM_SIZE])
            == GLYPH_TIM_SHA256,
        "KOUBAI dynamic glyph TIM identity changed"
    );
    Ok(())
}
