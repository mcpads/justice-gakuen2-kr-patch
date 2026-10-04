//! Eight introduction strips composed into the existing DANCE texture draft.
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

use super::model::ModeDescendantRecord;
use super::record_compositor::ModeDescendantRecordDraft;
use crate::decoded_record_write_plan::DecodedDataClaim;
use crate::pipeline::{difference_ranges, sha256_bytes};
use crate::tim::{Cell, read_indexed_cell_in_prefix, write_indexed_cell_in_prefix_with_report};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Artwork {
    source_indexed_sha256: String,
    indices_file: PathBuf,
    indices_sha256: String,
    imagegen_sha256: String,
    dotmend_art_id: String,
    lines: Vec<Line>,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Line {
    source_text: String,
    korean_text: String,
    source_indexed_sha256: String,
}
#[derive(Debug, Serialize)]
pub struct DanceIntroReport {
    pub manifest_sha256: String,
    pub indices_sha256: String,
    pub imagegen_sha256: String,
    pub dotmend_art_id: String,
    pub line_count: usize,
    pub source_strips_verified: bool,
    pub retry_panel_and_other_pixels_preserved: bool,
    pub runtime_verified: bool,
}

pub(super) fn apply(
    path: &Path,
    drafts: &mut [ModeDescendantRecordDraft],
) -> Result<DanceIntroReport> {
    let bytes = std::fs::read(path)?;
    let art: Artwork = serde_json::from_slice(&bytes)?;
    ensure!(
        art.lines.len() == 8,
        "dance intro must contain all eight source strips"
    );
    ensure!(
        art.lines
            .iter()
            .all(|l| !l.source_text.trim().is_empty() && !l.korean_text.trim().is_empty()),
        "dance intro wording is missing"
    );
    ensure!(
        art.imagegen_sha256.len() == 64
            && art.imagegen_sha256.bytes().all(|b| b.is_ascii_hexdigit())
            && art.dotmend_art_id.starts_with("art_"),
        "dance intro provenance is missing"
    );
    let pixels = std::fs::read(&art.indices_file).context("dance intro indexed artwork missing")?;
    ensure!(
        pixels.len() == 256 * 192 && pixels.iter().all(|p| *p < 16),
        "dance intro indexed canvas changed"
    );
    ensure!(
        sha256_bytes(&pixels) == art.indices_sha256,
        "dance intro artwork changed"
    );
    let draft = drafts
        .iter_mut()
        .find(|d| d.spec.record == ModeDescendantRecord::GorinDanceEffects)
        .context("dance intro requires the shared DANCE texture composition")?;
    let cell = Cell {
        x: 512,
        y: 0,
        width: 256,
        height: 192,
    };
    let original = read_indexed_cell_in_prefix(&draft.decoded, 0, cell)?;
    ensure!(
        sha256_bytes(&original) == art.source_indexed_sha256,
        "dance intro source changed or was already written"
    );
    for (index, line) in art.lines.iter().enumerate() {
        let row = index * 256 * 24..(index + 1) * 256 * 24;
        ensure!(
            sha256_bytes(&original[row.clone()]) == line.source_indexed_sha256,
            "dance intro source strip {index} changed"
        );
        ensure!(
            pixels[row.start..row.start + 256]
                .iter()
                .chain(pixels[row.end - 256..row.end].iter())
                .all(|p| *p == 0),
            "dance intro strip {index} lost its separator rows"
        );
        ensure!(
            pixels[row.clone()].iter().any(|p| *p != 0) && pixels[row.clone()] != original[row],
            "dance intro strip {index} is blank or untranslated"
        );
    }
    let before = draft.decoded.clone();
    write_indexed_cell_in_prefix_with_report(&mut draft.decoded, 0, cell, &pixels)?;
    ensure!(
        read_indexed_cell_in_prefix(&draft.decoded, 0, cell)? == pixels,
        "dance intro readback differs"
    );
    draft
        .decoded_write_claims
        .extend(DecodedDataClaim::from_ranges(
            "gorin-dance-intro",
            "Korean introduction lettering in eight original strips",
            difference_ranges(&before, &draft.decoded),
        ));
    Ok(DanceIntroReport {
        manifest_sha256: sha256_bytes(&bytes),
        indices_sha256: art.indices_sha256,
        imagegen_sha256: art.imagegen_sha256,
        dotmend_art_id: art.dotmend_art_id,
        line_count: art.lines.len(),
        source_strips_verified: true,
        retry_panel_and_other_pixels_preserved: true,
        runtime_verified: false,
    })
}
