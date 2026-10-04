//! The native STAFF overlay consumes EDSR, not the similarly named MGSTAFF1 archive.
use std::path::Path;

use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::compression::decompress;
use crate::development_build_spec::SizedFontSource;
use crate::font::{HorizontalTextAlignment, IndexedTextRasterizers};
use crate::pipeline::sha256_bytes;
use crate::source_disc::SupportedSourceDisc;
use crate::tim::{
    Cell, read_4bpp_indexed_image_in_prefix, read_indexed_cell_in_prefix,
    write_indexed_cell_in_prefix_with_report,
};

const OVERLAY: &str = "DAT1/STAFF.BIN";
const GRAPHICS: &str = "DAT2/EDSR.BIZ";
const OVERLAY_HASH: &str = "25ad7704b7100927350f4db900bf315ce76272b8a296a66ec051a0168abe18e5";
const GRAPHICS_HASH: &str = "c1dfe5def1b6a194d4e1b56ef9c4d6ce6b4601fb03642a64d1605b103b8108fe";
const TABLE: usize = 0x654;
const COUNT: usize = 118;
const ATLAS: [usize; 2] = [0xa0000, 0xc0800];
const WIDTH: [usize; 2] = [1024, 512];
const CLEAR: u8 = 7;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Credits {
    source_overlay_sha256: String,
    source_graphics_sha256: String,
    source_record: String,
    reading_references: Vec<String>,
    review_status: String,
    entries: Vec<Entry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    id: usize,
    source: String,
    text: String,
}

pub(crate) struct StaffRecord {
    pub path: &'static str,
    pub source_sha256: String,
    pub output: Vec<u8>,
}

pub(crate) struct StaffRollBuild {
    pub records: Vec<StaffRecord>,
}

pub fn build_staff_roll(cue: &Path, spec: &Path, output: &Path) -> Result<()> {
    ensure!(
        !output.exists(),
        "staff-roll output directory already exists"
    );
    let config = crate::development_build_spec::load_development_build_spec(spec)?;
    build(
        &SupportedSourceDisc::open(cue)?,
        config
            .assets
            .staff_roll
            .as_deref()
            .context("staff-roll assets not selected")?,
        config
            .fonts
            .staff_roll
            .as_ref()
            .context("staff-roll font not selected")?,
        output,
    )?;
    Ok(())
}

fn geometry(overlay: &[u8], id: usize) -> [u16; 8] {
    std::array::from_fn(|i| {
        u16::from_le_bytes(
            overlay[TABLE + id * 16 + i * 2..TABLE + id * 16 + i * 2 + 2]
                .try_into()
                .unwrap(),
        )
    })
}

/// Paired names and the song's role/name columns share one centering span.
fn align_pairs(
    widths: &[usize],
    geometry: &mut [[u16; 8]],
    ids: &[usize],
    right_align: bool,
) -> Result<()> {
    let left = ids.iter().step_by(2).map(|&i| widths[i]).max().unwrap();
    let right = ids
        .iter()
        .skip(1)
        .step_by(2)
        .map(|&i| widths[i])
        .max()
        .unwrap();
    let span = left + 12 + right;
    ensure!(
        span <= 256,
        "staff-roll paired columns exceed a half-screen: {span}px"
    );
    for pair in ids.as_chunks::<2>().0 {
        for &id in pair {
            geometry[id][6] = span as u16;
        }
        geometry[pair[0]][7] = if right_align {
            (left - widths[pair[0]]) as u16
        } else {
            0
        };
        geometry[pair[1]][7] = (left + 12) as u16;
    }
    Ok(())
}

fn render(
    source: &[u8],
    overlay: &[u8],
    entries: &[Entry],
    font: &SizedFontSource,
) -> Result<(Vec<u8>, Vec<u8>, Value)> {
    ensure!(
        source.len() == 856064 && overlay.len() == 15276,
        "staff source dimensions changed"
    );
    ensure!(
        entries.len() == COUNT
            && entries
                .iter()
                .enumerate()
                .all(|(i, e)| e.id == i && !e.source.is_empty() && !e.text.trim().is_empty()),
        "staff credit population changed"
    );
    let mut output = source.to_vec();
    let mut writable = Vec::new();
    let mut occupied = Vec::new();
    for (i, &offset) in ATLAS.iter().enumerate() {
        let image = read_4bpp_indexed_image_in_prefix(source, offset)?;
        ensure!(
            (image.width, image.height) == (WIDTH[i], 256),
            "staff atlas geometry changed"
        );
        writable.push(image.pixels.iter().map(|&p| p == CLEAR).collect::<Vec<_>>());
        occupied.push(vec![false; image.pixels.len()]);
    }
    let mut inks = Vec::new();
    let mut ranges = Vec::new();
    for id in 0..COUNT {
        let g = geometry(overlay, id);
        let atlas = usize::from(g[1] >= 256);
        let cell = Cell {
            x: usize::from(g[0] - 768) * 4 + usize::from(g[2]),
            y: usize::from(g[3]),
            width: usize::from(g[4]),
            height: usize::from(g[5]),
        };
        ensure!(cell.height == 20, "staff credit height changed");
        let pixels = read_indexed_cell_in_prefix(source, ATLAS[atlas], cell)?;
        // Index 7 is black. Cyan text uses the 1..6 ramp; names use 8..15.
        inks.push(if pixels.contains(&15) { 15 } else { 6 });
        for y in cell.y..cell.y + cell.height {
            for x in cell.x..cell.x + cell.width {
                writable[atlas][y * WIDTH[atlas] + x] = true;
            }
        }
        ranges.extend(
            write_indexed_cell_in_prefix_with_report(
                &mut output,
                ATLAS[atlas],
                cell,
                &vec![CLEAR; pixels.len()],
            )?
            .allowed_ranges,
        );
    }
    // The final native CAPCOM logo is a separate sprite, not a credit string.
    for y in 40..96 {
        for x in 256..496 {
            writable[1][y * WIDTH[1] + x] = false;
        }
    }
    let mut rasterizers = IndexedTextRasterizers::default();
    let rasterizer = rasterizers.for_font(&font.path)?;
    let mut geometries = Vec::new();
    let mut widths = Vec::new();
    let mut reports = Vec::new();
    for entry in entries {
        let raster = rasterizer.rasterize(
            &entry.text,
            248,
            20,
            font.font_px,
            0.0,
            CLEAR,
            None,
            inks[entry.id],
            HorizontalTextAlignment::Left,
        )?;
        let width = (raster.measured_advance_px.ceil() as usize).max(raster.ink_bounds[2]) + 2;
        ensure!(width <= 248, "staff credit exceeds native sprite width");
        let mut placement = None;
        'search: for atlas in 0..2 {
            for page in 0..WIDTH[atlas] / 256 {
                for y in (0..=236).step_by(20) {
                    for x in page * 256..=page * 256 + 256 - width {
                        if (y..y + 20).all(|yy| {
                            (x..x + width).all(|xx| {
                                let at = yy * WIDTH[atlas] + xx;
                                writable[atlas][at] && !occupied[atlas][at]
                            })
                        }) {
                            placement = Some((atlas, x, y));
                            break 'search;
                        }
                    }
                }
            }
        }
        let (atlas, x, y) =
            placement.context("staff credits do not fit their protected atlases")?;
        let mut pixels = Vec::new();
        for yy in 0..20 {
            pixels.extend_from_slice(&raster.pixels[yy * 248..yy * 248 + width]);
            for xx in x..x + width {
                occupied[atlas][(y + yy) * WIDTH[atlas] + xx] = true;
            }
        }
        let cell = Cell {
            x,
            y,
            width,
            height: 20,
        };
        ranges.extend(
            write_indexed_cell_in_prefix_with_report(&mut output, ATLAS[atlas], cell, &pixels)?
                .allowed_ranges,
        );
        let g = [
            768 + (x / 256 * 64) as u16,
            (atlas * 256) as u16,
            (x % 256) as u16,
            y as u16,
            width as u16,
            20,
            width as u16,
            0,
        ];
        geometries.push(g);
        widths.push(width);
        reports.push(
            json!({"id":entry.id,"source":entry.source,"text":entry.text,
            "atlas":atlas,"cell":cell,"ink":inks[entry.id],"ink_bounds":raster.ink_bounds,
            "font":raster.font_name,"font_sha256":raster.font_sha256,"font_px":font.font_px,
            "output_indexed_sha256":sha256_bytes(&pixels)}),
        );
    }
    align_pairs(&widths, &mut geometries, &[56, 57, 58, 59, 60, 61], false)?;
    align_pairs(
        &widths,
        &mut geometries,
        &[99, 100, 101, 102, 103, 104],
        true,
    )?;
    let mut patched_overlay = overlay.to_vec();
    for (id, g) in geometries.iter().enumerate() {
        for (i, value) in g.iter().enumerate() {
            patched_overlay[TABLE + id * 16 + i * 2..TABLE + id * 16 + i * 2 + 2]
                .copy_from_slice(&value.to_le_bytes());
        }
        reports[id]["geometry"] = json!(g);
    }
    for (i, (&a, &b)) in source.iter().zip(&output).enumerate() {
        ensure!(
            a == b || ranges.iter().any(|r| i >= r[0] && i < r[0] + r[1]),
            "staff graphics write escaped its owned cells at {i:#x}"
        );
    }
    ensure!(
        source[..ATLAS[0]] == output[..ATLAS[0]],
        "staff animation prefix changed"
    );
    Ok((
        output,
        patched_overlay,
        json!({"entries":reports,"allowed_graphics_ranges":ranges,
        "overlay_geometry_range":[TABLE,COUNT*16],"preserved_animation_prefix":ATLAS[0],
        "preserved_logo":{"atlas":1,"cell":[256,40,240,56]}}),
    ))
}

pub(crate) fn build(
    source: &SupportedSourceDisc,
    assets: &Path,
    font: &SizedFontSource,
    output_dir: &Path,
) -> Result<StaffRollBuild> {
    let bytes = std::fs::read(assets.join("credits.json"))?;
    let credits: Credits = serde_json::from_slice(&bytes)?;
    ensure!(
        credits.source_overlay_sha256 == OVERLAY_HASH
            && credits.source_graphics_sha256 == GRAPHICS_HASH
            && credits.source_record == GRAPHICS,
        "staff credit source binding changed"
    );
    let (_, overlay) = source.read_record(OVERLAY)?;
    let (_, stored) = source.read_record(GRAPHICS)?;
    ensure!(
        sha256_bytes(&overlay) == OVERLAY_HASH && sha256_bytes(&stored) == GRAPHICS_HASH,
        "unsupported staff-roll source"
    );
    let decoded = decompress(&stored, true)?;
    let (patched, patched_overlay, mut report) =
        render(&decoded, &overlay, &credits.entries, font)?;
    let (compressed, prefix) =
        crate::paged_compression::compress_on_source_final_page_with_preserved_prefix(
            &patched,
            &stored,
            crate::paged_compression::source_paged_compression_profile(&stored)?,
            ATLAS[0],
        )?;
    ensure!(
        compressed.len() <= stored.len() && compressed[..4] == stored[..4],
        "staff compression/catalog boundary changed"
    );
    let mut graphics = stored.clone();
    graphics[..compressed.len()].copy_from_slice(&compressed);
    graphics[compressed.len()..].fill(0);
    ensure!(
        decompress(&graphics, true)? == patched,
        "staff graphics readback differs"
    );
    std::fs::create_dir_all(output_dir)?;
    for (atlas, offset) in ATLAS.iter().enumerate() {
        let tim = crate::embedded_tim::parse_embedded_tim_at(&patched, *offset)?;
        let preview = crate::embedded_tim::decode_embedded_tim_preview(&patched, &tim)?;
        crate::tim_preview::write_tim_preview(
            &output_dir.join(format!("atlas-{atlas}.png")),
            &preview,
        )?;
    }
    report["manifest_sha256"] = json!(sha256_bytes(&bytes));
    report["review_status"] = json!(credits.review_status);
    report["reading_references"] = json!(credits.reading_references);
    report["source_overlay_sha256"] = json!(OVERLAY_HASH);
    report["source_graphics_sha256"] = json!(GRAPHICS_HASH);
    report["output_overlay_sha256"] = json!(sha256_bytes(&patched_overlay));
    report["output_graphics_sha256"] = json!(sha256_bytes(&graphics));
    report["output_decoded_sha256"] = json!(sha256_bytes(&patched));
    report["preserved_compressed_prefix_bytes"] = json!(prefix.encoded_byte_count);
    std::fs::write(output_dir.join("STAFF.BIN"), &patched_overlay)?;
    std::fs::write(output_dir.join("EDSR.BIZ"), &graphics)?;
    std::fs::write(
        output_dir.join("staff-roll-build.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    Ok(StaffRollBuild {
        records: vec![
            StaffRecord {
                path: OVERLAY,
                source_sha256: OVERLAY_HASH.into(),
                output: patched_overlay,
            },
            StaffRecord {
                path: GRAPHICS,
                source_sha256: GRAPHICS_HASH.into(),
                output: graphics,
            },
        ],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paired_names_share_center_without_overlapping() -> Result<()> {
        let widths = [140, 96, 105, 85];
        let mut geometry = [[0u16; 8]; 4];
        align_pairs(&widths, &mut geometry, &[0, 1, 2, 3], false)?;
        assert_eq!(geometry[0][6], 248);
        assert_eq!(geometry[3][7], 152);
        assert!(usize::from(geometry[1][7]) >= widths[0] + 12);
        assert!(align_pairs(&[180, 96], &mut geometry[..2], &[0, 1], false).is_err());
        Ok(())
    }

    #[test]
    #[ignore = "requires the supported source disc and selected fonts"]
    fn staff_credits_preserve_animation_logo_palettes_and_schedule() -> Result<()> {
        let config = crate::development_build_spec::load_development_build_spec(Path::new(
            "assets/build/development.json",
        ))?;
        let disc = SupportedSourceDisc::open(Path::new(
            "roms/Shiritsu Justice Gakuen - Nekketsu Seishun Nikki 2 (Japan).cue",
        ))?;
        let (_, overlay) = disc.read_record(OVERLAY)?;
        let (_, stored) = disc.read_record(GRAPHICS)?;
        let source = decompress(&stored, true)?;
        let credits: Credits = serde_json::from_slice(&std::fs::read(
            config.assets.staff_roll.unwrap().join("credits.json"),
        )?)?;
        let (output, patched_overlay, report) = render(
            &source,
            &overlay,
            &credits.entries,
            &config.fonts.staff_roll.unwrap(),
        )?;
        assert_eq!(overlay[..TABLE], patched_overlay[..TABLE]);
        assert_eq!(
            overlay[TABLE + COUNT * 16..],
            patched_overlay[TABLE + COUNT * 16..]
        );
        for offset in ATLAS {
            assert_eq!(
                source[offset..offset + 0x120],
                output[offset..offset + 0x120]
            );
        }
        let logo = Cell {
            x: 256,
            y: 40,
            width: 240,
            height: 56,
        };
        assert_eq!(
            read_indexed_cell_in_prefix(&source, ATLAS[1], logo)?,
            read_indexed_cell_in_prefix(&output, ATLAS[1], logo)?
        );
        assert_eq!(report["entries"].as_array().unwrap().len(), COUNT);
        let mut broken = credits.entries;
        broken[4].id = 3;
        assert!(render(&source, &overlay, &broken, &config.fonts.dialogue_body).is_err());
        Ok(())
    }
}
