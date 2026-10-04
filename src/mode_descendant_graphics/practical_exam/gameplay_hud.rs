//! Gameplay digits and prefix belong to CEFT4/CEFT6, not the menu atlas.

use anyhow::{Context, Result, ensure};

use crate::decoded_record_write_plan::DecodedDataClaim;
use crate::development_build_spec::ShiftedSizedFontSource;
use crate::font::{HorizontalTextAlignment, IndexedTextRasterizer};
use crate::pipeline::sha256_bytes;
use crate::tim::{Cell, read_indexed_cell_in_prefix, write_indexed_cell_in_prefix_with_report};

use super::super::gorin_announcements::SPECS;
use super::super::record_compositor::{ModeDescendantRecordDraft, source_for_spec};
use super::super::source::ModeDescendantSourceRecord;

const TIM_OFFSET: usize = 0x10800;
const CHARACTERS: &str = "0123456789과목:";
const SOURCE_CELLS_SHA256: &str =
    "15960cfc8f7ac6f4394aa5691ab43c1580c2d5d4f4841ad743f444b7ff221bb4";

fn cell(index: usize) -> Cell {
    let (x, y) = match index {
        0..=9 => (416 + index % 4 * 24, 48 + index / 4 * 16),
        10 => (184, 96),
        11 => (184, 112),
        12 => (208, 112),
        _ => unreachable!(),
    };
    Cell {
        x,
        y,
        width: 24,
        height: 16,
    }
}

fn render(font: &ShiftedSizedFontSource) -> Result<Vec<Vec<u8>>> {
    let rasterizer = IndexedTextRasterizer::load(&font.path)?;
    CHARACTERS
        .chars()
        .map(|character| {
            let glyph = rasterizer.rasterize_shifted_with_coverage_ramp(
                &character.to_string(),
                16,
                16,
                font.font_px,
                0.0,
                font.vertical_shift_px,
                0,
                1,
                15,
                HorizontalTextAlignment::Center,
            )?;
            ensure!(
                glyph.measured_advance_px <= 16.0,
                "gameplay HUD glyph does not fit: {character}"
            );
            // The subject uses 16 pixels; native timer consumers still sample 24.
            // Clear the full source cell so no Japanese/old numeral ink survives.
            let mut pixels = vec![0; 24 * 16];
            for y in 0..16 {
                pixels[y * 24..y * 24 + 16].copy_from_slice(&glyph.pixels[y * 16..(y + 1) * 16]);
            }
            Ok(pixels)
        })
        .collect()
}

fn write_cells(
    source: &[u8],
    candidate: &mut [u8],
    glyphs: &[Vec<u8>],
) -> Result<Vec<DecodedDataClaim>> {
    ensure!(glyphs.len() == 13, "gameplay HUD alphabet is incomplete");
    let original = (0..13)
        .map(|i| read_indexed_cell_in_prefix(source, TIM_OFFSET, cell(i)))
        .collect::<Result<Vec<_>>>()?
        .concat();
    ensure!(
        sha256_bytes(&original) == SOURCE_CELLS_SHA256,
        "gameplay HUD source alphabet changed"
    );
    let mut ranges = Vec::new();
    for (index, glyph) in glyphs.iter().enumerate() {
        let region = cell(index);
        ensure!(
            read_indexed_cell_in_prefix(candidate, TIM_OFFSET, region)?
                == read_indexed_cell_in_prefix(source, TIM_OFFSET, region)?,
            "gameplay HUD conflicts with an earlier texture writer"
        );
        ranges.extend(
            write_indexed_cell_in_prefix_with_report(candidate, TIM_OFFSET, region, glyph)?
                .allowed_ranges,
        );
        ensure!(
            read_indexed_cell_in_prefix(candidate, TIM_OFFSET, region)? == *glyph,
            "gameplay HUD producer readback differs"
        );
    }
    DecodedDataClaim::from_effective_ranges(
        "practical-gameplay-alphabet",
        "regenerate the complete shared decimal alphabet and term prefix in native gameplay cells",
        source,
        candidate,
        ranges,
    )
}

pub(in crate::mode_descendant_graphics) fn apply(
    sources: &[ModeDescendantSourceRecord],
    drafts: &mut [ModeDescendantRecordDraft],
    font: &ShiftedSizedFontSource,
) -> Result<()> {
    let glyphs = render(font)?;
    for spec in &SPECS {
        let source = source_for_spec(sources, spec)?;
        let draft = drafts
            .iter_mut()
            .find(|draft| draft.spec.source_path == spec.source_path)
            .context("gameplay HUD requires the shared CEFT composition")?;
        let claims = write_cells(&source.decoded, &mut draft.decoded, &glyphs)?;
        draft.decoded_write_claims.extend(claims);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    #[test]
    #[ignore = "requires the supported original disc and selected font"]
    fn both_gameplay_providers_clear_complete_native_cells_and_preserve_neighbors() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let disc = crate::source_disc::SupportedSourceDisc::open(
            &root.join("roms/Shiritsu Justice Gakuen - Nekketsu Seishun Nikki 2 (Japan).cue"),
        )
        .unwrap();
        let font = ShiftedSizedFontSource {
            path: root.join("../fonts/NEXON_Maplestory/TTF/Maplestory Bold.ttf"),
            font_px: 13.0,
            vertical_shift_px: -2,
        };
        let glyphs = render(&font).unwrap();
        let mut providers = Vec::new();
        for spec in &SPECS {
            let (_, compressed) = disc.read_record(spec.source_path).unwrap();
            let source = crate::compression::decompress(&compressed, true).unwrap();
            let mut candidate = source.clone();
            let claims = write_cells(&source, &mut candidate, &glyphs).unwrap();
            for (offset, (old, new)) in source.iter().zip(&candidate).enumerate() {
                assert!(old == new || claims.iter().any(|claim| claim.range.contains(&offset)));
            }
            for (index, glyph) in glyphs.iter().enumerate() {
                assert!(glyph.contains(&15));
                for row in glyph.as_chunks::<24>().0 {
                    assert!(row[16..].iter().all(|p| *p == 0));
                }
                assert_eq!(
                    read_indexed_cell_in_prefix(&candidate, TIM_OFFSET, cell(index)).unwrap(),
                    *glyph
                );
            }
            assert!(write_cells(&source, &mut candidate, &glyphs).is_err());
            providers.push(
                (0..13)
                    .map(|i| read_indexed_cell_in_prefix(&candidate, TIM_OFFSET, cell(i)).unwrap())
                    .collect::<Vec<_>>(),
            );
        }
        assert_eq!(providers[0], providers[1]);
    }
}
