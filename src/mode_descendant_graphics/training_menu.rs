//! TRAIN's pause/settings strings use CEFT3's 24-by-12 descriptor alphabet.
//! Replace complete strings and their data descriptors; retain native code,
//! pointers, palettes, damage counters, and every pixel outside the text cells.
//! Idle text modulation is separately verified through typed instruction writes.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[path = "training_menu_contrast.rs"]
mod contrast;
#[path = "training_damage_hud.rs"]
mod damage_hud;

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

use crate::decoded_record_write_plan::DecodedDataClaim;
use crate::font::{HorizontalTextAlignment, IndexedTextRasterizers};
use crate::pipeline::sha256_bytes;
use crate::tim::{Cell, write_indexed_cell_in_prefix_with_report};

use super::catalog::{ModeDescendantRecordSpec, ModeDescendantTextureOutputSpec};
use super::model::{
    ModeDescendantGraphicsBuildConfig, ModeDescendantRecord, ModeDescendantStorageKind,
    ModeDescendantSurface,
};
use super::record_compositor::{ModeDescendantRecordDraft, source_for_spec};
use super::source::ModeDescendantSourceRecord;

const TIM_OFFSET: usize = 0x10800;
const OFFSETS: [usize; 15] = [
    0x84, 0x88, 0xa8, 0xc4, 0xd8, 0xd8, 0xec, 0xf0, 0xfc, 0x100, 0x104, 0x108, 0x10c, 0x11c, 0x124,
];
const STRINGS: [(&str, &str); 14] = [
    ("heading", "メニュー"),
    ("continue", "トレーニング続行"),
    ("settings", "シチュエーション設定"),
    ("character-change", "キャラクター変更"),
    ("mode-menu", "モードメニューに戻る"),
    ("enemy-state", "敵の状態"),
    ("guard", "ガード"),
    ("recovery", "受け身"),
    ("back", "戻る"),
    ("standing", "立ち"),
    ("crouching", "しゃがみ"),
    ("jumping", "ジャンプ"),
    ("enabled", "する"),
    ("disabled", "しない"),
];
const SPECS: [ModeDescendantRecordSpec; 2] = [
    ModeDescendantRecordSpec {
        record: ModeDescendantRecord::TrainingMenuTexture,
        surface: ModeDescendantSurface::TrainingMenu,
        source_path: "DAT2/CEFT3.BIZ",
        storage_kind: ModeDescendantStorageKind::PagedCompressed,
        output_file: "ceft3-training-menu.biz",
        texture_outputs: &[ModeDescendantTextureOutputSpec {
            tim_offset: TIM_OFFSET,
            preview_file: "training-menu.png",
        }],
    },
    ModeDescendantRecordSpec {
        record: ModeDescendantRecord::TrainingMenuOverlay,
        surface: ModeDescendantSurface::TrainingMenu,
        source_path: "DAT1/TRAIN.BIN",
        storage_kind: ModeDescendantStorageKind::Raw,
        output_file: "train-menu.bin",
        texture_outputs: &[],
    },
];

pub(super) fn record_specs() -> impl Iterator<Item = &'static ModeDescendantRecordSpec> {
    SPECS.iter()
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Menu {
    kind: String,
    entries: Vec<Entry>,
    #[serde(default)]
    damage_hud: Option<PathBuf>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    id: String,
    source_text: String,
    korean_text: String,
}

#[derive(Debug, Serialize)]
pub struct TrainingMenuBuildReport {
    pub manifest_sha256: String,
    pub font_sha256: String,
    pub font_px: f32,
    pub vertical_shift_px: i32,
    pub native_pointer_count: usize,
    pub aliased_pointers_preserved: bool,
    pub non_text_pixels_and_menu_behavior_preserved: bool,
    pub idle_text_brightness: i16,
    pub texts: Vec<TrainingMenuTextBuild>,
    pub damage_hud: Option<damage_hud::TrainingDamageHudBuildReport>,
}

#[derive(Debug, Serialize)]
pub struct TrainingMenuTextBuild {
    pub id: String,
    pub source_text: String,
    pub korean_text: String,
    pub descriptor_offset: usize,
    pub cell: Cell,
}

// Each span is covered by the native descriptor family, including its sparse rows.
fn allocate(widths: &[usize]) -> Result<Vec<Cell>> {
    let mut spans = [
        (0, 0, 240),
        (0, 12, 24),
        (72, 12, 24),
        (168, 12, 72),
        (0, 24, 240),
        (0, 36, 240),
        (0, 48, 240),
        (0, 60, 48),
    ];
    let mut cells = vec![
        Cell {
            x: 0,
            y: 0,
            width: 0,
            height: 12
        };
        widths.len()
    ];
    let mut order = (0..widths.len()).collect::<Vec<_>>();
    order.sort_by_key(|&index| (std::cmp::Reverse(widths[index]), index));
    for index in order {
        let width = widths[index];
        ensure!(
            width > 0 && width <= 240 && width.is_multiple_of(24),
            "training string width is outside native descriptor units"
        );
        let span = spans
            .iter_mut()
            .find(|span| span.2 >= width)
            .context("training text exceeds its owned atlas rows")?;
        cells[index] = Cell {
            x: span.0,
            y: span.1,
            width,
            height: 12,
        };
        span.0 += width;
        span.2 -= width;
    }
    Ok(cells)
}

fn descriptor_ranges(overlay: &[u8]) -> Result<Vec<std::ops::Range<usize>>> {
    let mut ranges = Vec::new();
    let mut seen = BTreeSet::new();
    for (index, &offset) in OFFSETS.iter().enumerate() {
        let pointer = overlay
            .get(0x130 + index * 4..0x134 + index * 4)
            .context("training pointer table is truncated")?;
        ensure!(
            u32::from_le_bytes(pointer.try_into()?) == 0x800a2000 + offset as u32,
            "training string pointer changed"
        );
        if !seen.insert(offset) {
            continue;
        }
        let count = usize::from(
            *overlay
                .get(offset)
                .context("training descriptor is truncated")?,
        );
        ensure!(count > 0, "training descriptor has no chunks");
        let end = offset + 1 + count * 3;
        let bytes = overlay
            .get(offset + 1..end)
            .context("training chunks are truncated")?;
        for chunk in bytes.as_chunks::<3>().0 {
            ensure!(
                chunk[2] > 0 && usize::from(chunk[0]) + usize::from(chunk[2]) <= 10 && chunk[1] < 6,
                "training source descriptor leaves its text atlas"
            );
        }
        ranges.push(offset..end);
    }
    ensure!(
        ranges.windows(2).all(|pair| pair[0].end <= pair[1].start)
            && ranges.last().is_some_and(|r| r.end <= 0x130),
        "training descriptor ranges overlap code or pointers"
    );
    Ok(ranges)
}

pub(super) fn build(
    config: &ModeDescendantGraphicsBuildConfig,
    rasterizers: &mut IndexedTextRasterizers,
    sources: &[ModeDescendantSourceRecord],
    path: &Path,
) -> Result<(Vec<ModeDescendantRecordDraft>, TrainingMenuBuildReport)> {
    let bytes = std::fs::read(path)?;
    let menu: Menu = serde_json::from_slice(&bytes)?;
    let font = &config.fonts.training_text;
    ensure!(
        menu.kind == "justice_gakuen2_training_menu"
            && font.font_px.is_finite()
            && font.font_px > 0.0
            && font.font_px <= 12.0,
        "invalid training menu manifest"
    );
    ensure!(
        menu.entries.len() == STRINGS.len(),
        "training menu must cover the complete native string family"
    );
    let texture = source_for_spec(sources, &SPECS[0])?;
    let overlay = source_for_spec(sources, &SPECS[1])?;
    let ranges = descriptor_ranges(&overlay.decoded)?;
    let rasterizer = rasterizers.for_font(&font.path)?;
    let mut widths = Vec::new();
    for ((id, source_text), entry) in STRINGS.iter().zip(&menu.entries) {
        ensure!(
            entry.id == *id
                && entry.source_text == *source_text
                && !entry.korean_text.trim().is_empty()
                && !entry.korean_text.contains(['\n', '\r']),
            "training semantic entry does not match its native consumer: {id}"
        );
        let measured = rasterizer.rasterize_shifted(
            &entry.korean_text,
            240,
            12,
            font.font_px,
            0.0,
            font.vertical_shift_px,
            0,
            Some(1),
            15,
            HorizontalTextAlignment::Center,
        )?;
        // Include the outline's right-hand pixel when rounding native units.
        let width = ((measured.measured_advance_px.ceil() as usize * 2 + 4).div_ceil(24)) * 24;
        widths.push(width);
    }
    let cells = allocate(&widths)?;
    let mut owned_pixels = vec![false; 240 * 72];
    for range in &ranges {
        for &[u, v, width] in overlay.decoded[range.start + 1..range.end]
            .as_chunks::<3>()
            .0
        {
            for y in usize::from(v) * 12..usize::from(v + 1) * 12 {
                for x in usize::from(u) * 24..usize::from(u + width) * 24 {
                    owned_pixels[y * 240 + x] = true;
                }
            }
        }
    }
    for cell in &cells {
        ensure!(
            (cell.y..cell.y + cell.height)
                .all(|y| (cell.x..cell.x + cell.width).all(|x| owned_pixels[y * 240 + x])),
            "training allocation uses pixels outside the native string family"
        );
    }
    let mut patched_texture = texture.decoded.clone();
    let mut patched_overlay = contrast::apply(&overlay.decoded)?;
    let mut texture_claims = Vec::new();
    let mut overlay_claims = DecodedDataClaim::from_effective_ranges(
        "training-idle-contrast",
        "compose independently verified typed brightness writes",
        &overlay.decoded,
        &patched_overlay,
        contrast::IDLE_TEXT_SITES.map(|offset| [offset, offset + 4]),
    )?;
    let mut texts = Vec::new();
    let mut font_sha256 = String::new();
    // +0x1410 and +0x1514 consume these native XY tables. Keep all Y values,
    // center menu/header/back rows, and retain the settings values' right edge.
    for (xy_offset, text_index, right_edge) in [
        (0x40, 0, None),
        (0x44, 1, None),
        (0x48, 2, None),
        (0x4c, 3, None),
        (0x50, 4, None),
        (0x54, 2, None),
        (0x64, 8, None),
        (0x68, 9, Some(376)),
        (0x6c, 10, Some(376)),
        (0x70, 11, Some(376)),
        (0x74, 12, Some(376)),
        (0x78, 13, Some(376)),
        (0x7c, 12, Some(376)),
        (0x80, 13, Some(376)),
    ] {
        let width = cells[text_index].width;
        let x = right_edge.map_or((512 - width) / 2, |edge| edge - width) as u16;
        patched_overlay[xy_offset..xy_offset + 2].copy_from_slice(&x.to_le_bytes());
        overlay_claims.extend(DecodedDataClaim::from_effective_ranges(
            &format!("training-menu-layout:{xy_offset:x}"),
            "retain native centering and settings right alignment",
            &overlay.decoded,
            &patched_overlay,
            vec![[xy_offset, xy_offset + 2]],
        )?);
    }
    for ((entry, cell), range) in menu.entries.iter().zip(cells).zip(ranges) {
        let raster = rasterizer.rasterize_shifted(
            &entry.korean_text,
            cell.width / 2,
            cell.height,
            font.font_px,
            0.0,
            font.vertical_shift_px,
            0,
            Some(1),
            15,
            HorizontalTextAlignment::Center,
        )?;
        font_sha256 = raster.font_sha256;
        let pixels = raster
            .pixels
            .iter()
            .flat_map(|pixel| std::iter::repeat_n(*pixel, 2))
            .collect::<Vec<_>>();
        let write = write_indexed_cell_in_prefix_with_report(
            &mut patched_texture,
            TIM_OFFSET,
            cell,
            &pixels,
        )?;
        texture_claims.extend(DecodedDataClaim::from_effective_ranges(
            &format!("training-menu:{}", entry.id),
            "render a complete training menu string",
            &texture.decoded,
            &patched_texture,
            write.allowed_ranges,
        )?);
        patched_overlay[range.clone()].fill(0);
        patched_overlay[range.start..range.start + 4].copy_from_slice(&[
            1,
            (cell.x / 24) as u8,
            (cell.y / 12) as u8,
            (cell.width / 24) as u8,
        ]);
        overlay_claims.extend(DecodedDataClaim::from_effective_ranges(
            &format!("training-menu:{}", entry.id),
            "redirect the native string descriptor to a complete Korean strip",
            &overlay.decoded,
            &patched_overlay,
            vec![[range.start, range.end]],
        )?);
        texts.push(TrainingMenuTextBuild {
            id: entry.id.clone(),
            source_text: entry.source_text.clone(),
            korean_text: entry.korean_text.clone(),
            descriptor_offset: range.start,
            cell,
        });
    }
    let damage_hud = if let Some(relative) = &menu.damage_hud {
        super::assets::validate_relative_path(relative)?;
        let hud = damage_hud::apply(
            config,
            rasterizers,
            sources,
            &mut patched_texture,
            &mut patched_overlay,
            &path
                .parent()
                .context("training menu has no parent directory")?
                .join(relative),
        )?;
        texture_claims.extend(hud.texture_claims);
        overlay_claims.extend(hud.overlay_claims);
        Some(hud.report)
    } else {
        None
    };
    Ok((
        vec![
            ModeDescendantRecordDraft {
                spec: &SPECS[0],
                decoded: patched_texture,
                decoded_write_claims: texture_claims,
            },
            ModeDescendantRecordDraft {
                spec: &SPECS[1],
                decoded: patched_overlay,
                decoded_write_claims: overlay_claims,
            },
        ],
        TrainingMenuBuildReport {
            manifest_sha256: sha256_bytes(&bytes),
            font_sha256,
            font_px: font.font_px,
            vertical_shift_px: font.vertical_shift_px,
            native_pointer_count: OFFSETS.len(),
            aliased_pointers_preserved: true,
            non_text_pixels_and_menu_behavior_preserved: true,
            idle_text_brightness: contrast::IDLE_BRIGHTNESS,
            texts,
            damage_hud,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn atlas_allocation_uses_owned_spans_and_rejects_overflow() {
        let cells = allocate(&[240, 240, 240, 240, 72, 48, 24, 24]).unwrap();
        assert_eq!(
            cells[4],
            Cell {
                x: 168,
                y: 12,
                width: 72,
                height: 12
            }
        );
        assert_eq!(
            cells[5],
            Cell {
                x: 0,
                y: 60,
                width: 48,
                height: 12
            }
        );
        assert!(allocate(&[240, 240, 240, 240, 72, 48, 24, 24, 24]).is_err());
        assert!(allocate(&[241]).is_err());
        assert!(allocate(&[0]).is_err());
    }
    #[test]
    fn descriptor_aliases_share_one_write_and_invalid_source_is_rejected() {
        let mut overlay = vec![0; 0x16c];
        for (index, &offset) in OFFSETS.iter().enumerate() {
            overlay[0x130 + index * 4..0x134 + index * 4]
                .copy_from_slice(&(0x800a2000 + offset as u32).to_le_bytes());
            overlay[offset..offset + 4].copy_from_slice(&[1, 0, 0, 1]);
        }
        let ranges = descriptor_ranges(&overlay).unwrap();
        assert_eq!(ranges.iter().filter(|r| r.start == 0xd8).count(), 1);
        overlay[0x87] = 11;
        assert!(descriptor_ranges(&overlay).is_err());
        overlay[0x87] = 1;
        overlay[0x130] ^= 1;
        assert!(descriptor_ranges(&overlay).is_err());
    }
}
