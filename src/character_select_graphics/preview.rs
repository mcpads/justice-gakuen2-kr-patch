use std::path::Path;

use anyhow::{Context, Result};

use crate::embedded_tim::{decode_embedded_tim_preview, parse_embedded_tim_at};
use crate::pipeline::{sha256_bytes, sha256_file};

use super::model::CharacterSelectTargetTimBuild;

pub(super) use crate::tim_preview::write_tim_preview;

pub(super) fn tim_sha256(decoded: &[u8], tim_offset: usize) -> Result<String> {
    let source = decoded
        .get(tim_offset..)
        .context("character-select target TIM offset escaped its decoded record")?;
    let tim = crate::tim::parse_4bpp_prefix(source)?;
    Ok(sha256_bytes(&source[..tim.total_size]))
}

pub(super) fn write_atlas_preview(
    output_dir: &Path,
    patched: &[u8],
    tim_offset: usize,
    file_name: &str,
) -> Result<()> {
    let tim = parse_embedded_tim_at(patched, tim_offset)
        .context("patched character-select atlas disappeared")?;
    anyhow::ensure!(
        tim.bits_per_pixel == 4,
        "patched character-select atlas is not 4bpp"
    );
    let rgba = decode_embedded_tim_preview(patched, &tim)?;
    write_tim_preview(&output_dir.join(file_name), &rgba)
}

pub(super) fn build_target_tim_reports(
    output_dir: &Path,
    output_file: &str,
    source_decoded: &[u8],
    patched_decoded: &[u8],
    tim_offsets: impl IntoIterator<Item = usize>,
) -> Result<Vec<CharacterSelectTargetTimBuild>> {
    let stem = output_file
        .split_once('.')
        .map(|(stem, _)| stem)
        .unwrap_or(output_file);
    let mut reports = Vec::new();
    for tim_offset in tim_offsets {
        let preview_file = format!("character-select-{stem}-tim-{tim_offset:05x}.png");
        write_atlas_preview(output_dir, patched_decoded, tim_offset, &preview_file)?;
        reports.push(CharacterSelectTargetTimBuild {
            target_tim_offset: tim_offset,
            source_target_tim_sha256: tim_sha256(source_decoded, tim_offset)?,
            patched_target_tim_sha256: tim_sha256(patched_decoded, tim_offset)?,
            atlas_preview_sha256: sha256_file(&output_dir.join(&preview_file))?,
            atlas_preview_file: preview_file,
        });
    }
    Ok(reports)
}
