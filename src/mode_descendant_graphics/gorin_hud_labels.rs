//! Small fixed HUD labels use the existing ball-game, sprint and dance compositions.
use std::path::Path;

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

use super::model::{ModeDescendantGraphicsBuildConfig, ModeDescendantRecord};
use super::record_compositor::ModeDescendantRecordDraft;
use crate::decoded_record_write_plan::DecodedDataClaim;
use crate::font::{HorizontalTextAlignment, IndexedTextRasterizers};
use crate::pipeline::{difference_ranges, sha256_bytes};
use crate::tim::{Cell, read_indexed_cell_in_prefix, write_indexed_cell_in_prefix_with_report};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    labels: Vec<Label>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Label {
    id: String,
    source_text: String,
    korean_text: String,
    cell: Cell,
    source_indexed_sha256: String,
}
#[derive(Debug, Serialize)]
pub struct HudLabelReport {
    pub manifest_sha256: String,
    pub labels: Vec<LabelReport>,
    pub runtime_verified: bool,
}
#[derive(Debug, Serialize)]
pub struct LabelReport {
    pub id: String,
    pub source_text: String,
    pub korean_text: String,
    pub cell: Cell,
    pub font_name: String,
    pub font_sha256: String,
    pub font_px: f32,
    pub measured_advance_px: f32,
    pub ink_bounds: [usize; 4],
    pub indices_sha256: String,
    pub source_region_verified: bool,
    pub runtime_verified: bool,
}

pub(super) fn apply(
    path: &Path,
    config: &ModeDescendantGraphicsBuildConfig,
    drafts: &mut [ModeDescendantRecordDraft],
) -> Result<HudLabelReport> {
    let bytes = std::fs::read(path)?;
    let manifest: Manifest = serde_json::from_slice(&bytes)?;
    let style = config
        .fonts
        .gorin_small_label
        .as_ref()
        .context("small Gorin HUD labels require their font role")?;
    let mut rasterizers = IndexedTextRasterizers::default();
    let mut reports = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for label in manifest.labels {
        let is_push = label.id == "sprint-push";
        let font_px = if is_push { 9.0 } else { style.font_px };
        let is_dance = matches!(label.id.as_str(), "dance-you" | "dance-com");
        let is_sprint = matches!(
            label.id.as_str(),
            "sprint-speed" | "sprint-high-score" | "sprint-push"
        );
        let (record, tim_offset) = if is_dance {
            (ModeDescendantRecord::GorinDanceEffects, 0)
        } else if is_sprint {
            (ModeDescendantRecord::GorinSprintEffects, 0)
        } else {
            (ModeDescendantRecord::GorinBallGameEffects, 0x10800)
        };
        let draft = drafts
            .iter_mut()
            .find(|d| d.spec.record == record)
            .context("small Gorin label requires its texture composition")?;
        let height = if is_push {
            10
        } else if is_dance {
            18
        } else if label.id == "sprint-speed" {
            11
        } else {
            16
        };
        ensure!(seen.insert(label.id.clone()), "duplicate small Gorin label");
        let (source_text, korean_text, x, y, width) = match label.id.as_str() {
            "dance-you" => ("YOU", "나", 216, 32, 40),
            "dance-com" => ("COM", "컴퓨터", 216, 64, 40),
            "sprint-push" => ("PUSH!", "눌러!", 176, 80, 40),
            "sprint-speed" => ("SPEED", "속도", 256, 176, 80),
            "sprint-high-score" => ("HI SCORE", "최고 기록", 160, 32, 64),
            "current-record" => ("現在の記録", "현재 기록", 720, 128, 48),
            "point-unit" => ("点", "점", 240, 0, 16),
            "highest-record" => ("最高記録", "최고 기록", 200, 240, 56),
            "point" => ("POINT", "득점", 136, 56, 40),
            "score" => ("SCORE", "점수", 176, 56, 40),
            "artistic" => ("芸術点", "예술점", 216, 56, 40),
            "frame" => ("フレーム点", "프레임점", 136, 72, 40),
            _ => anyhow::bail!("unbound small Gorin label {}", label.id),
        };
        ensure!(
            label.source_text == source_text && label.korean_text == korean_text,
            "small Gorin label wording changed"
        );
        ensure!(
            label.cell
                == Cell {
                    x,
                    y,
                    width,
                    height
                },
            "small Gorin label leaves its reviewed source bounds"
        );
        let raster = rasterizers.for_font(&style.path)?.rasterize_shifted(
            &label.korean_text,
            width,
            if is_dance { 18 } else { 12 },
            font_px,
            0.0,
            if is_dance { 2 } else { -1 },
            0,
            Some(if is_push { 1 } else { 2 }),
            if is_push {
                15
            } else if is_dance {
                6
            } else {
                14
            },
            if is_dance || is_push {
                HorizontalTextAlignment::Center
            } else {
                HorizontalTextAlignment::Left
            },
        )?;
        if is_push {
            ensure!(
                raster.pixels[width * height..].iter().all(|p| *p == 0),
                "PUSH lettering leaves the owned text band"
            );
        } else if is_dance {
            ensure!(
                raster.pixels[..width].iter().all(|p| *p == 0)
                    && raster.pixels[width * (height - 1)..]
                        .iter()
                        .all(|p| *p == 0),
                "dance marker ink reaches vertical cell boundary"
            );
        } else {
            ensure!(
                !raster.pixels[width * 10..].contains(&14),
                "Gorin label ink leaves the original ten-row lettering band"
            );
        }
        let mut pixels = raster.pixels;
        // Remove the complete source shadow band, including its lower rows.
        pixels.resize(width * height, 0);
        let original = read_indexed_cell_in_prefix(&draft.decoded, tim_offset, label.cell)?;
        ensure!(
            sha256_bytes(&original) == label.source_indexed_sha256,
            "Gorin label source changed or is already owned"
        );
        if is_push {
            for (i, original_index) in original.iter().enumerate() {
                if (2..=7).contains(original_index) {
                    ensure!(
                        i / width == 9 && matches!(pixels[i], 0 | 1),
                        "PUSH lettering face overlaps protected arrow"
                    );
                    pixels[i] = *original_index;
                }
            }
        }
        let before = draft.decoded.clone();
        write_indexed_cell_in_prefix_with_report(
            &mut draft.decoded,
            tim_offset,
            label.cell,
            &pixels,
        )?;
        ensure!(
            read_indexed_cell_in_prefix(&draft.decoded, tim_offset, label.cell)? == pixels,
            "Gorin label readback differs"
        );
        draft
            .decoded_write_claims
            .extend(DecodedDataClaim::from_ranges(
                &format!("gorin-{}", label.id),
                "Small Korean HUD label with complete source-shadow clearing",
                difference_ranges(&before, &draft.decoded),
            ));
        reports.push(LabelReport {
            id: label.id,
            source_text: label.source_text,
            korean_text: label.korean_text,
            cell: label.cell,
            font_name: raster.font_name,
            font_sha256: raster.font_sha256,
            font_px,
            measured_advance_px: raster.measured_advance_px,
            ink_bounds: raster.ink_bounds,
            indices_sha256: sha256_bytes(&pixels),
            source_region_verified: true,
            runtime_verified: false,
        });
    }
    ensure!(seen.len() == 12, "small Gorin label coverage is incomplete");
    Ok(HudLabelReport {
        manifest_sha256: sha256_bytes(&bytes),
        labels: reports,
        runtime_verified: false,
    })
}
