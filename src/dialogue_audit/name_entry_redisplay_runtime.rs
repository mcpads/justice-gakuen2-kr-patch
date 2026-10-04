use anyhow::{Context, Result, ensure};
use serde::Serialize;

use crate::name_input::{
    NAME_INPUT_REDISPLAY_RUNTIME_BYTE_CAPACITY, NAME_INPUT_REDISPLAY_RUNTIME_ORIGIN,
};
use crate::pipeline::sha256_bytes;
use crate::tim::parse_4bpp_prefix;

use super::name_entry_font_build::FONT_TIM_OFFSET;

pub(super) const REDISPLAY_RUNTIME_REGION_OFFSET: usize = 0x312e0;
pub(super) const NEXT_EMBEDDED_TIM_OFFSET: usize = 0x31800;
const REDISPLAY_RUNTIME_REGION_SHA256: &str =
    "2c7663e809c9827df482ce260d079467d02f9f181a6d1fcc5a942b2a7e1bd3e6";

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct NameEntryRedisplayRuntimeRegionReport {
    pub source_state: String,
    pub decoded_byte_range: [usize; 2],
    pub runtime_address_range: [String; 2],
    pub byte_count: usize,
    pub source_sha256: String,
    pub source_bytes_are_zero: bool,
    pub follows_font_tim_exactly: bool,
    pub next_embedded_tim_offset: String,
    pub typed_code_installed: bool,
    pub runtime_execution_verified: bool,
}

pub(super) fn audit_name_entry_redisplay_runtime_region(
    decoded: &[u8],
    embedded_font_tim_size: usize,
) -> Result<NameEntryRedisplayRuntimeRegionReport> {
    ensure!(
        FONT_TIM_OFFSET + embedded_font_tim_size == REDISPLAY_RUNTIME_REGION_OFFSET,
        "name-entry font TIM no longer ends at the redisplay runtime padding"
    );
    let end = REDISPLAY_RUNTIME_REGION_OFFSET + NAME_INPUT_REDISPLAY_RUNTIME_BYTE_CAPACITY;
    ensure!(
        end == NEXT_EMBEDDED_TIM_OFFSET,
        "name-entry redisplay runtime padding no longer ends at the next embedded TIM"
    );
    parse_4bpp_prefix(
        decoded
            .get(NEXT_EMBEDDED_TIM_OFFSET..)
            .context("next name-entry embedded TIM is truncated")?,
    )
    .context("asset after name-entry redisplay runtime padding is no longer a 4-bpp TIM")?;
    let bytes = decoded
        .get(REDISPLAY_RUNTIME_REGION_OFFSET..end)
        .context("name-entry redisplay runtime padding is truncated")?;
    let source_sha256 = sha256_bytes(bytes);
    ensure!(
        source_sha256 == REDISPLAY_RUNTIME_REGION_SHA256 && bytes.iter().all(|byte| *byte == 0),
        "name-entry redisplay runtime padding source bytes changed"
    );
    let runtime_end = NAME_INPUT_REDISPLAY_RUNTIME_ORIGIN
        .checked_add(u32::try_from(NAME_INPUT_REDISPLAY_RUNTIME_BYTE_CAPACITY)?)
        .context("name-entry redisplay runtime address range overflow")?;

    Ok(NameEntryRedisplayRuntimeRegionReport {
        source_state: "source_zero_padding_between_embedded_tim_images".to_string(),
        decoded_byte_range: [REDISPLAY_RUNTIME_REGION_OFFSET, end],
        runtime_address_range: [
            format!("0x{NAME_INPUT_REDISPLAY_RUNTIME_ORIGIN:08x}"),
            format!("0x{runtime_end:08x}"),
        ],
        byte_count: bytes.len(),
        source_sha256,
        source_bytes_are_zero: true,
        follows_font_tim_exactly: true,
        next_embedded_tim_offset: format!("0x{NEXT_EMBEDDED_TIM_OFFSET:05x}"),
        typed_code_installed: false,
        runtime_execution_verified: false,
    })
}

pub(super) fn install_name_entry_redisplay_runtime_program(
    source: &[u8],
    patched: &mut [u8],
    program: &[u8],
) -> Result<()> {
    ensure!(
        !program.is_empty() && program.len() <= NAME_INPUT_REDISPLAY_RUNTIME_BYTE_CAPACITY,
        "typed name-entry redisplay runtime does not fit its source-bound padding"
    );
    let end = REDISPLAY_RUNTIME_REGION_OFFSET + NAME_INPUT_REDISPLAY_RUNTIME_BYTE_CAPACITY;
    let source_region = source
        .get(REDISPLAY_RUNTIME_REGION_OFFSET..end)
        .context("name-entry redisplay runtime source padding is truncated")?;
    ensure!(
        sha256_bytes(source_region) == REDISPLAY_RUNTIME_REGION_SHA256
            && source_region.iter().all(|byte| *byte == 0),
        "name-entry redisplay runtime source bytes changed before install"
    );
    let patched_region = patched
        .get_mut(REDISPLAY_RUNTIME_REGION_OFFSET..end)
        .context("name-entry redisplay runtime destination is truncated")?;
    ensure!(
        patched_region == source_region,
        "another name-entry writer changed the redisplay runtime padding"
    );
    patched_region[..program.len()].copy_from_slice(program);
    ensure!(
        patched_region[..program.len()] == *program
            && patched_region[program.len()..]
                .iter()
                .all(|byte| *byte == 0),
        "typed name-entry redisplay runtime readback changed"
    );
    Ok(())
}
