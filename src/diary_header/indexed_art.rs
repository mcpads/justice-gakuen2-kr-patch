use std::path::{Component, Path};

use anyhow::{Context, Result, ensure};
use serde::Deserialize;

use crate::font::RasterizedIndexedText;
use crate::pipeline::sha256_bytes;

use super::model::DiaryHeaderEntry;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IndexedArt {
    kind: String,
    text: String,
    width: usize,
    height: usize,
    rows: Vec<String>,
    provenance: serde_json::Value,
}

pub(super) fn load(root: &Path, entry: &DiaryHeaderEntry) -> Result<Option<RasterizedIndexedText>> {
    let Some(reference) = &entry.indexed_art else {
        return Ok(None);
    };
    ensure!(
        !reference.file.as_os_str().is_empty()
            && reference
                .file
                .components()
                .all(|part| matches!(part, Component::Normal(_))),
        "indexed art path must stay within the asset directory"
    );
    let bytes = std::fs::read(root.join(&reference.file))?;
    ensure!(
        sha256_bytes(&bytes) == reference.sha256,
        "indexed art source hash changed"
    );
    let art: IndexedArt = serde_json::from_slice(&bytes)?;
    ensure!(
        art.kind == "justice_gakuen2_indexed_art" && art.text == entry.korean_text,
        "indexed art identity or wording differs from its entry"
    );
    ensure!(
        art.width == entry.cell.width && art.height == entry.cell.height,
        "indexed art does not fit its source-owned cell"
    );
    ensure!(
        art.provenance.is_object(),
        "indexed art lacks producer provenance"
    );
    let pixels = decode_rows(&art.rows, art.width, art.height)?;
    let mut bounds = [art.width, art.height, 0, 0];
    for (index, pixel) in pixels.iter().enumerate().filter(|(_, pixel)| **pixel != 0) {
        let (x, y) = (index % art.width, index / art.width);
        let _ = pixel;
        bounds[0] = bounds[0].min(x);
        bounds[1] = bounds[1].min(y);
        bounds[2] = bounds[2].max(x + 1);
        bounds[3] = bounds[3].max(y + 1);
    }
    ensure!(
        bounds[2] > bounds[0] && bounds[3] > bounds[1],
        "indexed art is blank"
    );
    Ok(Some(RasterizedIndexedText {
        font_name: "indexed artwork".into(),
        font_sha256: reference.sha256.clone(),
        pixels,
        measured_advance_px: (bounds[2] - bounds[0]) as f32,
        ink_bounds: bounds,
    }))
}

fn decode_rows(rows: &[String], width: usize, height: usize) -> Result<Vec<u8>> {
    ensure!(
        width > 0 && height > 0 && rows.len() == height,
        "indexed art row count differs from its cell"
    );
    ensure!(
        rows.iter().all(|row| row.len() == width),
        "indexed art row width differs from its cell"
    );
    rows.iter()
        .flat_map(|row| row.chars())
        .map(|pixel| {
            pixel
                .to_digit(16)
                .map(|value| value as u8)
                .context("indexed art pixel is not a 4-bpp index")
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexed_art_preserves_palette_indices_and_rejects_bad_geometry() {
        assert_eq!(decode_rows(&["01af".into()], 4, 1).unwrap(), [0, 1, 10, 15]);
        assert!(decode_rows(&["01ag".into()], 4, 1).is_err());
        assert!(decode_rows(&["01a".into()], 4, 1).is_err());
        assert!(decode_rows(&["01af".into()], 4, 2).is_err());
    }
}
