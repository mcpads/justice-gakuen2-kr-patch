//! Palette-indexed retry panels; frame and non-lettering pixels remain source-owned.
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

use super::catalog::{ModeDescendantRecordSpec, ModeDescendantTextureOutputSpec};
use super::model::{ModeDescendantRecord, ModeDescendantStorageKind, ModeDescendantSurface};
use super::record_compositor::{ModeDescendantRecordDraft, source_for_spec};
use super::source::ModeDescendantSourceRecord;
use crate::decoded_record_write_plan::DecodedDataClaim;
use crate::pipeline::{difference_ranges, sha256_bytes};
use crate::tim::{Cell, read_indexed_cell_in_prefix, write_indexed_cell_in_prefix_with_report};

pub(super) const SPECS: [ModeDescendantRecordSpec; 3] = [
    ModeDescendantRecordSpec {
        record: ModeDescendantRecord::GorinBallGameEffects,
        surface: ModeDescendantSurface::GorinGameplayHud,
        source_path: "DAT2/CEFT5.BIZ",
        storage_kind: ModeDescendantStorageKind::PagedCompressed,
        output_file: "ceft5-gorin-gameplay.biz",
        texture_outputs: &[ModeDescendantTextureOutputSpec {
            tim_offset: 0x10800,
            preview_file: "ceft5-gorin-gameplay.png",
        }],
    },
    ModeDescendantRecordSpec {
        record: ModeDescendantRecord::GorinSprintEffects,
        surface: ModeDescendantSurface::GorinGameplayHud,
        source_path: "DAT2/SPRINT.BIZ",
        storage_kind: ModeDescendantStorageKind::PagedCompressed,
        output_file: "sprint-gorin-gameplay.biz",
        texture_outputs: &[ModeDescendantTextureOutputSpec {
            tim_offset: 0,
            preview_file: "sprint-gorin-gameplay.png",
        }],
    },
    ModeDescendantRecordSpec {
        record: ModeDescendantRecord::GorinDanceEffects,
        surface: ModeDescendantSurface::GorinGameplayHud,
        source_path: "DAT2/DANCE.BIZ",
        storage_kind: ModeDescendantStorageKind::PagedCompressed,
        output_file: "dance-gorin-gameplay.biz",
        texture_outputs: &[ModeDescendantTextureOutputSpec {
            tim_offset: 0,
            preview_file: "dance-gorin-gameplay.png",
        }],
    },
];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Family {
    artworks: Vec<Artwork>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Artwork {
    id: String,
    source_text: String,
    korean_text: String,
    indices_file: PathBuf,
    indices_sha256: String,
    source_indexed_sha256: String,
    palette_index: usize,
    source_palette_sha256: String,
    imagegen_sha256: String,
    dotmend_art_id: String,
}
#[derive(Debug, Serialize)]
pub struct RetryPanelReport {
    pub source_path: String,
    pub source_text: String,
    pub korean_text: String,
    pub indices_sha256: String,
    pub palette_index: usize,
    pub source_palette_sha256: String,
    pub imagegen_sha256: String,
    pub dotmend_art_id: String,
    pub frame_and_unowned_pixels_preserved: bool,
    pub runtime_verified: bool,
}
#[derive(Debug, Serialize)]
pub struct GorinRetryReport {
    pub manifest_sha256: String,
    pub panels: Vec<RetryPanelReport>,
}

pub(super) fn apply(
    path: &Path,
    sources: &[ModeDescendantSourceRecord],
    drafts: &mut Vec<ModeDescendantRecordDraft>,
) -> Result<GorinRetryReport> {
    let bytes = std::fs::read(path)?;
    let family: Family = serde_json::from_slice(&bytes)?;
    ensure!(
        family.artworks.len() == SPECS.len(),
        "retry family must cover its three physical panels"
    );
    let mut panels = Vec::new();
    for (spec, (id, x, y)) in
        SPECS
            .iter()
            .zip([("ball", 656, 180), ("sprint", 0, 68), ("dance", 256, 82)])
    {
        let matching = family
            .artworks
            .iter()
            .filter(|a| a.id == id)
            .collect::<Vec<_>>();
        ensure!(matching.len() == 1, "missing or repeated retry panel {id}");
        let art = matching[0];
        ensure!(
            art.source_text == "もう一回する？ / はい / いいえ"
                && art.korean_text == "다시 할까? / 예 / 아니요",
            "retry panel wording changed"
        );
        ensure!(
            art.imagegen_sha256.len() == 64
                && art.imagegen_sha256.bytes().all(|b| b.is_ascii_hexdigit())
                && art.dotmend_art_id.starts_with("art_"),
            "retry artwork provenance missing"
        );
        let pixels = std::fs::read(&art.indices_file)
            .with_context(|| format!("retry artwork missing: {}", art.indices_file.display()))?;
        ensure!(
            sha256_bytes(&pixels) == art.indices_sha256,
            "retry artwork changed"
        );
        let source = source_for_spec(sources, spec)?;
        ensure!(
            art.palette_index == if id == "ball" { 49 } else { 21 },
            "retry panel selected a palette outside its bound consumer"
        );
        let offset = spec.texture_outputs[0].tim_offset;
        let palette = offset + 20 + art.palette_index * 32;
        ensure!(
            sha256_bytes(
                source
                    .decoded
                    .get(palette..palette + 32)
                    .context("retry CLUT missing")?
            ) == art.source_palette_sha256,
            "retry bound CLUT changed"
        );
        let cell = Cell {
            x,
            y,
            width: 112,
            height: 60,
        };
        let original = read_indexed_cell_in_prefix(&source.decoded, offset, cell)?;
        ensure!(
            sha256_bytes(&original) == art.source_indexed_sha256,
            "retry source panel changed"
        );
        validate_pixels(&original, &pixels)?;
        let mut decoded = source.decoded.clone();
        write_indexed_cell_in_prefix_with_report(&mut decoded, offset, cell, &pixels)?;
        ensure!(
            read_indexed_cell_in_prefix(&decoded, offset, cell)? == pixels,
            "retry index readback differs"
        );
        let claims = DecodedDataClaim::from_ranges(
            &format!("gorin-{id}-retry"),
            "Korean retry question and answers with original frame",
            difference_ranges(&source.decoded, &decoded),
        );
        drafts.push(ModeDescendantRecordDraft {
            spec,
            decoded,
            decoded_write_claims: claims,
        });
        panels.push(RetryPanelReport {
            source_path: spec.source_path.into(),
            source_text: art.source_text.clone(),
            korean_text: art.korean_text.clone(),
            indices_sha256: art.indices_sha256.clone(),
            palette_index: art.palette_index,
            source_palette_sha256: art.source_palette_sha256.clone(),
            imagegen_sha256: art.imagegen_sha256.clone(),
            dotmend_art_id: art.dotmend_art_id.clone(),
            frame_and_unowned_pixels_preserved: true,
            runtime_verified: false,
        });
    }
    Ok(GorinRetryReport {
        manifest_sha256: sha256_bytes(&bytes),
        panels,
    })
}

fn validate_pixels(original: &[u8], pixels: &[u8]) -> Result<()> {
    ensure!(
        original.len() == 112 * 60 && pixels.len() == original.len(),
        "retry canvas changed dimensions"
    );
    ensure!(
        pixels.iter().all(|p| *p < 16),
        "retry pixel leaves 4-bpp palette"
    );
    ensure!(
        original != pixels,
        "retry artwork did not change Japanese source"
    );
    for (i, (before, after)) in original.iter().zip(pixels).enumerate() {
        let (x, y) = (i % 112, i / 112);
        let writable = [(10, 8, 94, 22), (18, 30, 35, 22), (58, 30, 44, 22)]
            .iter()
            .any(|&(rx, ry, w, h)| x >= rx && x < rx + w && y >= ry && y < ry + h);
        if writable {
            let allowed = if y < 30 {
                matches!(*after, 0..=7 | 15)
            } else {
                matches!(*after, 0 | 1 | 8..=12 | 15)
            };
            ensure!(
                allowed,
                "retry question/answer palette ramp changed at {x},{y}"
            );
        }
        ensure!(
            writable || before == after,
            "retry artwork changed protected frame or background at {x},{y}"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate_pixels;
    #[test]
    fn retry_answers_keep_their_separate_palette_ramp() {
        let original = vec![0; 112 * 60];
        let mut candidate = original.clone();
        candidate[36 * 112 + 24] = 12;
        assert!(validate_pixels(&original, &candidate).is_ok());
        candidate[36 * 112 + 24] = 7;
        assert!(validate_pixels(&original, &candidate).is_err());
        candidate[36 * 112 + 24] = 12;
        candidate[12 * 112 + 24] = 12;
        assert!(validate_pixels(&original, &candidate).is_err());
    }
    #[test]
    fn retry_artwork_cannot_erase_its_frame_or_leave_the_palette() {
        let original = vec![0; 112 * 60];
        let mut candidate = original.clone();
        candidate[12 * 112 + 12] = 1;
        assert!(validate_pixels(&original, &candidate).is_ok());
        candidate[0] = 1;
        assert!(validate_pixels(&original, &candidate).is_err());
        candidate[0] = 0;
        candidate[12 * 112 + 12] = 16;
        assert!(validate_pixels(&original, &candidate).is_err());
    }
}
