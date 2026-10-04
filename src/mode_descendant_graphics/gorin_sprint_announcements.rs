//! Start/false-start and shared result lettering in the SPRINT and CEFT5 compositions.
use std::collections::BTreeSet;
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
struct Manifest {
    imagegen_sha256: String,
    palette_sha256: String,
    result_imagegen_sha256: String,
    result_palette_sha256: String,
    entries: Vec<Artwork>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Artwork {
    id: String,
    source_text: String,
    korean_lines: Vec<String>,
    source_indexed_sha256: String,
    indices_file: PathBuf,
    indices_sha256: String,
    dotmend_art_id: String,
}

#[derive(Debug, Serialize)]
pub struct SprintAnnouncementReport {
    pub manifest_sha256: String,
    pub indices_sha256: Vec<String>,
    pub source_palettes_preserved: bool,
    pub runtime_verified: bool,
}

pub(super) fn apply(
    path: &Path,
    drafts: &mut [ModeDescendantRecordDraft],
) -> Result<SprintAnnouncementReport> {
    let bytes = std::fs::read(path)?;
    let manifest: Manifest = serde_json::from_slice(&bytes)?;
    for hash in [&manifest.imagegen_sha256, &manifest.result_imagegen_sha256] {
        ensure!(
            hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()),
            "sprint announcement generation provenance missing"
        );
    }
    let mut seen = BTreeSet::new();
    let mut hashes = Vec::new();
    for art in manifest.entries {
        ensure!(seen.insert(art.id.clone()), "duplicate sprint announcement");
        let ball = art.id.starts_with("ball-");
        let record = if ball {
            ModeDescendantRecord::GorinBallGameEffects
        } else {
            ModeDescendantRecord::GorinSprintEffects
        };
        let draft = drafts
            .iter_mut()
            .find(|d| d.spec.record == record)
            .context("announcements require the matching shared texture composition")?;
        let tim_offset = if ball { 0x10800 } else { 0 };
        let palette_count = if ball { 64 } else { 24 };
        let result_palette = if ball { 47 } else { 19 };
        ensure!(
            sha256_bytes(
                &draft.decoded[tim_offset + 20 + result_palette * 32
                    ..tim_offset + 20 + (result_palette + 1) * 32]
            ) == manifest.result_palette_sha256,
            "result CLUT changed"
        );
        if !ball {
            ensure!(
                sha256_bytes(&draft.decoded[20 + 17 * 32..20 + 18 * 32]) == manifest.palette_sha256,
                "sprint announcement CLUT changed"
            );
        }
        let (text, lines, x, y, width, height, source_sha) = match art.id.as_str() {
            "start" => (
                "START",
                vec!["시작"],
                384,
                88,
                127,
                80,
                "b8ebc31a4ab2d81a8b8fd739e6be1150510aedb2f8b9d3c3693ad42d9a379639",
            ),
            "false-start" => (
                "FLYING",
                vec!["부정", "출발"],
                384,
                168,
                127,
                80,
                "7d8945922d76551a1cee399c365025ad916f78815ac075db19b50aacb4e3f1f2",
            ),
            "win" => (
                "WIN",
                vec!["승리"],
                104,
                128,
                56,
                24,
                "f3ac87174e0c4579d18ead303dfddf15fa834ffcefe0b9e46c69a126f414dad6",
            ),
            "lose" => (
                "LOSE",
                vec!["패배"],
                160,
                128,
                80,
                24,
                "d1a23eee7670fb12a1ddbbe9ba10950e74a6db37a86fd4c9d882be74609dcc26",
            ),
            "ball-win" => (
                "WIN",
                vec!["승리"],
                592,
                232,
                56,
                24,
                "f3ac87174e0c4579d18ead303dfddf15fa834ffcefe0b9e46c69a126f414dad6",
            ),
            "ball-lose" => (
                "LOSE",
                vec!["패배"],
                576,
                208,
                80,
                24,
                "d1a23eee7670fb12a1ddbbe9ba10950e74a6db37a86fd4c9d882be74609dcc26",
            ),
            _ => anyhow::bail!("unbound sprint announcement"),
        };
        ensure!(
            art.source_text == text && art.korean_lines == lines,
            "sprint announcement wording changed"
        );
        ensure!(
            art.dotmend_art_id.starts_with("art_"),
            "sprint announcement export provenance missing"
        );
        let cell = Cell {
            x,
            y,
            width,
            height,
        };
        let original = read_indexed_cell_in_prefix(&draft.decoded, tim_offset, cell)?;
        ensure!(
            sha256_bytes(&original) == source_sha && art.source_indexed_sha256 == source_sha,
            "sprint announcement source changed or already owned"
        );
        let pixels = std::fs::read(&art.indices_file)?;
        ensure!(
            sha256_bytes(&pixels) == art.indices_sha256,
            "sprint announcement artwork changed"
        );
        ensure!(
            pixels.len() == width * height && pixels.iter().all(|p| *p < 16),
            "sprint announcement canvas changed"
        );
        for (i, index) in pixels.iter().enumerate() {
            if i % width == 0 || i % width == width - 1 || i / width == 0 || i / width == height - 1
            {
                ensure!(*index == 0, "sprint announcement edge is not clear");
            }
        }
        let before = draft.decoded.clone();
        write_indexed_cell_in_prefix_with_report(&mut draft.decoded, tim_offset, cell, &pixels)?;
        ensure!(
            read_indexed_cell_in_prefix(&draft.decoded, tim_offset, cell)? == pixels,
            "sprint announcement readback differs"
        );
        ensure!(
            before[tim_offset + 20..tim_offset + 20 + palette_count * 32]
                == draft.decoded[tim_offset + 20..tim_offset + 20 + palette_count * 32],
            "sprint palette write forbidden"
        );
        draft
            .decoded_write_claims
            .extend(DecodedDataClaim::from_ranges(
                &format!("sprint-{}", art.id),
                "Korean sprint announcement in original native sprite extent",
                difference_ranges(&before, &draft.decoded),
            ));
        hashes.push(art.indices_sha256);
    }
    ensure!(seen.len() == 6, "sprint announcement coverage incomplete");
    Ok(SprintAnnouncementReport {
        manifest_sha256: sha256_bytes(&bytes),
        indices_sha256: hashes,
        source_palettes_preserved: true,
        runtime_verified: false,
    })
}
