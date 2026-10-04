//! Shared source-bound small Korean logo used by cards and MODE SELECT.
use crate::{
    compression::decompress, pipeline::sha256_bytes, source_disc::SupportedSourceDisc, tim::Cell,
};
use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use std::path::Path;

#[derive(Deserialize)]
pub(crate) struct LogoUnit {
    pub(crate) id: String,
    pub(crate) cell: Cell,
    pub(crate) clear_index: u8,
    pub(crate) korean_text: String,
    pub(crate) artwork: LogoAsset,
}

#[derive(Deserialize)]
pub(crate) struct LogoAsset {
    pub(crate) indices_file: std::path::PathBuf,
    pub(crate) indices_sha256: String,
    pub(crate) palette_index: usize,
    pub(crate) source_palette_sha256: String,
}

pub(crate) struct SmallLogo {
    pub(crate) unit: LogoUnit,
    pub(crate) pixels: Vec<u8>,
    pub(crate) palette: Vec<[u8; 3]>,
    pub(crate) unit_sha256: String,
}

impl SmallLogo {
    pub fn load(source: &SupportedSourceDisc, assets: &Path) -> Result<Self> {
        let menu = assets
            .parent()
            .context("logo consumer asset parent missing")?;
        let logo_assets = menu.join("title-graphics");
        let bytes = std::fs::read(logo_assets.join("franchise-logo-small.json"))?;
        let unit: LogoUnit = serde_json::from_slice(&bytes)?;
        ensure!(
            unit.id == "franchise_logo_small"
                && unit.cell.width == 85
                && unit.cell.height == 62
                && unit.clear_index == 0
                && unit.artwork.palette_index == 8,
            "shared small logo identity changed"
        );
        let path = logo_assets
            .join(&unit.artwork.indices_file)
            .canonicalize()?;
        let asset_root = menu
            .parent()
            .context("asset root missing")?
            .canonicalize()?;
        ensure!(
            path.starts_with(asset_root),
            "shared logo leaves the asset root"
        );
        let pixels = std::fs::read(path)?;
        ensure!(
            pixels.len() == 85 * 62
                && pixels.iter().all(|&p| p < 16)
                && sha256_bytes(&pixels) == unit.artwork.indices_sha256,
            "shared small logo indexed art changed"
        );
        let (_, stored) = source.read_record("DAT2/MA_TIT.BIZ")?;
        let decoded = decompress(&stored, true)?;
        let start = 20 + unit.artwork.palette_index * 32;
        let raw = decoded
            .get(start..start + 32)
            .context("shared logo CLUT missing")?;
        ensure!(
            sha256_bytes(raw) == unit.artwork.source_palette_sha256,
            "shared small logo source palette changed"
        );
        let palette = raw
            .as_chunks::<2>()
            .0
            .iter()
            .map(|b| {
                let color = u16::from_le_bytes([b[0], b[1]]);
                [0, 5, 10].map(|shift| (((color >> shift) & 31) * 255 / 31) as u8)
            })
            .collect();
        Ok(Self {
            unit,
            pixels,
            palette,
            unit_sha256: sha256_bytes(&bytes),
        })
    }
}
