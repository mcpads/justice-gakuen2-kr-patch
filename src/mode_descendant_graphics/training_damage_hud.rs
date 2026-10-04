//! The two damage captions share ten 16-by-8 source glyph cells. Their
//! three-descriptor renderers and independently drawn numeric digits stay native.
//! The combo counter owns a separate 32x16 unit cell selected by its colon sentinel.

use std::path::Path;

use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

use super::super::model::ModeDescendantGraphicsBuildConfig;
use super::super::record_compositor::source_for_spec;
use super::super::source::ModeDescendantSourceRecord;
use super::{SPECS, TIM_OFFSET};
use crate::decoded_record_write_plan::DecodedDataClaim;
use crate::font::{HorizontalTextAlignment, IndexedTextRasterizers};
use crate::pipeline::sha256_bytes;
use crate::tim::{Cell, write_indexed_cell_in_prefix_with_report};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    kind: String,
    entries: Vec<Entry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    id: String,
    source_text: String,
    korean_text: String,
}

#[derive(Debug, Serialize)]
pub struct TrainingDamageHudBuildReport {
    pub manifest_sha256: String,
    pub font_sha256: String,
    pub font_px: f32,
    pub vertical_shift_px: i32,
    pub total_combo_damage: String,
    pub damage: String,
    pub caption_extent_px: [usize; 2],
    pub combo_unit: String,
    pub combo_unit_font_px: f32,
    pub numeric_digit_code_and_pixels_preserved: bool,
}

pub(super) struct HudBuild {
    pub texture_claims: Vec<DecodedDataClaim>,
    pub overlay_claims: Vec<DecodedDataClaim>,
    pub report: TrainingDamageHudBuildReport,
}

fn cells_for_captions(total: &str, damage: &str) -> Result<Vec<char>> {
    let text = total.chars().collect::<Vec<_>>();
    ensure!(
        !text.is_empty()
            && text.len() <= 8
            && damage.chars().count() == 2
            && total.ends_with(damage)
            && !total.contains(['\n', '\r']),
        "training damage captions must fit the shared native suffix and blank cells"
    );
    let mut cells = vec![' '; 10 - text.len()];
    cells.extend(text);
    Ok(cells)
}

pub(super) fn apply(
    config: &ModeDescendantGraphicsBuildConfig,
    rasterizers: &mut IndexedTextRasterizers,
    sources: &[ModeDescendantSourceRecord],
    patched_texture: &mut [u8],
    patched_overlay: &mut [u8],
    path: &Path,
) -> Result<HudBuild> {
    let bytes = std::fs::read(path)?;
    let manifest: Manifest = serde_json::from_slice(&bytes)?;
    ensure!(
        manifest.kind == "justice_gakuen2_training_damage_hud" && manifest.entries.len() == 3,
        "training damage HUD must cover both captions and the combo unit"
    );
    let total = &manifest.entries[0];
    let damage = &manifest.entries[1];
    ensure!(
        total.id == "total-combo-damage"
            && total.source_text == "トータルコンボダメージ"
            && damage.id == "damage"
            && damage.source_text == "ダメージ",
        "training damage caption source identity changed"
    );
    let cells = cells_for_captions(&total.korean_text, &damage.korean_text)?;
    let texture = source_for_spec(sources, &SPECS[0])?;
    let overlay = source_for_spec(sources, &SPECS[1])?;
    // +0x9bc/+0xcdc bind these arrays; +0xb30/+0xe30 fix the draw count
    // at three. U/width multiply by 16, V multiplies by 8 and adds 112.
    for (offset, expected) in [
        (0x16c, [0, 0, 9, 1, 0, 1, 9, 0, 1]),
        (0x178, [7, 0, 2, 1, 0, 1, 9, 0, 1]),
    ] {
        ensure!(
            overlay.decoded.get(offset..offset + 9) == Some(expected.as_slice()),
            "training damage HUD descriptor changed"
        );
    }
    let font = &config.fonts.training_text;
    ensure!(
        font.font_px == 8.0 && font.vertical_shift_px == 0,
        "training pixel font must fit the native eight-pixel HUD"
    );
    let rasterizer = rasterizers.for_font(&font.path)?;
    let mut pixels = vec![0; 160 * 8];
    let mut font_sha256 = String::new();
    for (index, glyph) in cells.into_iter().enumerate() {
        if glyph == ' ' {
            continue;
        }
        // Dalmoori's native eight-pixel grid fits these cells without resampling.
        let raster = rasterizer.rasterize_shifted(
            &glyph.to_string(),
            8,
            8,
            font.font_px,
            0.0,
            font.vertical_shift_px,
            0,
            None,
            15,
            HorizontalTextAlignment::Center,
        )?;
        font_sha256 = raster.font_sha256;
        let ink = raster.pixels.iter().map(|p| *p == 15).collect::<Vec<_>>();
        ensure!(ink.iter().any(|p| *p), "training HUD glyph has no ink");
        for y in 0usize..8 {
            for x in 0..8 {
                let value = if ink[y * 8 + x] {
                    15
                } else if (y.saturating_sub(1)..=(y + 1).min(7))
                    .any(|ny| (x.saturating_sub(1)..=(x + 1).min(7)).any(|nx| ink[ny * 8 + nx]))
                {
                    1
                } else {
                    0
                };
                pixels[y * 160 + index * 16 + x * 2] = value;
                pixels[y * 160 + index * 16 + x * 2 + 1] = value;
            }
        }
    }
    let write = write_indexed_cell_in_prefix_with_report(
        patched_texture,
        TIM_OFFSET,
        Cell {
            x: 0,
            y: 112,
            width: 160,
            height: 8,
        },
        &pixels,
    )?;
    let mut texture_claims = DecodedDataClaim::from_effective_ranges(
        "training-damage-hud",
        "render both native captions through their shared glyph suffix",
        &texture.decoded,
        patched_texture,
        write.allowed_ranges,
    )?;
    // The counter formats "%2d:" at +0x1e8c. Its three-character loop
    // subtracts '0', so ':' selects UV entry 10 at +0x3c, (96,136).
    // The same loop sets 32x16 sprites; digits use entries 0..9.
    let unit = &manifest.entries[2];
    ensure!(
        unit.id == "combo-hit-unit"
            && unit.source_text == "発"
            && unit.korean_text.chars().count() == 1,
        "training combo unit source changed"
    );
    ensure!(
        overlay.decoded.get(0x1e8c..0x1e91) == Some(b"%2d:\0".as_slice())
            && overlay.decoded.get(0x3c..0x3e) == Some([96, 136].as_slice())
            && sha256_bytes(&overlay.decoded[0x1230..0x13dc])
                == "4e057179c9964628369b33cd44c77bcf5a073cf91d647eea9135a7c1425c4acd",
        "training combo unit consumer changed"
    );
    let unit_font_px = font.font_px * 2.0;
    let unit_raster = rasterizer.rasterize_shifted(
        &unit.korean_text,
        16,
        16,
        unit_font_px,
        0.0,
        0,
        0,
        Some(1),
        15,
        HorizontalTextAlignment::Center,
    )?;
    ensure!(
        unit_raster.pixels.contains(&15),
        "training combo unit has no ink"
    );
    let unit_pixels = unit_raster
        .pixels
        .iter()
        .flat_map(|pixel| std::iter::repeat_n(*pixel, 2))
        .collect::<Vec<_>>();
    let unit_write = write_indexed_cell_in_prefix_with_report(
        patched_texture,
        TIM_OFFSET,
        Cell {
            x: 96,
            y: 136,
            width: 32,
            height: 16,
        },
        &unit_pixels,
    )?;
    texture_claims.extend(DecodedDataClaim::from_effective_ranges(
        "training-combo-unit",
        "replace the complete combo unit cell and preserve numeric digits",
        &texture.decoded,
        patched_texture,
        unit_write.allowed_ranges,
    )?);
    let mut overlay_claims = Vec::new();
    // The first two atlas cells are blank; trailing blank spans preserve the
    // original caption advances (176/64). Number origins and digits are untouched.
    for (offset, descriptors) in [
        (0x16c, [0, 0, 9, 9, 0, 1, 0, 0, 1]),
        (0x178, [8, 0, 1, 9, 0, 1, 0, 0, 2]),
    ] {
        patched_overlay[offset..offset + 9].copy_from_slice(&descriptors);
        overlay_claims.extend(DecodedDataClaim::from_effective_ranges(
            &format!("training-damage-hud:{offset:x}"),
            "bind the complete Korean caption while preserving numeric layout",
            &overlay.decoded,
            patched_overlay,
            vec![[offset, offset + 9]],
        )?);
    }
    Ok(HudBuild {
        texture_claims,
        overlay_claims,
        report: TrainingDamageHudBuildReport {
            manifest_sha256: sha256_bytes(&bytes),
            font_sha256,
            font_px: font.font_px,
            vertical_shift_px: font.vertical_shift_px,
            total_combo_damage: total.korean_text.clone(),
            damage: damage.korean_text.clone(),
            caption_extent_px: [176, 64],
            combo_unit: unit.korean_text.clone(),
            combo_unit_font_px: unit_font_px,
            numeric_digit_code_and_pixels_preserved: true,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn damage_suffix_is_shared_without_consuming_native_blank_spans() {
        let cells = cells_for_captions("총 콤보 피해", "피해").unwrap();
        assert_eq!(&cells[..2], &[' ', ' ']);
        assert_eq!(&cells[8..], &['피', '해']);
        assert!(cells_for_captions("너무 긴 콤보 피해", "피해").is_err());
        assert!(cells_for_captions("총 콤보 피해", "손상").is_err());
    }
}
