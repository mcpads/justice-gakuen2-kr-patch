//! MODESEL's separate 120x72 game-logo sprite uses the shared Korean artwork.
use crate::{
    decoded_record_write_plan::DecodedDataClaim,
    pipeline::sha256_bytes,
    source_disc::SupportedSourceDisc,
    tim::{
        Cell, read_4bpp_palette_words_in_prefix, read_indexed_cell_in_prefix,
        write_indexed_cell_in_prefix_with_report,
    },
    title_graphics::shared_logo::SmallLogo,
};
use anyhow::{Result, ensure};
use std::path::Path;

pub(super) fn apply(
    disc: &SupportedSourceDisc,
    assets: &Path,
    overlay: &[u8],
    source: &[u8],
    patched: &mut [u8],
    claims: &mut Vec<DecodedDataClaim>,
) -> Result<serde_json::Value> {
    // Native background descriptor: page(768,256), CLUT(160,482), UV(96,0),
    // size(120,72), destination(368,16). Atlas origin is VRAM(512,256).
    ensure!(
        overlay.get(0xae..0xc4)
            == Some(&[
                0, 0, 0, 3, 0, 1, 160, 0, 226, 1, 96, 0, 0, 0, 120, 0, 72, 0, 112, 1, 16, 0
            ]),
        "MODE game-logo consumer changed"
    );
    let tim = super::source::ATLAS_TIM_OFFSET;
    let cell = Cell {
        x: 1136,
        y: 0,
        width: 104,
        height: 72,
    };
    let original = read_indexed_cell_in_prefix(source, tim, cell)?;
    ensure!(
        sha256_bytes(&original)
            == "4376e28bd94d4aa18814853890b5770803d944026b72e547abd3689d07ea9ffe"
            && read_indexed_cell_in_prefix(patched, tim, cell)? == original,
        "MODE game logo changed or already has another writer"
    );
    let words = read_4bpp_palette_words_in_prefix(source, tim, 10)?;
    let raw: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
    ensure!(
        sha256_bytes(&raw) == "19ef1ad9a1250997ffa0da6fcab28bfca13cc509ac89368216d45a767acb2cba"
            && words[0] == 0,
        "MODE game-logo palette changed"
    );
    let logo = SmallLogo::load(disc, assets)?;
    let colors: Vec<[u8; 3]> = words
        .iter()
        .map(|w| [0, 5, 10].map(|s| (((w >> s) & 31) * 255 / 31) as u8))
        .collect();
    let mapping: Vec<u8> = logo
        .palette
        .iter()
        .enumerate()
        .map(|(index, rgb)| {
            if index == usize::from(logo.unit.clear_index) {
                return 0;
            }
            (1..16)
                .filter(|&i| words[i] != 0)
                .min_by_key(|&i| {
                    (0..3)
                        .map(|c| (i32::from(rgb[c]) - i32::from(colors[i][c])).pow(2))
                        .sum::<i32>()
                })
                .expect("source palette has opaque colors") as u8
        })
        .collect();
    let mut pixels = vec![0; cell.width * cell.height];
    // The first sixteen pixels of the native sprite overlap three title cells.
    // Their owner clears them; do not claim or overwrite that shared margin.
    let margin = Cell {
        x: 1120,
        y: 0,
        width: 16,
        height: 72,
    };
    ensure!(
        read_indexed_cell_in_prefix(patched, tim, margin)?
            .iter()
            .all(|p| *p == 0),
        "MODE logo shared title margin is not clear"
    );
    let x = (120 - logo.unit.cell.width) / 2 - margin.width;
    let y = (cell.height - logo.unit.cell.height) / 2;
    for row in 0..logo.unit.cell.height {
        for col in 0..logo.unit.cell.width {
            pixels[(y + row) * cell.width + x + col] =
                mapping[usize::from(logo.pixels[row * logo.unit.cell.width + col])];
        }
    }
    let before = patched.to_vec();
    let write = write_indexed_cell_in_prefix_with_report(patched, tim, cell, &pixels)?;
    claims.extend(DecodedDataClaim::from_effective_ranges(
        "mode-game-logo",
        "Reuse Korean game logo in native MODE sprite",
        source,
        patched,
        write.allowed_ranges,
    )?);
    ensure!(
        read_indexed_cell_in_prefix(patched, tim, cell)? == pixels
            && read_4bpp_palette_words_in_prefix(patched, tim, 10)? == words,
        "MODE logo readback or palette preservation failed"
    );
    ensure!(before != patched, "MODE Korean logo writes no pixels");
    Ok(serde_json::json!({"cell":cell,"artwork_offset":[x,y],
        "source_asset":"title-graphics/franchise-logo-small.json",
        "asset_sha256":logo.unit_sha256,"indices_sha256":logo.unit.artwork.indices_sha256,
        "palette_mapping":mapping,"output_pixels_sha256":sha256_bytes(&pixels),
        "palette_preserved":true}))
}
