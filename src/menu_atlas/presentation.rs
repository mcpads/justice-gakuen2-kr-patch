//! Compose shared menu typography into MENU and the standalone MOJI2 reload.
use std::path::Path;

use anyhow::{Result, ensure};
use serde::Deserialize;

use crate::compression::decompress;
use crate::decoded_record_write_plan::{DecodedDataClaim, DecodedRecordWritePlan};
use crate::disc::rebuild::DiscRecordSourceIdentity;
use crate::font::{HorizontalTextAlignment, IndexedTextRasterizer};
use crate::menu_compression::compress_menu_with_source_limits;
use crate::options::OptionsFontStyle;
use crate::pipeline::{sha256_bytes, write_pretty_json_and_hash};
use crate::source_disc::SupportedSourceDisc;
use crate::tim::{Cell, read_indexed_cell_in_prefix, write_indexed_cell_in_prefix_with_report};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Asset {
    source_record: String,
    source_decoded_sha256: String,
    reload_record: String,
    reload_stored_sha256: String,
    reload_decoded_sha256: String,
    font_role: String,
    release_status: String,
    units: Vec<Unit>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Unit {
    id: String,
    source_text: String,
    text: String,
    #[serde(default)]
    right_text: Option<String>,
    cell: Cell,
    source_pixel_sha256: String,
}

pub(crate) struct SharedMenuAtlasBuild {
    pub(crate) reload_path: String,
    pub(crate) reload_source: DiscRecordSourceIdentity,
    pub(crate) reload_stored: Vec<u8>,
    source_sha256: String,
    candidate: Vec<u8>,
    claims: Vec<DecodedDataClaim>,
}

impl SharedMenuAtlasBuild {
    pub(crate) fn register(&self, plan: &mut DecodedRecordWritePlan<'_>) -> Result<()> {
        plan.register_data_candidate(
            "shared menu typography",
            &self.source_sha256,
            &self.candidate,
            &self.claims,
        )
    }
}

pub(crate) fn prepare(
    source: &SupportedSourceDisc,
    menu: &[u8],
    style: &OptionsFontStyle,
    numeric_style: &crate::development_build_spec::SizedFontSource,
    output: &Path,
) -> Result<SharedMenuAtlasBuild> {
    let asset_bytes = crate::product_assets::read("menu/bonus-inventory/viewer-panel.json")?;
    let asset: Asset = serde_json::from_slice(asset_bytes)?;
    ensure!(
        asset.source_record == "DAT2/MENU.BIZ" && asset.font_role == "options.help",
        "viewer panel source or role changed"
    );
    let source_sha256 = sha256_bytes(menu);
    ensure!(
        source_sha256 == asset.source_decoded_sha256,
        "viewer panel MENU source changed"
    );
    ensure!(
        asset.release_status == "needs_human_review",
        "unreviewed viewer panel release status changed"
    );
    let (_, overlay) = source.read_record("DAT1/KOUBAI2.BIN")?;
    // Both native SPRT paths: page 15, UV (120,160), size 128x72,
    // CLUT (448,480), RGB 128, and screen position (368,392).
    for (start, end, hash) in [
        (
            0x51a0,
            0x51dc,
            "c05d45fc9e8547019480930e238695621f3595fa7caf2f3086083561fe191ba4",
        ),
        (
            0xeebc,
            0xeef8,
            "c05d45fc9e8547019480930e238695621f3595fa7caf2f3086083561fe191ba4",
        ),
        (
            0x4e70,
            0x4f48,
            "db56c470881570c6187e24d6ae8f5d1493c08159450fc210f982c1bf3304f242",
        ),
        (
            0xfc6c,
            0xfd2c,
            "8bc5b93c81bfb834b3078be44fb275175312e4a40eec398d4a2743de53658306",
        ),
    ] {
        ensure!(
            overlay
                .get(start..end)
                .is_some_and(|b| sha256_bytes(b) == hash),
            "viewer panel native consumer changed at {start:#x}"
        );
    }
    let expected = [
        (
            "next",
            "次ページへ",
            Cell {
                x: 892,
                y: 164,
                width: 88,
                height: 22,
            },
        ),
        (
            "previous",
            "前ページへ",
            Cell {
                x: 892,
                y: 186,
                width: 88,
                height: 22,
            },
        ),
        (
            "return",
            "おまけ画面に戻る START",
            Cell {
                x: 892,
                y: 208,
                width: 120,
                height: 20,
            },
        ),
    ];
    ensure!(
        asset.units.len() == expected.len(),
        "viewer panel command population changed"
    );
    let rasterizer = IndexedTextRasterizer::load(&style.font)?;
    let mut candidate = menu.to_vec();
    let mut claims = Vec::new();
    let mut reports = Vec::new();
    for (unit, (id, text, cell)) in asset.units.iter().zip(expected) {
        ensure!(
            unit.id == id && unit.source_text == text && unit.cell == cell,
            "viewer panel source rectangle changed for {id}"
        );
        let original = read_indexed_cell_in_prefix(menu, 0, cell)?;
        ensure!(
            sha256_bytes(&original) == unit.source_pixel_sha256,
            "viewer panel source pixels changed for {id}"
        );
        // This CLUT's light face is index 8, dark outline 1; 15 is pink.
        let raster = rasterizer.rasterize(
            &unit.text,
            cell.width,
            cell.height,
            style.font_px,
            0.0,
            0,
            Some(1),
            8,
            HorizontalTextAlignment::Left,
        )?;
        let mut ink = raster.pixels.clone();
        if let Some(right_text) = &unit.right_text {
            let right = rasterizer.rasterize(
                right_text,
                cell.width,
                cell.height,
                style.font_px,
                0.0,
                0,
                Some(1),
                8,
                HorizontalTextAlignment::Right,
            )?;
            ensure!(
                raster.ink_bounds[2] + 8 < right.ink_bounds[0],
                "viewer command and button label overlap"
            );
            for (pixel, right_pixel) in ink.iter_mut().zip(right.pixels) {
                if right_pixel != 0 {
                    *pixel = right_pixel;
                }
            }
        }
        let pixels = on_checkerboard(&ink, cell);
        let write = write_indexed_cell_in_prefix_with_report(&mut candidate, 0, cell, &pixels)?;
        claims.extend(DecodedDataClaim::from_ranges(
            &format!("viewer-panel-{id}"),
            "replace source text inside the shared viewer frame",
            write.allowed_ranges,
        ));
        reports.push(serde_json::json!({
            "id": id, "source_text": text, "text": unit.text,
            "right_text": unit.right_text, "cell": cell,
            "source_pixel_sha256": unit.source_pixel_sha256,
            "output_pixel_sha256": sha256_bytes(&pixels),
            "font_name": raster.font_name, "font_sha256": raster.font_sha256,
            "font_px": style.font_px, "ink_bounds": raster.ink_bounds,
        }));
    }
    let (numeric_claims, numeral_report) =
        super::native_text::apply_common_numerals(menu, &mut candidate, numeric_style)?;
    claims.extend(numeric_claims);
    // KOUBAI2 requests resource 0x2b into 0x800d4000, then uploads its TIM.
    // The standalone MOJI2 copy is byte-identical to the MENU prefix but is
    // loaded again on viewer entry. Updating MENU alone cannot survive it.
    ensure!(
        asset.reload_record == "DAT2/MOJI2.TIZ",
        "viewer reload record changed"
    );
    let (_, reload_source) = source.read_record(&asset.reload_record)?;
    ensure!(
        sha256_bytes(&reload_source) == asset.reload_stored_sha256,
        "viewer reload stored identity changed"
    );
    let reload_decoded = decompress(&reload_source, false)?;
    ensure!(
        sha256_bytes(&reload_decoded) == asset.reload_decoded_sha256
            && menu.get(..reload_decoded.len()) == Some(reload_decoded.as_slice()),
        "viewer reload atlas is not the bound MENU prefix"
    );
    let mut reload_plan = DecodedRecordWritePlan::new(
        &asset.reload_record,
        &reload_decoded,
        &asset.reload_decoded_sha256,
    )?;
    reload_plan.register_data_candidate(
        "shared menu typography reload",
        &asset.reload_decoded_sha256,
        &candidate[..reload_decoded.len()],
        &claims,
    )?;
    let reloaded = reload_plan.apply(None)?;
    let (mut reload_stored, _, _) = compress_menu_with_source_limits(&reloaded, &reload_source)?;
    reload_stored.resize(reload_source.len(), 0);
    ensure!(
        reload_stored[..4] == reload_source[..4] && decompress(&reload_stored, true)? == reloaded,
        "viewer reload record failed compressed readback"
    );
    std::fs::create_dir_all(output)?;
    write_pretty_json_and_hash(
        &output.join("viewer-panel-build.json"),
        &serde_json::json!({
            "source_record": asset.source_record, "source_decoded_sha256": source_sha256,
            "asset_sha256": sha256_bytes(asset_bytes), "font_role": asset.font_role,
            "release_status": asset.release_status, "units": reports,
            "consumer_record": "DAT1/KOUBAI2.BIN",
            "reload_record": asset.reload_record,
            "reload_source_stored_sha256": asset.reload_stored_sha256,
            "reload_source_decoded_sha256": asset.reload_decoded_sha256,
            "reload_output_decoded_sha256": sha256_bytes(&reloaded),
            "reload_consumer_spans": [[0x51a0,0x51dc],[0xeebc,0xeef8]],
            "consumer_spans": [[0x4e70,0x4f48],[0xfc6c,0xfd2c]],
            "palette_and_button_icons_preserved": true,
            "numerals": numeral_report,
        }),
        true,
    )?;
    Ok(SharedMenuAtlasBuild {
        reload_path: asset.reload_record,
        reload_source: DiscRecordSourceIdentity::from_bytes(&reload_source),
        reload_stored,
        source_sha256,
        candidate,
        claims,
    })
}

fn on_checkerboard(ink: &[u8], cell: Cell) -> Vec<u8> {
    ink.iter()
        .enumerate()
        .map(|(i, &pixel)| {
            if pixel == 0 && (cell.x + i % cell.width + cell.y + i / cell.width) % 2 == 1 {
                9
            } else {
                pixel
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewer_panel_restores_native_background_phase_without_overwriting_ink() {
        let cell = Cell {
            x: 893,
            y: 164,
            width: 3,
            height: 2,
        };
        assert_eq!(
            on_checkerboard(&[0, 0, 8, 1, 0, 0], cell),
            [9, 0, 8, 1, 9, 0]
        );
    }
}
