use anyhow::{Context, Result, ensure};
use serde::Serialize;

use crate::pipeline::sha256_bytes;

const DECODED_RUNTIME_BASE: u32 = 0x800d_0000;
pub(super) const RUNTIME_CODE_REGION_OFFSET: usize = 0x33340;
pub(super) const RUNTIME_CODE_REGION_SIZE: usize = 0x04c0;
const RUNTIME_CODE_REGION_SHA256: &str =
    "97d364e2d3d35f030a038c41bbadc42d0c15fa8d79ba569987e19fddb2e80f9a";

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct NameEntryRuntimeCodeRegionAudit {
    pub source_state: String,
    pub decoded_byte_range: [usize; 2],
    pub runtime_address_range: [String; 2],
    pub byte_count: usize,
    pub source_sha256: String,
    pub source_bytes_are_zero: bool,
    pub typed_code_installed: bool,
    pub runtime_execution_verified: bool,
}

pub(super) fn audit_name_entry_runtime_code_region(
    decoded: &[u8],
) -> Result<NameEntryRuntimeCodeRegionAudit> {
    let end = RUNTIME_CODE_REGION_OFFSET + RUNTIME_CODE_REGION_SIZE;
    ensure!(
        end == decoded.len(),
        "name-entry runtime code candidate is no longer the decoded asset tail"
    );
    let bytes = decoded
        .get(RUNTIME_CODE_REGION_OFFSET..end)
        .context("name-entry runtime code candidate is truncated")?;
    let source_sha256 = sha256_bytes(bytes);
    ensure!(
        source_sha256 == RUNTIME_CODE_REGION_SHA256 && bytes.iter().all(|byte| *byte == 0),
        "name-entry runtime code candidate source bytes changed"
    );
    let runtime_start = DECODED_RUNTIME_BASE + u32::try_from(RUNTIME_CODE_REGION_OFFSET)?;
    let runtime_end = runtime_start + u32::try_from(RUNTIME_CODE_REGION_SIZE)?;

    Ok(NameEntryRuntimeCodeRegionAudit {
        source_state: "source_zero_tail_bound".to_string(),
        decoded_byte_range: [RUNTIME_CODE_REGION_OFFSET, end],
        runtime_address_range: [
            format!("0x{runtime_start:08x}"),
            format!("0x{runtime_end:08x}"),
        ],
        byte_count: bytes.len(),
        source_sha256,
        source_bytes_are_zero: true,
        typed_code_installed: false,
        runtime_execution_verified: false,
    })
}

pub(super) fn install_name_entry_runtime_program(decoded: &mut [u8], program: &[u8]) -> Result<()> {
    ensure!(
        !program.is_empty() && program.len() <= RUNTIME_CODE_REGION_SIZE,
        "typed name-entry runtime does not fit its source-bound region"
    );
    let end = RUNTIME_CODE_REGION_OFFSET + RUNTIME_CODE_REGION_SIZE;
    let region = decoded
        .get_mut(RUNTIME_CODE_REGION_OFFSET..end)
        .context("name-entry runtime code region is truncated")?;
    ensure!(
        region.iter().all(|byte| *byte == 0),
        "name-entry runtime code region source bytes changed before install"
    );
    region[..program.len()].copy_from_slice(program);
    ensure!(
        region[..program.len()] == *program
            && region[program.len()..].iter().all(|byte| *byte == 0),
        "typed name-entry runtime readback changed"
    );
    Ok(())
}
