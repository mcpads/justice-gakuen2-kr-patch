//! Korean byte-coordinate strings for the source-bound PLSEL1 settings renderer.
use crate::font::rasterize_menu_glyphs;
use crate::pipeline::{difference_ranges, sha256_bytes};
use crate::tim::{Cell, read_indexed_cell_in_prefix, write_indexed_cell_in_prefix_with_report};
use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

#[derive(Deserialize)]
struct Manifest {
    entries: Vec<Entry>,
}
#[derive(Deserialize)]
struct Entry {
    offset: usize,
    korean: String,
    source_stream_sha256: String,
    source_glyph_count: usize,
    palette: u8,
}

pub struct SystemSettingsText {
    pub overlay: Vec<u8>,
    pub texture: Vec<u8>,
    pub overlay_ranges: Vec<[usize; 2]>,
    pub texture_ranges: Vec<[usize; 2]>,
    pub string_count: usize,
    pub glyph_count: usize,
}

fn encode(text: &str, palette: u8, mapping: &BTreeMap<char, [u8; 3]>) -> Result<Vec<u8>> {
    let mut bytes = vec![u8::try_from(text.chars().count())?, palette];
    for character in text.chars() {
        bytes.extend_from_slice(mapping.get(&character).context("missing settings glyph")?);
    }
    Ok(bytes)
}

pub fn build(
    overlay: &[u8],
    texture: &[u8],
    manifest: &[u8],
    font: &Path,
) -> Result<SystemSettingsText> {
    ensure!(
        sha256_bytes(overlay) == "3e791ff2c265bf85a2b11c36119e24586d41450a3a81647f77f18f7b6b9a5b35",
        "PLSEL1 source identity changed"
    );
    ensure!(
        sha256_bytes(texture) == "17336b981e3fb8b0c8af8a6e2c4290b7a27cefb1012c2b28a63158a890091257",
        "SELP1 source identity changed"
    );
    let manifest: Manifest = serde_json::from_slice(manifest)?;
    let expected: BTreeSet<usize> = [
        0x754, 0x774, 0x788, 0x79c, 0x7a8, 0x7bc, 0x7c8, 0x7e0, 0x7f8, 0x80c, 0x81c, 0x82c, 0x83c,
        0x84c, 0x854, 0x85c, 0x870, 0x884, 0x898, 0x8ac, 0x8b8, 0x8c4, 0x8d8, 0x930, 0x940, 0x948,
    ]
    .into_iter()
    .collect();
    let mut seen = BTreeSet::new();
    let mut source_cells = BTreeSet::new();
    let mut characters = BTreeSet::new();
    for entry in &manifest.entries {
        ensure!(
            expected.contains(&entry.offset) && seen.insert(entry.offset),
            "unknown or duplicate settings stream"
        );
        let count = usize::from(overlay[entry.offset]);
        ensure!(
            count == entry.source_glyph_count
                && overlay[entry.offset + 1] == entry.palette
                && entry.palette < 13,
            "settings header changed"
        );
        let end = entry.offset + 2 + count * 3;
        ensure!(
            sha256_bytes(&overlay[entry.offset..end]) == entry.source_stream_sha256,
            "settings stream source hash changed"
        );
        ensure!(
            !entry.korean.is_empty() && entry.korean.chars().count() <= count,
            "settings text exceeds source stream"
        );
        for triple in overlay[entry.offset + 2..end].as_chunks::<3>().0 {
            ensure!(
                (8..=10).contains(&triple[0]) && triple[1] < 12 && triple[2] < 12,
                "invalid settings source coordinate"
            );
            source_cells.insert([triple[0], triple[1], triple[2]]);
        }
        characters.extend(entry.korean.chars());
    }
    ensure!(seen == expected, "missing settings streams");
    ensure!(
        characters.len() <= source_cells.len(),
        "settings glyph allocation exceeds owned cells"
    );
    let mapping: BTreeMap<_, _> = characters.iter().copied().zip(source_cells).collect();
    let visible: String = characters.iter().copied().filter(|&c| c != ' ').collect();
    let raster = rasterize_menu_glyphs(font, &visible, 16.0, 4, 14)?;
    ensure!(
        raster.font_sha256 == "389ad546769c0cb958b1c5c5c1d4b473867b433e0a6697b01907c7d7e1565c60",
        "settings font identity changed"
    );
    let mut glyphs: BTreeMap<_, _> = raster
        .glyphs
        .into_iter()
        .map(|g| (g.character, g.pixels))
        .collect();
    glyphs.insert(' ', vec![0; 400]);
    let mut patched_texture = texture.to_vec();
    for (&character, &[page, col, row]) in &mapping {
        let cell = Cell {
            x: usize::from(page - 8) * 256 + usize::from(col) * 20,
            y: usize::from(row) * 20,
            width: 20,
            height: 20,
        };
        let before = read_indexed_cell_in_prefix(texture, 0x38000, cell)?;
        ensure!(before.len() == 400, "settings cell geometry changed");
        write_indexed_cell_in_prefix_with_report(
            &mut patched_texture,
            0x38000,
            cell,
            &glyphs[&character],
        )?;
    }
    let mut patched_overlay = overlay.to_vec();
    for entry in &manifest.entries {
        let encoded = encode(&entry.korean, entry.palette, &mapping)?;
        patched_overlay[entry.offset..entry.offset + encoded.len()].copy_from_slice(&encoded);
    }
    Ok(SystemSettingsText {
        overlay_ranges: difference_ranges(overlay, &patched_overlay),
        texture_ranges: difference_ranges(texture, &patched_texture),
        overlay: patched_overlay,
        texture: patched_texture,
        string_count: manifest.entries.len(),
        glyph_count: mapping.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn count_is_unicode_glyphs_and_palette_is_preserved() {
        let mapping = BTreeMap::from([('예', [8, 2, 3]), (' ', [10, 1, 0])]);
        assert_eq!(
            encode("예 예", 12, &mapping).unwrap(),
            [3, 12, 8, 2, 3, 10, 1, 0, 8, 2, 3]
        );
        assert!(encode("아니요", 0, &mapping).is_err());
    }
}

#[derive(Debug, serde::Serialize)]
pub struct SystemSettingsBuildReport {
    pub string_count: usize,
    pub glyph_count: usize,
    pub translation_sha256: String,
    pub font_sha256: String,
    pub runtime_verified: bool,
}
impl SystemSettingsText {
    pub(crate) fn register_texture(
        &self,
        source: &[u8],
        plan: &mut crate::decoded_record_write_plan::DecodedRecordWritePlan<'_>,
    ) -> Result<()> {
        let claims = crate::decoded_record_write_plan::DecodedDataClaim::from_ranges(
            "system-settings",
            "Korean system-setting glyph cells",
            self.texture_ranges.iter().copied(),
        );
        plan.register_data_candidate(
            "system settings",
            &sha256_bytes(source),
            &self.texture,
            &claims,
        )
    }
}
