//! Source-bound oval badges distinguish air-allowed moves from air-only commands.
use super::model::ModeDescendantGraphicsBuildConfig;
use crate::{
    font::{HorizontalTextAlignment, IndexedTextRasterizers},
    pipeline::sha256_bytes,
    tim::{
        Cell, cells_overlap, read_4bpp_palette_words_in_prefix, read_indexed_cell_in_prefix,
        write_indexed_cell_in_prefix_with_report,
    },
};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};
#[derive(Debug)]
pub(super) struct Plan {
    sha256: String,
    manifest: Manifest,
    backgrounds: BTreeMap<(usize, usize), Vec<u8>>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    source_text: String,
    korean_lines: Vec<String>,
    imagegen_sha256: String,
    backgrounds: Vec<Background>,
    entries: Vec<Entry>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Background {
    width: usize,
    height: usize,
    path: PathBuf,
    sha256: String,
    dotmend_bundle: String,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    id: String,
    member_index: usize,
    cell: Cell,
    source_indexed_sha256: String,
    red_index: u8,
    white_index: u8,
}
#[derive(Debug, Serialize)]
pub struct Report {
    pub manifest_sha256: String,
    pub imagegen_sha256: String,
    pub authored_occurrence_count: usize,
    pub members: Vec<usize>,
    pub font_sha256: String,
    pub font_px: f32,
    pub runtime_verified: bool,
}
pub(super) fn load(path: &Path) -> Result<Plan> {
    let bytes = std::fs::read(path)?;
    let manifest: Manifest = serde_json::from_slice(&bytes)?;
    ensure!(
        manifest.source_text == "空中可" && manifest.korean_lines == ["공중", "가능"],
        "unreviewed air-allowed wording"
    );
    let mut backgrounds = BTreeMap::new();
    for b in &manifest.backgrounds {
        ensure!(
            matches!((b.width, b.height), (27, 18) | (35, 18) | (35, 19))
                && b.dotmend_bundle.len() == 64,
            "unreviewed badge background"
        );
        let data = std::fs::read(
            path.parent()
                .context("badge manifest has no parent")?
                .join(&b.path),
        )?;
        ensure!(
            sha256_bytes(&data) == b.sha256
                && data.len() == b.width * b.height
                && data.iter().all(|v| *v <= 1),
            "badge background identity or palette changed"
        );
        ensure!(
            backgrounds.insert((b.width, b.height), data).is_none(),
            "duplicate badge background"
        );
    }
    let mut ids = BTreeSet::new();
    for (n, e) in manifest.entries.iter().enumerate() {
        ensure!(
            ids.insert(&e.id)
                && e.member_index < 32
                && e.cell.x >= 284
                && e.cell.x.checked_add(e.cell.width).is_some_and(|v| v <= 409)
                && e.cell
                    .y
                    .checked_add(e.cell.height)
                    .is_some_and(|v| v <= 320)
                && backgrounds.contains_key(&(e.cell.width, e.cell.height)),
            "invalid badge occurrence"
        );
        ensure!(
            e.red_index < 16 && e.white_index < 16 && e.red_index != e.white_index,
            "invalid badge palette roles"
        );
        for other in &manifest.entries[..n] {
            ensure!(
                e.member_index != other.member_index || !cells_overlap(e.cell, other.cell),
                "overlapping badge cells"
            );
        }
    }
    ensure!(!manifest.entries.is_empty(), "empty badge plan");
    Ok(Plan {
        sha256: sha256_bytes(&bytes),
        manifest,
        backgrounds,
    })
}
pub(super) fn apply(
    plan: &Plan,
    config: &ModeDescendantGraphicsBuildConfig,
    source: &[u8],
    patched: &mut [u8],
    allowed: &mut Vec<[usize; 2]>,
) -> Result<Report> {
    let style = config
        .fonts
        .edit_badge
        .as_ref()
        .context("badge font role missing")?;
    let mut rasterizers = IndexedTextRasterizers::default();
    let rasterizer = rasterizers.for_font(&style.path)?;
    for e in &plan.manifest.entries {
        let tim = e.member_index * 0x2b800;
        let original = read_indexed_cell_in_prefix(source, tim, e.cell)?;
        ensure!(
            sha256_bytes(&original) == e.source_indexed_sha256,
            "badge {} source changed",
            e.id
        );
        ensure!(
            original == read_indexed_cell_in_prefix(patched, tim, e.cell)?,
            "badge overlaps earlier writer"
        );
        let palette = read_4bpp_palette_words_in_prefix(source, tim, 0)?;
        ensure!(
            palette[usize::from(e.red_index)] & 0x7fff == 0x001f
                && palette[usize::from(e.white_index)] & 0x7fff == 0x7fff,
            "badge palette changed"
        );
        let background = &plan.backgrounds[&(e.cell.width, e.cell.height)];
        let mut pixels = background
            .iter()
            .map(|v| if *v == 0 { e.white_index } else { e.red_index })
            .collect::<Vec<_>>();
        for (line, text) in plan.manifest.korean_lines.iter().enumerate() {
            let raster = rasterizer.rasterize_shifted(
                text,
                e.cell.width,
                9,
                style.font_px,
                0.0,
                0,
                0,
                None,
                1,
                HorizontalTextAlignment::Center,
            )?;
            for (i, v) in raster.pixels.iter().enumerate() {
                if *v == 1 {
                    let dest = line * 9 * e.cell.width + i;
                    ensure!(
                        background[dest] == 1,
                        "badge Korean lettering leaves red silhouette"
                    );
                    pixels[dest] = e.white_index;
                }
            }
        }
        let write = write_indexed_cell_in_prefix_with_report(patched, tim, e.cell, &pixels)?;
        ensure!(write.changed_byte_count > 0, "badge changed no bytes");
        allowed.extend(write.allowed_ranges);
    }
    Ok(Report {
        manifest_sha256: plan.sha256.clone(),
        imagegen_sha256: plan.manifest.imagegen_sha256.clone(),
        authored_occurrence_count: plan.manifest.entries.len(),
        members: plan
            .manifest
            .entries
            .iter()
            .map(|e| e.member_index)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        font_sha256: sha256_bytes(&std::fs::read(&style.path)?),
        font_px: style.font_px,
        runtime_verified: false,
    })
}
