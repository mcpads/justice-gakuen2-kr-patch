//! Continue selects one MA_TIT sprite from the saved school index. Keep that
//! selection, its descriptor table, palettes and slot data unchanged.

use super::catalog::{ModeDescendantRecordSpec, ModeDescendantTextureOutputSpec};
use super::model::{
    ModeDescendantGraphicsBuildConfig, ModeDescendantRecord, ModeDescendantStorageKind,
    ModeDescendantSurface,
};
use super::record_compositor::{ModeDescendantRecordDraft, source_for_spec};
use super::source::ModeDescendantSourceRecord;
use crate::decoded_record_write_plan::DecodedDataClaim;
use crate::font::{HorizontalTextAlignment, IndexedTextRasterizers};
use crate::pipeline::sha256_bytes;
use crate::tim::{Cell, write_indexed_cell_in_prefix_with_report};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub(super) const SPEC: ModeDescendantRecordSpec = ModeDescendantRecordSpec {
    record: ModeDescendantRecord::ContinueSchoolTexture,
    surface: ModeDescendantSurface::ContinueSlots,
    source_path: "DAT2/MA_TIT.BIZ",
    storage_kind: ModeDescendantStorageKind::PagedCompressed,
    output_file: "ma-tit-continue-schools.biz",
    texture_outputs: &[ModeDescendantTextureOutputSpec {
        tim_offset: 0,
        preview_file: "continue-schools.png",
    }],
};

const LABELS: [(&str, &str, [u16; 7]); 5] = [
    ("taiyo", "太陽学園", [0, 832, 0, 64, 200, 72, 20]),
    ("gorin", "五輪高校", [0, 832, 0, 0, 220, 72, 20]),
    ("pacific", "パシフィックHS", [0, 832, 0, 136, 200, 80, 20]),
    ("gedo", "外道高校", [0, 832, 0, 72, 220, 72, 20]),
    ("justice", "ジャスティス学園", [0, 832, 0, 144, 220, 80, 20]),
];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    kind: String,
    entries: Vec<Entry>,
    date_labels: PathBuf,
    title_graphics: Option<PathBuf>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DateLabels {
    kind: String,
    entries: Vec<DateEntry>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DateEntry {
    id: String,
    source_text: String,
    korean_text: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    id: String,
    source_text: String,
    korean_text: String,
    font_px: f32,
}
#[derive(Debug, Serialize)]
pub struct ContinueSchoolsBuildReport {
    pub manifest_sha256: String,
    pub consumer_sha256: String,
    pub font_sha256: String,
    pub school_index_and_slot_data_preserved: bool,
    pub labels: Vec<SchoolLabelBuild>,
    pub date_manifest_sha256: String,
    pub date_labels: Vec<ContinueDateLabelBuild>,
    pub title_graphics: Vec<crate::title_graphics::TitleArtworkBuild>,
}
#[derive(Debug, Serialize)]
pub struct ContinueDateLabelBuild {
    pub source_text: String,
    pub korean_text: String,
    pub cell: Cell,
    pub font_sha256: String,
    pub font_px: f32,
    pub vertical_shift_px: i32,
    pub ink_bounds: [usize; 4],
}
#[derive(Debug, Serialize)]
pub struct SchoolLabelBuild {
    pub school_index: usize,
    pub source_text: String,
    pub korean_text: String,
    pub cell: Cell,
    pub source_pixel_sha256: String,
    pub fill_gradient: SchoolGradient,
}

#[derive(Debug, Serialize)]
pub struct SchoolGradient {
    pub ink_span: [usize; 2],
    pub indices: Vec<u8>,
}

// Source CLUT 7: red through orange to gold. Explicit ink roles, not histogram
// inference. Normalize against target fill ink, never the source cell width.
pub(super) fn apply_school_gradient(pixels: &mut [u8], width: usize) -> Result<SchoolGradient> {
    const RAMP: [u8; 7] = [11, 9, 10, 12, 13, 14, 15];
    ensure!(
        width > 0 && pixels.len().is_multiple_of(width),
        "invalid school raster"
    );
    let positions = pixels
        .iter()
        .enumerate()
        .filter(|(_, p)| **p == 15)
        .map(|(i, _)| i % width)
        .collect::<Vec<_>>();
    let left = *positions.iter().min().context("school fill is empty")?;
    let right = *positions.iter().max().context("school fill is empty")?;
    for (i, pixel) in pixels.iter_mut().enumerate() {
        if *pixel == 15 {
            let step = if right == left {
                0
            } else {
                ((i % width - left) * (RAMP.len() - 1) + (right - left) / 2) / (right - left)
            };
            *pixel = RAMP[step];
        }
    }
    Ok(SchoolGradient {
        ink_span: [left, right + 1],
        indices: RAMP.to_vec(),
    })
}

#[cfg(test)]
mod gradient_tests {
    use super::*;
    #[test]
    fn gradient_tracks_target_ink_not_padding_or_alignment() {
        let mut short = vec![1, 15, 15, 15, 15, 15, 15, 15, 1];
        let mut shifted = vec![0, 0, 1, 15, 15, 15, 15, 15, 15, 15, 1, 0];
        apply_school_gradient(&mut short, 9).unwrap();
        apply_school_gradient(&mut shifted, 12).unwrap();
        assert_eq!(&short[1..8], &shifted[3..10]);
        assert_eq!((short[1], short[7]), (11, 15));
        assert_eq!((shifted[0], shifted[2], shifted[10]), (0, 1, 1));
        let mut long = vec![15; 40];
        apply_school_gradient(&mut long, 40).unwrap();
        assert_eq!((long[0], long[39]), (11, 15));
    }
}

fn native_cells(overlay: &[u8]) -> Result<Vec<Cell>> {
    let mut cells = Vec::new();
    for (index, (_, _, expected)) in LABELS.iter().enumerate() {
        let pointer = overlay
            .get(0x6b0 + index * 4..0x6b4 + index * 4)
            .context("missing school pointer")?;
        let address = u32::from_le_bytes(pointer.try_into()?);
        ensure!(
            address == 0x800a2660 + index as u32 * 16,
            "Continue school pointer order changed"
        );
        let offset = usize::try_from(address - 0x800a2000)?;
        let bytes = overlay
            .get(offset..offset + 14)
            .context("missing school descriptor")?;
        let actual = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
            .collect::<Vec<_>>();
        ensure!(
            actual == expected.as_slice(),
            "Continue school descriptor changed"
        );
        cells.push(Cell {
            x: usize::from((actual[1] - 768) * 4 + actual[3]),
            y: usize::from(actual[4]),
            width: usize::from(actual[5]),
            height: usize::from(actual[6]),
        });
    }
    Ok(cells)
}

pub(super) fn build(
    config: &ModeDescendantGraphicsBuildConfig,
    rasterizers: &mut IndexedTextRasterizers,
    sources: &[ModeDescendantSourceRecord],
    path: &Path,
) -> Result<(ModeDescendantRecordDraft, ContinueSchoolsBuildReport)> {
    let bytes = std::fs::read(path)?;
    let manifest: Manifest = serde_json::from_slice(&bytes)?;
    ensure!(
        manifest.kind == "justice_gakuen2_continue_school_labels"
            && manifest.entries.len() == LABELS.len(),
        "Continue labels must cover all five native schools"
    );
    let source = source_for_spec(sources, &SPEC)?;
    let consumer = sources
        .iter()
        .find(|s| s.path == "DAT1/MGTIT.BIZ")
        .context("missing Continue consumer")?;
    let cells = native_cells(&consumer.decoded)?;
    let mut decoded = source.decoded.clone();
    let mut claims = Vec::new();
    let mut labels = Vec::new();
    let mut font_sha256 = String::new();
    let rasterizer = rasterizers.for_font(&config.fonts.edit_school_label.path)?;
    for (index, (entry, cell)) in manifest.entries.iter().zip(cells).enumerate() {
        ensure!(
            entry.id == LABELS[index].0 && entry.source_text == LABELS[index].1,
            "Continue school identity changed"
        );
        ensure!(
            !entry.korean_text.trim().is_empty() && !entry.korean_text.contains(['\n', '\r']),
            "invalid Continue school text"
        );
        let mut raster = rasterizer.rasterize_shifted(
            &entry.korean_text,
            cell.width,
            cell.height,
            entry.font_px,
            0.0,
            -1,
            0,
            Some(1),
            15,
            HorizontalTextAlignment::Left,
        )?;
        let fill_gradient = apply_school_gradient(&mut raster.pixels, cell.width)?;
        font_sha256 = raster.font_sha256;
        let write =
            write_indexed_cell_in_prefix_with_report(&mut decoded, 0, cell, &raster.pixels)?;
        claims.extend(DecodedDataClaim::from_effective_ranges(
            &format!("continue-school:{}", entry.id),
            "translate the school sprite selected by the native saved school index",
            &source.decoded,
            &decoded,
            write.allowed_ranges,
        )?);
        labels.push(SchoolLabelBuild {
            school_index: index,
            fill_gradient,
            source_text: entry.source_text.clone(),
            korean_text: entry.korean_text.clone(),
            cell,
            source_pixel_sha256: sha256_bytes(&crate::tim::read_indexed_cell_in_prefix(
                &source.decoded,
                0,
                cell,
            )?),
        });
    }
    super::assets::validate_relative_path(&manifest.date_labels)?;
    let date_bytes = std::fs::read(
        path.parent()
            .context("Continue labels have no parent")?
            .join(&manifest.date_labels),
    )?;
    let dates: DateLabels = serde_json::from_slice(&date_bytes)?;
    ensure!(
        dates.kind == "justice_gakuen2_continue_date_labels" && dates.entries.len() == 2,
        "Continue date unit must cover month and day"
    );
    let mut date_labels = Vec::new();
    let style = &config.fonts.calendar_text;
    let rasterizer = rasterizers.for_font(&style.path)?;
    // MA_TIT reloads the same VRAM page as MENU. Editing MENU/MOJI2 alone
    // cannot replace these digits. MGTIT's decimal reader uses this UV table
    // for dates and slot ordinals; retain its values, geometry and palette.
    let digit_cells = native_digit_cells(&consumer.decoded)?;
    let mut calendar_cells = Vec::new();
    for (digit, cell) in digit_cells.into_iter().enumerate() {
        let text = digit.to_string();
        calendar_cells.push((format!("digit-{digit}"), text.clone(), text, cell));
    }
    for (entry, (id, source_text, x)) in dates
        .entries
        .iter()
        .zip([("month", "月", 296), ("day", "日", 316)])
    {
        ensure!(
            entry.id == id
                && entry.source_text == source_text
                && entry.korean_text.chars().count() == 1,
            "Continue date label identity changed"
        );
        // MGTIT +0x44b4/+0x44bc and +0x460c/+0x4614 select these
        // 20x20 cells with CLUT (0,481). The numeric date readers stay native.
        let cell = Cell {
            x,
            y: 180,
            width: 20,
            height: 20,
        };
        calendar_cells.push((
            id.to_owned(),
            entry.source_text.clone(),
            entry.korean_text.clone(),
            cell,
        ));
    }
    for (id, source_text, text, cell) in calendar_cells {
        let raster = rasterizer.rasterize_shifted(
            &text,
            20,
            20,
            style.font_px,
            0.0,
            style.vertical_shift_px,
            0,
            Some(2),
            14,
            HorizontalTextAlignment::Center,
        )?;
        let [left, top, right, bottom] = raster.ink_bounds;
        ensure!(
            left > 0 && top > 0 && right < 20 && bottom < 20,
            "Continue calendar {id} clips its outline"
        );
        let write =
            write_indexed_cell_in_prefix_with_report(&mut decoded, 0, cell, &raster.pixels)?;
        claims.extend(DecodedDataClaim::from_effective_ranges(
            &format!("continue-date:{id}"),
            "redraw Continue calendar digits and suffixes with one calendar style",
            &source.decoded,
            &decoded,
            write.allowed_ranges,
        )?);
        date_labels.push(ContinueDateLabelBuild {
            source_text,
            korean_text: text,
            cell,
            font_sha256: raster.font_sha256,
            font_px: style.font_px,
            vertical_shift_px: style.vertical_shift_px,
            ink_bounds: raster.ink_bounds,
        });
    }
    let title_graphics = if let Some(root) = &manifest.title_graphics {
        let root = path
            .parent()
            .context("Continue manifest parent missing")?
            .join(root);
        let (logo_claims, reports) =
            crate::title_graphics::apply_title_graphics(&config.cue, &root, &mut decoded)?;
        claims.extend(logo_claims);
        reports
    } else {
        Vec::new()
    };
    Ok((
        ModeDescendantRecordDraft {
            spec: &SPEC,
            decoded,
            decoded_write_claims: claims,
        },
        ContinueSchoolsBuildReport {
            manifest_sha256: sha256_bytes(&bytes),
            consumer_sha256: sha256_bytes(&consumer.decoded),
            font_sha256,
            school_index_and_slot_data_preserved: true,
            labels,
            date_manifest_sha256: sha256_bytes(&date_bytes),
            date_labels,
            title_graphics,
        },
    ))
}

fn native_digit_cells(overlay: &[u8]) -> Result<Vec<Cell>> {
    (0..10)
        .map(|digit| {
            let x = if digit == 0 { 180 } else { (digit - 1) * 20 };
            let offset = 0x638 + digit * 4;
            ensure!(
                overlay.get(offset..offset + 4) == Some([x as u8, 0, 20, 20].as_slice()),
                "Continue decimal UV table changed for {digit}"
            );
            Ok(Cell {
                x,
                y: 0,
                width: 20,
                height: 20,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn calendar_digits_follow_decimal_values_and_reject_retargeted_cells() {
        let mut overlay = vec![0; 0x660];
        for digit in 0..10 {
            let x = if digit == 0 { 180 } else { (digit - 1) * 20 };
            overlay[0x638 + digit * 4..0x63c + digit * 4].copy_from_slice(&[x as u8, 0, 20, 20]);
        }
        let cells = native_digit_cells(&overlay).unwrap();
        assert_eq!(cells[0].x, 180);
        assert_eq!(cells[1].x, 0);
        overlay[0x638] = 0;
        assert!(native_digit_cells(&overlay).is_err());
        assert!(native_digit_cells(&[]).is_err());
    }

    #[test]
    fn date_suffixes_cannot_override_the_shared_calendar_size() {
        assert!(
            serde_json::from_str::<DateLabels>(
                r#"{
            "kind":"justice_gakuen2_continue_date_labels",
            "entries":[{"id":"month","source_text":"月","korean_text":"월","font_px":16}]
        }"#
            )
            .is_err()
        );
    }

    #[test]
    fn school_sprites_cannot_be_reordered_or_retargeted() {
        let mut source = vec![0u8; 0x6c4];
        for (i, (_, _, descriptor)) in LABELS.iter().enumerate() {
            source[0x6b0 + i * 4..0x6b4 + i * 4]
                .copy_from_slice(&(0x800a2660u32 + i as u32 * 16).to_le_bytes());
            for (n, v) in descriptor.iter().enumerate() {
                source[0x660 + i * 16 + n * 2..0x662 + i * 16 + n * 2]
                    .copy_from_slice(&v.to_le_bytes());
            }
        }
        assert_eq!(native_cells(&source).unwrap()[3].x, 328);
        let mut swapped = source.clone();
        swapped[0x6b0..0x6b4].copy_from_slice(&source[0x6b4..0x6b8]);
        assert!(native_cells(&swapped).is_err());
        source[0x660 + 6] ^= 8;
        assert!(native_cells(&source).is_err());
    }
}
