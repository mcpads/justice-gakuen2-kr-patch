use anyhow::{Result, ensure};

pub use crate::menu_atlas::MenuAtlasPosition as AtlasPosition;

pub const SKIP_GLYPH_CODE: u16 = 0x0fff;

/// Returns the source-proven glyph code in the shared MENU atlas.
///
/// `_`, `:`, `.`, and ASCII quotes are deliberately absent: the retail atlas has no matching
/// glyph in this ASCII row, so consumers that admit them must provide their
/// own cells instead of borrowing Japanese or graphic slots.
pub fn common_menu_ascii_glyph_code(character: char) -> Option<u16> {
    Some(match character {
        ' ' => SKIP_GLYPH_CODE,
        '1'..='9' => u16::try_from(u32::from(character) - u32::from('1')).ok()?,
        '0' => 0x0009,
        'A' => 0x000a,
        'B' => 0x000b,
        'C'..='N' => 0x0010 + u16::try_from(u32::from(character) - u32::from('C')).ok()?,
        'O'..='Z' => 0x0020 + u16::try_from(u32::from(character) - u32::from('O')).ok()?,
        'a'..='l' => 0x0030 + u16::try_from(u32::from(character) - u32::from('a')).ok()?,
        'm'..='x' => 0x0040 + u16::try_from(u32::from(character) - u32::from('m')).ok()?,
        'y' => 0x0050,
        'z' => 0x0051,
        '@' => 0x0052,
        '#' => 0x0053,
        '%' => 0x0054,
        '?' => 0x0055,
        '&' => 0x0056,
        '/' => 0x0059,
        '+' => 0x005a,
        '!' => 0x0063,
        '-' => 0x0064,
        _ => return None,
    })
}

pub fn fixed_menu_glyph_code(character: char) -> Option<u16> {
    Some(match character {
        ' ' => SKIP_GLYPH_CODE,
        '(' => 0x0057,
        ')' => 0x0058,
        '?' => 0x0055,
        '0' => 0x0009,
        '1' => 0x0000,
        '2' => 0x0001,
        '3' => 0x0002,
        '4' => 0x0003,
        '5' => 0x0004,
        '6' => 0x0005,
        '7' => 0x0006,
        '8' => 0x0007,
        '9' => 0x0008,
        'A' => 0x000a,
        'C' => 0x0010,
        'P' => 0x0021,
        'R' => 0x0023,
        'S' => 0x0024,
        'T' => 0x0025,
        'U' => 0x0026,
        'V' => 0x0027,
        _ => return None,
    })
}

pub fn atlas_position(code: u16) -> Result<AtlasPosition> {
    ensure!(code != SKIP_GLYPH_CODE, "0x0fff is the skip code");
    crate::menu_atlas::logical_code_position(code)
}

pub fn read_length_prefixed_codes(data: &[u8], offset: usize) -> Result<Vec<u16>> {
    ensure!(offset + 2 <= data.len(), "truncated glyph-string length");
    let length = usize::from(u16::from_le_bytes(data[offset..offset + 2].try_into()?));
    let end = offset + 2 + length * 2;
    ensure!(end <= data.len(), "truncated glyph-string codes");
    Ok(data[offset + 2..end]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u16::from_le_bytes(*pair))
        .collect())
}

pub fn replace_code(data: &mut [u8], offset: usize, expected: u16, replacement: u16) -> Result<()> {
    ensure!(
        offset + 2 <= data.len(),
        "glyph-code write is out of bounds"
    );
    let actual = u16::from_le_bytes(data[offset..offset + 2].try_into()?);
    ensure!(
        actual == expected,
        "glyph code at 0x{offset:x} is 0x{actual:04x}, expected 0x{expected:04x}"
    );
    data[offset..offset + 2].copy_from_slice(&replacement.to_le_bytes());
    Ok(())
}

#[cfg(test)]
#[path = "text_tests.rs"]
mod tests;
