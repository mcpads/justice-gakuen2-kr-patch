//! The native stat arrow uses a separate Japanese-width table. Preparation
//! derives its replacement from the same translated clauses as the message bytes.
use crate::{
    decoded_record_write_plan::{DecodedDataClaim, DecodedRecordWritePlan},
    pipeline::sha256_bytes,
};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
fn asset() -> Result<&'static [u8]> {
    crate::product_assets::read("dialogue/stat-result-layout.json")
}
const TABLE: usize = 0x21d4;
const GLYPH_ADVANCE: usize = 20;
const ARROW_WIDTH: usize = 48;
const MINIMUM_CLEARANCE: usize = 12;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Asset {
    arrow_alignment: String,
    stats: Vec<Binding>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Binding {
    source_stat: String,
    source_arrow_offset: u16,
    up: String,
    down: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatResultLayout {
    pub arrow_offsets: [u16; 9],
}
pub(super) fn source_layout() -> StatResultLayout {
    StatResultLayout {
        arrow_offsets: [88, 88, 68, 68, 108, 108, 88, 88, 68],
    }
}
pub(super) fn asset_sha256() -> Result<String> {
    Ok(sha256_bytes(asset()?))
}
fn offset(segments: &[String]) -> Result<u16> {
    ensure!(
        segments.len() == 6
            && segments[0].is_empty()
            && segments[3].is_empty()
            && segments[5].is_empty(),
        "stat result segment shape changed"
    );
    let prefix = format!("{}{}", segments[1], segments[2]);
    ensure!(
        prefix.ends_with(' ') && !prefix.trim_end().contains([' ', '\n', '\r', '\t']),
        "stat result label needs a compact prefix and an arrow separator"
    );
    ensure!(
        segments[4].starts_with(' ') && !segments[4].contains(['\n', '\r', '\t']),
        "stat result needs clearance after its arrow"
    );
    let visible = prefix.trim_end().chars().count();
    ensure!(
        (2..=5).contains(&visible),
        "stat result prefix exceeds the native row"
    );
    let label_end = visible * GLYPH_ADVANCE;
    // Two protected padding controls plus authored spaces reserve the sprite.
    let result_x =
        (prefix.chars().count() + 2 + segments[4].chars().take_while(|c| *c == ' ').count())
            * GLYPH_ADVANCE;
    let clearance = result_x
        .checked_sub(label_end + ARROW_WIDTH)
        .context("stat result has no room for its arrow")?;
    // Share the free space between the end of the complete label (including
    // its particle) and the first result glyph. Neither side gets a bias.
    let arrow = label_end + clearance / 2;
    ensure!(
        clearance >= 2 * MINIMUM_CLEARANCE
            && (prefix.chars().count() + 2 + segments[4].chars().count()) * GLYPH_ADVANCE <= 400,
        "stat result arrow or text escapes its row"
    );
    Ok(arrow as u16)
}
pub(super) fn prepare(translations: &BTreeMap<String, Vec<String>>) -> Result<StatResultLayout> {
    let asset: Asset = serde_json::from_slice(asset()?)?;
    ensure!(
        asset.arrow_alignment == "center_between_text_runs",
        "unsupported stat arrow alignment"
    );
    let rows = asset.stats;
    ensure!(rows.len() == 9, "stat result source population changed");
    let mut layout = source_layout();
    for (index, row) in rows.iter().enumerate() {
        ensure!(
            !row.source_stat.is_empty() && row.source_arrow_offset == layout.arrow_offsets[index],
            "stat result source ordering changed"
        );
        match (translations.get(&row.up), translations.get(&row.down)) {
            (None, None) => (),
            (Some(up), Some(down)) => {
                let up = offset(up)?;
                ensure!(
                    up == offset(down)?,
                    "stat increase/decrease prefixes differ"
                );
                layout.arrow_offsets[index] = up;
            }
            _ => anyhow::bail!("stat result has only one translated direction"),
        }
    }
    Ok(layout)
}
pub(super) fn register(
    source: &[u8],
    layout: StatResultLayout,
    plan: &mut DecodedRecordWritePlan<'_>,
) -> Result<()> {
    let expected: Vec<u8> = source_layout()
        .arrow_offsets
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .collect();
    ensure!(
        source.get(TABLE..TABLE + 18) == Some(expected.as_slice()),
        "native stat-arrow offsets changed"
    );
    // 800bc128..150 reads stat index +55a, this table and the row origin
    // at 800a415c, then writes packet X. It does not affect the stat value.
    let consumer = source
        .get(0x1a128..0x1a154)
        .context("stat arrow consumer truncated")?;
    ensure!(
        sha256_bytes(consumer)
            == "c32e6247055dcda3ce65f1928d550e0bc0f753fed0b8ebc33b87b43cfec54116",
        "native stat arrow lookup changed"
    );
    ensure!(
        layout
            .arrow_offsets
            .iter()
            .enumerate()
            .all(|(index, v)| *v == source_layout().arrow_offsets[index]
                || [56, 76, 96, 116].contains(v)),
        "invalid prepared stat arrow coordinate"
    );
    if layout == source_layout() {
        return Ok(());
    }
    let mut candidate = source.to_vec();
    let data: Vec<u8> = layout
        .arrow_offsets
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .collect();
    candidate[TABLE..TABLE + 18].copy_from_slice(&data);
    plan.register_data_candidate(
        "stat result layout",
        &sha256_bytes(source),
        &candidate,
        &DecodedDataClaim::from_ranges(
            "stat-result-arrow-offsets",
            "align the native arrow with the prepared result text",
            vec![[TABLE, TABLE + 18]],
        ),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    fn row(label: &str) -> Vec<String> {
        ["", label, "이 ", "", " 올랐다!!", ""]
            .map(String::from)
            .to_vec()
    }
    #[test]
    fn arrow_clearance_tracks_short_and_long_korean_labels() {
        assert_eq!(offset(&row("킥")).unwrap(), 56);
        assert_eq!(offset(&row("펀치")).unwrap(), 76);
        assert_eq!(offset(&row("스피드")).unwrap(), 96);
        let potential = offset(&row("잠재능력")).unwrap() as usize;
        assert_eq!(potential - 100, 180 - (potential + ARROW_WIDTH));
        assert!(potential >= 100 + MINIMUM_CLEARANCE);
        assert!(offset(&row("잠재 능력")).is_err());
        let mut text = row("체력");
        text[4] = "올랐다!!".into();
        assert!(offset(&text).is_err());
    }
    #[test]
    #[ignore = "requires assets/"]
    fn untranslated_population_retains_original_geometry() {
        assert_eq!(prepare(&BTreeMap::new()).unwrap(), source_layout());
    }
}
