use anyhow::{Result, bail, ensure};

use crate::paged_compression::{compress_page_safe_image, source_paged_compression_profile};

const INPUT_PAGE_BYTES: usize = 0x800;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MenuCompressionProfile {
    pub(crate) maximum_match_words: usize,
    pub(crate) maximum_control_block_output_words: usize,
    pub(crate) control_blocks_crossing_input_pages: usize,
    pub(crate) stream_byte_count: usize,
}

pub(crate) fn compress_menu_with_source_limits(
    decoded: &[u8],
    source_stored: &[u8],
) -> Result<(Vec<u8>, MenuCompressionProfile, MenuCompressionProfile)> {
    let source_profile = profile_menu_stream(source_stored)?;
    let compressed =
        compress_page_safe_image(decoded, source_paged_compression_profile(source_stored)?)?;
    let rebuilt_profile = profile_menu_stream(&compressed)?;
    ensure!(
        rebuilt_profile.maximum_match_words <= source_profile.maximum_match_words,
        "rebuilt menu match length exceeds its source profile"
    );
    ensure!(
        rebuilt_profile.maximum_control_block_output_words
            <= source_profile.maximum_control_block_output_words,
        "rebuilt menu control-block expansion exceeds its source profile"
    );
    ensure!(
        rebuilt_profile.stream_byte_count <= source_stored.len(),
        "rebuilt menu stream exceeds its source record"
    );
    Ok((compressed, source_profile, rebuilt_profile))
}

pub(crate) fn profile_menu_stream(stored: &[u8]) -> Result<MenuCompressionProfile> {
    let mut input_offset = 0usize;
    let mut maximum_match_words = 0usize;
    let mut maximum_control_block_output_words = 0usize;
    let mut control_blocks_crossing_input_pages = 0usize;
    loop {
        let control_block_start = input_offset;
        let mut control = read_u16(stored, &mut input_offset)?;
        let mut control_block_output_words = 0usize;
        for _ in 0..16 {
            if control & 0x8000 == 0 {
                read_u16(stored, &mut input_offset)?;
                control_block_output_words += 1;
            } else {
                let token = read_u16(stored, &mut input_offset)?;
                let distance = usize::from(token & 0x07ff);
                let mut length = usize::from(token >> 11);
                if length == 0 {
                    length = usize::from(read_u16(stored, &mut input_offset)?);
                }
                if distance == 0 && length == 0 {
                    maximum_control_block_output_words =
                        maximum_control_block_output_words.max(control_block_output_words);
                    control_blocks_crossing_input_pages += usize::from(
                        control_block_start / INPUT_PAGE_BYTES
                            != (input_offset - 1) / INPUT_PAGE_BYTES,
                    );
                    ensure!(
                        maximum_match_words >= 2,
                        "menu source stream has no reusable match limit"
                    );
                    return Ok(MenuCompressionProfile {
                        maximum_match_words,
                        maximum_control_block_output_words,
                        control_blocks_crossing_input_pages,
                        stream_byte_count: input_offset,
                    });
                }
                maximum_match_words = maximum_match_words.max(length);
                control_block_output_words += length;
            }
            control <<= 1;
        }
        maximum_control_block_output_words =
            maximum_control_block_output_words.max(control_block_output_words);
        control_blocks_crossing_input_pages += usize::from(
            control_block_start / INPUT_PAGE_BYTES != (input_offset - 1) / INPUT_PAGE_BYTES,
        );
    }
}

fn read_u16(data: &[u8], offset: &mut usize) -> Result<u16> {
    if *offset + 2 > data.len() {
        bail!("truncated menu compression token at 0x{offset:x}");
    }
    let value = u16::from_le_bytes([data[*offset], data[*offset + 1]]);
    *offset += 2;
    Ok(value)
}

#[cfg(test)]
#[path = "menu_compression_tests.rs"]
mod tests;
