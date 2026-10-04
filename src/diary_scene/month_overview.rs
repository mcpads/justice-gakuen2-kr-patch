//! Monthly overview plates, independent of the event-banner catalogue.
use super::{
    artwork::{SceneArtworkArchiveBuild, SceneArtworkArchiveReport, SceneArtworkMemberReport},
    model::DiarySceneBuildConfig,
    runtime_bundle_build::encode_runtime_member,
};
use crate::{
    compression::decompress,
    decoded_record_write_plan::{DecodedDataClaim, DecodedRecordWritePlan},
    font::{HorizontalTextAlignment, IndexedTextRasterizer},
    pipeline::sha256_bytes,
    source_disc::SupportedSourceDisc,
    tim::{Cell, parse_8bpp},
    tzz::parse_tzz,
};
use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use std::{
    collections::BTreeSet,
    path::{Component, Path, PathBuf},
};
const RECORD: &str = "DAT2/MGBETA.TZZ";
const SOURCE: &str = "df06f1c8d1e410516c78a2ad56e966d4e0657282547468775412a51b8e018189";
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Asset {
    source_record: String,
    source_sha256: String,
    font_role: String,
    release_status: String,
    entries: Vec<Entry>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    member: usize,
    source_tim_sha256: String,
    background: PathBuf,
    background_sha256: String,
    mask: PathBuf,
    mask_sha256: String,
    regions: Vec<Cell>,
    labels: Vec<Label>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Label {
    source_text: String,
    text: String,
    cell: Cell,
    font_scale: f32,
    fill_rgb: [u8; 3],
}
fn input(config: &DiarySceneBuildConfig, path: &Path, expected: &str) -> Result<Vec<u8>> {
    ensure!(
        path.components().all(|c| matches!(c, Component::Normal(_))),
        "unsafe month artwork input"
    );
    let bytes = std::fs::read(
        config
            .assets
            .join("../../private-artwork/calendar/months")
            .join(path),
    )?;
    ensure!(
        bytes.len() == 512 * 480 && sha256_bytes(&bytes) == expected,
        "month artwork input changed: {}",
        path.display()
    );
    Ok(bytes)
}
fn palette(source: &[u8], rgb: [u8; 3]) -> u8 {
    (0..256)
        .min_by_key(|&i| {
            let v = u16::from_le_bytes([source[20 + i * 2], source[21 + i * 2]]);
            [0, 5, 10]
                .iter()
                .enumerate()
                .map(|(c, s)| (i32::from((v >> s) & 31) * 255 / 31 - i32::from(rgb[c])).pow(2))
                .sum::<i32>()
        })
        .unwrap() as u8
}
fn owned(regions: &[Cell], x: usize, y: usize) -> bool {
    regions
        .iter()
        .any(|c| x >= c.x && x < c.x + c.width && y >= c.y && y < c.y + c.height)
}
fn apply_background(
    source: &[u8],
    background: &[u8],
    mask: &[u8],
    regions: &[Cell],
) -> Result<Vec<u8>> {
    let tim = parse_8bpp(source)?;
    ensure!(
        tim.pixel_width() == 512 && tim.image_height == 480 && tim.total_size == source.len(),
        "month TIM extent changed"
    );
    ensure!(
        background.len() == 512 * 480 && mask.len() == background.len(),
        "month background extent changed"
    );
    ensure!(
        !regions.is_empty()
            && regions.iter().all(|c| c.width > 0
                && c.height > 0
                && c.x.checked_add(c.width).is_some_and(|end| end <= 512)
                && c.y.checked_add(c.height).is_some_and(|end| end <= 480)),
        "invalid month text regions"
    );
    for i in 0..mask.len() {
        ensure!(
            mask[i] <= 1 && (mask[i] == 1) == owned(regions, i % 512, i / 512),
            "month mask ownership mismatch"
        );
        ensure!(
            mask[i] == 1 || source[tim.pixel_offset + i] == background[i],
            "month background changed protected pixels"
        );
    }
    let mut result = source.to_vec();
    result[tim.pixel_offset..].copy_from_slice(background);
    Ok(result)
}
pub(super) fn build(
    config: &DiarySceneBuildConfig,
    source_disc: &SupportedSourceDisc,
    path: &Path,
) -> Result<SceneArtworkArchiveBuild> {
    let asset: Asset = serde_json::from_slice(&std::fs::read(path)?)?;
    ensure!(
        asset.source_record == RECORD
            && asset.source_sha256 == SOURCE
            && asset.font_role == "diary_scene.calendar_title"
            && asset.release_status == "needs_human_review",
        "month source, font role or review boundary changed"
    );
    let (_, original) = source_disc.read_record(RECORD)?;
    ensure!(
        sha256_bytes(&original) == SOURCE,
        "month overview source changed"
    );
    let members = parse_tzz(&original)?;
    let seen = asset
        .entries
        .iter()
        .map(|e| e.member)
        .collect::<BTreeSet<_>>();
    ensure!(
        members.len() == 12 && asset.entries.len() == 12 && seen == (0..12).collect(),
        "month overview population changed"
    );
    let style = config
        .fonts
        .calendar_title
        .as_ref()
        .context("month calendar font missing")?;
    let rasterizer = IndexedTextRasterizer::load(&style.path)?;
    let mut archive = original.clone();
    let mut reports = Vec::new();
    let mut layout_reports = Vec::new();
    let preview_dir = config.output_dir.join("month-overviews");
    std::fs::create_dir_all(&preview_dir)?;
    for entry in asset.entries {
        let member = members[entry.member];
        let stored = &original[member.compressed_range()];
        let source = decompress(stored, false)?;
        ensure!(
            sha256_bytes(&source) == entry.source_tim_sha256,
            "month member {} source changed",
            entry.member
        );
        let background = input(config, &entry.background, &entry.background_sha256)?;
        let mask = input(config, &entry.mask, &entry.mask_sha256)?;
        let mut candidate = apply_background(&source, &background, &mask, &entry.regions)?;
        let tim = parse_8bpp(&source)?;
        ensure!(!entry.labels.is_empty(), "month labels missing");
        let mut label_reports = Vec::new();
        for label in &entry.labels {
            let c = label.cell;
            ensure!(
                !label.source_text.is_empty()
                    && !label.text.is_empty()
                    && label.font_scale.is_finite()
                    && (0.25..=3.0).contains(&label.font_scale),
                "invalid month label"
            );
            ensure!(
                c.width > 0
                    && c.height > 0
                    && c.x.checked_add(c.width).is_some_and(|end| end <= 512)
                    && c.y.checked_add(c.height).is_some_and(|end| end <= 480),
                "month label outside image"
            );
            let raster = rasterizer.rasterize(
                &label.text,
                c.width,
                c.height,
                style.font_px * label.font_scale,
                0.0,
                0,
                Some(1),
                2,
                HorizontalTextAlignment::Center,
            )?;
            label_reports.push(serde_json::json!({"source_text":label.source_text,"text":label.text,"cell":c,"font_sha256":raster.font_sha256,"font_name":raster.font_name,"font_px":style.font_px*label.font_scale,"ink_bounds":raster.ink_bounds}));
            let outline = palette(&source, [15, 20, 25]);
            let fill = palette(&source, label.fill_rgb);
            for y in 0..c.height {
                for x in 0..c.width {
                    let ink = raster.pixels[y * c.width + x];
                    if ink == 0 {
                        continue;
                    }
                    let at = (c.y + y) * 512 + c.x + x;
                    ensure!(mask[at] == 1, "month label escaped declared text mask");
                    candidate[tim.pixel_offset + at] = if ink == 1 { outline } else { fill };
                }
            }
        }
        let ranges = crate::pipeline::difference_ranges(&source, &candidate);
        let claims = DecodedDataClaim::from_effective_ranges(
            "month overview",
            "compose the full edited image and typeset its month schedule",
            &source,
            &candidate,
            ranges,
        )?;
        let target = format!("{RECORD}:{}", entry.member);
        let mut plan = DecodedRecordWritePlan::new(&target, &source, &entry.source_tim_sha256)?;
        plan.register_data_candidate(
            "month overview",
            &entry.source_tim_sha256,
            &candidate,
            &claims,
        )?;
        ensure!(
            plan.apply(None)? == candidate,
            "month decoded plan mismatch"
        );
        let encoded = encode_runtime_member(stored, &source, &candidate)?;
        ensure!(
            encoded.bytes.len() <= stored.len(),
            "month {} compressed extent overflow",
            entry.member
        );
        let mut padded = encoded.bytes.clone();
        padded.resize(stored.len(), 0);
        ensure!(
            decompress(&padded, true)? == candidate,
            "month roundtrip failed"
        );
        archive[member.compressed_range()].copy_from_slice(&padded);
        std::fs::write(
            preview_dir.join(format!("member-{:02}.tim", entry.member)),
            &candidate,
        )?;
        layout_reports.push(serde_json::json!({"member":entry.member,"full_image_plate":true,"source_palette_preserved":true,"background_sha256":entry.background_sha256,"mask_sha256":entry.mask_sha256,"labels":label_reports}));
        reports.push(SceneArtworkMemberReport {
            index: entry.member,
            source_decoded_sha256: entry.source_tim_sha256,
            patched_decoded_sha256: sha256_bytes(&candidate),
            source_compressed_size: stored.len(),
            rebuilt_compressed_size: encoded.bytes.len(),
            outside_mask_preserved: true,
            compression_requirement_satisfied: encoded.compression_requirement_satisfied,
        });
    }
    ensure!(
        parse_tzz(&archive)? == members,
        "month member table changed"
    );
    std::fs::write(config.output_dir.join("MGBETA.TZZ"), &archive)?;
    crate::pipeline::write_pretty_json_and_hash(
        &config.output_dir.join("month-overview-build.json"),
        &serde_json::json!({"source_record":RECORD,"font_role":asset.font_role,"release_status":asset.release_status,"members":layout_reports}),
        true,
    )?;
    Ok(SceneArtworkArchiveBuild {
        report: SceneArtworkArchiveReport {
            path: RECORD.into(),
            source_size: original.len(),
            source_sha256: SOURCE.into(),
            patched_sha256: sha256_bytes(&archive),
            members: reports,
        },
        data: archive,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn source() -> Vec<u8> {
        let mut bytes = vec![0u8; 544 + 512 * 480];
        for (at, value) in [(0, 0x10u32), (4, 9), (8, 524), (532, 12 + 512 * 480)] {
            bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
        }
        for (at, value) in [(16, 256u16), (18, 1), (540, 256), (542, 480)] {
            bytes[at..at + 2].copy_from_slice(&value.to_le_bytes());
        }
        bytes
    }
    #[test]
    fn month_edits_retain_header_palette_and_reject_unowned_pixels() {
        let source = source();
        let mut pixels = vec![0; 512 * 480];
        let mut mask = pixels.clone();
        let cell = Cell {
            x: 4,
            y: 5,
            width: 2,
            height: 1,
        };
        for x in 4..6 {
            pixels[5 * 512 + x] = 42;
            mask[5 * 512 + x] = 1;
        }
        let result = apply_background(&source, &pixels, &mask, &[cell]).unwrap();
        assert_eq!(&result[..544], &source[..544]);
        assert_eq!(&result[544..], pixels);
        pixels[0] = 1;
        assert!(apply_background(&source, &pixels, &mask, &[cell]).is_err());
        pixels[0] = 0;
        mask[5 * 512 + 4] = 2;
        assert!(apply_background(&source, &pixels, &mask, &[cell]).is_err());
        let overflow = Cell {
            x: usize::MAX,
            y: 0,
            width: 2,
            height: 1,
        };
        assert!(apply_background(&source, &pixels, &mask, &[overflow]).is_err());
        assert!(apply_background(&source, &pixels[..12], &mask, &[cell]).is_err());
    }
}
