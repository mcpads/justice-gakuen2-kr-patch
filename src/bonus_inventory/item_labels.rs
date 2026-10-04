use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};
use psx_r3000a::{Instruction, Register, decode};
use serde::{Deserialize, Serialize};

use crate::bonus_inventory_source::BonusInventorySource;
use crate::font::{HorizontalTextAlignment, IndexedTextRasterizer};
use crate::pipeline::{sha256_bytes, sha256_file};
use crate::tim::{
    Cell, cells_overlap, read_indexed_cell_in_prefix, write_indexed_cell_in_prefix_with_report,
};

use super::model::BonusInventoryBuildConfig;

#[path = "item_label_renderer.rs"]
mod renderer;

const BASE: u32 = 0x800a_2000;
const TABLE: usize = 0x11e0;
const TABLE_END: usize = 0x1418;
const DATA_START: usize = 0x10;
const DATA_END: usize = 0x7e8;
const COUNT: usize = 87;
const GLYPH_TIM: usize = 0x21000;
const ADVANCE: usize = 14;
// Each row has 728 bytes for double-buffered 56-byte glyph packets. Blank
// commands advance x without consuming a packet.
const GLYPHS_PER_LINE: usize = 13;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Assets {
    kind: String,
    source_overlay_sha256: String,
    entries: Vec<Entry>,
    glyphs: Vec<Glyph>,
    release_status: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    index: usize,
    source_offset: usize,
    source_hex: String,
    source_text: String,
    korean_text: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Glyph {
    text: String,
    code: u16,
    source_indexed_sha256: String,
}

pub(crate) struct ItemLabelBuild {
    pub overlay: Vec<u8>,
    pub overlay_ranges: Vec<[usize; 2]>,
}

#[derive(Debug, Serialize)]
pub struct ItemLabelReport {
    pub translation_sha256: String,
    pub font_sha256: String,
    pub font_px: f32,
    pub advance_px: usize,
    pub glyph_packets_per_line: usize,
    pub encoded_size: usize,
    pub capacity: usize,
    pub runtime_verification_required: bool,
    pub release_status: String,
    entries: Vec<Entry>,
    glyphs: Vec<Glyph>,
}

pub(super) fn build(
    config: &BonusInventoryBuildConfig,
    source: &BonusInventorySource,
    inventory: &mut [u8],
) -> Result<(ItemLabelBuild, ItemLabelReport, Vec<[usize; 2]>)> {
    let asset_path = config.assets.join("item-labels.json");
    let assets: Assets = serde_json::from_slice(&std::fs::read(&asset_path)?)?;
    ensure!(
        assets.kind == "justice_gakuen2_bonus_item_labels",
        "wrong bonus item-label asset kind"
    );
    ensure!(
        assets.release_status == "needs_human_review",
        "bonus item-label release status requires review"
    );
    ensure!(
        sha256_bytes(&source.overlay) == assets.source_overlay_sha256,
        "bonus item-label source overlay changed"
    );
    validate_source(&source.overlay, &assets)?;

    let font = &config.fonts.item_label;
    let rasterizer = IndexedTextRasterizer::load(&font.path)?;
    let mut inventory_ranges = Vec::new();
    for glyph in &assets.glyphs {
        let cell = glyph_cell(glyph.code);
        let pixels = read_indexed_cell_in_prefix(inventory, GLYPH_TIM, cell)?;
        ensure!(
            sha256_bytes(&pixels) == glyph.source_indexed_sha256,
            "bonus item glyph source changed: 0x{:04x}",
            glyph.code
        );
        // The native sprite remains 20 pixels wide; its rightmost six pixels are
        // transparent, so adjacent 14-pixel characters cannot erase each other.
        let raster = rasterizer.rasterize_shifted(
            &glyph.text,
            ADVANCE,
            20,
            font.font_px,
            font.tracking_px,
            font.vertical_shift_px,
            0,
            Some(1),
            15,
            HorizontalTextAlignment::Center,
        )?;
        ensure!(
            raster.ink_bounds[2] <= ADVANCE && raster.ink_bounds[3] <= 20,
            "bonus item-label glyph clips"
        );
        let mut output = vec![0; 400];
        for y in 0..20 {
            output[y * 20..y * 20 + ADVANCE]
                .copy_from_slice(&raster.pixels[y * ADVANCE..(y + 1) * ADVANCE]);
        }
        inventory_ranges.extend(
            write_indexed_cell_in_prefix_with_report(inventory, GLYPH_TIM, cell, &output)?
                .allowed_ranges,
        );
    }
    let codes = assets
        .glyphs
        .iter()
        .map(|g| (g.text.chars().next().unwrap(), g.code))
        .collect::<BTreeMap<_, _>>();
    let mut overlay = source.overlay.clone();
    overlay[DATA_START..renderer::CD_ENTRY].fill(0x81);
    let mut cursor = DATA_START;
    for entry in &assets.entries {
        let encoded = encode(&entry.korean_text, &codes)?;
        ensure!(
            cursor + encoded.len() <= renderer::CD_ENTRY,
            "bonus item labels exceed their owned string region"
        );
        // Pointer consumers use LBU, so the repacked records need no padding.
        let pointer = BASE + u32::try_from(cursor)?;
        let offset = TABLE + entry.index * 4;
        overlay[offset..offset + 4].copy_from_slice(&pointer.to_le_bytes());
        overlay[cursor..cursor + encoded.len()].copy_from_slice(&encoded);
        cursor += encoded.len();
    }
    let (overlay, overlay_ranges) = renderer::install(
        &source.overlay,
        &overlay,
        vec![[DATA_START, renderer::CD_ENTRY], [TABLE, TABLE + COUNT * 4]],
    )?;
    let report = ItemLabelReport {
        translation_sha256: sha256_file(&asset_path)?,
        font_sha256: sha256_file(&font.path)?,
        font_px: font.font_px,
        advance_px: ADVANCE,
        glyph_packets_per_line: GLYPHS_PER_LINE,
        encoded_size: cursor - DATA_START,
        capacity: renderer::CD_ENTRY - DATA_START,
        runtime_verification_required: true,
        release_status: assets.release_status,
        entries: assets.entries,
        glyphs: assets.glyphs,
    };
    Ok((
        ItemLabelBuild {
            overlay,
            overlay_ranges,
        },
        report,
        inventory_ranges,
    ))
}

fn encode(text: &str, codes: &BTreeMap<char, u16>) -> Result<Vec<u8>> {
    ensure!(
        !text.is_empty() && text.chars().count() * ADVANCE <= 224,
        "bonus item-label text is empty or exceeds its list pane"
    );
    ensure!(
        text.chars().filter(|ch| *ch != ' ').count() <= GLYPHS_PER_LINE,
        "bonus item label exceeds its native glyph-packet row"
    );
    let mut result = Vec::new();
    for ch in text.chars() {
        if ch == ' ' {
            result.extend_from_slice(&[0x63; 3]);
        } else {
            let code = *codes
                .get(&ch)
                .with_context(|| format!("unallocated bonus item-label character {ch}"))?;
            result.extend_from_slice(&[
                (code >> 8) as u8,
                (code & 15) as u8,
                ((code >> 4) & 15) as u8,
            ]);
        }
    }
    result.push(0x81);
    Ok(result)
}

fn glyph_cell(code: u16) -> Cell {
    Cell {
        x: (code as usize >> 8) * 256 + (((code as usize & 15) * 20) & 255),
        y: ((code as usize >> 4 & 15) * 20) & 255,
        width: 20,
        height: 20,
    }
}

fn validate_source(source: &[u8], assets: &Assets) -> Result<()> {
    ensure!(
        assets.entries.len() == COUNT,
        "bonus item-label source scope changed"
    );
    for (index, entry) in assets.entries.iter().enumerate() {
        ensure!(
            entry.index == index && (DATA_START..DATA_END).contains(&entry.source_offset),
            "bonus item-label index/offset changed"
        );
        ensure!(
            read_word(source, TABLE + index * 4)? == BASE + entry.source_offset as u32,
            "bonus item-label pointer changed"
        );
        ensure!(
            entry.source_hex.len().is_multiple_of(2) && entry.source_hex.is_ascii(),
            "invalid source hexadecimal commands"
        );
        let bytes = (0..entry.source_hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&entry.source_hex[i..i + 2], 16))
            .collect::<std::result::Result<Vec<_>, _>>()?;
        ensure!(
            bytes.last() == Some(&0x81)
                && entry.source_offset + bytes.len() <= DATA_END
                && source.get(entry.source_offset..entry.source_offset + bytes.len())
                    == Some(bytes.as_slice()),
            "bonus item-label source commands changed"
        );
    }
    ensure!(
        read_word(source, TABLE + COUNT * 4)? == BASE + DATA_END as u32,
        "bonus card-action string boundary changed"
    );
    let remaining = referenced_glyphs(source, TABLE + COUNT * 4, TABLE_END)?;
    let original = referenced_glyphs(source, TABLE, TABLE + COUNT * 4)?;
    let reserved =
        crate::bonus_inventory_source::reserved_dynamic_glyph_codes().collect::<BTreeSet<_>>();
    let mut allocated = BTreeSet::new();
    let mut characters = BTreeSet::new();
    for glyph in &assets.glyphs {
        ensure!(
            glyph.text.chars().count() == 1
                && characters.insert(glyph.text.clone())
                && allocated.insert(glyph.code),
            "duplicate bonus item-label allocation"
        );
        ensure!(
            ((2..=3).contains(&(glyph.code >> 8))
                || [0x0172, 0x0175, 0x0180].contains(&glyph.code)
                || (glyph.code >> 8 == 1 && original.contains(&glyph.code)))
                && (glyph.code & 15) < 12
                && ((glyph.code >> 4) & 15) < 12,
            "bonus item-label allocation touches an unowned page or wrapped cell"
        );
        let cell = glyph_cell(glyph.code);
        let aliases = (0x0100..0x0400)
            .filter(|code| cells_overlap(cell, glyph_cell(*code)))
            .collect::<BTreeSet<_>>();
        ensure!(
            aliases.is_disjoint(&remaining)
                && aliases.is_disjoint(&reserved)
                && aliases.is_disjoint(&[0x017a, 0x0197, 0x0198, 0x0199].into_iter().collect())
                && aliases.is_disjoint(&[0x0271, 0x026a].into_iter().collect()),
            "bonus item-label allocation touches a retained/computed native glyph"
        );
        ensure!(
            original.contains(&glyph.code)
                || glyph.source_indexed_sha256
                    == crate::source_disc::profile::BONUS_INVENTORY_BLANK_GLYPH_INDEXED_SHA256,
            "bonus item-label allocation is neither a replaced source glyph nor a blank"
        );
        for alias in aliases {
            let selector = [
                8 + (alias >> 8) as u8,
                (alias & 15) as u8,
                ((alias >> 4) & 15) as u8,
            ];
            ensure!(
                !source[TABLE_END..0x3afc].windows(3).any(|s| s == selector),
                "bonus item-label allocation aliases a direct sprite selector"
            );
        }
    }
    for (i, left) in assets.glyphs.iter().enumerate() {
        ensure!(
            assets.glyphs[i + 1..]
                .iter()
                .all(|right| !cells_overlap(glyph_cell(left.code), glyph_cell(right.code))),
            "bonus item-label cells overlap"
        );
    }
    let callers = (0x3afc..source.len() - 3)
        .step_by(4)
        .filter(|offset| {
            decode(read_word(source, *offset).unwrap(), BASE + *offset as u32).ok()
                == Some(Instruction::Jal {
                    target: 0x800a_b86c,
                })
        })
        .collect::<Vec<_>>();
    ensure!(
        callers == [0x6fe0, 0x70bc, 0x76e0],
        "bonus item-label parser has an unexpected caller"
    );
    expect_instruction(
        source,
        0x986c,
        Instruction::Addiu {
            rt: Register::SP,
            rs: Register::SP,
            immediate: -56,
        },
    )?;
    expect_instruction(
        source,
        0x98a8,
        Instruction::Lbu {
            rt: Register::A2,
            base: Register::S2,
            offset: 0,
        },
    )?;
    Ok(())
}

fn referenced_glyphs(source: &[u8], start: usize, end: usize) -> Result<BTreeSet<u16>> {
    let mut codes = BTreeSet::new();
    for offset in (start..end).step_by(4) {
        let mut cursor = usize::try_from(
            read_word(source, offset)?
                .checked_sub(BASE)
                .context("invalid bonus pointer")?,
        )?;
        ensure!(cursor < TABLE, "bonus pointer outside strings");
        loop {
            let op = *source.get(cursor).context("unterminated bonus text")?;
            match op {
                0x81 => break,
                0x80 => cursor += 1,
                0x63 => {
                    ensure!(
                        source.get(cursor..cursor + 3) == Some(&[0x63; 3]),
                        "invalid bonus blank command"
                    );
                    cursor += 3;
                }
                page => {
                    let triplet = source
                        .get(cursor..cursor + 3)
                        .context("truncated bonus glyph")?;
                    ensure!(
                        page < 4 && triplet[1] < 16 && triplet[2] < 16,
                        "invalid bonus glyph"
                    );
                    codes.insert(
                        u16::from(page) << 8 | u16::from(triplet[2]) << 4 | u16::from(triplet[1]),
                    );
                    cursor += 3;
                }
            }
            ensure!(cursor < TABLE, "unterminated bonus text reaches pointers");
        }
    }
    Ok(codes)
}

fn read_word(source: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        source
            .get(offset..offset + 4)
            .context("truncated bonus overlay word")?
            .try_into()?,
    ))
}

fn expect_instruction(source: &[u8], offset: usize, expected: Instruction) -> Result<()> {
    ensure!(
        decode(read_word(source, offset)?, BASE + offset as u32)? == expected,
        "bonus item-label consumer changed at +0x{offset:x}"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixed_labels_keep_spaces_ascii_and_terminators() {
        let codes = [('가', 0x0205), ('E', 0x0308), ('D', 0x0309), ('1', 0x030a)]
            .into_iter()
            .collect();
        assert_eq!(
            encode("가 ED1", &codes).unwrap(),
            [2, 5, 0, 99, 99, 99, 3, 8, 0, 3, 9, 0, 3, 10, 0, 129]
        );
        assert!(encode("가 E2", &codes).is_err());
        assert!(encode(&"가".repeat(17), &codes).is_err());
        assert!(encode(&"가".repeat(13), &codes).is_ok());
        assert!(encode(&"가".repeat(14), &codes).is_err());
        assert!(encode(&format!("{}   ", "가".repeat(13)), &codes).is_ok());
    }

    #[test]
    #[ignore = "requires private source disc and selected fonts"]
    fn source_bound_item_labels_preserve_unowned_records() -> Result<()> {
        let spec = crate::development_build_spec::load_development_build_spec(
            std::path::Path::new("assets/build/development.json"),
        )?;
        let source_disc = crate::source_disc::SupportedSourceDisc::open(std::path::Path::new(
            "roms/Shiritsu Justice Gakuen - Nekketsu Seishun Nikki 2 (Japan).cue",
        ))?;
        let source = crate::bonus_inventory_source::load_bonus_inventory_source(&source_disc)?;
        let config = BonusInventoryBuildConfig {
            cue: std::path::PathBuf::new(),
            assets: spec.assets.bonus_inventory,
            fonts: spec.fonts.bonus_inventory,
            build_spec_sha256: spec.sha256,
            output_dir: std::path::PathBuf::new(),
            force: false,
        };
        let mut inventory = source.inventory_decoded.clone();
        let (built, report, _) = build(&config, &source, &mut inventory)?;
        assert_eq!(report.entries.len(), COUNT);
        assert_eq!(
            &built.overlay[DATA_END..TABLE],
            &source.overlay[DATA_END..TABLE]
        );
        assert_eq!(
            &built.overlay[TABLE + COUNT * 4..TABLE_END],
            &source.overlay[TABLE + COUNT * 4..TABLE_END]
        );
        assert!(report.encoded_size <= report.capacity);
        for glyph in &report.glyphs {
            let pixels =
                read_indexed_cell_in_prefix(&inventory, GLYPH_TIM, glyph_cell(glyph.code))?;
            assert!(pixels.iter().all(|p| [0, 1, 15].contains(p)));
            assert!(
                pixels
                    .as_chunks::<20>()
                    .0
                    .iter()
                    .all(|row| row[ADVANCE..].iter().all(|p| *p == 0))
            );
        }
        let mut assets: Assets =
            serde_json::from_slice(&std::fs::read(config.assets.join("item-labels.json"))?)?;
        let bar = assets.glyphs.iter_mut().find(|g| g.text == "바").unwrap();
        let safe_code = bar.code;
        bar.code = 0x035b;
        assert!(validate_source(&source.overlay, &assets).is_err());
        assets
            .glyphs
            .iter_mut()
            .find(|g| g.text == "바")
            .unwrap()
            .code = safe_code;
        let mut changed = source.overlay.clone();
        changed[TABLE] ^= 1;
        assert!(validate_source(&changed, &assets).is_err());
        assets.glyphs[0].code = 0x0271;
        assert!(validate_source(&source.overlay, &assets).is_err());
        Ok(())
    }
}
