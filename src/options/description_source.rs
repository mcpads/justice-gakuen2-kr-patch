use anyhow::{Context, Result, ensure};

use crate::pipeline::sha256_bytes;
use crate::source_disc::profile::OPTIONS_INFO_RECORD;
use crate::tim::{Tim4bpp, parse_4bpp_prefix};

pub(crate) const OPTINFO_PATH: &str = OPTIONS_INFO_RECORD.path;
pub(super) const OPTINFO_STORED_SHA256: &str = OPTIONS_INFO_RECORD.stored_sha256;
pub(super) const OPTINFO_DECODED_SHA256: &str = OPTIONS_INFO_RECORD.decoded_sha256;
pub(super) const ITEM_COUNT: usize = 5;
pub(super) const ITEM_SLOT_SIZE: usize = 0x8800;
pub(super) const ITEM_STATE_COUNTS: [usize; ITEM_COUNT] = [4, 6, 2, 2, 3];
pub(super) const STREAM_ARENA_OFFSET: usize = 0x0174;
pub(super) const STREAM_ARENA_END: usize = 0x0294;
pub(super) const ITEM_TABLE_OFFSETS: [usize; ITEM_COUNT] = [0x0294, 0x02a8, 0x02c4, 0x02d0, 0x02dc];
pub(super) const ROOT_TABLE_OFFSET: usize = 0x02ec;

pub(super) fn inspect_optinfo_slots(decoded: &[u8]) -> Result<Vec<Tim4bpp>> {
    ensure!(
        decoded.len() == ITEM_COUNT * ITEM_SLOT_SIZE,
        "OPTINFO decoded length changed"
    );
    ensure!(
        sha256_bytes(decoded) == OPTINFO_DECODED_SHA256,
        "OPTINFO decoded identity changed"
    );
    (0..ITEM_COUNT)
        .map(|item_index| {
            let offset = item_index * ITEM_SLOT_SIZE;
            let tim = parse_4bpp_prefix(
                decoded
                    .get(offset..offset + ITEM_SLOT_SIZE)
                    .context("OPTINFO item slot is truncated")?,
            )?;
            ensure!(
                tim.image_x == 0x0300
                    && tim.image_y == 0x0100
                    && tim.pixel_width() == 256
                    && tim.image_height == 256,
                "OPTINFO item {item_index} TIM geometry changed"
            );
            ensure!(
                tim.clut_x == 0 && tim.clut_y == 0x01e4,
                "OPTINFO item {item_index} CLUT location changed"
            );
            ensure!(
                tim.total_size <= ITEM_SLOT_SIZE,
                "OPTINFO item {item_index} TIM exceeds its slot"
            );
            Ok(tim)
        })
        .collect()
}
