//! Complete card-back artwork shared by the fifteen grid slots and their palettes.
use std::path::Path;

use anyhow::{Result, ensure};
use serde::Deserialize;
use serde_json::{Value, json};

use super::model::BonusInventoryTextStyleSource;
use crate::font::{HorizontalTextAlignment, IndexedTextRasterizers};
use crate::pipeline::sha256_bytes;
use crate::tim::{Cell, read_indexed_cell_in_prefix, write_indexed_cell_in_prefix_with_report};

const CELL: Cell = Cell {
    x: 608,
    y: 56,
    width: 80,
    height: 112,
};
// The viewer overlays its card number from local row 80 downward.
const TITLE_TOP: usize = 8;
const NUMBER_TOP: usize = 80;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Artwork {
    source_indexed_sha256: String,
    indices_file: String,
    indices_sha256: String,
    cell: Cell,
    lines: Vec<String>,
    provenance: Value,
    release_status: String,
}

pub(super) fn apply(
    root: &Path,
    source: &[u8],
    output: &mut [u8],
    font: &BonusInventoryTextStyleSource,
    rasterizers: &mut IndexedTextRasterizers,
) -> Result<(Value, Vec<[usize; 2]>)> {
    let bytes = std::fs::read(root.join("card-back.json"))?;
    let artwork: Artwork = serde_json::from_slice(&bytes)?;
    ensure!(artwork.cell == CELL, "card-back texture cell changed");
    let before = read_indexed_cell_in_prefix(source, 0, CELL)?;
    ensure!(
        sha256_bytes(&before) == artwork.source_indexed_sha256,
        "card-back source changed"
    );
    ensure!(
        read_indexed_cell_in_prefix(output, 0, CELL)? == before,
        "card-back writer overlaps an earlier edit"
    );
    let path = root.join(&artwork.indices_file).canonicalize()?;
    ensure!(
        path.starts_with(root.canonicalize()?),
        "card-back artwork escapes its asset directory"
    );
    let mut pixels = std::fs::read(path)?;
    ensure!(
        pixels.len() == CELL.width * CELL.height
            && pixels.iter().all(|&p| p < 16)
            && sha256_bytes(&pixels) == artwork.indices_sha256,
        "card-back indexed artwork changed"
    );
    ensure!(
        artwork.lines.join("").replace(' ', "") == "사립저스티스학원열혈청춘일기2"
            && artwork.lines.len() == 5,
        "card-back title meaning changed"
    );
    let mut line_reports = Vec::new();
    ensure!(
        TITLE_TOP + artwork.lines.len() * 14 <= NUMBER_TOP,
        "card-back title overlaps the viewer number"
    );
    for (row, text) in artwork.lines.iter().enumerate() {
        let raster = rasterizers.for_font(&font.path)?.rasterize(
            text,
            68,
            14,
            font.font_px,
            0.0,
            0,
            Some(2),
            1,
            HorizontalTextAlignment::Center,
        )?;
        for y in 0..14 {
            for x in 0..68 {
                let v = raster.pixels[y * 68 + x];
                if v != 0 {
                    pixels[(TITLE_TOP + row * 14 + y) * CELL.width + 6 + x] =
                        if v == 1 { 1 } else { 10 };
                }
            }
        }
        line_reports.push(
            json!({"text":text,"ink_bounds":raster.ink_bounds,"font":raster.font_name,
            "font_sha256":raster.font_sha256,"font_px":font.font_px}),
        );
    }
    let write = write_indexed_cell_in_prefix_with_report(output, 0, CELL, &pixels)?;
    Ok((
        json!({"cell":CELL,"source_indexed_sha256":artwork.source_indexed_sha256,
        "indices_sha256":artwork.indices_sha256,"output_indexed_sha256":sha256_bytes(&pixels),"lines":line_reports,"manifest_sha256":sha256_bytes(&bytes),
        "title_top":TITLE_TOP,"viewer_number_top":NUMBER_TOP,
        "provenance":artwork.provenance,"release_status":artwork.release_status,
        "changed_bytes":write.changed_byte_count}),
        write.allowed_ranges,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires the supported source disc and selected fonts"]
    fn complete_card_back_preserves_other_cells_and_rejects_prior_writes() -> Result<()> {
        let spec = crate::development_build_spec::load_development_build_spec(Path::new(
            "assets/build/development.json",
        ))?;
        let disc = crate::source_disc::SupportedSourceDisc::open(Path::new(
            "roms/Shiritsu Justice Gakuen - Nekketsu Seishun Nikki 2 (Japan).cue",
        ))?;
        let source = crate::bonus_inventory_source::load_bonus_inventory_source(&disc)?;
        let mut output = source.inventory_decoded.clone();
        let mut rasterizers = IndexedTextRasterizers::default();
        let (report, ranges) = apply(
            &spec.assets.bonus_inventory,
            &source.inventory_decoded,
            &mut output,
            &spec.fonts.bonus_inventory.item_label,
            &mut rasterizers,
        )?;
        for (i, (&a, &b)) in source.inventory_decoded.iter().zip(&output).enumerate() {
            if a != b {
                assert!(ranges.iter().any(|r| i >= r[0] && i < r[0] + r[1]));
            }
        }
        assert_eq!(&source.inventory_decoded[..512], &output[..512]);
        assert!(
            apply(
                &spec.assets.bonus_inventory,
                &source.inventory_decoded,
                &mut output,
                &spec.fonts.bonus_inventory.item_label,
                &mut rasterizers
            )
            .is_err()
        );
        if let Ok(dir) = std::env::var("CARD_BACK_PREVIEW_DIR") {
            let dir = Path::new(&dir);
            std::fs::create_dir_all(dir)?;
            std::fs::write(dir.join("inventory-preview.decoded"), &output)?;
            std::fs::write(
                dir.join("card-back-preview.json"),
                serde_json::to_vec_pretty(&report)?,
            )?;
        }
        Ok(())
    }
}
