//! Source-bound Korean diagnosis component; callers compose the returned bounded writes.
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Context, Result, ensure};
use serde::Deserialize;

use crate::font::rasterize_menu_glyphs;
use crate::pipeline::{difference_ranges, sha256_bytes};
use crate::tim::{Cell, read_indexed_cell_in_prefix, write_indexed_cell_in_prefix_with_report};

#[derive(Deserialize)]
struct Drafts {
    entries: Vec<Draft>,
}
#[derive(Deserialize)]
struct Draft {
    id: String,
    source_stream_sha256: String,
    korean_lines: Vec<String>,
}

pub struct DiagnosisComponent {
    pub overlay: Vec<u8>,
    pub texture: Vec<u8>,
    pub overlay_ranges: Vec<[usize; 2]>,
    pub texture_ranges: Vec<[usize; 2]>,
    pub glyph_count: usize,
    pub page_count: usize,
}

fn word(data: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(
        data.get(offset..offset + 2)
            .context("truncated diagnosis word")?
            .try_into()?,
    ))
}
fn source_page(data: &[u8], start: usize) -> Result<(usize, BTreeSet<(usize, usize)>)> {
    let mut pos = start;
    let mut cells = BTreeSet::new();
    loop {
        ensure!(pos < 0x3314, "diagnosis page crossed group table");
        let page = word(data, pos)?;
        pos += 2;
        match page {
            0x81 => return Ok((pos, cells)),
            0x80 => (),
            _ => {
                let col = word(data, pos)?;
                let row = word(data, pos + 2)?;
                ensure!(
                    page < 4 && col < 12 && row < 12,
                    "invalid diagnosis coordinate"
                );
                cells.insert((
                    usize::from(page) * 256 + usize::from(col) * 20,
                    usize::from(row) * 20,
                ));
                pos += 4;
            }
        }
    }
}
fn cell(x: usize, y: usize) -> Cell {
    Cell {
        x,
        y,
        width: 20,
        height: 20,
    }
}

fn encode(lines: &[String], mapping: &BTreeMap<char, (usize, usize)>) -> Result<Vec<u8>> {
    ensure!(!lines.is_empty(), "empty diagnosis page");
    let mut bytes = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if i != 0 {
            bytes.extend_from_slice(&0x80u16.to_le_bytes());
        }
        for character in line.chars() {
            let &(x, y) = mapping.get(&character).context("missing diagnosis glyph")?;
            bytes.extend_from_slice(&u16::try_from(x / 256)?.to_le_bytes());
            bytes.push(u8::try_from(x % 256 / 20)?);
            bytes.push(u8::try_from(y / 20)?);
        }
    }
    bytes.extend_from_slice(&0x81u16.to_le_bytes());
    Ok(bytes)
}

/// Build from exact original inputs. Does not authorize unknown atlas consumers or runtime claims.
pub fn build(
    overlay: &[u8],
    texture: &[u8],
    drafts: &[u8],
    font: &Path,
) -> Result<DiagnosisComponent> {
    ensure!(
        sha256_bytes(overlay) == "3461b0ed24bb41acbbf44546d2c65ded2ac1d4d295423ed355d3f6b8352eca11",
        "diagnosis overlay identity changed"
    );
    ensure!(
        sha256_bytes(texture) == "e91b22fe572c61da07ab1779f35a59fbaa2d21588555ef639e8ce39b2ce0e862",
        "diagnosis texture identity changed"
    );
    let drafts: Drafts = serde_json::from_slice(drafts)?;
    let mut expected_pages = BTreeSet::new();
    let mut cursor = 0x3314;
    while cursor < 0x34cc {
        let count = u32::from_le_bytes(overlay[cursor..cursor + 4].try_into()?) as usize;
        ensure!((1..=3).contains(&count), "invalid diagnosis group count");
        for index in 0..count {
            let p = cursor + 4 + index * 4;
            let pointer = u32::from_le_bytes(overlay[p..p + 4].try_into()?);
            expected_pages.insert(usize::try_from(
                pointer
                    .checked_sub(0x800a2000)
                    .context("invalid page pointer")?,
            )?);
        }
        cursor += 4 + count * 4;
    }
    ensure!(
        cursor == 0x34cc && expected_pages.len() == 72,
        "diagnosis group coverage changed"
    );
    let mut used = BTreeSet::new();
    let mut pages = BTreeMap::new();
    let mut characters = BTreeSet::new();
    for draft in &drafts.entries {
        let start = usize::from_str_radix(
            draft.id.strip_prefix("text-").context("invalid page id")?,
            16,
        )?;
        ensure!(expected_pages.contains(&start), "unknown diagnosis page");
        let (end, cells) = source_page(overlay, start)?;
        ensure!(
            sha256_bytes(&overlay[start..end]) == draft.source_stream_sha256,
            "diagnosis page source hash changed"
        );
        ensure!(
            pages.insert(start, end).is_none(),
            "duplicate diagnosis page"
        );
        used.extend(cells);
        characters.extend(draft.korean_lines.iter().flat_map(|s| s.chars()));
    }
    ensure!(
        pages.keys().copied().collect::<BTreeSet<_>>() == expected_pages,
        "missing diagnosis pages"
    );
    let mut available: Vec<_> = used.iter().copied().collect();
    for page in 0..4 {
        for row in 0..12 {
            for col in 0..12 {
                let (x, y) = (page * 256 + col * 20, row * 20);
                if used.contains(&(x, y)) || (x, y) == (908, 0) {
                    continue;
                }
                if read_indexed_cell_in_prefix(texture, 0, cell(x, y))?
                    .iter()
                    .all(|&v| v == 0)
                {
                    available.push((x, y));
                }
            }
        }
    }
    ensure!(
        characters.len() <= available.len(),
        "insufficient diagnosis cells"
    );
    let mapping: BTreeMap<_, _> = characters.iter().copied().zip(available).collect();
    let visible: String = characters.iter().copied().filter(|&c| c != ' ').collect();
    let raster = rasterize_menu_glyphs(font, &visible, 16.0, 14, 2)?;
    ensure!(
        raster.font_sha256 == "389ad546769c0cb958b1c5c5c1d4b473867b433e0a6697b01907c7d7e1565c60",
        "diagnosis font identity changed"
    );
    let mut glyphs: BTreeMap<_, _> = raster
        .glyphs
        .into_iter()
        .map(|g| (g.character, g.pixels))
        .collect();
    glyphs.insert(' ', vec![0; 400]);
    let mut patched_texture = texture.to_vec();
    for (&character, &(x, y)) in &mapping {
        write_indexed_cell_in_prefix_with_report(
            &mut patched_texture,
            0,
            cell(x, y),
            &glyphs[&character],
        )?;
    }
    let mut patched_overlay = overlay.to_vec();
    for draft in &drafts.entries {
        let start = usize::from_str_radix(&draft.id[5..], 16)?;
        let packed = encode(&draft.korean_lines, &mapping)?;
        ensure!(
            packed.len() <= pages[&start] - start,
            "Korean page exceeds source span: {}",
            draft.id
        );
        patched_overlay[start..start + packed.len()].copy_from_slice(&packed);
    }
    for (offset, before, after) in [
        (0xbd8c, 0x96230000u32, 0x92230000u32),
        (0xbd90, 0x26310002, 0),
        (0xbdac, 0x96230000, 0x92230001),
    ] {
        ensure!(
            overlay[offset..offset + 4] == before.to_le_bytes(),
            "diagnosis instruction changed"
        );
        patched_overlay[offset..offset + 4].copy_from_slice(&after.to_le_bytes());
    }
    Ok(DiagnosisComponent {
        overlay_ranges: difference_ranges(overlay, &patched_overlay),
        texture_ranges: difference_ranges(texture, &patched_texture),
        overlay: patched_overlay,
        texture: patched_texture,
        glyph_count: mapping.len(),
        page_count: drafts.entries.len(),
    })
}

/// Compose against original bytes; reject a competing writer before changing the destination.
pub fn compose(
    original: &[u8],
    destination: &mut [u8],
    candidate: &[u8],
    ranges: &[[usize; 2]],
) -> Result<()> {
    ensure!(
        original.len() == destination.len() && original.len() == candidate.len(),
        "diagnosis composition size mismatch"
    );
    for &[start, end] in ranges {
        ensure!(
            start <= end && end <= original.len(),
            "invalid diagnosis write range"
        );
        ensure!(
            destination[start..end] == original[start..end],
            "diagnosis write conflicts at {start:#x}"
        );
    }
    for &[start, end] in ranges {
        destination[start..end].copy_from_slice(&candidate[start..end]);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compact_text_keeps_empty_line_and_end_tokens() {
        let mapping = BTreeMap::from([('가', (256, 40))]);
        assert_eq!(
            encode(&["가".into(), "".into(), "가".into()], &mapping).unwrap(),
            vec![1, 0, 0, 2, 128, 0, 128, 0, 1, 0, 0, 2, 129, 0]
        );
    }
    #[test]
    fn composition_checks_every_range_before_mutating() {
        let original = [0; 6];
        let mut destination = [0, 0, 0, 0, 9, 0];
        let before = destination;
        assert!(compose(&original, &mut destination, &[1; 6], &[[0, 2], [4, 6]]).is_err());
        assert_eq!(destination, before);
        compose(&original, &mut destination, &[1; 6], &[[0, 2]]).unwrap();
        assert_eq!(destination, [1, 1, 0, 0, 9, 0]);
    }
}

#[derive(Debug, serde::Serialize)]
pub struct DiagnosisBuildReport {
    pub title_sha256: String,
    pub score_label_count: usize,
    pub score_labels_sha256: String,
    pub page_count: usize,
    pub glyph_count: usize,
    pub translation_sha256: String,
    pub font_sha256: String,
    pub overlay_candidate_sha256: String,
    pub texture_candidate_sha256: String,
    pub runtime_verified: bool,
}
impl DiagnosisComponent {
    pub(crate) fn register_texture(
        &self,
        source: &[u8],
        plan: &mut crate::decoded_record_write_plan::DecodedRecordWritePlan<'_>,
    ) -> Result<()> {
        let claims = crate::decoded_record_write_plan::DecodedDataClaim::from_ranges(
            "cooperative-diagnosis",
            "Korean diagnosis glyph cells",
            self.texture_ranges.iter().copied(),
        );
        plan.register_data_candidate(
            "cooperative diagnosis",
            &sha256_bytes(source),
            &self.texture,
            &claims,
        )
    }
}

#[derive(Deserialize)]
struct ScoreLabels {
    tim_offset: usize,
    entries: Vec<ScoreLabel>,
}
#[derive(Deserialize)]
struct ScoreLabel {
    korean: String,
    descriptor_offset: usize,
    descriptor: [u16; 6],
    cell: Cell,
    source_cell_sha256: String,
}

/// Add the six native score-label sprites, preserving digits, stars and all palettes.
pub fn add_score_labels(
    component: &mut DiagnosisComponent,
    source_overlay: &[u8],
    source_texture: &[u8],
    manifest: &[u8],
    font: &Path,
) -> Result<usize> {
    let labels: ScoreLabels = serde_json::from_slice(manifest)?;
    ensure!(
        labels.tim_offset == 0x20800 && labels.entries.len() == 6,
        "score label coverage changed"
    );
    let rasterizer = crate::font::IndexedTextRasterizer::load(font)?;
    let mut candidate = component.texture.clone();
    let mut offsets = BTreeSet::new();
    for entry in &labels.entries {
        ensure!(
            offsets.insert(entry.descriptor_offset),
            "duplicate score descriptor"
        );
        ensure!(
            (0x3654..0x369c).contains(&entry.descriptor_offset)
                && (entry.descriptor_offset - 0x3654) % 12 == 0,
            "unknown score descriptor"
        );
        for (i, &expected) in entry.descriptor.iter().enumerate() {
            ensure!(
                word(source_overlay, entry.descriptor_offset + i * 2)? == expected,
                "score descriptor changed"
            );
        }
        let c = entry.cell;
        ensure!(
            [c.x, c.y, c.width, c.height]
                == entry.descriptor[2..]
                    .iter()
                    .map(|&v| usize::from(v))
                    .collect::<Vec<_>>()
                    .as_slice(),
            "score cell disagrees with native descriptor"
        );
        let pixels = read_indexed_cell_in_prefix(source_texture, labels.tim_offset, c)?;
        ensure!(
            sha256_bytes(&pixels) == entry.source_cell_sha256,
            "score source cell changed"
        );
        ensure!(
            read_indexed_cell_in_prefix(&candidate, labels.tim_offset, c)? == pixels,
            "score label conflicts with previous writer"
        );
        let rendered = rasterizer.rasterize(
            &entry.korean,
            c.width,
            c.height,
            18.0,
            0.0,
            0,
            Some(4),
            13,
            crate::font::HorizontalTextAlignment::Center,
        )?;
        write_indexed_cell_in_prefix_with_report(
            &mut candidate,
            labels.tim_offset,
            c,
            &rendered.pixels,
        )?;
    }
    component.texture_ranges = difference_ranges(source_texture, &candidate);
    component.texture = candidate;
    Ok(labels.entries.len())
}

#[derive(Deserialize)]
struct Title {
    korean: String,
    tim_offset: usize,
    font_px: f32,
    cells: Vec<TitleCell>,
}
#[derive(Deserialize)]
struct TitleCell {
    cell: Cell,
    source_cell_sha256: String,
}

/// Split one continuous 300-pixel title around the native unused atlas column.
pub fn add_title(
    component: &mut DiagnosisComponent,
    source: &[u8],
    manifest: &[u8],
    font: &Path,
) -> Result<()> {
    let title: Title = serde_json::from_slice(manifest)?;
    ensure!(
        title.tim_offset == 0x20800 && title.font_px == 30.0 && title.cells.len() == 2,
        "diagnosis title layout changed"
    );
    let rasterizer = crate::font::IndexedTextRasterizer::load(font)?;
    let raster = rasterizer.rasterize(
        &title.korean,
        300,
        37,
        title.font_px,
        0.0,
        0,
        Some(2),
        15,
        crate::font::HorizontalTextAlignment::Center,
    )?;
    let mut candidate = component.texture.clone();
    let mut consumed = 0;
    for (entry, (x, width)) in title.cells.iter().zip([(0, 255), (256, 45)]) {
        ensure!(
            entry.cell
                == Cell {
                    x,
                    y: 208,
                    width,
                    height: 37
                },
            "diagnosis title source slice changed"
        );
        let pixels = read_indexed_cell_in_prefix(source, title.tim_offset, entry.cell)?;
        ensure!(
            sha256_bytes(&pixels) == entry.source_cell_sha256,
            "diagnosis title source hash changed"
        );
        ensure!(
            read_indexed_cell_in_prefix(&candidate, title.tim_offset, entry.cell)? == pixels,
            "diagnosis title conflicts with previous writer"
        );
        let sliced: Vec<u8> = (0..37)
            .flat_map(|y| {
                raster.pixels[y * 300 + consumed..y * 300 + consumed + width]
                    .iter()
                    .copied()
            })
            .collect();
        write_indexed_cell_in_prefix_with_report(
            &mut candidate,
            title.tim_offset,
            entry.cell,
            &sliced,
        )?;
        consumed += width;
    }
    component.texture_ranges = difference_ranges(source, &candidate);
    component.texture = candidate;
    Ok(())
}
