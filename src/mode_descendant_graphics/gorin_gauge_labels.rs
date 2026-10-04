//! Lettering on the original slanted gauge strips, retaining number space.
use super::{model::ModeDescendantRecord, record_compositor::ModeDescendantRecordDraft};
use crate::decoded_record_write_plan::DecodedDataClaim;
use crate::pipeline::{difference_ranges, sha256_bytes};
use crate::tim::{Cell, read_indexed_cell_in_prefix, write_indexed_cell_in_prefix_with_report};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    labels: Vec<Artwork>,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Artwork {
    id: String,
    source_text: String,
    korean_text: String,
    source_indexed_sha256: String,
    indices_file: PathBuf,
    indices_sha256: String,
    imagegen_sha256: String,
    dotmend_art_id: String,
}
#[derive(Debug, Serialize)]
pub struct GaugeLabelReport {
    pub manifest_sha256: String,
    pub label_indices_sha256: Vec<String>,
    pub borders_and_number_space_preserved: bool,
    pub runtime_verified: bool,
}
fn lettering_pixel(id: &str, x: usize, y: usize) -> bool {
    y < 14 && ((6..54).contains(&x) || (id == "remaining" && x >= 104))
}
fn validate_pixels(id: &str, width: usize, original: &[u8], pixels: &[u8]) -> Result<()> {
    ensure!(
        pixels.len() == width * 16 && pixels.iter().all(|p| *p < 16),
        "invalid gauge canvas"
    );
    for (i, (&before, &after)) in original.iter().zip(pixels).enumerate() {
        if !lettering_pixel(id, i % width, i / width) {
            ensure!(before == after, "gauge border or number space changed");
        }
    }
    ensure!(
        pixels != original && pixels.iter().any(|p| *p >= 6),
        "gauge lettering is missing"
    );
    Ok(())
}
pub(super) fn apply(
    path: &Path,
    drafts: &mut [ModeDescendantRecordDraft],
) -> Result<GaugeLabelReport> {
    let bytes = std::fs::read(path)?;
    let manifest: Manifest = serde_json::from_slice(&bytes)?;
    let draft = drafts
        .iter_mut()
        .find(|d| d.spec.record == ModeDescendantRecord::GorinBallGameEffects)
        .context("gauge labels require the shared CEFT5 composition")?;
    let mut seen = std::collections::BTreeSet::new();
    let mut hashes = Vec::new();
    for art in manifest.labels {
        ensure!(seen.insert(art.id.clone()), "duplicate gauge label");
        let (y, width, jp, ko) = match art.id.as_str() {
            "guts" => (56, 88, "根性", "근성"),
            "remaining" => (72, 128, "あと … 球", "남은 … 구"),
            _ => anyhow::bail!("unbound gauge label"),
        };
        ensure!(
            art.source_text == jp && art.korean_text == ko,
            "gauge wording changed"
        );
        ensure!(
            art.imagegen_sha256.len() == 64
                && art.imagegen_sha256.bytes().all(|b| b.is_ascii_hexdigit())
                && art.dotmend_art_id.starts_with("art_"),
            "gauge provenance missing"
        );
        let pixels = std::fs::read(&art.indices_file)?;
        ensure!(
            sha256_bytes(&pixels) == art.indices_sha256,
            "gauge artwork changed"
        );
        let cell = Cell {
            x: 0,
            y,
            width,
            height: 16,
        };
        let original = read_indexed_cell_in_prefix(&draft.decoded, 0x10800, cell)?;
        ensure!(
            sha256_bytes(&original) == art.source_indexed_sha256,
            "gauge source changed or already owned"
        );
        validate_pixels(&art.id, width, &original, &pixels)?;
        let before = draft.decoded.clone();
        write_indexed_cell_in_prefix_with_report(&mut draft.decoded, 0x10800, cell, &pixels)?;
        ensure!(
            read_indexed_cell_in_prefix(&draft.decoded, 0x10800, cell)? == pixels,
            "gauge readback differs"
        );
        draft
            .decoded_write_claims
            .extend(DecodedDataClaim::from_ranges(
                &format!("gorin-gauge-{}", art.id),
                "Korean gauge lettering with preserved strip and number space",
                difference_ranges(&before, &draft.decoded),
            ));
        hashes.push(art.indices_sha256);
    }
    ensure!(seen.len() == 2, "gauge label coverage incomplete");
    Ok(GaugeLabelReport {
        manifest_sha256: sha256_bytes(&bytes),
        label_indices_sha256: hashes,
        borders_and_number_space_preserved: true,
        runtime_verified: false,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gauge_art_cannot_overwrite_number_space_or_border() {
        let source = vec![0; 128 * 16];
        let mut pixels = source.clone();
        pixels[6] = 15;
        assert!(validate_pixels("remaining", 128, &source, &pixels).is_ok());
        pixels[80] = 15;
        assert!(validate_pixels("remaining", 128, &source, &pixels).is_err());
        pixels[80] = 0;
        pixels[15 * 128 + 6] = 15;
        assert!(validate_pixels("remaining", 128, &source, &pixels).is_err());
    }
}
