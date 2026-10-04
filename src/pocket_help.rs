//! Full-image PocketStation help/J-BANK art with deterministic, role-based lettering.
use crate::{
    development_build_spec::DevelopmentFontSources,
    font::{HorizontalTextAlignment, IndexedTextRasterizers},
    indexed_member_archive::{decode_indexed_member_archive, rebuild_indexed_member_archive},
    pipeline::sha256_bytes,
    source_disc::SupportedSourceDisc,
};
use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use serde_json::json;
use std::{collections::BTreeSet, path::Path};
mod source;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Background {
    index: usize,
    imagegen_sha256: String,
    pixels_sha256: String,
    source_decoded_sha256: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Layout {
    review_status: String,
    panels: Vec<Panel>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Panel {
    index: usize,
    labels: Vec<Label>,
    illustration_rectangles: Vec<[usize; 4]>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Label {
    text: String,
    role: String,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    fill: [i32; 3],
    outline: [i32; 3],
    alignment: String,
}
pub(crate) struct Build {
    pub source_sha256: String,
    pub output: Vec<u8>,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires assets/"]
    fn banner_ink_is_not_clipped_by_its_layout_rectangle() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let spec = crate::development_build_spec::load_development_build_spec(
            &root.join("assets/build/development.json"),
        )
        .unwrap();
        let layout: Layout = serde_json::from_slice(
            &std::fs::read(root.join("assets/menu/pocket-help/layout.json")).unwrap(),
        )
        .unwrap();
        let mut fonts = IndexedTextRasterizers::default();
        for panel in layout.panels {
            for label in panel.labels.into_iter().filter(|l| l.role == "banner") {
                let font = &spec.fonts.pocket_help[&label.role];
                let rasterizer = fonts.for_font(&font.path).unwrap();
                let render = |height| {
                    rasterizer
                        .rasterize(
                            &label.text,
                            label.width,
                            height,
                            font.font_px,
                            0.0,
                            0,
                            Some(1),
                            2,
                            HorizontalTextAlignment::Center,
                        )
                        .unwrap()
                };
                let native = render(label.height);
                let padded = render(label.height + 4);
                assert!(
                    padded.pixels[..2 * label.width].iter().all(|p| *p == 0),
                    "panel {} top clipped",
                    panel.index
                );
                assert!(
                    padded.pixels[(label.height + 2) * label.width..]
                        .iter()
                        .all(|p| *p == 0),
                    "panel {} bottom clipped",
                    panel.index
                );
                assert_eq!(
                    native.pixels,
                    padded.pixels[2 * label.width..(label.height + 2) * label.width],
                    "panel {} lost banner ink",
                    panel.index
                );
            }
        }
    }
}
fn palette(tim: &[u8], rgb: [i32; 3]) -> u8 {
    (0..256)
        .min_by_key(|&i| {
            let c = u16::from_le_bytes([tim[20 + i * 2], tim[21 + i * 2]]);
            [0, 5, 10]
                .iter()
                .enumerate()
                .map(|(n, s)| (i32::from((c >> s) & 31) * 255 / 31 - rgb[n]).pow(2))
                .sum::<i32>()
        })
        .unwrap() as u8
}
pub(crate) fn build(
    disc: &SupportedSourceDisc,
    assets: &Path,
    fonts: &DevelopmentFontSources,
    out: &Path,
) -> Result<Build> {
    let (_, stored) = disc.read_record("DAT2/PDATIM.BIZ")?;
    let original = decode_indexed_member_archive(&stored, &source::ARCHIVE)?;
    let backgrounds: Vec<Background> =
        serde_json::from_slice(&std::fs::read(assets.join("backgrounds.json"))?)?;
    let layout: Layout = serde_json::from_slice(&std::fs::read(assets.join("layout.json"))?)?;
    ensure!(
        backgrounds.len() == 5 && layout.panels.len() == 5,
        "PocketStation panel population changed"
    );
    let mut decoded = original.clone();
    let mut rasterizers = IndexedTextRasterizers::default();
    let mut seen = BTreeSet::new();
    let mut reports = Vec::new();
    std::fs::create_dir_all(out)?;
    for panel in layout.panels {
        ensure!(
            panel.index < 5 && seen.insert(panel.index),
            "duplicate PocketStation panel"
        );
        let offset = panel.index * 246304;
        let tim = &original[offset..offset + 246304];
        let background = &backgrounds[panel.index];
        ensure!(
            background.index == panel.index
                && sha256_bytes(tim) == background.source_decoded_sha256
                && background.imagegen_sha256.len() == 64,
            "PocketStation background source changed"
        );
        let private = assets.join("../../private-artwork/pocket-help");
        ensure!(
            sha256_bytes(&std::fs::read(
                private.join(format!("member-{}.png", panel.index))
            )?) == background.imagegen_sha256,
            "PocketStation generated image changed"
        );
        let mut pixels = std::fs::read(private.join(format!("member-{}.pixels", panel.index)))?;
        ensure!(
            pixels.len() == 512 * 480 && sha256_bytes(&pixels) == background.pixels_sha256,
            "PocketStation background pixels changed"
        );
        let mut occupied = vec![false; pixels.len()];
        let mut labels = Vec::new();
        for label in panel.labels {
            ensure!(
                label.x + label.width <= 512 && label.y + label.height <= 480,
                "PocketStation label out of image"
            );
            let font = fonts
                .pocket_help
                .get(&label.role)
                .context("PocketStation font role not selected")?;
            let alignment = match label.alignment.as_str() {
                "left" => HorizontalTextAlignment::Left,
                "center" => HorizontalTextAlignment::Center,
                _ => anyhow::bail!("unknown PocketStation alignment"),
            };
            let raster = rasterizers
                .for_font(&font.path)?
                .rasterize(
                    &label.text,
                    label.width,
                    label.height,
                    font.font_px,
                    0.0,
                    0,
                    Some(1),
                    2,
                    alignment,
                )
                .with_context(|| format!("PocketStation panel {}: {}", panel.index, label.text))?;
            let colors = [0, palette(tim, label.outline), palette(tim, label.fill)];
            for (i, &v) in raster.pixels.iter().enumerate() {
                if v != 0 {
                    let at = (label.y + i / label.width) * 512 + label.x + i % label.width;
                    ensure!(!occupied[at], "PocketStation text overlap: {}", label.text);
                    let (x, y) = (at % 512, at / 512);
                    ensure!(
                        !panel
                            .illustration_rectangles
                            .iter()
                            .any(|r| x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3]),
                        "PocketStation lettering overlaps an LCD illustration: {}",
                        label.text
                    );
                    occupied[at] = true;
                    pixels[at] = colors[v as usize];
                }
            }
            labels.push(json!({"text":label.text,"font_sha256":raster.font_sha256,"font_px":font.font_px,"ink_bounds":raster.ink_bounds,"rectangle":[label.x,label.y,label.width,label.height]}));
        }
        decoded[offset + 0x220..offset + 246304].copy_from_slice(&pixels);
        ensure!(
            decoded[offset..offset + 0x220] == original[offset..offset + 0x220],
            "PocketStation palette/VRAM header changed"
        );
        let image = crate::embedded_tim::parse_embedded_tim_at(&decoded, offset)?;
        crate::tim_preview::write_tim_preview(
            &out.join(format!("panel-{}.png", panel.index)),
            &crate::embedded_tim::decode_embedded_tim_preview(&decoded, &image)?,
        )?;
        reports.push(json!({"index":panel.index,"labels":labels,"background_pixels_sha256":background.pixels_sha256}));
    }
    let archive = rebuild_indexed_member_archive(&stored, &source::ARCHIVE, &decoded)?;
    std::fs::write(out.join("PDATIM.BIZ"), &archive.physical)?;
    std::fs::write(
        out.join("pocket-help.json"),
        serde_json::to_vec_pretty(
            &json!({"panels":reports,"review_status":layout.review_status,"runtime_verified":false,"source_sha256":source::ARCHIVE.record_sha256,"output_sha256":sha256_bytes(&archive.physical)}),
        )?,
    )?;
    Ok(Build {
        source_sha256: source::ARCHIVE.record_sha256.into(),
        output: archive.physical,
    })
}
