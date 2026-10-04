//! MODE SELECT illustration, preview and thumbnail image slots. Preparation is
//! offline; the installer owns frozen image regions and declared preview palettes.
use std::collections::BTreeSet;
use std::path::{Component, Path};

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

use crate::decoded_record_write_plan::DecodedDataClaim;
use crate::font::{HorizontalTextAlignment, IndexedTextRasterizers};
use crate::pipeline::sha256_bytes;
use crate::tim::{
    Cell, RgbaImage, decode_4bpp_rgba_in_prefix, install_indexed_glyph_in_prefix,
    parse_4bpp_prefix, read_indexed_cell_in_prefix,
};
use crate::tim_preview::write_tim_preview;

use super::model::{ModeSelectModeBuild, ReleaseStatus};
use super::source::{
    ATLAS_TIM_OFFSET, MODE_ARTWORK_FIRST_OFFSET, MODE_ARTWORK_STRIDE, MODE_COUNT,
    MODE_PREVIEW_FIRST_OFFSET, MODE_PREVIEW_STRIDE, PANEL_INDEX_BY_MODE_INDEX,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    kind: String,
    entries: Vec<Entry>,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
enum Family {
    Artwork,
    Preview,
    Thumbnail,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Action {
    Preserve,
    Pending,
    Replace,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    id: String,
    mode_index: usize,
    panel_index: usize,
    family: Family,
    source_tim_sha256: String,
    action: Action,
    release_status: ReleaseStatus,
    reason: String,
    indices_file: Option<String>,
    indices_sha256: Option<String>,
    palette_file: Option<String>,
    palette_sha256: Option<String>,
    #[serde(default)]
    labels: Vec<Label>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Label {
    text: String,
    cell: Cell,
    font_px: f32,
    fill_rgb: [i32; 3],
    outline_rgb: [i32; 3],
}
#[derive(Debug, Serialize)]
pub struct ImageFamilyBuild {
    pub manifest_sha256: String,
    pub release_input_eligible: bool,
    pub replaced_count: usize,
    pub preserved_count: usize,
    pub pending_count: usize,
    pub entries: Vec<serde_json::Value>,
}

fn validate_population(entries: &[Entry]) -> Result<()> {
    ensure!(
        entries.len() == MODE_COUNT * 3,
        "MODE SELECT image population is incomplete"
    );
    let mut seen = BTreeSet::new();
    for entry in entries {
        ensure!(
            entry.mode_index < MODE_COUNT
                && PANEL_INDEX_BY_MODE_INDEX[entry.mode_index] == entry.panel_index
                && seen.insert((entry.mode_index, entry.family)),
            "duplicate or unbound MODE SELECT image slot"
        );
        ensure!(
            !entry.id.is_empty() && !entry.reason.trim().is_empty(),
            "image lacks identity or disposition"
        );
        if entry.action != Action::Replace {
            ensure!(
                entry.indices_file.is_none()
                    && entry.indices_sha256.is_none()
                    && entry.palette_file.is_none()
                    && entry.palette_sha256.is_none()
                    && entry.labels.is_empty(),
                "preserved/pending image contains replacement input"
            );
        }
        if entry.family != Family::Preview {
            ensure!(
                entry.palette_file.is_none() && entry.palette_sha256.is_none(),
                "illustration and shared-atlas palettes are source-owned"
            );
        }
        ensure!(
            entry.family == Family::Artwork || entry.labels.is_empty(),
            "native previews and thumbnails cannot contain separately drawn text"
        );
    }
    Ok(())
}

fn nearest_palette(tim: &[u8], rgb: [i32; 3]) -> u8 {
    (0..16)
        .min_by_key(|&i| {
            let color = u16::from_le_bytes([tim[20 + i * 2], tim[21 + i * 2]]);
            [0, 5, 10]
                .iter()
                .enumerate()
                .map(|(c, s)| (i32::from((color >> s) & 31) * 255 / 31 - rgb[c]).pow(2))
                .sum::<i32>()
        })
        .unwrap() as u8
}

fn private_input(root: &Path, file: Option<&str>, hash: Option<&str>) -> Result<Vec<u8>> {
    let file = Path::new(file.context("frozen image input missing")?);
    ensure!(
        !file.is_absolute()
            && file.components().count() == 1
            && file.components().all(|c| matches!(c, Component::Normal(_))),
        "unsafe image input path"
    );
    let bytes = std::fs::read(root.join("../../private-artwork/mode-select").join(file))?;
    ensure!(
        Some(sha256_bytes(&bytes).as_str()) == hash,
        "MODE SELECT frozen image input changed: {}",
        file.display()
    );
    Ok(bytes)
}

/// The screenshot window is surrounded by the source bevel and drop shadow.
/// Preserve both its indices and every colour word used by those frame pixels.
fn validate_preview_frame(
    source: &[u8],
    pixels: &[u8],
    source_palette: &[u8],
    palette: &[u8],
) -> Result<()> {
    ensure!(
        source.len() == 128 * 96
            && pixels.len() == source.len()
            && source_palette.len() == 32
            && palette.len() == 32,
        "MODE SELECT preview input geometry changed"
    );
    for (old, new) in source_palette
        .as_chunks::<2>()
        .0
        .iter()
        .zip(palette.as_chunks::<2>().0)
    {
        ensure!(
            old[1] & 0x80 == new[1] & 0x80,
            "preview transparency flag changed"
        );
    }
    for (i, &index) in source.iter().enumerate() {
        if (3..124).contains(&(i % 128)) && (3..91).contains(&(i / 128)) {
            continue;
        }
        let color = usize::from(index) * 2;
        ensure!(
            pixels[i] == index && source_palette[color..color + 2] == palette[color..color + 2],
            "MODE SELECT preview changed source bevel or shadow"
        );
    }
    Ok(())
}

fn validate_thumbnail_frame(source: &[u8], pixels: &[u8]) -> Result<()> {
    ensure!(
        source.len() == 52 * 40 && pixels.len() == source.len(),
        "thumbnail geometry changed"
    );
    for (i, &p) in source.iter().enumerate() {
        if (1..50).contains(&(i % 52)) && (1..38).contains(&(i / 52)) {
            continue;
        }
        ensure!(
            pixels[i] == p,
            "thumbnail changed its source frame or shadow"
        );
    }
    Ok(())
}

pub(super) fn install(
    root: &Path,
    source: &[u8],
    patched: &mut [u8],
    font: &Path,
    output: &Path,
    claims: &mut Vec<DecodedDataClaim>,
    modes: &[ModeSelectModeBuild],
) -> Result<ImageFamilyBuild> {
    let bytes = std::fs::read(root.join("images.json"))?;
    let manifest: Manifest = serde_json::from_slice(&bytes)?;
    ensure!(
        manifest.kind == "mode_select_embedded_images",
        "unknown MODE SELECT image manifest"
    );
    validate_population(&manifest.entries)?;
    let mut report = ImageFamilyBuild {
        manifest_sha256: sha256_bytes(&bytes),
        release_input_eligible: manifest
            .entries
            .iter()
            .all(|e| e.action != Action::Pending && e.release_status == ReleaseStatus::Approved),
        replaced_count: 0,
        preserved_count: 0,
        pending_count: 0,
        entries: Vec::new(),
    };
    let mut rasterizers = IndexedTextRasterizers::default();
    for entry in manifest.entries {
        ensure!(
            modes.iter().any(|mode| mode.mode_index == entry.mode_index
                && mode.panel_index == entry.panel_index
                && mode.id == entry.id),
            "MODE SELECT image identity differs from its mode: {}",
            entry.id
        );
        let (offset, cell, geometry) = match entry.family {
            Family::Artwork => (
                MODE_ARTWORK_FIRST_OFFSET + entry.panel_index * MODE_ARTWORK_STRIDE,
                Cell {
                    x: 0,
                    y: 0,
                    width: 100,
                    height: 114,
                },
                (100, 114, 16, 1),
            ),
            Family::Preview => (
                MODE_PREVIEW_FIRST_OFFSET + entry.panel_index * MODE_PREVIEW_STRIDE,
                Cell {
                    x: 0,
                    y: 0,
                    width: 128,
                    height: 96,
                },
                (128, 96, 16, 1),
            ),
            Family::Thumbnail => (
                ATLAS_TIM_OFFSET,
                super::source::thumbnail_cell(entry.mode_index),
                (1280, 256, 512, 2),
            ),
        };
        let (width, height) = (cell.width, cell.height);
        let tim = parse_4bpp_prefix(source.get(offset..).context("missing image TIM")?)?;
        ensure!(
            (
                tim.pixel_width(),
                tim.image_height,
                tim.clut_width,
                tim.clut_height
            ) == geometry,
            "MODE SELECT image geometry changed: {}",
            entry.id
        );
        let source_pixels = read_indexed_cell_in_prefix(source, offset, cell)?;
        let original = &source[offset..offset + tim.total_size];
        ensure!(
            sha256_bytes(original) == entry.source_tim_sha256,
            "MODE SELECT source image changed: {}",
            entry.id
        );
        let mut labels = Vec::new();
        let mut preview_file = None;
        match entry.action {
            Action::Preserve => report.preserved_count += 1,
            Action::Pending => report.pending_count += 1,
            Action::Replace => {
                let mut pixels = private_input(
                    root,
                    entry.indices_file.as_deref(),
                    entry.indices_sha256.as_deref(),
                )?;
                ensure!(
                    pixels.len() == width * height && pixels.iter().all(|&p| p < 16),
                    "invalid MODE SELECT image indices: {}",
                    entry.id
                );
                if entry.family == Family::Preview {
                    let palette = private_input(
                        root,
                        entry.palette_file.as_deref(),
                        entry.palette_sha256.as_deref(),
                    )?;
                    validate_preview_frame(&source_pixels, &pixels, &original[20..52], &palette)?;
                    patched[offset + 20..offset + 52].copy_from_slice(&palette);
                    claims.extend(DecodedDataClaim::from_ranges(
                        &format!("menu:mode-select:preview-palette:{}", entry.id),
                        &format!(
                            "install native preview palette {} with source frame colours",
                            entry.id
                        ),
                        vec![[offset + 20, offset + 52]],
                    ));
                }
                if entry.family == Family::Thumbnail {
                    validate_thumbnail_frame(&source_pixels, &pixels)?;
                }
                for label in &entry.labels {
                    let c = label.cell;
                    ensure!(
                        c.x <= width
                            && c.width <= width - c.x
                            && c.y <= height
                            && c.height <= height - c.y,
                        "MODE SELECT artwork label exceeds image: {}",
                        entry.id
                    );
                    let fill = nearest_palette(original, label.fill_rgb);
                    let outline = nearest_palette(original, label.outline_rgb);
                    ensure!(
                        fill != outline,
                        "MODE SELECT artwork text contrast collapsed"
                    );
                    // Use a separate 0/1/2 coverage plane; palette index zero is valid artwork.
                    let rendered = rasterizers.for_font(font)?.rasterize(
                        &label.text,
                        c.width,
                        c.height,
                        label.font_px,
                        0.0,
                        0,
                        Some(1),
                        2,
                        HorizontalTextAlignment::Center,
                    )?;
                    for y in 0..c.height {
                        for x in 0..c.width {
                            let p = rendered.pixels[y * c.width + x];
                            if p != 0 {
                                pixels[(c.y + y) * width + c.x + x] =
                                    if p == 2 { fill } else { outline };
                            }
                        }
                    }
                    labels.push(serde_json::json!({"text":label.text,"cell":c,"font_px":label.font_px,
                        "font_name":rendered.font_name,"font_sha256":rendered.font_sha256,
                        "measured_advance_px":rendered.measured_advance_px,"fill_index":fill,"outline_index":outline}));
                }
                let installed = install_indexed_glyph_in_prefix(
                    patched,
                    offset,
                    cell,
                    &pixels,
                    "MODE SELECT complete image",
                )?;
                claims.extend(DecodedDataClaim::from_ranges(
                    &format!("menu:mode-select:image:{:?}:{}", entry.family, entry.id),
                    &format!(
                        "install complete MODE SELECT {:?} {}",
                        entry.family, entry.id
                    ),
                    installed.allowed_decoded_byte_ranges,
                ));
                let name = format!("image-{}-{:?}.png", entry.id, entry.family).to_lowercase();
                let rgba = decode_4bpp_rgba_in_prefix(
                    patched,
                    offset,
                    if entry.family == Family::Thumbnail {
                        4
                    } else {
                        0
                    },
                )?;
                let preview = RgbaImage {
                    width,
                    height,
                    pixels: (cell.y..cell.y + height)
                        .flat_map(|y| {
                            let start = (y * rgba.width + cell.x) * 4;
                            rgba.pixels[start..start + width * 4].iter().copied()
                        })
                        .collect(),
                };
                write_tim_preview(&output.join(&name), &preview)?;
                preview_file = Some(name);
                report.replaced_count += 1;
            }
        }
        ensure!(
            source[offset..offset + 20] == patched[offset..offset + 20]
                && source[offset + 52..offset + tim.pixel_offset]
                    == patched[offset + 52..offset + tim.pixel_offset],
            "MODE SELECT image changed header"
        );
        let palette_preserved =
            source[offset + 20..offset + 52] == patched[offset + 20..offset + 52];
        ensure!(
            entry.family == Family::Preview || palette_preserved,
            "illustration palette changed"
        );
        if entry.action != Action::Replace {
            ensure!(
                source_pixels == read_indexed_cell_in_prefix(patched, offset, cell)?,
                "unselected MODE SELECT image region changed"
            );
        }
        report.entries.push(
            serde_json::json!({"id":entry.id,"mode_index":entry.mode_index,
            "panel_index":entry.panel_index,"family":entry.family,"action":entry.action,
            "reason":entry.reason,"release_status":entry.release_status,"tim_offset":offset,"source_tim_sha256":entry.source_tim_sha256,
            "cell":cell,"output_region_sha256":sha256_bytes(&read_indexed_cell_in_prefix(patched, offset, cell)?),
            "indices_sha256":entry.indices_sha256,"palette_sha256":entry.palette_sha256,
            "header_preserved":true,"palette_preserved":palette_preserved,
            "preview_frame_preserved":entry.family == Family::Preview,"thumbnail_frame_preserved":entry.family == Family::Thumbnail,
            "labels":labels,"preview_file":preview_file}),
        );
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn thumbnail_replacement_keeps_every_frame_and_shadow_pixel() {
        let source: Vec<u8> = (0..52 * 40).map(|i| (i % 16) as u8).collect();
        let mut picture = source.clone();
        for y in 1..38 {
            for x in 1..50 {
                picture[y * 52 + x] = (picture[y * 52 + x] + 1) % 16;
            }
        }
        assert!(validate_thumbnail_frame(&source, &picture).is_ok());
        for y in 0..40 {
            for x in 0..52 {
                if (1..50).contains(&x) && (1..38).contains(&y) {
                    continue;
                }
                let i = y * 52 + x;
                picture[i] = (source[i] + 1) % 16;
                assert!(validate_thumbnail_frame(&source, &picture).is_err());
                picture[i] = source[i];
            }
        }
        assert!(validate_thumbnail_frame(&source, &picture[..picture.len() - 1]).is_err());
    }

    #[test]
    fn native_preview_can_change_its_picture_but_not_the_source_frame_or_alpha() {
        let mut source = vec![0; 128 * 96];
        source[4 * 128 + 4] = 1;
        let mut picture = source.clone();
        picture[4 * 128 + 4] = 2;
        let palette = [0x8000_u16; 16]
            .into_iter()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        let mut updated = palette.clone();
        updated[2] = 31;
        assert!(validate_preview_frame(&source, &picture, &palette, &updated).is_ok());
        updated[0] = 31;
        assert!(validate_preview_frame(&source, &picture, &palette, &updated).is_err());
        updated[0] = 0;
        updated[3] = 0;
        assert!(validate_preview_frame(&source, &picture, &palette, &updated).is_err());
        picture[0] = 1;
        assert!(validate_preview_frame(&source, &picture, &palette, &palette).is_err());
    }

    fn entries() -> Vec<Entry> {
        PANEL_INDEX_BY_MODE_INDEX
            .iter()
            .enumerate()
            .flat_map(|(mode, &panel)| {
                [Family::Artwork, Family::Preview, Family::Thumbnail].map(move |family| Entry {
                    id: format!("mode-{mode}"),
                    mode_index: mode,
                    panel_index: panel,
                    family,
                    source_tim_sha256: String::new(),
                    action: Action::Pending,
                    release_status: ReleaseStatus::NeedsHumanReview,
                    reason: "await capture".into(),
                    indices_file: None,
                    indices_sha256: None,
                    palette_file: None,
                    palette_sha256: None,
                    labels: Vec::new(),
                })
            })
            .collect()
    }
    #[test]
    fn image_population_cannot_drop_or_duplicate_a_consumer() {
        let mut all = entries();
        assert!(validate_population(&all).is_ok());
        all.pop();
        assert!(validate_population(&all).is_err());
        all.push(entries().remove(0));
        assert!(validate_population(&all).is_err());
    }
    #[test]
    fn image_population_rejects_wrong_panel_and_hidden_replacement() {
        let mut all = entries();
        all[0].panel_index = 0;
        assert!(validate_population(&all).is_err());
        let mut all = entries();
        all[0].indices_file = Some("unexpected.indices".into());
        assert!(validate_population(&all).is_err());
    }
}
