//! Source-bound portrait lettering and its original archive.
use super::runtime_bundle_build::encode_runtime_member;
use crate::{
    compression::decompress, pipeline::sha256_bytes, source_disc::SupportedSourceDisc,
    tim::parse_8bpp, tzz::parse_tzz,
};
use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Authoring {
    archive_path: String,
    archive_sha256: String,
    member_count: usize,
    entries: Vec<Entry>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    member: usize,
    source_sha256: String,
    indices: PathBuf,
    indices_sha256: String,
    mask: PathBuf,
    mask_sha256: String,
}
use super::artwork::{
    SceneArtworkArchiveBuild, SceneArtworkArchiveReport, SceneArtworkMemberReport,
};
#[derive(Default)]
pub(super) struct PortraitBuild {
    pub archives: Vec<SceneArtworkArchiveBuild>,
    pub sources: Vec<Vec<u8>>,
    pub patched: Vec<Vec<u8>>,
}
fn read_bound(path: &Path, expected: &str) -> Result<Vec<u8>> {
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    ensure!(
        sha256_bytes(&bytes) == expected,
        "portrait input changed: {}",
        path.display()
    );
    Ok(bytes)
}
fn apply_indices(source: &[u8], indices: &[u8], mask: &[u8]) -> Result<Vec<u8>> {
    let tim = parse_8bpp(source)?;
    ensure!(
        tim.total_size == source.len(),
        "portrait TIM has unowned trailing data"
    );
    let pixels = &source[tim.pixel_offset..];
    ensure!(
        pixels.len() == indices.len() && pixels.len() == mask.len(),
        "portrait extent changed"
    );
    ensure!(
        mask.iter().all(|&v| v <= 1) && mask.contains(&1),
        "invalid portrait mask"
    );
    ensure!(
        pixels
            .iter()
            .zip(indices)
            .zip(mask)
            .all(|((&a, &b), &m)| m == 1 || a == b),
        "portrait write outside mask"
    );
    ensure!(pixels != indices, "portrait candidate has no change");
    let mut result = source.to_vec();
    result[tim.pixel_offset..].copy_from_slice(indices);
    Ok(result)
}
pub(super) fn build(
    source_disc: &SupportedSourceDisc,
    path: Option<&Path>,
    output: &Path,
) -> Result<PortraitBuild> {
    let Some(path) = path else {
        return Ok(PortraitBuild::default());
    };
    let plan: Authoring = serde_json::from_slice(&std::fs::read(path)?)?;
    ensure!(
        !plan.entries.is_empty(),
        "portrait authoring has no entries"
    );
    let (_, original) = source_disc.read_record(&plan.archive_path)?;
    ensure!(
        sha256_bytes(&original) == plan.archive_sha256,
        "portrait archive source changed"
    );
    let members = parse_tzz(&original)?;
    ensure!(
        members.len() == plan.member_count,
        "portrait member population changed"
    );
    let mut archive = original.clone();
    let mut result = PortraitBuild::default();
    let mut reports = Vec::new();
    let mut seen = BTreeSet::new();
    for entry in plan.entries {
        ensure!(seen.insert(entry.member), "duplicate portrait member");
        let member = members
            .get(entry.member)
            .context("portrait member out of range")?;
        let stored = &original[member.compressed_range()];
        let source = decompress(stored, false)?;
        ensure!(
            sha256_bytes(&source) == entry.source_sha256,
            "portrait member source changed"
        );
        let indices = read_bound(&entry.indices, &entry.indices_sha256)?;
        let mask = read_bound(&entry.mask, &entry.mask_sha256)?;
        let patched = apply_indices(&source, &indices, &mask)?;
        let encoded = encode_runtime_member(stored, &source, &patched)?;
        ensure!(
            encoded.bytes.len() <= stored.len(),
            "portrait exceeds original stream slot"
        );
        let mut padded = encoded.bytes.clone();
        padded.resize(stored.len(), 0);
        ensure!(
            decompress(&padded, true)? == patched,
            "portrait stream roundtrip changed"
        );
        archive[member.compressed_range()].copy_from_slice(&padded);
        reports.push(SceneArtworkMemberReport {
            index: entry.member,
            source_decoded_sha256: sha256_bytes(&source),
            patched_decoded_sha256: sha256_bytes(&patched),
            source_compressed_size: stored.len(),
            rebuilt_compressed_size: encoded.bytes.len(),
            outside_mask_preserved: true,
            compression_requirement_satisfied: encoded.compression_requirement_satisfied,
        });
        result.sources.push(source);
        result.patched.push(patched);
    }
    // Only original compressed extents were assigned; tables, unused members and padding remain original.
    let report = SceneArtworkArchiveReport {
        path: plan.archive_path.clone(),
        source_size: original.len(),
        source_sha256: plan.archive_sha256,
        patched_sha256: sha256_bytes(&archive),
        members: reports,
    };
    std::fs::write(
        output.join(
            Path::new(&plan.archive_path)
                .file_name()
                .context("portrait archive filename missing")?,
        ),
        &archive,
    )?;
    result.archives.push(SceneArtworkArchiveBuild {
        data: archive,
        report,
    });
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn source() -> Vec<u8> {
        let mut b = vec![0; 548];
        for (offset, value) in [(0, 16u32), (4, 9), (8, 524), (532, 16)] {
            b[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        for (offset, value) in [(16, 256u16), (18, 1), (540, 2), (542, 1)] {
            b[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
        }
        b[544..].copy_from_slice(&[1, 2, 3, 4]);
        b
    }
    #[test]
    fn lettering_preserves_palette_and_unselected_indices() {
        let source = source();
        let output = apply_indices(&source, &[1, 9, 3, 4], &[0, 1, 0, 0]).unwrap();
        assert_eq!(&source[..544], &output[..544]);
        assert_eq!(&output[544..], &[1, 9, 3, 4]);
    }
    #[test]
    fn rejects_unselected_writes_and_invalid_masks() {
        let source = source();
        assert!(apply_indices(&source, &[1, 9, 3, 5], &[0, 1, 0, 0]).is_err());
        assert!(apply_indices(&source, &[1, 9, 3, 4], &[0, 2, 0, 0]).is_err());
        assert!(apply_indices(&source, &[1, 9, 3], &[0, 1, 0]).is_err());
    }
}
