//! Logo contributions to the existing MA_TIT physical-record owner.
use super::assets::load_title_graphics_assets;
use super::source::load_title_graphics_source;
use crate::decoded_record_write_plan::DecodedDataClaim;
use crate::pipeline::sha256_bytes;
use crate::tim::{read_indexed_cell_in_prefix, write_indexed_cell_in_prefix_with_report};
use anyhow::{Context, Result, ensure};
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Serialize)]
pub struct TitleArtworkBuild {
    pub id: String,
    pub korean_text: String,
    pub indices_sha256: String,
    pub asset_set_sha256: String,
    pub palette_index: usize,
    pub imagegen_sha256: String,
    pub dotmend_art_id: String,
}

pub(crate) fn apply_title_graphics(
    cue: &Path,
    assets_root: &Path,
    decoded: &mut [u8],
) -> Result<(Vec<DecodedDataClaim>, Vec<TitleArtworkBuild>)> {
    let source = load_title_graphics_source(cue)?;
    let assets = load_title_graphics_assets(assets_root, &source)?;
    let mut claims = Vec::new();
    let mut reports = Vec::new();
    for unit in assets.units {
        let Some(art) = unit.artwork else { continue };
        // Source native packets select distinct palettes for the two sizes.
        let expected_palette = match unit.id.as_str() {
            "franchise_logo_large" => 9,
            "franchise_logo_small" => 8,
            _ => unreachable!("asset loader checks the finite logo family"),
        };
        ensure!(
            art.palette_index == expected_palette,
            "logo draw palette changed"
        );
        let palette_start = 20 + art.palette_index * 32;
        ensure!(
            sha256_bytes(
                decoded
                    .get(palette_start..palette_start + 32)
                    .context("logo palette missing")?
            ) == art.source_palette_sha256
                && decoded.get(palette_start..palette_start + 32)
                    == source.decoded.get(palette_start..palette_start + 32),
            "logo source palette changed"
        );
        ensure!(
            sha256_bytes(&read_indexed_cell_in_prefix(decoded, 0, unit.cell)?)
                == unit.source_indexed_pixel_sha256,
            "logo cell is already owned or source changed"
        );
        let pixels = std::fs::read(assets_root.join(&art.indices_file))?;
        ensure!(
            pixels.len() == unit.cell.width * unit.cell.height
                && pixels.iter().all(|p| *p < 16)
                && sha256_bytes(&pixels) == art.indices_sha256,
            "logo indexed artwork changed"
        );
        let before = decoded.to_vec();
        let write = write_indexed_cell_in_prefix_with_report(decoded, 0, unit.cell, &pixels)?;
        claims.extend(DecodedDataClaim::from_effective_ranges(
            &unit.id,
            "Korean title logo; preserve source palette and adjacent UI",
            &before,
            decoded,
            write.allowed_ranges,
        )?);
        reports.push(TitleArtworkBuild {
            id: unit.id,
            korean_text: unit
                .korean_text
                .context("authored logo has no Korean text")?,
            indices_sha256: art.indices_sha256,
            asset_set_sha256: assets.asset_set_sha256.clone(),
            palette_index: art.palette_index,
            imagegen_sha256: art.imagegen_sha256,
            dotmend_art_id: art.dotmend_art_id,
        });
    }
    Ok((claims, reports))
}
