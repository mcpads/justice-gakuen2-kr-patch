//! Source-bound illustrated text panels with privately retained restoration plates.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

use super::model::{ModeDescendantGraphicsBuildConfig, ModeDescendantRecord};
use super::record_compositor::ModeDescendantRecordDraft;
use crate::decoded_record_write_plan::DecodedDataClaim;
use crate::font::{HorizontalTextAlignment, IndexedTextRasterizers};
use crate::pipeline::sha256_bytes;
use crate::tim::{
    Cell, cells_overlap, parse_8bpp_prefix, read_8bpp_indexed_cell_in_prefix,
    write_8bpp_indexed_cell_in_prefix_with_report,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Panel {
    kind: String,
    id: String,
    records: Vec<ModeDescendantRecord>,
    tim_offset: usize,
    source_tim_sha256: String,
    restoration_file: PathBuf,
    restoration_sha256: String,
    mask_file: PathBuf,
    mask_sha256: String,
    #[serde(default)]
    lettering_source: LetteringSource,
    regions: Vec<Region>,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "snake_case")]
enum LetteringSource {
    #[default]
    Font,
    Artwork,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Region {
    id: String,
    source_text: String,
    korean_lines: Vec<String>,
    cell: Cell,
    font_role: Option<FontRole>,
    font_px: Option<f32>,
    source_indexed_sha256: String,
    fill_index: Option<u8>,
    outline_index: Option<u8>,
    text_cell: Option<Cell>,
    #[serde(default)]
    overlays: Vec<TextLayer>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TextLayer {
    source_text: String,
    korean_lines: Vec<String>,
    cell: Cell,
    font_role: FontRole,
    font_px: Option<f32>,
    fill_index: u8,
    outline_index: u8,
    #[serde(default)]
    clockwise_rotation_degrees: f64,
    alignment: Option<super::model::TextAlignment>,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum FontRole {
    Heading,
    Subheading,
    Body,
    Hint,
}

#[derive(Debug, Serialize)]
pub struct IllustratedPanelReport {
    pub id: String,
    pub source_record: String,
    pub source_tim_sha256: String,
    pub panel_spec_sha256: String,
    pub restoration_sha256: String,
    pub mask_sha256: String,
    pub region_count: usize,
    pub restored_pixel_count: usize,
    pub changed_pixel_count: usize,
    pub outside_owned_regions_preserved: bool,
    pub palette_and_header_preserved: bool,
}

pub(super) fn apply(
    config: &ModeDescendantGraphicsBuildConfig,
    drafts: &mut [ModeDescendantRecordDraft],
    paths: &[PathBuf],
) -> Result<Vec<IllustratedPanelReport>> {
    let mut reports = Vec::new();
    for path in paths {
        let spec_bytes = std::fs::read(path)?;
        let panel: Panel = serde_json::from_slice(&spec_bytes)?;
        ensure!(
            !panel.records.is_empty(),
            "illustrated panel has no consumers"
        );
        let mut records = BTreeSet::new();
        for record in &panel.records {
            ensure!(
                records.insert(*record),
                "duplicate illustrated panel consumer"
            );
            reports.push(apply_panel(
                config,
                drafts,
                path,
                &spec_bytes,
                &panel,
                *record,
            )?);
        }
    }
    Ok(reports)
}

fn pinned_file(path: &Path, expected: &str) -> Result<Vec<u8>> {
    let data = std::fs::read(path).with_context(|| {
        format!(
            "required illustrated-panel restoration input missing: {}",
            path.display()
        )
    })?;
    ensure!(
        sha256_bytes(&data) == expected,
        "illustrated-panel input identity changed: {}",
        path.display()
    );
    Ok(data)
}

fn apply_panel(
    config: &ModeDescendantGraphicsBuildConfig,
    drafts: &mut [ModeDescendantRecordDraft],
    path: &Path,
    spec_bytes: &[u8],
    panel: &Panel,
    record: ModeDescendantRecord,
) -> Result<IllustratedPanelReport> {
    ensure!(
        panel.kind == "justice_gakuen2_illustrated_panel" && !panel.id.is_empty(),
        "invalid illustrated panel identity"
    );
    ensure!(
        !panel.regions.is_empty(),
        "illustrated panel has no owned regions"
    );
    let parent = path.parent().context("illustrated panel has no parent")?;
    let restoration = pinned_file(
        &parent.join(&panel.restoration_file),
        &panel.restoration_sha256,
    )?;
    let mask = pinned_file(&parent.join(&panel.mask_file), &panel.mask_sha256)?;
    let draft = drafts
        .iter_mut()
        .find(|draft| draft.spec.record == record)
        .context("illustrated panel has no admitted physical record")?;
    let source = draft.decoded.clone();
    let tim_data = source
        .get(panel.tim_offset..)
        .context("illustrated TIM outside record")?;
    let tim = parse_8bpp_prefix(tim_data)?;
    ensure!(
        sha256_bytes(&tim_data[..tim.total_size]) == panel.source_tim_sha256,
        "illustrated panel source TIM changed or another writer already owns it"
    );
    let full = Cell {
        x: 0,
        y: 0,
        width: tim.pixel_width(),
        height: tim.image_height,
    };
    let source_pixels = read_8bpp_indexed_cell_in_prefix(&source, panel.tim_offset, full)?;
    let cells = panel
        .regions
        .iter()
        .map(|region| region.cell)
        .collect::<Vec<_>>();
    let (restored, owners) = restore_masked_pixels(
        &source_pixels,
        &restoration,
        &mask,
        full.width,
        full.height,
        &cells,
    )?;
    let mut ids = BTreeSet::new();
    let mut rasterizers = IndexedTextRasterizers::default();
    let mut restored_pixel_count = 0;
    for (index, region) in panel.regions.iter().enumerate() {
        ensure!(
            !region.id.is_empty() && ids.insert(&region.id) && !region.source_text.is_empty(),
            "invalid illustrated region identity"
        );
        ensure!(
            !panel.regions[..index]
                .iter()
                .any(|other| cells_overlap(other.cell, region.cell)),
            "illustrated text regions overlap"
        );
        let mut pixels = read_8bpp_indexed_cell_in_prefix(&source, panel.tim_offset, region.cell)?;
        ensure!(
            sha256_bytes(&pixels) == region.source_indexed_sha256,
            "illustrated source region {} changed",
            region.id
        );
        for y in 0..region.cell.height {
            for x in 0..region.cell.width {
                let global = (region.cell.y + y) * full.width + region.cell.x + x;
                if mask[global] == 1 {
                    pixels[y * region.cell.width + x] = restored[global];
                    restored_pixel_count += 1;
                }
            }
        }
        ensure!(
            !region.korean_lines.is_empty()
                && region
                    .korean_lines
                    .iter()
                    .all(|line| !line.trim().is_empty()),
            "illustrated region has empty translation lines"
        );
        if matches!(panel.lettering_source, LetteringSource::Font) {
            let main_text = TextLayer {
                source_text: region.source_text.clone(),
                korean_lines: region.korean_lines.clone(),
                cell: region.text_cell.unwrap_or(region.cell),
                font_role: region
                    .font_role
                    .context("font lettering has no font role")?,
                font_px: region.font_px,
                fill_index: region
                    .fill_index
                    .context("font lettering has no fill index")?,
                outline_index: region
                    .outline_index
                    .context("font lettering has no outline index")?,
                clockwise_rotation_degrees: 0.0,
                alignment: None,
            };
            let mut ink_owners = vec![false; pixels.len()];
            for layer in std::iter::once(&main_text).chain(&region.overlays) {
                render_layer(
                    config,
                    &mut rasterizers,
                    region.cell,
                    &mut pixels,
                    &mut ink_owners,
                    layer,
                )?;
            }
        } else {
            ensure!(
                region.font_role.is_none()
                    && region.font_px.is_none()
                    && region.fill_index.is_none()
                    && region.outline_index.is_none()
                    && region.text_cell.is_none()
                    && region.overlays.is_empty(),
                "artwork lettering must not carry ignored font or overlay settings"
            );
        }
        let write = write_8bpp_indexed_cell_in_prefix_with_report(
            &mut draft.decoded,
            panel.tim_offset,
            region.cell,
            &pixels,
        )?;
        draft
            .decoded_write_claims
            .extend(DecodedDataClaim::from_effective_ranges(
                &format!("illustrated-panel:{}:{}", panel.id, region.id),
                "compose source-bound illustrated panel lettering",
                &source,
                &draft.decoded,
                write.allowed_ranges,
            )?);
    }
    ensure!(
        mask.iter()
            .zip(&owners)
            .all(|(value, owned)| *value == 0 || *owned),
        "restoration mask escapes owned text regions"
    );
    let output = read_8bpp_indexed_cell_in_prefix(&draft.decoded, panel.tim_offset, full)?;
    ensure!(
        source_pixels
            .iter()
            .zip(&output)
            .zip(&owners)
            .all(|((before, after), owned)| *owned || before == after),
        "illustrated panel changed protected artwork"
    );
    let header_end = panel.tim_offset + tim.pixel_offset;
    ensure!(
        source[panel.tim_offset..header_end] == draft.decoded[panel.tim_offset..header_end],
        "illustrated panel changed TIM header or palettes"
    );
    Ok(IllustratedPanelReport {
        id: panel.id.clone(),
        source_record: draft.spec.source_path.to_string(),
        source_tim_sha256: panel.source_tim_sha256.clone(),
        panel_spec_sha256: sha256_bytes(spec_bytes),
        restoration_sha256: panel.restoration_sha256.clone(),
        mask_sha256: panel.mask_sha256.clone(),
        region_count: panel.regions.len(),
        restored_pixel_count,
        changed_pixel_count: source_pixels
            .iter()
            .zip(&output)
            .filter(|(a, b)| a != b)
            .count(),
        outside_owned_regions_preserved: true,
        palette_and_header_preserved: true,
    })
}

fn render_layer(
    config: &ModeDescendantGraphicsBuildConfig,
    rasterizers: &mut IndexedTextRasterizers,
    owner: Cell,
    pixels: &mut [u8],
    ink_owners: &mut [bool],
    layer: &TextLayer,
) -> Result<()> {
    let cell = layer.cell;
    ensure!(
        cell.width > 0
            && cell.height > 0
            && cell.x >= owner.x
            && cell.y >= owner.y
            && cell
                .x
                .checked_add(cell.width)
                .is_some_and(|end| end <= owner.x + owner.width)
            && cell
                .y
                .checked_add(cell.height)
                .is_some_and(|end| end <= owner.y + owner.height),
        "illustrated text layer leaves owned source region"
    );
    ensure!(
        !layer.source_text.is_empty()
            && !layer.korean_lines.is_empty()
            && layer
                .korean_lines
                .iter()
                .all(|line| !line.trim().is_empty()),
        "illustrated region has empty translation lines"
    );
    let style = match layer.font_role {
        FontRole::Heading => &config.fonts.practical_title,
        FontRole::Subheading => &config.fonts.practical_result_heading,
        FontRole::Body => &config.fonts.practical_result_label,
        FontRole::Hint => &config.fonts.practical_result_hint,
    };
    let font_px = layer.font_px.unwrap_or(style.font_px);
    ensure!(
        font_px.is_finite() && font_px > 0.0 && layer.clockwise_rotation_degrees.is_finite(),
        "invalid illustrated text transform"
    );
    let rasterizer = rasterizers.for_font(&style.path)?;
    let line_height = cell.height / layer.korean_lines.len();
    ensure!(line_height > 0, "illustrated region has too many lines");
    let mut markers = vec![0; cell.width * cell.height];
    for (line_index, line) in layer.korean_lines.iter().enumerate() {
        let raster = rasterizer.rasterize(
            line,
            cell.width,
            line_height,
            font_px,
            0.0,
            0,
            Some(1),
            2,
            if matches!(layer.alignment, Some(super::model::TextAlignment::Center))
                || (layer.alignment.is_none() && matches!(layer.font_role, FontRole::Heading))
            {
                HorizontalTextAlignment::Center
            } else {
                HorizontalTextAlignment::Left
            },
        )?;
        let start = line_index * line_height * cell.width;
        markers[start..start + raster.pixels.len()].copy_from_slice(&raster.pixels);
    }
    ensure_rotation_fits(
        &markers,
        cell.width,
        cell.height,
        layer.clockwise_rotation_degrees,
    )?;
    let markers = crate::font::rotate_indexed_raster(
        &markers,
        cell.width,
        cell.height,
        0,
        layer.clockwise_rotation_degrees,
    )?;
    for (offset, value) in markers.iter().enumerate().filter(|(_, value)| **value != 0) {
        let destination = (cell.y - owner.y + offset / cell.width) * owner.width + cell.x - owner.x
            + offset % cell.width;
        ensure!(
            !ink_owners[destination],
            "illustrated text layers overlap visible ink"
        );
        ink_owners[destination] = true;
        pixels[destination] = match value {
            1 => layer.outline_index,
            2 => layer.fill_index,
            _ => unreachable!("three-marker text raster"),
        };
    }
    Ok(())
}

fn ensure_rotation_fits(pixels: &[u8], width: usize, height: usize, degrees: f64) -> Result<()> {
    let (sin, cos) = degrees.to_radians().sin_cos();
    let cx = (width - 1) as f64 / 2.0;
    let cy = (height - 1) as f64 / 2.0;
    for (offset, _) in pixels.iter().enumerate().filter(|(_, value)| **value != 0) {
        let dx = (offset % width) as f64 - cx;
        let dy = (offset / width) as f64 - cy;
        let x = (cx + dx * cos - dy * sin).round();
        let y = (cy + dx * sin + dy * cos).round();
        ensure!(
            x >= 0.0 && y >= 0.0 && x < width as f64 && y < height as f64,
            "rotated illustrated text would clip"
        );
    }
    Ok(())
}

fn restore_masked_pixels(
    source: &[u8],
    restoration: &[u8],
    mask: &[u8],
    width: usize,
    height: usize,
    cells: &[Cell],
) -> Result<(Vec<u8>, Vec<bool>)> {
    ensure!(
        width.checked_mul(height) == Some(source.len())
            && restoration.len() == source.len()
            && mask.len() == source.len(),
        "illustrated restoration geometry differs from native TIM"
    );
    ensure!(
        mask.iter().all(|value| *value <= 1),
        "illustrated restoration mask is not binary"
    );
    let mut owners = vec![false; source.len()];
    for cell in cells {
        ensure!(
            cell.width > 0
                && cell.height > 0
                && cell
                    .x
                    .checked_add(cell.width)
                    .is_some_and(|end| end <= width)
                && cell
                    .y
                    .checked_add(cell.height)
                    .is_some_and(|end| end <= height),
            "illustrated ownership leaves the source image"
        );
        for y in cell.y..cell.y + cell.height {
            for x in cell.x..cell.x + cell.width {
                let index = y * width + x;
                ensure!(!owners[index], "illustrated ownership overlaps");
                owners[index] = true;
            }
        }
    }
    let mut output = source.to_vec();
    for (index, value) in mask.iter().enumerate() {
        ensure!(
            *value == 0 || owners[index],
            "restoration mask escapes owned text regions"
        );
        if *value == 1 {
            output[index] = restoration[index];
        }
    }
    Ok((output, owners))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tilted_stamp_text_rejects_ink_that_would_be_cut_off() {
        assert!(ensure_rotation_fits(&[1; 15], 5, 3, 30.0).is_err());
        let mut pixels = vec![0; 77];
        pixels[3 * 11 + 5] = 2;
        assert!(ensure_rotation_fits(&pixels, 11, 7, -14.0).is_ok());
    }

    #[test]
    fn restoration_preserves_source_art_even_when_generated_plate_changes_it() {
        let source = [1, 2, 3, 4, 5, 6];
        let plate = [9; 6];
        let cells = [Cell {
            x: 1,
            y: 0,
            width: 2,
            height: 1,
        }];
        let (output, _) =
            restore_masked_pixels(&source, &plate, &[0, 1, 0, 0, 0, 0], 3, 2, &cells).unwrap();
        assert_eq!(output, [1, 9, 3, 4, 5, 6]);
    }
    #[test]
    fn restoration_rejects_a_mask_covering_a_protected_button() {
        let cells = [Cell {
            x: 1,
            y: 0,
            width: 2,
            height: 1,
        }];
        let error =
            restore_masked_pixels(&[1; 6], &[9; 6], &[1, 0, 0, 0, 0, 0], 3, 2, &cells).unwrap_err();
        assert!(error.to_string().contains("escapes owned text"));
    }
}
