//! Home-run announcement with the original two independently cycling color ramps.
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
    source_text: String,
    korean_text: String,
    source_indexed_sha256: String,
    indices_file: PathBuf,
    indices_sha256: String,
    imagegen_sha256: String,
    dotmend_art_id: String,
}

#[derive(Debug, Serialize)]
pub struct HomeRunReport {
    pub manifest_sha256: String,
    pub indices_sha256: String,
    pub cell: Cell,
    pub source_palettes_preserved: bool,
    pub runtime_verified: bool,
}

fn validate_pixels(pixels: &[u8]) -> Result<()> {
    ensure!(pixels.len() == 216 * 48, "home-run native extent changed");
    for (offset, &index) in pixels.iter().enumerate() {
        let x = offset % 216;
        let y = offset / 216;
        ensure!(
            index <= 1
                || if x < 98 {
                    (9..=15).contains(&index)
                } else {
                    (2..=8).contains(&index)
                },
            "home-run lettering crosses source color roles"
        );
        if x == 0 || x == 215 || x == 97 || x == 98 || y == 0 || y == 47 {
            ensure!(
                index == 0,
                "home-run outer boundary or syllable divider is not clear"
            );
        }
    }
    ensure!(
        pixels.contains(&15) && pixels.contains(&2),
        "home-run face missing"
    );
    Ok(())
}

pub(super) fn apply(
    path: &Path,
    drafts: &mut [ModeDescendantRecordDraft],
) -> Result<HomeRunReport> {
    let bytes = std::fs::read(path)?;
    let art: Artwork = serde_json::from_slice(&bytes)?;
    ensure!(
        art.source_text == "HOME RUN!" && art.korean_text == "홈런!",
        "home-run wording changed"
    );
    ensure!(
        art.imagegen_sha256.len() == 64
            && art.imagegen_sha256.bytes().all(|b| b.is_ascii_hexdigit())
            && art.dotmend_art_id.starts_with("art_"),
        "home-run provenance missing"
    );
    let pixels = std::fs::read(&art.indices_file)?;
    ensure!(
        sha256_bytes(&pixels) == art.indices_sha256,
        "home-run artwork changed"
    );
    validate_pixels(&pixels)?;
    let draft = drafts
        .iter_mut()
        .find(|d| d.spec.record == ModeDescendantRecord::GorinBallGameEffects)
        .context("home-run lettering requires the shared CEFT5 composition")?;
    let cell = Cell {
        x: 0,
        y: 88,
        width: 216,
        height: 48,
    };
    let original = read_indexed_cell_in_prefix(&draft.decoded, 0x10800, cell)?;
    ensure!(
        sha256_bytes(&original) == art.source_indexed_sha256
            && art.source_indexed_sha256
                == "c2d130efd39bac6218b13496c3fba476b9351354715d30a321b65f91da8c62c3",
        "home-run source changed or already owned"
    );
    let before = draft.decoded.clone();
    write_indexed_cell_in_prefix_with_report(&mut draft.decoded, 0x10800, cell, &pixels)?;
    ensure!(
        read_indexed_cell_in_prefix(&draft.decoded, 0x10800, cell)? == pixels,
        "home-run readback differs"
    );
    ensure!(
        before[0x10814..0x11014] == draft.decoded[0x10814..0x11014],
        "home-run palette changed"
    );
    draft
        .decoded_write_claims
        .extend(DecodedDataClaim::from_ranges(
            "gorin-home-run",
            "Korean home-run quad, original cycling palettes retained",
            difference_ranges(&before, &draft.decoded),
        ));
    Ok(HomeRunReport {
        manifest_sha256: sha256_bytes(&bytes),
        indices_sha256: art.indices_sha256,
        cell,
        source_palettes_preserved: true,
        runtime_verified: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn home_run_preserves_independent_color_roles() {
        let mut pixels = vec![0; 216 * 48];
        pixels[216 + 20] = 15;
        pixels[216 + 120] = 2;
        assert!(validate_pixels(&pixels).is_ok());
        pixels[216 + 120] = 15;
        assert!(validate_pixels(&pixels).is_err());
        pixels[216 + 120] = 2;
        pixels[216 + 97] = 15;
        assert!(validate_pixels(&pixels).is_err());
    }
}
