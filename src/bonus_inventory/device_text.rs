//! PocketStation and J-BANK pointer text shares the inventory message atlas.
//! Source discovery stays in analysis; this build accepts an exact authored set.
use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

use super::model::BonusInventoryBuildConfig;
use crate::bonus_inventory_source::{
    BonusInventorySource, rasterize_message_glyph, reserved_dynamic_glyph_codes,
};
use crate::font::IndexedTextRasterizer;
use crate::pipeline::{sha256_bytes, sha256_file};
use crate::tim::{
    Cell, cells_overlap, read_indexed_cell_in_prefix, write_indexed_cell_in_prefix_with_report,
};

const BASE: u32 = 0x800a2000;
const TABLE: usize = 0x11e0;
const TABLE_END: usize = 0x1418;
const TIM: usize = 0x21000;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Assets {
    kind: String,
    source_overlay_sha256: String,
    entries: Vec<Entry>,
    glyphs: Vec<Glyph>,
    reused_glyphs: Vec<SharedGlyph>,
    release_status: String,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    index: usize,
    source_offset: usize,
    storage_end: usize,
    source_hex: String,
    source_text: String,
    korean_text: String,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Glyph {
    text: String,
    code: u16,
    source_indexed_sha256: String,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SharedGlyph {
    text: String,
    code: u16,
}

pub(crate) struct DeviceTextBuild {
    pub overlay: Vec<u8>,
    pub overlay_ranges: Vec<[usize; 2]>,
    allocated: BTreeSet<u16>,
    expected_pixels: Vec<(u16, Vec<u8>)>,
}

pub(super) fn build(
    config: &BonusInventoryBuildConfig,
    source: &BonusInventorySource,
    inventory: &mut [u8],
) -> Result<(DeviceTextBuild, serde_json::Value, Vec<[usize; 2]>)> {
    let path = config.assets.join("device-text.json");
    let assets: Assets = serde_json::from_slice(&std::fs::read(&path)?)?;
    ensure!(
        assets.kind == "justice_gakuen2_bonus_device_text"
            && assets.release_status == "needs_human_review"
            && assets.source_overlay_sha256 == sha256_bytes(&source.overlay),
        "bonus device source/asset identity changed"
    );
    ensure!(
        assets.entries.iter().map(|e| e.index).collect::<Vec<_>>()
            == owned_indices().collect::<Vec<_>>(),
        "bonus device text population changed"
    );
    let reserved = reserved_dynamic_glyph_codes().collect::<BTreeSet<_>>();
    let item: serde_json::Value =
        serde_json::from_slice(&std::fs::read(config.assets.join("item-labels.json"))?)?;
    let mut protected = reserved.clone();
    protected.extend(0..0x70); // Retain native ASCII/digits and their computed consumers.
    protected.extend([0x17a, 0x197, 0x198, 0x199, 0x237, 0x271, 0x26a]);
    for g in item["glyphs"]
        .as_array()
        .context("missing item glyph allocations")?
    {
        protected.insert(u16::try_from(
            g["code"].as_u64().context("invalid item glyph code")?,
        )?);
    }
    let rasterizer = IndexedTextRasterizer::load(&config.fonts.device_text.path)?;
    let mut allocated = BTreeSet::new();
    let mut codes = BTreeMap::new();
    let mut ranges = Vec::new();
    let mut expected_pixels = Vec::new();
    for glyph in &assets.glyphs {
        let ch = single_character(&glyph.text)?;
        ensure!(
            codes.insert(ch, glyph.code).is_none() && allocated.insert(glyph.code),
            "duplicate device glyph"
        );
        ensure!(
            glyph.code < 0x400 && glyph.code & 15 < 12 && (glyph.code >> 4) & 15 < 12,
            "device glyph wraps its native page"
        );
        let aliases = aliases(glyph.code);
        ensure!(
            aliases.is_disjoint(&protected),
            "device glyph overlaps retained allocation: {:04x}",
            glyph.code
        );
        for alias in aliases {
            let selector = [
                8 + (alias >> 8) as u8,
                (alias & 15) as u8,
                ((alias >> 4) & 15) as u8,
            ];
            ensure!(
                !source.overlay[TABLE_END..0x3afc]
                    .windows(3)
                    .any(|s| s == selector),
                "device glyph aliases a direct native selector"
            );
        }
        let before = read_indexed_cell_in_prefix(inventory, TIM, cell(glyph.code))?;
        ensure!(
            sha256_bytes(&before) == glyph.source_indexed_sha256,
            "device glyph has changed source or another writer"
        );
        let raster = rasterize_message_glyph(&rasterizer, &config.fonts.device_text, &glyph.text)?;
        ranges.extend(
            write_indexed_cell_in_prefix_with_report(
                inventory,
                TIM,
                cell(glyph.code),
                &raster.pixels,
            )?
            .allowed_ranges,
        );
        expected_pixels.push((glyph.code, raster.pixels));
    }
    for (i, left) in assets.glyphs.iter().enumerate() {
        ensure!(
            assets.glyphs[i + 1..]
                .iter()
                .all(|right| !cells_overlap(cell(left.code), cell(right.code))),
            "device glyph allocations overlap"
        );
    }
    for glyph in &assets.reused_glyphs {
        ensure!(
            reserved.contains(&glyph.code),
            "device shared glyph has no inventory owner"
        );
        ensure!(
            codes
                .insert(single_character(&glyph.text)?, glyph.code)
                .is_none(),
            "duplicate shared device glyph"
        );
        // Checked against the final atlas, after each shared owner has rendered.
        let raster = rasterize_message_glyph(&rasterizer, &config.fonts.device_text, &glyph.text)?;
        expected_pixels.push((glyph.code, raster.pixels));
    }
    // Native codes remain stable for computed consumers. Restyle their ink
    // in the same role as the surrounding message, with source-bound cells.
    let symbols: Vec<Glyph> = serde_json::from_slice(crate::product_assets::read(
        "menu/bonus-inventory/message-symbols.json",
    )?)?;
    for glyph in &symbols {
        let before = read_indexed_cell_in_prefix(inventory, TIM, cell(glyph.code))?;
        ensure!(
            sha256_bytes(&before) == glyph.source_indexed_sha256,
            "native message symbol source changed: {}",
            glyph.text
        );
        let raster = rasterize_message_glyph(&rasterizer, &config.fonts.device_text, &glyph.text)?;
        ranges.extend(
            write_indexed_cell_in_prefix_with_report(
                inventory,
                TIM,
                cell(glyph.code),
                &raster.pixels,
            )?
            .allowed_ranges,
        );
        expected_pixels.push((glyph.code, raster.pixels));
    }
    let mut overlay = source.overlay.clone();
    let mut overlay_ranges = Vec::new();
    for entry in &assets.entries {
        let start = pointer(&source.overlay, entry.index)?;
        let end = pointer(&source.overlay, entry.index + 1)?;
        ensure!(
            entry.source_offset == start && entry.storage_end == end && start < end && end <= TABLE,
            "device text bounds changed"
        );
        let original = decode_hex(&entry.source_hex)?;
        ensure!(
            source.overlay.get(start..start + original.len()) == Some(original.as_slice())
                && original.last() == Some(&0x81),
            "device source commands changed"
        );
        let encoded = encode(&entry.korean_text, &codes)?;
        ensure!(
            encoded.len() <= end - start,
            "device text {} exceeds source storage",
            entry.index
        );
        overlay[start..end].fill(0);
        overlay[start..start + encoded.len()].copy_from_slice(&encoded);
        overlay_ranges.push([start, end]);
    }
    let report = serde_json::json!({
        "translation_sha256": sha256_file(&path)?, "font_sha256": sha256_file(&config.fonts.device_text.path)?,
        "font_px": config.fonts.device_text.font_px, "advance_px": 20, "line_advance_px": 26,
        "runtime_verification_required": true, "source_pointer_table_preserved": true,
        "assets": assets, "native_message_symbols": symbols,
    });
    Ok((
        DeviceTextBuild {
            overlay,
            overlay_ranges,
            allocated,
            expected_pixels,
        },
        report,
        ranges,
    ))
}

impl DeviceTextBuild {
    pub(crate) fn validate_composed(&self, inventory: &[u8], overlay: &[u8]) -> Result<()> {
        for (code, pixels) in &self.expected_pixels {
            ensure!(
                read_indexed_cell_in_prefix(inventory, TIM, cell(*code))? == *pixels,
                "device glyph {code:04x} differs from the shared message font or was overwritten"
            );
        }
        let allocated_aliases = self
            .allocated
            .iter()
            .flat_map(|c| aliases(*c))
            .collect::<BTreeSet<_>>();
        // Source Japanese slots become reusable only after ALL of their pointer
        // consumers have been translated by this build and the existing owners.
        let owned = owned_indices().collect::<BTreeSet<_>>();
        for index in 0..142 {
            if owned.contains(&index) {
                continue;
            }
            ensure!(
                referenced(overlay, pointer(overlay, index)?)?.is_disjoint(&allocated_aliases),
                "device glyph still supplies another pointer record: {index}"
            );
        }
        Ok(())
    }
}

fn owned_indices() -> impl Iterator<Item = usize> {
    (89..117).chain(118..131).chain(136..140)
}
fn cell(code: u16) -> Cell {
    Cell {
        x: usize::from(code >> 8) * 256 + ((usize::from(code & 15) * 20) & 255),
        y: (usize::from((code >> 4) & 15) * 20) & 255,
        width: 20,
        height: 20,
    }
}
fn aliases(code: u16) -> BTreeSet<u16> {
    (0..0x400)
        .filter(|c| cells_overlap(cell(code), cell(*c)))
        .collect()
}
fn pointer(data: &[u8], index: usize) -> Result<usize> {
    ensure!(index < 142, "device pointer index outside table");
    let p = u32::from_le_bytes(data[TABLE + index * 4..TABLE + index * 4 + 4].try_into()?);
    Ok(usize::try_from(
        p.checked_sub(BASE)
            .context("invalid inventory text pointer")?,
    )?)
}
fn single_character(text: &str) -> Result<char> {
    ensure!(
        text.chars().count() == 1,
        "device glyph must be one character"
    );
    Ok(text.chars().next().unwrap())
}
fn decode_hex(text: &str) -> Result<Vec<u8>> {
    ensure!(
        text.is_ascii() && text.len().is_multiple_of(2),
        "invalid source command hex"
    );
    (0..text.len())
        .step_by(2)
        .map(|i| Ok(u8::from_str_radix(&text[i..i + 2], 16)?))
        .collect()
}
fn encode(text: &str, codes: &BTreeMap<char, u16>) -> Result<Vec<u8>> {
    ensure!(
        !text.is_empty()
            && text.lines().count() <= 3
            && text.lines().all(|line| line.chars().count() <= 25),
        "device text exceeds native message pane"
    );
    let mut bytes = Vec::new();
    for ch in text.chars() {
        match ch {
            '\n' => bytes.push(0x80),
            ' ' => bytes.extend([0x63; 3]),
            _ => {
                let code = codes
                    .get(&ch)
                    .with_context(|| format!("missing device glyph {ch}"))?;
                bytes.extend([
                    (code >> 8) as u8,
                    (code & 15) as u8,
                    ((code >> 4) & 15) as u8,
                ]);
            }
        }
    }
    bytes.push(0x81);
    Ok(bytes)
}
fn referenced(data: &[u8], mut cursor: usize) -> Result<BTreeSet<u16>> {
    let mut result = BTreeSet::new();
    while cursor < TABLE {
        match data[cursor] {
            0x81 => return Ok(result),
            0x80 => cursor += 1,
            0x63 => {
                ensure!(
                    data.get(cursor..cursor + 3) == Some(&[0x63; 3]),
                    "bad inventory blank"
                );
                cursor += 3;
            }
            page => {
                let triplet = data
                    .get(cursor..cursor + 3)
                    .context("truncated inventory glyph")?;
                ensure!(
                    page < 4 && triplet[1] < 16 && triplet[2] < 16,
                    "bad inventory glyph"
                );
                result.insert(
                    u16::from(page) * 256 + u16::from(triplet[2]) * 16 + u16::from(triplet[1]),
                );
                cursor += 3;
            }
        }
    }
    anyhow::bail!("inventory string reaches pointer table")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn device_commands_preserve_choice_spacing_and_line_controls() {
        let codes = [('예', 0x100), ('아', 0x101), ('니', 0x102), ('요', 0x103)]
            .into_iter()
            .collect();
        let bytes = encode("예    아니요", &codes).unwrap();
        assert_eq!(&bytes[3..15], &[0x63; 12]);
        assert_eq!(bytes.len(), 25);
        assert_eq!(encode("예\n아니요", &codes).unwrap()[3], 0x80);
        assert!(encode("미", &codes).is_err());
        assert!(encode(&"예".repeat(26), &codes).is_err());
        assert!(encode("예\n예\n예\n예", &codes).is_err());
    }
}
