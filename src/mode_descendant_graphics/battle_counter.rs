//! The first-strike caption and hit unit share CEFT1's yellow/blue palette.
//! Clear their full cells; the adjacent zero and the animated digits stay intact.

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

use super::{ModeDescendantGraphicsBuildConfig, ModeDescendantRecord, ModeDescendantRecordDraft};
use crate::decoded_record_write_plan::DecodedDataClaim;
use crate::font::{HorizontalTextAlignment, IndexedTextRasterizers};
use crate::pipeline::sha256_bytes;
use crate::tim::{Cell, read_indexed_cell_in_prefix, write_indexed_cell_in_prefix_with_report};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Label {
    id: String,
    source_text: String,
    korean_text: String,
}

#[derive(Debug, Serialize)]
pub struct LabelReport {
    pub id: String,
    pub source_text: String,
    pub korean_text: String,
    pub cell: [usize; 4],
    pub font_sha256: String,
    pub font_px: f32,
    pub indices_sha256: String,
}

pub(super) fn apply(
    config: &ModeDescendantGraphicsBuildConfig,
    rasterizers: &mut IndexedTextRasterizers,
    labels: &[Label],
    drafts: &mut [ModeDescendantRecordDraft],
) -> Result<Vec<LabelReport>> {
    let source_labels = [
        (
            "first-strike",
            "先制",
            664,
            48,
            "759fc3cc03d57ea1bba2ec4de9c4292316cfd07c2918d175f7ef8a89302ef069",
        ),
        (
            "combo-hit-unit",
            "発",
            712,
            24,
            "8c9ea104e1bc927f687978589cd4e28199063b899ddc65248b286a8817625f2a",
        ),
    ];
    ensure!(
        labels.len() == source_labels.len(),
        "battle counter family is incomplete"
    );
    let draft = drafts
        .iter_mut()
        .find(|d| d.spec.record == ModeDescendantRecord::BattleEffects)
        .context("battle counter requires CEFT1")?;
    let tim_offset = 0x10800;
    let palette = tim_offset + 20 + 1088 * 2;
    ensure!(
        sha256_bytes(&draft.decoded[palette..palette + 32])
            == "51d094c6c2625da1c0b03f0ab3c9ea1d7d7151b5d55a85d7893c23c1af1f0170",
        "battle counter palette changed"
    );
    let font = &config.fonts.battle_counter;
    let rasterizer = rasterizers.for_font(&font.path)?;
    let mut reports = Vec::new();
    for (label, (id, source, x, width, source_hash)) in labels.iter().zip(source_labels) {
        ensure!(
            label.id == id && label.source_text == source && !label.korean_text.is_empty(),
            "battle counter source label changed"
        );
        let cell = Cell {
            x,
            y: 144,
            width,
            height: 16,
        };
        ensure!(
            sha256_bytes(&read_indexed_cell_in_prefix(
                &draft.decoded,
                tim_offset,
                cell
            )?) == source_hash,
            "battle counter source cell changed or is already owned"
        );
        let glyphs = rasterizer.rasterize_shifted(
            &label.korean_text,
            width,
            16,
            font.font_px,
            0.0,
            font.vertical_shift_px,
            0,
            Some(11),
            10,
            HorizontalTextAlignment::Center,
        )?;
        ensure!(
            glyphs.measured_advance_px <= (width - 2) as f32 && glyphs.pixels.contains(&10),
            "battle counter label does not fit: {}",
            label.korean_text
        );
        let before = draft.decoded.clone();
        let write = write_indexed_cell_in_prefix_with_report(
            &mut draft.decoded,
            tim_offset,
            cell,
            &glyphs.pixels,
        )?;
        ensure!(
            read_indexed_cell_in_prefix(&draft.decoded, tim_offset, cell)? == glyphs.pixels,
            "battle counter readback differs"
        );
        draft
            .decoded_write_claims
            .extend(DecodedDataClaim::from_effective_ranges(
                id,
                "replace complete battle counter cell, retaining palette and adjacent digits",
                &before,
                &draft.decoded,
                write.allowed_ranges,
            )?);
        reports.push(LabelReport {
            id: label.id.clone(),
            source_text: label.source_text.clone(),
            korean_text: label.korean_text.clone(),
            cell: [x, 144, width, 16],
            font_sha256: glyphs.font_sha256,
            font_px: font.font_px,
            indices_sha256: sha256_bytes(&glyphs.pixels),
        });
    }
    Ok(reports)
}
