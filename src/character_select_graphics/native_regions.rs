//! Reserve consumed native rectangles, including transparent sprite interiors.
use anyhow::{Result, ensure};
use serde::Deserialize;

use super::texture_targets::SHARED_ATLAS_OFFSET;
use crate::pipeline::sha256_bytes;
use crate::tim::{Cell, read_indexed_cell_in_prefix};

#[derive(Deserialize)]
struct Asset {
    units: Vec<Region>,
}
#[derive(Deserialize)]
struct Region {
    cell: Cell,
    source_pixel_sha256: String,
}

fn retained() -> Result<&'static [u8]> {
    crate::product_assets::read("menu/common/selection-retained-sprites.json")
}

pub(super) fn protected_cells() -> Result<Vec<Cell>> {
    let mut cells = Vec::new();
    for (bytes, limit) in [
        (
            crate::product_assets::read("menu/common/numerals.json")?,
            10,
        ),
        (
            crate::product_assets::read("menu/common/selection-symbols.json")?,
            usize::MAX,
        ),
        (retained()?, usize::MAX),
    ] {
        let asset: Asset = serde_json::from_slice(bytes)?;
        // The final MENU slash is not a SELP supplier and has another owner.
        cells.extend(asset.units.into_iter().take(limit).map(|u| u.cell));
    }
    Ok(cells)
}

pub(super) fn validate_retained(source: &[u8], output: &[u8]) -> Result<()> {
    let asset: Asset = serde_json::from_slice(retained()?)?;
    for unit in asset.units {
        let before = read_indexed_cell_in_prefix(source, SHARED_ATLAS_OFFSET, unit.cell)?;
        let after = read_indexed_cell_in_prefix(output, SHARED_ATLAS_OFFSET, unit.cell)?;
        ensure!(
            sha256_bytes(&before) == unit.source_pixel_sha256,
            "retained sprite source changed"
        );
        ensure!(
            before == after,
            "retained native sprite or transparent interior overwritten at {:?}",
            unit.cell
        );
    }
    Ok(())
}
