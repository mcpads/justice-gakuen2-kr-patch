//! Counter label glyphs live in the loaded term instruction member.
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

use super::{
    DynamicOverlayMemberPatch, HumanApproval, PracticalInstructionGraphicsBuildConfig,
    PracticalInstructionMemberBinding, tile_cell,
};
use crate::compression::decompress;
use crate::decoded_record_write_plan::DecodedDataClaim;
use crate::font::{HorizontalTextAlignment, IndexedTextRasterizer};
use crate::pipeline::sha256_bytes;
use crate::practical_instruction_graphics::counter_layout::{LABEL_CELLS, MEMBER, OWNED_TILES};
use crate::source_disc::SupportedSourceDisc;
use crate::tim::{read_indexed_cell_in_prefix, write_indexed_cell_in_prefix_with_report};
use crate::tzz::TzzMember;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Translation {
    source_text: String,
    korean_text: String,
    unit: String,
    ordinal_suffix: String,
    human_approval: HumanApproval,
}
#[derive(Debug, Serialize)]
pub struct AttemptCounterReport {
    pub member_index: usize,
    pub source_text: String,
    pub korean_text: String,
    pub translation_sha256: String,
    pub source_consumer_sha256: String,
    pub labels: Vec<String>,
    pub owned_tiles: Vec<usize>,
    pub dynamic_digits_provider: &'static str,
    pub source_transparent_allocation_verified: bool,
    pub human_approval: HumanApproval,
}

pub(super) fn build(
    config: &PracticalInstructionGraphicsBuildConfig,
    source: &SupportedSourceDisc,
    bindings: &[PracticalInstructionMemberBinding],
    archive: &[u8],
    members: &[TzzMember],
) -> Result<(DynamicOverlayMemberPatch, AttemptCounterReport)> {
    let bytes = std::fs::read(config.assets.join("attempt-counter.json"))?;
    let text: Translation = serde_json::from_slice(&bytes)?;
    ensure!(
        text.source_text == "①回目 / {count}回"
            && text.korean_text
                == format!(
                    "{{attempt}}{}{} / {{count}}{}",
                    text.unit, text.ordinal_suffix, text.unit
                )
            && text.unit.chars().count() == 1
            && text.ordinal_suffix.chars().count() == 1,
        "attempt counter wording no longer fits its source-bound fragments"
    );
    let (_, consumer) = source.read_record("DAT1/SIKEN2.BIN")?;
    let consumer_hash = sha256_bytes(&consumer);
    ensure!(
        consumer_hash == "0b23fd0dc314143ad13c16bb97816384d64cda15f7e205863bed42f091190f10",
        "attempt counter source consumer changed"
    );
    let binding = bindings
        .get(MEMBER)
        .context("attempt counter member disappeared")?;
    let member = members
        .get(MEMBER)
        .context("attempt counter source member disappeared")?;
    let decoded = decompress(&archive[member.compressed_range()], true)?;
    ensure!(
        sha256_bytes(&decoded) == binding.source_decoded_sha256,
        "attempt counter producer changed"
    );
    let panel = binding
        .presentation_layout
        .as_ref()
        .context("attempt counter panel binding missing")?;
    let owned_tiles = OWNED_TILES.into_iter().collect::<BTreeSet<_>>();
    ensure!(
        panel
            .source_tile_indices
            .iter()
            .all(|tile| !owned_tiles.contains(tile)),
        "attempt counter overlaps instruction text"
    );
    for tile in OWNED_TILES {
        ensure!(
            read_indexed_cell_in_prefix(&decoded, 0, tile_cell(tile))?
                .iter()
                .all(|p| *p == 0),
            "attempt counter allocation is not untouched transparent source"
        );
    }
    let labels = vec![
        "1".into(),
        "2".into(),
        "3".into(),
        text.unit,
        text.ordinal_suffix,
    ];
    let font = IndexedTextRasterizer::load(&config.body_font.path)?;
    let mut candidate = decoded.clone();
    let mut ranges = Vec::new();
    for (label, cell) in labels.iter().zip(LABEL_CELLS) {
        let raster = font.rasterize_shifted_with_coverage_ramp(
            label,
            cell.width,
            cell.height,
            config.body_font.font_px,
            0.0,
            config.body_font.vertical_shift_px,
            0,
            1,
            15,
            HorizontalTextAlignment::Center,
        )?;
        ensure!(
            raster.measured_advance_px <= cell.width as f32,
            "attempt counter label does not fit: {label}"
        );
        ranges.extend(
            write_indexed_cell_in_prefix_with_report(&mut candidate, 0, cell, &raster.pixels)?
                .allowed_ranges,
        );
    }
    let claims = DecodedDataClaim::from_effective_ranges(
        "practical-attempt-counter",
        "render every ordinal and unit label into the shared source-bound gameplay allocation",
        &decoded,
        &candidate,
        ranges,
    )?;
    Ok((
        DynamicOverlayMemberPatch {
            candidate,
            claims,
            owned_tiles,
            overlay_ids: vec!["practical_attempt_counter".into()],
        },
        AttemptCounterReport {
            member_index: MEMBER,
            source_text: text.source_text,
            korean_text: text.korean_text,
            translation_sha256: sha256_bytes(&bytes),
            source_consumer_sha256: consumer_hash,
            labels,
            owned_tiles: OWNED_TILES.to_vec(),
            dynamic_digits_provider: "CEFT4/CEFT6 shared decimal atlas; native values and UV table retained",
            source_transparent_allocation_verified: true,
            human_approval: text.human_approval,
        },
    ))
}
