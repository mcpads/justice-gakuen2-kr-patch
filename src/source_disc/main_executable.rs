use anyhow::{Result, ensure};

use super::profile::MAIN_EXECUTABLE_RECORD;

pub(crate) const PSX_EXE_HEADER_SIZE: usize = 0x800;
pub(crate) const MAIN_TEXT_RUNTIME_BASE: u32 = 0x8001_0000;
pub(crate) const MAIN_TEXT_SIZE: usize = 0x91000;

pub(crate) fn main_executable_entrypoint(executable: &[u8]) -> Result<u32> {
    validate_header(executable)?;
    little_u32(executable, 0x10)
}

pub(crate) fn main_executable_text(executable: &[u8]) -> Result<&[u8]> {
    validate_header(executable)?;
    let runtime_base = little_u32(executable, 0x18)?;
    let text_size = usize::try_from(little_u32(executable, 0x1c)?)?;
    ensure!(
        runtime_base == MAIN_TEXT_RUNTIME_BASE && text_size == MAIN_TEXT_SIZE,
        "unexpected {} loaded text range",
        MAIN_EXECUTABLE_RECORD.path
    );
    let text_end = PSX_EXE_HEADER_SIZE
        .checked_add(text_size)
        .expect("fixed executable text range fits usize");
    ensure!(
        text_end == executable.len(),
        "{} has bytes outside its declared loaded text",
        MAIN_EXECUTABLE_RECORD.path
    );
    Ok(&executable[PSX_EXE_HEADER_SIZE..text_end])
}

fn validate_header(executable: &[u8]) -> Result<()> {
    ensure!(
        executable.len() >= PSX_EXE_HEADER_SIZE,
        "{} is smaller than its PS-X EXE header",
        MAIN_EXECUTABLE_RECORD.path
    );
    ensure!(
        executable.starts_with(b"PS-X EXE"),
        "{} lacks the PS-X EXE signature",
        MAIN_EXECUTABLE_RECORD.path
    );
    Ok(())
}

fn little_u32(data: &[u8], offset: usize) -> Result<u32> {
    let end = offset + 4;
    ensure!(end <= data.len(), "truncated PS-X EXE header");
    Ok(u32::from_le_bytes(data[offset..end].try_into()?))
}
