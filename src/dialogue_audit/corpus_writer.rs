use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};
use serde::Serialize;

use crate::pipeline::sha256_bytes;

use super::corpus_model::{
    DialogueCorpusEntry, DialogueSourceCorpus, DialogueSourceCorpusBuildReport,
};

const CORPUS_MANIFEST_NAME: &str = "dialogue-source-corpus.json";
const MAXIMUM_CORPUS_SHARD_BYTES: usize = 4 * 1024 * 1024;
const MAXIMUM_CORPUS_SHARD_ENTRIES: usize = 256;

#[derive(Serialize)]
struct DialogueCorpusBankShard<'a> {
    kind: &'static str,
    source_bin_sha256: &'a str,
    codebook_sha256: &'a str,
    source_path: &'a str,
    stored_sha256: &'a str,
    decoded_sha256: &'a str,
    fixed_cell_count: usize,
    selector_index: usize,
    unit_index: usize,
    entry_count: usize,
    entries: &'a [DialogueCorpusEntry],
}

#[derive(Serialize)]
struct DialogueCorpusShardReference {
    source_path: String,
    selector_index: usize,
    unit_index: usize,
    path: String,
    entry_count: usize,
    byte_count: usize,
    content_sha256: String,
}

#[derive(Serialize)]
struct DialogueSourceCorpusManifest<'a> {
    kind: &'static str,
    implementation: &'a str,
    source_bin_sha256: &'a str,
    codebook_sha256: &'a str,
    asset_count: usize,
    bank_count: usize,
    shard_count: usize,
    coordinate_count: usize,
    raw_unique_entry_count: usize,
    semantic_shared_group_count: usize,
    glyph_occurrence_count: usize,
    control_occurrence_count: usize,
    alignment_padding_word_count: usize,
    roundtrip_verified_coordinate_count: usize,
    ready_for_translation: bool,
    markup_contract: &'a str,
    maximum_shard_entries: usize,
    maximum_shard_bytes: usize,
    coordinate_partition_complete: bool,
    shards: Vec<DialogueCorpusShardReference>,
}

pub(super) fn write_dialogue_source_corpus_shards(
    output_dir: &Path,
    corpus: &DialogueSourceCorpus,
) -> Result<DialogueSourceCorpusBuildReport> {
    ensure!(
        !output_dir.exists(),
        "dialogue source corpus output already exists: {}",
        output_dir.display()
    );
    if let Some(parent) = output_dir.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    std::fs::create_dir(output_dir)
        .with_context(|| format!("failed to create {}", output_dir.display()))?;

    let write_result = write_corpus_contents(output_dir, corpus);
    if write_result.is_err() {
        let _ = std::fs::remove_dir_all(output_dir);
    }
    write_result
}

fn write_corpus_contents(
    output_dir: &Path,
    corpus: &DialogueSourceCorpus,
) -> Result<DialogueSourceCorpusBuildReport> {
    let mut shard_references = Vec::new();
    let mut maximum_shard_bytes = 0usize;
    let mut partitioned_coordinate_count = 0usize;
    let mut bank_count = 0usize;

    for asset in &corpus.assets {
        let asset_directory = PathBuf::from("assets").join(asset_stem(&asset.source_path)?);
        for bank in &asset.banks {
            bank_count += 1;
            let ranges = partition_entry_ranges(&bank.entries, |unit_index, entries| {
                json_bytes(&DialogueCorpusBankShard {
                    kind: "Justice Gakuen 2 reversible dialogue source bank shard",
                    source_bin_sha256: &corpus.source_bin_sha256,
                    codebook_sha256: &corpus.codebook_sha256,
                    source_path: &asset.source_path,
                    stored_sha256: &asset.stored_sha256,
                    decoded_sha256: &asset.decoded_sha256,
                    fixed_cell_count: asset.fixed_cell_count,
                    selector_index: bank.selector_index,
                    unit_index,
                    entry_count: entries.len(),
                    entries,
                })
                .map(|bytes| bytes.len())
            })?;
            for (unit_index, range) in ranges.into_iter().enumerate() {
                let entries = &bank.entries[range];
                let shard = DialogueCorpusBankShard {
                    kind: "Justice Gakuen 2 reversible dialogue source bank shard",
                    source_bin_sha256: &corpus.source_bin_sha256,
                    codebook_sha256: &corpus.codebook_sha256,
                    source_path: &asset.source_path,
                    stored_sha256: &asset.stored_sha256,
                    decoded_sha256: &asset.decoded_sha256,
                    fixed_cell_count: asset.fixed_cell_count,
                    selector_index: bank.selector_index,
                    unit_index,
                    entry_count: entries.len(),
                    entries,
                };
                let relative_path = asset_directory.join(format!(
                    "bank-{:03}-unit-{unit_index:03}.json",
                    bank.selector_index
                ));
                let bytes = json_bytes(&shard)?;
                ensure!(
                    bytes.len() <= MAXIMUM_CORPUS_SHARD_BYTES,
                    "dialogue source corpus shard exceeds byte limit"
                );
                write_bytes(output_dir, &relative_path, &bytes)?;
                maximum_shard_bytes = maximum_shard_bytes.max(bytes.len());
                partitioned_coordinate_count += entries.len();
                shard_references.push(DialogueCorpusShardReference {
                    source_path: asset.source_path.clone(),
                    selector_index: bank.selector_index,
                    unit_index,
                    path: path_string(&relative_path),
                    entry_count: entries.len(),
                    byte_count: bytes.len(),
                    content_sha256: sha256_bytes(&bytes),
                });
            }
        }
    }

    let coordinate_partition_complete = partitioned_coordinate_count == corpus.coordinate_count;
    ensure!(
        coordinate_partition_complete,
        "dialogue source corpus shard partition changed the coordinate denominator"
    );
    let manifest = DialogueSourceCorpusManifest {
        kind: "Justice Gakuen 2 sharded reversible main-dialogue source corpus",
        implementation: &corpus.implementation,
        source_bin_sha256: &corpus.source_bin_sha256,
        codebook_sha256: &corpus.codebook_sha256,
        asset_count: corpus.assets.len(),
        bank_count,
        shard_count: shard_references.len(),
        coordinate_count: corpus.coordinate_count,
        raw_unique_entry_count: corpus.raw_unique_entry_count,
        semantic_shared_group_count: corpus.semantic_shared_group_count,
        glyph_occurrence_count: corpus.glyph_occurrence_count,
        control_occurrence_count: corpus.control_occurrence_count,
        alignment_padding_word_count: corpus.alignment_padding_word_count,
        roundtrip_verified_coordinate_count: corpus.roundtrip_verified_coordinate_count,
        ready_for_translation: corpus.ready_for_translation,
        markup_contract: &corpus.markup_contract,
        maximum_shard_entries: MAXIMUM_CORPUS_SHARD_ENTRIES,
        maximum_shard_bytes,
        coordinate_partition_complete,
        shards: shard_references,
    };
    let manifest_bytes = json_bytes(&manifest)?;
    write_bytes(output_dir, Path::new(CORPUS_MANIFEST_NAME), &manifest_bytes)?;

    Ok(DialogueSourceCorpusBuildReport {
        output_directory: output_dir.display().to_string(),
        manifest_sha256: sha256_bytes(&manifest_bytes),
        asset_count: manifest.asset_count,
        bank_count: manifest.bank_count,
        shard_count: manifest.shard_count,
        coordinate_count: manifest.coordinate_count,
        semantic_shared_group_count: manifest.semantic_shared_group_count,
        roundtrip_verified_coordinate_count: manifest.roundtrip_verified_coordinate_count,
        maximum_shard_bytes: manifest.maximum_shard_bytes,
        coordinate_partition_complete: manifest.coordinate_partition_complete,
    })
}

fn partition_entry_ranges(
    entries: &[DialogueCorpusEntry],
    measure_shard: impl Fn(usize, &[DialogueCorpusEntry]) -> Result<usize> + Copy,
) -> Result<Vec<std::ops::Range<usize>>> {
    let mut ranges = Vec::new();
    let mut start = 0usize;
    while start < entries.len() {
        let end = (start + MAXIMUM_CORPUS_SHARD_ENTRIES).min(entries.len());
        split_oversized_range(
            entries,
            start..end,
            ranges.len(),
            measure_shard,
            &mut ranges,
        )?;
        start = end;
    }
    Ok(ranges)
}

fn split_oversized_range(
    entries: &[DialogueCorpusEntry],
    range: std::ops::Range<usize>,
    unit_index: usize,
    measure_shard: impl Fn(usize, &[DialogueCorpusEntry]) -> Result<usize> + Copy,
    output: &mut Vec<std::ops::Range<usize>>,
) -> Result<()> {
    let byte_count = measure_shard(unit_index, &entries[range.clone()])?;
    if byte_count <= MAXIMUM_CORPUS_SHARD_BYTES {
        output.push(range);
        return Ok(());
    }
    ensure!(
        range.len() > 1,
        "one dialogue source corpus entry exceeds the shard byte limit"
    );
    let middle = range.start + range.len() / 2;
    split_oversized_range(
        entries,
        range.start..middle,
        unit_index,
        measure_shard,
        output,
    )?;
    split_oversized_range(
        entries,
        middle..range.end,
        unit_index + 1,
        measure_shard,
        output,
    )
}

fn asset_stem(source_path: &str) -> Result<String> {
    ensure!(
        source_path
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'.' | b'_' | b'-')),
        "dialogue source path cannot become a shard directory: {source_path}"
    );
    Ok(source_path
        .to_ascii_lowercase()
        .replace(['/', '.', '_'], "-"))
}

fn json_bytes(value: &impl Serialize) -> Result<Vec<u8>> {
    Ok(format!("{}\n", serde_json::to_string_pretty(value)?).into_bytes())
}

fn write_bytes(root: &Path, relative_path: &Path, bytes: &[u8]) -> Result<()> {
    ensure!(
        relative_path.is_relative()
            && relative_path
                .components()
                .all(|component| matches!(component, std::path::Component::Normal(_))),
        "dialogue corpus shard path must stay below its output directory"
    );
    let output_path = root.join(relative_path);
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    std::fs::write(&output_path, bytes)
        .with_context(|| format!("failed to write {}", output_path.display()))
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
