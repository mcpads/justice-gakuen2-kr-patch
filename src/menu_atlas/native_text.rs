//! Restyle existing shared text without reallocating its codes or cells.
use anyhow::{Result, ensure};
use serde::Deserialize;

use crate::decoded_record_write_plan::DecodedDataClaim;
use crate::development_build_spec::SizedFontSource;
use crate::font::{HorizontalTextAlignment, IndexedTextRasterizer};
use crate::pipeline::sha256_bytes;
use crate::tim::{Cell, read_indexed_cell_in_prefix, write_indexed_cell_in_prefix_with_report};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NumeralAsset {
    font_role: String,
    source_moji2_sha256: String,
    release_status: String,
    units: Vec<Unit>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SymbolAsset {
    font_role: String,
    release_status: String,
    units: Vec<Unit>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Unit {
    character: String,
    cell: Cell,
    source_pixel_sha256: String,
}

pub(super) fn apply_common_numerals(
    source: &[u8],
    candidate: &mut [u8],
    style: &SizedFontSource,
) -> Result<(Vec<DecodedDataClaim>, serde_json::Value)> {
    let (bytes, asset) = load_numerals()?;
    ensure!(
        source
            .get(..0x201c0)
            .is_some_and(|b| sha256_bytes(b) == asset.source_moji2_sha256),
        "shared numeral source atlas changed"
    );
    let (claims, units) = render_units(
        source,
        candidate,
        0,
        style,
        0,
        &asset.units.iter().collect::<Vec<_>>(),
    )?;
    Ok((
        claims,
        serde_json::json!({
            "asset_sha256": sha256_bytes(bytes), "font_role": asset.font_role,
            "tim_offset": 0, "decimal_only": false,
            "release_status": asset.release_status, "units": units,
            "glyph_codes_and_geometry_preserved": true,
        }),
    ))
}

/// SELP shares the ten decimal cells, but not MENU's slash position. Its
/// adjacent symbols and team markers have their own source-bound repertoire.
pub(crate) fn apply_selection_text(
    source: &[u8],
    candidate: &mut [u8],
    tim_offset: usize,
    style: &SizedFontSource,
    prompt_style: &crate::character_select_graphics::CharacterSelectFontStyle,
) -> Result<(Vec<DecodedDataClaim>, serde_json::Value)> {
    let (numeral_bytes, numerals) = load_numerals()?;
    let symbol_bytes = crate::product_assets::read("menu/common/selection-symbols.json")?;
    let symbols: SymbolAsset = serde_json::from_slice(symbol_bytes)?;
    ensure!(
        symbols.font_role == numerals.font_role
            && symbols.release_status == numerals.release_status,
        "selection symbols must share their adjacent numerals' profile and release state"
    );
    let units = numerals.units[..10]
        .iter()
        .chain(&symbols.units)
        .collect::<Vec<_>>();
    let (prompt, common): (Vec<_>, Vec<_>) = units.iter().copied().partition(|unit| {
        unit.character == "?"
            && unit.cell
                == (Cell {
                    x: 120,
                    y: 20,
                    width: 20,
                    height: 20,
                })
    });
    ensure!(
        prompt.len() == 1,
        "participant punctuation source binding changed"
    );
    let (mut claims, mut reports) = render_units(source, candidate, tim_offset, style, 0, &common)?;
    let prompt_font = SizedFontSource {
        path: prompt_style.path.clone(),
        font_px: prompt_style.font_px,
    };
    let (prompt_claims, prompt_reports) = render_units(
        source,
        candidate,
        tim_offset,
        &prompt_font,
        prompt_style.vertical_shift_px,
        &prompt,
    )?;
    claims.extend(prompt_claims);
    reports.extend(prompt_reports);
    reports.sort_by_key(|report| {
        units
            .iter()
            .position(|unit| {
                report["cell"]["x"].as_u64() == Some(unit.cell.x as u64)
                    && report["cell"]["y"].as_u64() == Some(unit.cell.y as u64)
            })
            .expect("rendered native text belongs to its source-bound unit")
    });
    Ok((
        claims,
        serde_json::json!({
            "asset_sha256": sha256_bytes(numeral_bytes),
            "symbol_asset_sha256": sha256_bytes(symbol_bytes),
            "font_role": numerals.font_role, "tim_offset": tim_offset,
            "repertoire": "selection_native_text", "release_status": numerals.release_status,
            "units": reports, "glyph_codes_and_geometry_preserved": true,
        }),
    ))
}

fn load_numerals() -> Result<(&'static [u8], NumeralAsset)> {
    let bytes = crate::product_assets::read("menu/common/numerals.json")?;
    let asset: NumeralAsset = serde_json::from_slice(bytes)?;
    ensure!(
        asset.font_role == "shared_menu_numerals" && asset.release_status == "needs_human_review",
        "shared numeral role or release state changed"
    );
    ensure!(
        asset.units.len() == 11,
        "shared numeral character population changed"
    );
    for (index, (unit, character)) in asset.units.iter().zip("1234567890/".chars()).enumerate() {
        let cell = if index < 10 {
            Cell {
                x: index * 20,
                y: 0,
                width: 20,
                height: 20,
            }
        } else {
            Cell {
                x: 160,
                y: 100,
                width: 20,
                height: 20,
            }
        };
        ensure!(
            unit.character == character.to_string() && unit.cell == cell,
            "shared numeral character meaning or source cell changed"
        );
    }
    Ok((bytes, asset))
}

fn render_units(
    source: &[u8],
    candidate: &mut [u8],
    tim_offset: usize,
    style: &SizedFontSource,
    role_vertical_shift_px: i32,
    units: &[&Unit],
) -> Result<(Vec<DecodedDataClaim>, Vec<serde_json::Value>)> {
    let renderer = IndexedTextRasterizer::load(&style.path)?;
    let mut claims = Vec::new();
    let mut reports = Vec::new();
    for (index, unit) in units.iter().enumerate() {
        let cell = unit.cell;
        ensure!(
            unit.character.len() == 1
                && unit.character.is_ascii()
                && matches!((cell.width, cell.height), (12, 16) | (16, 16) | (20, 20)),
            "unsupported shared native text unit"
        );
        ensure!(
            !units[..index]
                .iter()
                .any(|other| crate::tim::cells_overlap(cell, other.cell)),
            "overlapping shared native text cells"
        );
        let original = read_indexed_cell_in_prefix(source, tim_offset, cell)?;
        ensure!(
            sha256_bytes(&original) == unit.source_pixel_sha256,
            "shared native text source pixels changed for {} at {},{}",
            unit.character,
            cell.x,
            cell.y
        );
        // The 12x16 alphabet bank includes both capitals and descenders.
        // At 16px they cannot share a baseline with outline padding at both
        // edges. Fit the whole bank at 14px with one shared baseline shift,
        // never fit individual characters.
        let (font_px, vertical_shift_px) = match cell.width {
            12 => (style.font_px.min(14.0), -1),
            _ => (style.font_px, role_vertical_shift_px),
        };
        // Native face/edge/transparent indices remain under each reader's CLUT.
        let raster = renderer.rasterize_shifted(
            &unit.character,
            cell.width,
            cell.height,
            font_px,
            0.0,
            vertical_shift_px,
            0,
            Some(3),
            14,
            HorizontalTextAlignment::Center,
        )?;
        // An outline may occupy the cell edge; the face must leave its full
        // one-pixel dilation inside the cell. No extra blank row is required.
        let fill_has_padding = raster.pixels.iter().enumerate().all(|(i, p)| {
            *p != 14
                || (i % cell.width > 0
                    && i % cell.width + 1 < cell.width
                    && i / cell.width > 0
                    && i / cell.width + 1 < cell.height)
        });
        ensure!(
            fill_has_padding,
            "shared native character {} lacks outline padding: {:?}",
            unit.character,
            raster.ink_bounds
        );
        let pixels = raster.pixels.clone();
        let write = write_indexed_cell_in_prefix_with_report(candidate, tim_offset, cell, &pixels)?;
        claims.extend(DecodedDataClaim::from_effective_ranges(
            &format!("shared-native-text-{}-{}", cell.x, cell.y),
            "redraw the same character in its existing shared cell",
            source,
            candidate,
            write.allowed_ranges,
        )?);
        reports.push(serde_json::json!({
            "character": unit.character, "cell": cell,
            "source_pixel_sha256": unit.source_pixel_sha256,
            "output_pixel_sha256": sha256_bytes(&pixels),
            "font_name": raster.font_name, "font_sha256": raster.font_sha256,
            "font_px": font_px, "vertical_shift_px": vertical_shift_px,
            "ink_bounds": raster.ink_bounds,
        }));
    }
    Ok((claims, reports))
}
