use std::path::Path;

use anyhow::{Context, Result, ensure};
use serde::Deserialize;

use crate::decoded_record_write_plan::{DecodedDataClaim, DecodedRecordWritePlan};
use crate::pipeline::{difference_ranges, sha256_bytes};
use crate::tim::{Cell, read_indexed_cell_in_prefix, write_indexed_cell_in_prefix_with_report};

#[derive(Deserialize)]
struct Binding {
    tim_offset: usize,
    cell: Cell,
    entries: Vec<Source>,
}

#[derive(Deserialize)]
struct Source {
    source_path: String,
    decoded_sha256: String,
    cell_indexed_sha256: String,
    palette28_sha256: String,
}

#[derive(Deserialize)]
struct Artwork {
    indices_file: String,
    indices_sha256: String,
}

pub(super) struct SmallLogo {
    binding: Binding,
    pixels: Vec<u8>,
    pub report: serde_json::Value,
}

impl SmallLogo {
    pub fn load(assets: &Path) -> Result<Self> {
        let root = assets.join("shared-logo");
        let source = std::fs::read(root.join("source-binding.json"))?;
        let art = std::fs::read(root.join("artwork.json"))?;
        let binding: Binding = serde_json::from_slice(&source)?;
        let artwork: Artwork = serde_json::from_slice(&art)?;
        let pixels = std::fs::read(root.join(&artwork.indices_file))?;
        ensure!(
            binding.tim_offset == 0x17800
                && binding.cell
                    == (Cell {
                        x: 876,
                        y: 152,
                        width: 148,
                        height: 66
                    })
                && binding.entries.len() == 5,
            "small logo source geometry changed"
        );
        ensure!(
            pixels.len() == 148 * 66
                && pixels.iter().all(|p| (1..=15).contains(p))
                && sha256_bytes(&pixels) == artwork.indices_sha256,
            "small logo artwork changed or introduces transparent pixels"
        );
        let report = serde_json::json!({
            "source_binding_sha256": sha256_bytes(&source),
            "artwork_manifest_sha256": sha256_bytes(&art),
            "indices_sha256": artwork.indices_sha256,
            "record_count": 5, "runtime_verified": false
        });
        Ok(Self {
            binding,
            pixels,
            report,
        })
    }

    pub fn register(
        &self,
        path: &str,
        source: &[u8],
        plan: &mut DecodedRecordWritePlan<'_>,
    ) -> Result<()> {
        let entry = self
            .binding
            .entries
            .iter()
            .find(|e| e.source_path == path)
            .context("small logo source record is not bound")?;
        ensure!(
            sha256_bytes(source) == entry.decoded_sha256,
            "small logo source changed"
        );
        let cell = read_indexed_cell_in_prefix(source, self.binding.tim_offset, self.binding.cell)?;
        ensure!(
            sha256_bytes(&cell) == entry.cell_indexed_sha256,
            "small logo source pixels changed"
        );
        let palette = self.binding.tim_offset + 20 + 28 * 32;
        ensure!(
            sha256_bytes(&source[palette..palette + 32]) == entry.palette28_sha256,
            "small logo source palette changed"
        );
        let mut candidate = source.to_vec();
        write_indexed_cell_in_prefix_with_report(
            &mut candidate,
            self.binding.tim_offset,
            self.binding.cell,
            &self.pixels,
        )?;
        plan.register_data_candidate(
            "shared small game logo",
            &entry.decoded_sha256,
            &candidate,
            &DecodedDataClaim::from_ranges(
                "shared-small-game-logo",
                "replace the source logo sprite",
                difference_ranges(source, &candidate),
            ),
        )?;
        Ok(())
    }
}
