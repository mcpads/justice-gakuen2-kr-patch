//! Full-frame scene lettering, composed beside the separately owned location strip.
use super::model::DiarySceneBuildConfig;
use crate::{
    pipeline::{sha256_bytes, write_pretty_json_and_hash},
    tim::parse_8bpp,
};
use anyhow::{Result, ensure};
use serde::Deserialize;
use std::{
    collections::BTreeMap,
    path::{Component, Path, PathBuf},
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Asset {
    source_record: String,
    release_status: String,
    entries: Vec<Entry>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    member: usize,
    source_tim_sha256: String,
    indices: PathBuf,
    indices_sha256: String,
    wording: Vec<[String; 2]>,
}
pub(super) struct Background {
    pub bytes: Vec<u8>,
    pub range: [usize; 2],
}
fn apply(source: &[u8], indices: &[u8]) -> Result<Background> {
    let tim = parse_8bpp(source)?;
    ensure!(
        tim.pixel_width() == 256 && tim.image_height == 244 && tim.total_size == source.len(),
        "scene background geometry changed"
    );
    ensure!(
        indices.len() == 256 * 220,
        "scene background must own exactly the two 256x110 image halves"
    );
    let range = [tim.pixel_offset, tim.pixel_offset + indices.len()];
    let mut bytes = source.to_vec();
    bytes[range[0]..range[1]].copy_from_slice(indices);
    ensure!(bytes != source, "scene background candidate has no change");
    Ok(Background { bytes, range })
}
pub(super) fn load(
    config: &DiarySceneBuildConfig,
    path: Option<&Path>,
    source: &[Vec<u8>],
) -> Result<BTreeMap<usize, Background>> {
    let Some(path) = path else {
        return Ok(BTreeMap::new());
    };
    let asset_bytes = std::fs::read(path)?;
    let asset: Asset = serde_json::from_slice(&asset_bytes)?;
    ensure!(
        asset.source_record == "DAT2/MGBG.TZZ"
            && asset.release_status == "needs_human_review"
            && !asset.entries.is_empty(),
        "invalid scene background source or review boundary"
    );
    let mut result = BTreeMap::new();
    let mut reports = Vec::new();
    for entry in asset.entries {
        ensure!(
            entry
                .indices
                .components()
                .all(|c| matches!(c, Component::Normal(_))),
            "unsafe scene background path"
        );
        let original = source
            .get(entry.member)
            .ok_or_else(|| anyhow::anyhow!("scene background member outside catalogue"))?;
        ensure!(
            sha256_bytes(original) == entry.source_tim_sha256,
            "scene background source changed"
        );
        let indices = std::fs::read(
            config
                .assets
                .join("../../private-artwork/scenes")
                .join(&entry.indices),
        )?;
        ensure!(
            sha256_bytes(&indices) == entry.indices_sha256,
            "scene background authored bytes changed"
        );
        ensure!(
            !entry.wording.is_empty()
                && entry
                    .wording
                    .iter()
                    .all(|w| w.iter().all(|s| !s.trim().is_empty())),
            "missing scene lettering review"
        );
        let candidate = apply(original, &indices)?;
        reports.push(serde_json::json!({"member":entry.member,"source_tim_sha256":entry.source_tim_sha256,"indices_sha256":entry.indices_sha256,"wording":entry.wording,"owned_range":candidate.range,"full_background_image":true,"palette_header_and_location_strip_preserved":true,"release_status":asset.release_status}));
        ensure!(
            result.insert(entry.member, candidate).is_none(),
            "duplicate scene background member"
        );
    }
    write_pretty_json_and_hash(
        &config.output_dir.join("scene-backgrounds-build.json"),
        &serde_json::json!({"asset_sha256":sha256_bytes(&asset_bytes),"entries":reports}),
        true,
    )?;
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn background_cannot_overwrite_palette_or_location_strip() {
        let mut source = vec![0u8; 544 + 256 * 244];
        for (at, v) in [(0, 0x10u32), (4, 9), (8, 524), (532, 12 + 256 * 244)] {
            source[at..at + 4].copy_from_slice(&v.to_le_bytes());
        }
        for (at, v) in [(16, 256u16), (18, 1), (540, 128), (542, 244)] {
            source[at..at + 2].copy_from_slice(&v.to_le_bytes());
        }
        source[544 + 256 * 220..].fill(0x7f);
        let result = apply(&source, &vec![19; 256 * 220]).unwrap();
        assert_eq!(&result.bytes[..544], &source[..544]);
        assert_eq!(&result.bytes[544 + 256 * 220..], &source[544 + 256 * 220..]);
        assert!(result.bytes[544..544 + 256 * 220].iter().all(|&v| v == 19));
        assert!(apply(&source, &vec![19; 256 * 244]).is_err());
    }
}
