//! Masked calendar artwork in the source's two horizontally joined TIM halves.
use super::model::DiarySceneBuildConfig;
use crate::font::{HorizontalTextAlignment, IndexedTextRasterizers};
use crate::pipeline::sha256_bytes;
use crate::tim::{Cell, parse_8bpp};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::{Component, Path};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Backgrounds {
    kind: String,
    entries: Vec<Background>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Background {
    id: String,
    member_index: usize,
    source_tim_sha256: String,
    unwrapped_size: [usize; 2],
    background_indices: String,
    background_sha256: String,
    text_mask: String,
    text_mask_sha256: String,
    regions: Vec<Cell>,
    labels: Vec<Label>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Label {
    text: String,
    cell: Cell,
    font_scale: f32,
    horizontal_scale: f32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Wording {
    kind: String,
    entries: Vec<Words>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Words {
    id: String,
    member_index: usize,
    source_title: String,
    korean_title: String,
    source_date: Option<String>,
    korean_date: Option<String>,
}
#[derive(Debug, Serialize)]
pub struct CalendarPanelReport {
    pub id: String,
    pub member_index: usize,
    pub source_tim_sha256: String,
    pub background_sha256: String,
    pub mask_sha256: String,
    pub patched_tim_sha256: String,
    pub korean_title: String,
    pub korean_date: Option<String>,
    pub font_sha256: String,
    pub font_px: f32,
    pub changed_pixels: usize,
}
pub(super) struct Candidate {
    pub index: usize,
    pub bytes: Vec<u8>,
    pub report: CalendarPanelReport,
}

fn private_input(config: &DiarySceneBuildConfig, path: &str, expected: &str) -> Result<Vec<u8>> {
    let relative = Path::new(path)
        .strip_prefix("assets/private-artwork/calendar")
        .context("calendar input outside its private asset directory")?;
    ensure!(
        relative
            .components()
            .all(|c| matches!(c, Component::Normal(_))),
        "unsafe calendar input path"
    );
    let data = std::fs::read(
        config
            .assets
            .join("../../private-artwork/calendar")
            .join(relative),
    )?;
    ensure!(
        data.len() == 512 * 110 && sha256_bytes(&data) == expected,
        "calendar private input changed: {path}"
    );
    Ok(data)
}
fn native_offset(x: usize, y: usize) -> usize {
    (y + (x / 256) * 110) * 256 + x % 256
}
fn palette_index(source: &[u8], rgb: [i32; 3]) -> u8 {
    (0..256)
        .min_by_key(|&i| {
            let v = u16::from_le_bytes([source[20 + i * 2], source[21 + i * 2]]);
            [0, 5, 10]
                .iter()
                .enumerate()
                .map(|(c, s)| (i32::from((v >> s) & 31) * 255 / 31 - rgb[c]).pow(2))
                .sum::<i32>()
        })
        .unwrap() as u8
}

pub(super) fn render(
    config: &DiarySceneBuildConfig,
    path: &Path,
    sources: &[Vec<u8>],
) -> Result<Vec<Candidate>> {
    let backgrounds: Backgrounds = serde_json::from_slice(&std::fs::read(path)?)?;
    let words: Wording =
        serde_json::from_slice(&std::fs::read(path.with_file_name("wording.json"))?)?;
    ensure!(
        backgrounds.kind == "justice_gakuen2_calendar_backgrounds"
            && words.kind == "justice_gakuen2_calendar_wording",
        "unsupported calendar assets"
    );
    let ids = words
        .entries
        .iter()
        .map(|w| w.member_index)
        .collect::<BTreeSet<_>>();
    ensure!(
        words.entries.len() == 19 && ids == (93..112).collect(),
        "calendar wording coverage changed"
    );
    let style = config
        .fonts
        .calendar_title
        .as_ref()
        .context("calendar font role is missing")?;
    let mut rasterizers = IndexedTextRasterizers::default();
    let rasterizer = rasterizers.for_font(&style.path)?;
    let mut used = BTreeSet::new();
    let mut result = Vec::new();
    for entry in backgrounds.entries {
        ensure!(
            used.insert(entry.member_index) && (93..112).contains(&entry.member_index),
            "duplicate or foreign calendar background"
        );
        let word = words
            .entries
            .iter()
            .find(|w| w.member_index == entry.member_index && w.id == entry.id)
            .context("calendar wording identity mismatch")?;
        ensure!(
            !word.source_title.is_empty()
                && !word.korean_title.is_empty()
                && word.source_date.is_some() == word.korean_date.is_some(),
            "calendar wording incomplete"
        );
        let source = sources
            .get(entry.member_index)
            .context("calendar source missing")?;
        ensure!(
            sha256_bytes(source) == entry.source_tim_sha256,
            "calendar source TIM changed"
        );
        let tim = parse_8bpp(source)?;
        ensure!(
            tim.pixel_width() == 256
                && tim.image_height == 220
                && tim.clut_width * tim.clut_height == 256
                && entry.unwrapped_size == [512, 110],
            "calendar geometry changed"
        );
        let background =
            private_input(config, &entry.background_indices, &entry.background_sha256)?;
        let mask = private_input(config, &entry.text_mask, &entry.text_mask_sha256)?;
        let mut pixels = vec![0; 512 * 110];
        for y in 0..110 {
            for x in 0..512 {
                let i = y * 512 + x;
                let original = source[tim.pixel_offset + native_offset(x, y)];
                ensure!(mask[i] <= 1, "calendar mask is not binary");
                let owned = entry
                    .regions
                    .iter()
                    .any(|c| x >= c.x && x < c.x + c.width && y >= c.y && y < c.y + c.height);
                ensure!(
                    (mask[i] == 1) == owned,
                    "calendar mask and regions disagree"
                );
                ensure!(
                    mask[i] != 0 || background[i] == original,
                    "calendar background changes protected pixel"
                );
                pixels[i] = background[i];
            }
        }
        let white = palette_index(source, [255, 255, 255]);
        let rainbow = [
            [245, 20, 65],
            [230, 10, 235],
            [30, 70, 255],
            [0, 220, 255],
            [45, 240, 30],
            [255, 235, 0],
            [255, 120, 10],
        ]
        .map(|rgb| palette_index(source, rgb));
        let mut font_sha = String::new();
        let mut roles = BTreeSet::new();
        for label in &entry.labels {
            ensure!(roles.insert(label.text.as_str()), "duplicate calendar line");
            let text = match label.text.as_str() {
                "title" => &word.korean_title,
                "date" => word.korean_date.as_ref().context("unexpected date label")?,
                _ => anyhow::bail!("unknown calendar line"),
            };
            let c = label.cell;
            ensure!(
                c.width > 0 && c.height > 0 && c.x + c.width <= 512 && c.y + c.height <= 110,
                "calendar label outside image"
            );
            ensure!(
                label.font_scale.is_finite() && label.font_scale > 0.0 && label.font_scale <= 1.0,
                "invalid calendar font scale"
            );
            ensure!(
                label.horizontal_scale.is_finite() && (1.0..=3.0).contains(&label.horizontal_scale),
                "invalid calendar horizontal scale"
            );
            let raster_width = (c.width as f32 / label.horizontal_scale).floor() as usize;
            let raster = rasterizer.rasterize_shifted(
                text,
                raster_width,
                c.height,
                style.font_px * label.font_scale,
                0.0,
                -2,
                0,
                Some(1),
                2,
                HorizontalTextAlignment::Center,
            )?;
            font_sha = raster.font_sha256;
            let mut boundaries = Vec::new();
            let mut prefix = String::new();
            let mut color = 0;
            let origin = (raster_width as f32 - raster.measured_advance_px) / 2.0;
            for ch in text.chars() {
                prefix.push(ch);
                let measured = rasterizer.rasterize_shifted(
                    &prefix,
                    raster_width,
                    c.height,
                    style.font_px * label.font_scale,
                    0.0,
                    -2,
                    0,
                    Some(1),
                    2,
                    HorizontalTextAlignment::Center,
                )?;
                boundaries.push((
                    origin + measured.measured_advance_px,
                    rainbow[color % rainbow.len()],
                ));
                if !ch.is_whitespace() {
                    color += 1;
                }
            }
            for row in 0..c.height {
                for col in 0..c.width {
                    let source_x = col * raster_width / c.width;
                    let ink = raster.pixels[row * raster_width + source_x];
                    if ink == 0 {
                        continue;
                    }
                    let dest = (c.y + row) * 512 + c.x + col;
                    ensure!(mask[dest] == 1, "calendar text exceeds restoration mask");
                    pixels[dest] = if ink == 1 {
                        white
                    } else {
                        boundaries
                            .iter()
                            .find(|(end, _)| (source_x as f32) < *end)
                            .map(|(_, index)| *index)
                            .unwrap_or(rainbow[0])
                    };
                }
            }
        }
        ensure!(
            roles.contains("title") && roles.contains("date") == word.korean_date.is_some(),
            "calendar line coverage incomplete"
        );
        let mut candidate = source.clone();
        for y in 0..110 {
            for x in 0..512 {
                candidate[tim.pixel_offset + native_offset(x, y)] = pixels[y * 512 + x];
            }
        }
        let changed = source
            .iter()
            .zip(&candidate)
            .filter(|(a, b)| a != b)
            .count();
        ensure!(changed > 0, "calendar changed no pixels");
        result.push(Candidate {
            index: entry.member_index,
            report: CalendarPanelReport {
                id: entry.id,
                member_index: entry.member_index,
                source_tim_sha256: entry.source_tim_sha256,
                background_sha256: entry.background_sha256,
                mask_sha256: entry.text_mask_sha256,
                patched_tim_sha256: sha256_bytes(&candidate),
                korean_title: word.korean_title.clone(),
                korean_date: word.korean_date.clone(),
                font_sha256: font_sha,
                font_px: style.font_px,
                changed_pixels: changed,
            },
            bytes: candidate,
        });
    }
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn joined_halves_round_trip_without_crossing_rows() {
        let mut seen = vec![false; 256 * 220];
        for y in 0..110 {
            for x in 0..512 {
                let o = native_offset(x, y);
                assert!(!seen[o]);
                seen[o] = true;
                assert_eq!(o % 256, x % 256);
            }
        }
        assert!(seen.iter().all(|v| *v));
        assert_eq!(native_offset(256, 0), 256 * 110);
    }
}
