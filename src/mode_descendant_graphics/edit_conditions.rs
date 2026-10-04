//! Isolated command qualifiers; mixed button rows and oval badges are not write cells.
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
use std::{collections::BTreeSet, path::Path};

#[derive(Debug)]
pub(super) struct Plan {
    sha256: String,
    entries: Vec<Entry>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    entries: Vec<Entry>,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    id: String,
    member_index: usize,
    source_text: String,
    korean_text: String,
    cell: Cell,
    source_indexed_sha256: String,
    background_index: u8,
    fill_index: u8,
}
#[derive(Debug, Serialize)]
pub struct Report {
    pub manifest_sha256: String,
    pub authored_occurrence_count: usize,
    pub members: Vec<usize>,
    pub font_sha256: String,
    pub font_px: f32,
    pub runtime_verified: bool,
}
pub(super) fn load(path: &Path) -> Result<Plan> {
    let bytes = std::fs::read(path)?;
    let manifest: Manifest = serde_json::from_slice(&bytes)?;
    let mut ids = BTreeSet::new();
    for (i, e) in manifest.entries.iter().enumerate() {
        ensure!(ids.insert(&e.id), "duplicate EDIT condition");
        ensure!(
            e.member_index < 32
                && e.cell.x >= 284
                && e.cell.width > 0
                && e.cell.height > 0
                && e.cell.x.checked_add(e.cell.width).is_some_and(|v| v <= 409)
                && e.cell
                    .y
                    .checked_add(e.cell.height)
                    .is_some_and(|v| v <= 320),
            "EDIT condition leaves command panel"
        );
        ensure!(
            matches!(
                (e.source_text.as_str(), e.korean_text.as_str()),
                ("空中で", "공중")
                    | ("連打", "연타")
                    | ("ため", "모으기")
                    | ("連続入力可", "연속입력가능")
                    | ("後", "후")
            ),
            "unreviewed EDIT condition wording"
        );
        ensure!(
            e.fill_index < 16 && e.background_index < 16 && e.fill_index != e.background_index,
            "invalid EDIT condition palette roles"
        );
        for other in &manifest.entries[..i] {
            ensure!(
                e.member_index != other.member_index || !cells_overlap(e.cell, other.cell),
                "overlapping EDIT conditions"
            );
        }
    }
    ensure!(!manifest.entries.is_empty(), "empty EDIT condition plan");
    Ok(Plan {
        sha256: sha256_bytes(&bytes),
        entries: manifest.entries,
    })
}
pub(super) fn apply(
    plan: &Plan,
    config: &ModeDescendantGraphicsBuildConfig,
    source: &[u8],
    patched: &mut [u8],
    allowed_ranges: &mut Vec<[usize; 2]>,
) -> Result<Report> {
    let style = config
        .fonts
        .edit_condition
        .as_ref()
        .context("EDIT conditions require their font role")?;
    let mut rasterizers = IndexedTextRasterizers::default();
    let rasterizer = rasterizers.for_font(&style.path)?;
    for e in &plan.entries {
        let tim = e.member_index * 0x2b800;
        let original = read_indexed_cell_in_prefix(source, tim, e.cell)?;
        ensure!(
            sha256_bytes(&original) == e.source_indexed_sha256,
            "EDIT condition {} source changed",
            e.id
        );
        ensure!(
            original == read_indexed_cell_in_prefix(patched, tim, e.cell)?,
            "EDIT condition overlaps earlier writer"
        );
        let palette = read_4bpp_palette_words_in_prefix(source, tim, 0)?;
        ensure!(
            palette[usize::from(e.fill_index)] & 0x7fff == 0x001f
                && palette[usize::from(e.background_index)] & 0x7fff == 0x7fff,
            "EDIT condition palette changed"
        );
        let raster = rasterizer
            .rasterize_shifted(
                &e.korean_text,
                e.cell.width,
                e.cell.height,
                style.font_px,
                0.0,
                -1,
                e.background_index,
                None,
                e.fill_index,
                HorizontalTextAlignment::Center,
            )
            .with_context(|| format!("EDIT condition {} layout", e.id))?;
        let write = write_indexed_cell_in_prefix_with_report(patched, tim, e.cell, &raster.pixels)?;
        ensure!(
            write.changed_byte_count > 0,
            "EDIT condition changed no bytes"
        );
        allowed_ranges.extend(write.allowed_ranges);
    }
    Ok(Report {
        manifest_sha256: plan.sha256.clone(),
        authored_occurrence_count: plan.entries.len(),
        members: plan
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
