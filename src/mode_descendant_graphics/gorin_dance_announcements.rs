//! Native-size ready/finish and performance judgment lettering in the shared DANCE composition.
use super::model::ModeDescendantRecord;
use super::record_compositor::ModeDescendantRecordDraft;
use crate::decoded_record_write_plan::DecodedDataClaim;
use crate::pipeline::{difference_ranges, sha256_bytes};
use crate::tim::{Cell, read_indexed_cell_in_prefix, write_indexed_cell_in_prefix_with_report};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    imagegen_sha256: String,
    palette_sha256: String,
    judgment_imagegen_sha256: String,
    judgment_palette_sha256: Vec<String>,
    entries: Vec<Artwork>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Artwork {
    id: String,
    source_text: String,
    korean_text: String,
    source_indexed_sha256: String,
    indices_file: PathBuf,
    indices_sha256: String,
    dotmend_art_id: String,
}
#[derive(Debug, Serialize)]
pub struct DanceAnnouncementReport {
    pub manifest_sha256: String,
    pub indices_sha256: Vec<String>,
    pub source_palettes_preserved: bool,
    pub runtime_verified: bool,
}
pub(super) fn apply(
    path: &Path,
    drafts: &mut [ModeDescendantRecordDraft],
) -> Result<DanceAnnouncementReport> {
    let bytes = std::fs::read(path)?;
    let m: Manifest = serde_json::from_slice(&bytes)?;
    for hash in [&m.imagegen_sha256, &m.judgment_imagegen_sha256] {
        ensure!(
            hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()),
            "dance generation provenance missing"
        );
    }
    let draft = drafts
        .iter_mut()
        .find(|d| d.spec.record == ModeDescendantRecord::GorinDanceEffects)
        .context("dance announcements require shared DANCE composition")?;
    ensure!(
        sha256_bytes(&draft.decoded[20 + 11 * 32..20 + 12 * 32]) == m.palette_sha256,
        "dance announcement CLUT changed"
    );
    ensure!(
        m.judgment_palette_sha256.len() == 6,
        "dance judgment palette coverage incomplete"
    );
    for (palette, hash) in [32, 33, 28, 29, 30, 31]
        .into_iter()
        .zip(&m.judgment_palette_sha256)
    {
        ensure!(
            sha256_bytes(&draft.decoded[20 + palette * 32..20 + (palette + 1) * 32]) == *hash,
            "dance judgment palette changed"
        );
    }
    let mut seen = BTreeSet::new();
    let mut hashes = Vec::new();
    for art in m.entries {
        ensure!(seen.insert(art.id.clone()), "duplicate dance announcement");
        let (source, korean, x, y, width, height) = match art.id.as_str() {
            "ready" => ("READY!", "준비!", 0, 104, 200, 64),
            "finish" => ("FINISH!", "종료!", 0, 168, 200, 64),
            "great" => ("GREAT!!", "최고!!", 400, 80, 112, 24),
            "good" => ("GOOD!", "좋아!", 400, 104, 112, 24),
            "bad" => ("BAD", "아쉬워", 400, 128, 112, 24),
            _ => anyhow::bail!("unbound dance announcement"),
        };
        ensure!(
            art.source_text == source && art.korean_text == korean,
            "dance wording changed"
        );
        ensure!(
            art.dotmend_art_id.starts_with("art_"),
            "dance export provenance missing"
        );
        let cell = Cell {
            x,
            y,
            width,
            height,
        };
        let original = read_indexed_cell_in_prefix(&draft.decoded, 0, cell)?;
        ensure!(
            sha256_bytes(&original) == art.source_indexed_sha256,
            "dance source cell changed or already owned"
        );
        let pixels = std::fs::read(&art.indices_file)?;
        ensure!(
            sha256_bytes(&pixels) == art.indices_sha256,
            "dance artwork changed"
        );
        ensure!(
            pixels.len() == width * height && pixels.iter().all(|p| *p < 16),
            "dance canvas changed"
        );
        for (i, p) in pixels.iter().enumerate() {
            if i % width == 0 || i % width == width - 1 || i / width == 0 || i / width == height - 1
            {
                ensure!(*p == 0, "dance announcement edge is not clear");
            }
        }
        let before = draft.decoded.clone();
        write_indexed_cell_in_prefix_with_report(&mut draft.decoded, 0, cell, &pixels)?;
        ensure!(
            read_indexed_cell_in_prefix(&draft.decoded, 0, cell)? == pixels,
            "dance readback differs"
        );
        ensure!(
            before[20..20 + 64 * 32] == draft.decoded[20..20 + 64 * 32],
            "dance palette write forbidden"
        );
        draft
            .decoded_write_claims
            .extend(DecodedDataClaim::from_ranges(
                &format!("dance-{}", art.id),
                "Korean dance announcement within original sprite extent",
                difference_ranges(&before, &draft.decoded),
            ));
        hashes.push(art.indices_sha256);
    }
    ensure!(seen.len() == 5, "dance announcement coverage incomplete");
    Ok(DanceAnnouncementReport {
        manifest_sha256: sha256_bytes(&bytes),
        indices_sha256: hashes,
        source_palettes_preserved: true,
        runtime_verified: false,
    })
}
