use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

use anyhow::{Context, Result, ensure};

use crate::compression::decompress;
use crate::cue::CueSheet;
use crate::pipeline::{BASELINE_BIN_SHA256, sha256_bytes, sha256_file};
use crate::source_disc::SupportedSourceDisc;

use super::atlas::parse_dialogue_atlas;
use super::codebook::{
    load_resolved_dialogue_glyphs, mgk_source_glyph_inventory, runtime_source_glyph_inventory,
};
use super::corpus_codec::{
    bytes_to_words, reconstruct_words, resolve_tokens, semantic_sha256, source_markup,
};
use super::corpus_model::{
    DialogueCorpusAsset, DialogueCorpusBank, DialogueCorpusEntry, DialogueCorpusToken,
    DialogueSourceCorpus, DialogueSourceCorpusBuildReport, DialogueSourceCorpusConfig,
};
use super::corpus_writer::write_dialogue_source_corpus_shards;
use super::format::{hex_address, hex_code, hex_offset};
use super::parser::{DECODED_RUNTIME_BASE, parse_dialogue_banks};
use super::sources::{load_dialogue_runtime_image_sources, load_mgk_dialogue_sources};
use super::tokens::tokenize_dialogue_message;

struct PendingEntry {
    coordinate_id: String,
    entry_index: usize,
    decoded_offset: usize,
    raw_entry_sha256: String,
    semantic_source_sha256: String,
    raw_words: Vec<u16>,
    alignment_padding_word_count: usize,
    source_markup: String,
    tokens: Vec<DialogueCorpusToken>,
}

struct PendingAsset {
    source_path: String,
    stored_sha256: String,
    decoded_sha256: String,
    fixed_cell_count: usize,
    banks: Vec<(usize, Vec<PendingEntry>)>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum DialogueCorpusPopulation {
    MgkDevelopmentSubset,
    AllRuntimeImages,
}

type CorpusCacheKey = (String, String, DialogueCorpusPopulation);

static CORPUS_CACHE: OnceLock<Mutex<HashMap<CorpusCacheKey, Arc<DialogueSourceCorpus>>>> =
    OnceLock::new();

struct DialogueCorpusDenominators {
    asset_count: usize,
    coordinate_count: usize,
    raw_unique_entry_count: usize,
    semantic_shared_group_count: usize,
    glyph_occurrence_count: usize,
    control_occurrence_count: usize,
    alignment_padding_word_count: usize,
}

const MGK_DEVELOPMENT_DENOMINATORS: DialogueCorpusDenominators = DialogueCorpusDenominators {
    asset_count: 10,
    coordinate_count: 10_156,
    raw_unique_entry_count: 7_258,
    semantic_shared_group_count: 5_186,
    glyph_occurrence_count: 234_368,
    control_occurrence_count: 33_032,
    alignment_padding_word_count: 5_232,
};

const ALL_RUNTIME_IMAGE_DENOMINATORS: DialogueCorpusDenominators = DialogueCorpusDenominators {
    asset_count: 78,
    coordinate_count: 62_639,
    raw_unique_entry_count: 42_681,
    semantic_shared_group_count: 18_807,
    glyph_occurrence_count: 1_539_426,
    control_occurrence_count: 218_286,
    alignment_padding_word_count: 32_787,
};

pub fn build_dialogue_source_corpus(
    config: &DialogueSourceCorpusConfig,
) -> Result<DialogueSourceCorpusBuildReport> {
    let corpus = extract_dialogue_source_corpus_for_population(
        &config.cue,
        &config.codebook,
        DialogueCorpusPopulation::AllRuntimeImages,
    )?;
    write_dialogue_source_corpus_shards(&config.output_dir, &corpus)
}

pub(super) fn extract_dialogue_source_corpus(
    cue_path: &Path,
    codebook_path: &Path,
) -> Result<Arc<DialogueSourceCorpus>> {
    extract_dialogue_source_corpus_for_population(
        cue_path,
        codebook_path,
        DialogueCorpusPopulation::MgkDevelopmentSubset,
    )
}

pub(super) fn extract_dialogue_source_corpus_from_source(
    source: &SupportedSourceDisc,
    codebook_path: &Path,
) -> Result<Arc<DialogueSourceCorpus>> {
    extract_dialogue_source_corpus_for_verified_source(
        source,
        codebook_path,
        DialogueCorpusPopulation::MgkDevelopmentSubset,
    )
}

pub(super) fn extract_all_dialogue_source_corpus(
    cue_path: &Path,
    codebook_path: &Path,
) -> Result<Arc<DialogueSourceCorpus>> {
    extract_dialogue_source_corpus_for_population(
        cue_path,
        codebook_path,
        DialogueCorpusPopulation::AllRuntimeImages,
    )
}

pub(super) fn extract_all_dialogue_source_corpus_from_source(
    source: &SupportedSourceDisc,
    codebook_path: &Path,
) -> Result<Arc<DialogueSourceCorpus>> {
    extract_dialogue_source_corpus_for_verified_source(
        source,
        codebook_path,
        DialogueCorpusPopulation::AllRuntimeImages,
    )
}

fn extract_dialogue_source_corpus_for_population(
    cue_path: &Path,
    codebook_path: &Path,
    population: DialogueCorpusPopulation,
) -> Result<Arc<DialogueSourceCorpus>> {
    let cue = CueSheet::parse(cue_path)?;
    let source_bin_sha256 = sha256_file(&cue.image_path)?;
    ensure!(
        source_bin_sha256 == BASELINE_BIN_SHA256,
        "unsupported source BIN SHA-256: {source_bin_sha256}"
    );
    extract_dialogue_source_corpus_from_image(
        &cue.image_path,
        &source_bin_sha256,
        codebook_path,
        population,
    )
}

fn extract_dialogue_source_corpus_for_verified_source(
    source: &SupportedSourceDisc,
    codebook_path: &Path,
    population: DialogueCorpusPopulation,
) -> Result<Arc<DialogueSourceCorpus>> {
    ensure!(
        source.source_bin_sha256() == BASELINE_BIN_SHA256,
        "unsupported source BIN SHA-256: {}",
        source.source_bin_sha256()
    );
    extract_dialogue_source_corpus_from_image(
        source.image_path(),
        source.source_bin_sha256(),
        codebook_path,
        population,
    )
}

fn extract_dialogue_source_corpus_from_image(
    image_path: &Path,
    source_bin_sha256: &str,
    codebook_path: &Path,
    population: DialogueCorpusPopulation,
) -> Result<Arc<DialogueSourceCorpus>> {
    let codebook_file_sha256 = sha256_file(codebook_path)?;
    let key = (
        source_bin_sha256.to_string(),
        codebook_file_sha256.clone(),
        population,
    );
    let cache = CORPUS_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(corpus) = cache
        .lock()
        .expect("dialogue corpus cache lock poisoned")
        .get(&key)
        .cloned()
    {
        return Ok(corpus);
    }

    let (source_pixel_hashes, usage, sources, denominators) = match population {
        DialogueCorpusPopulation::MgkDevelopmentSubset => {
            let (source_pixel_hashes, usage) = mgk_source_glyph_inventory(image_path)?;
            (
                source_pixel_hashes,
                usage,
                load_mgk_dialogue_sources(image_path)?,
                &MGK_DEVELOPMENT_DENOMINATORS,
            )
        }
        DialogueCorpusPopulation::AllRuntimeImages => {
            let (source_pixel_hashes, usage) = runtime_source_glyph_inventory(image_path)?;
            (
                source_pixel_hashes,
                usage,
                load_dialogue_runtime_image_sources(image_path)?,
                &ALL_RUNTIME_IMAGE_DENOMINATORS,
            )
        }
    };
    let (codebook_sha256, glyphs) = load_resolved_dialogue_glyphs(
        codebook_path,
        source_bin_sha256,
        &source_pixel_hashes,
        &usage,
    )?;
    ensure!(
        sha256_file(codebook_path)? == codebook_file_sha256,
        "dialogue codebook file changed while extracting the source corpus"
    );

    let mut assets = Vec::new();
    let mut raw_hashes = BTreeSet::new();
    let mut semantic_counts = BTreeMap::new();
    let mut coordinate_count = 0usize;
    let mut glyph_occurrence_count = 0usize;
    let mut control_occurrence_count = 0usize;
    let mut alignment_padding_word_count = 0usize;

    for source in sources {
        let decoded = decompress(&source.data, true)
            .with_context(|| format!("failed to decode {}", source.path))?;
        let atlas = parse_dialogue_atlas(&decoded)
            .with_context(|| format!("failed to parse {} dialogue atlas", source.path))?;
        let mut banks = Vec::new();
        for bank in parse_dialogue_banks(&decoded)? {
            let mut entries = Vec::new();
            for (entry_index, message) in bank.messages.into_iter().enumerate() {
                let raw_words = bytes_to_words(&message.data);
                let tokenized = tokenize_dialogue_message(
                    &raw_words,
                    atlas.fixed_cell_count,
                    atlas.addressable_slot_count,
                )?;
                let tokens = resolve_tokens(
                    &source.path,
                    &atlas.fixed_cell_sha256,
                    &glyphs,
                    &tokenized.tokens,
                )?;
                let reconstructed =
                    reconstruct_words(&tokens, tokenized.alignment_padding_word_count)?;
                ensure!(
                    reconstructed == raw_words,
                    "corpus roundtrip changed {} bank {} entry {entry_index}",
                    source.path,
                    bank.selector_index
                );

                let raw_entry_sha256 = sha256_bytes(&message.data);
                let semantic_source_sha256 = semantic_sha256(&tokens);
                raw_hashes.insert(raw_entry_sha256.clone());
                *semantic_counts
                    .entry(semantic_source_sha256.clone())
                    .or_insert(0usize) += 1;
                coordinate_count += 1;
                glyph_occurrence_count += tokens
                    .iter()
                    .filter(|token| matches!(token, DialogueCorpusToken::Glyph { .. }))
                    .count();
                control_occurrence_count += tokens
                    .iter()
                    .filter(|token| matches!(token, DialogueCorpusToken::Control { .. }))
                    .count();
                alignment_padding_word_count += tokenized.alignment_padding_word_count;
                entries.push(PendingEntry {
                    coordinate_id: format!(
                        "{}#bank-{}-entry-{entry_index:04}",
                        source.path, bank.selector_index
                    ),
                    entry_index,
                    decoded_offset: message.decoded_offset,
                    raw_entry_sha256,
                    semantic_source_sha256,
                    raw_words,
                    alignment_padding_word_count: tokenized.alignment_padding_word_count,
                    source_markup: source_markup(&tokens),
                    tokens,
                });
            }
            banks.push((bank.selector_index, entries));
        }
        assets.push(PendingAsset {
            source_path: source.path,
            stored_sha256: sha256_bytes(&source.data),
            decoded_sha256: sha256_bytes(&decoded),
            fixed_cell_count: atlas.fixed_cell_count,
            banks,
        });
    }

    ensure!(
        assets.len() == denominators.asset_count,
        "dialogue source asset denominator changed"
    );
    ensure!(
        coordinate_count == denominators.coordinate_count,
        "dialogue coordinate denominator changed"
    );
    ensure!(
        raw_hashes.len() == denominators.raw_unique_entry_count,
        "dialogue raw-entry denominator changed"
    );
    ensure!(
        semantic_counts.len() == denominators.semantic_shared_group_count,
        "dialogue semantic-group denominator changed"
    );
    ensure!(
        glyph_occurrence_count == denominators.glyph_occurrence_count,
        "dialogue glyph occurrence denominator changed"
    );
    ensure!(
        control_occurrence_count == denominators.control_occurrence_count,
        "dialogue control occurrence denominator changed"
    );
    ensure!(
        alignment_padding_word_count == denominators.alignment_padding_word_count,
        "dialogue alignment-padding denominator changed"
    );
    let assets = finish_assets(assets, &semantic_counts);
    let corpus = Arc::new(DialogueSourceCorpus {
        kind: "Justice Gakuen 2 reversible main-dialogue source corpus".to_string(),
        implementation: "Rust original-media extraction with source-pixel codebook and exact word roundtrip"
            .to_string(),
        source_bin_sha256: source_bin_sha256.to_string(),
        codebook_sha256,
        coordinate_count,
        raw_unique_entry_count: raw_hashes.len(),
        semantic_shared_group_count: semantic_counts.len(),
        glyph_occurrence_count,
        control_occurrence_count,
        alignment_padding_word_count,
        roundtrip_verified_coordinate_count: coordinate_count,
        ready_for_translation: true,
        markup_contract: "glyph text escapes backslash and braces; controls use {#semantic_name} or {#semantic_name:0xNNNN}; structured tokens and raw_words are authoritative"
            .to_string(),
        assets,
    });
    cache
        .lock()
        .expect("dialogue corpus cache lock poisoned")
        .insert(key, Arc::clone(&corpus));
    Ok(corpus)
}

fn finish_assets(
    assets: Vec<PendingAsset>,
    semantic_counts: &BTreeMap<String, usize>,
) -> Vec<DialogueCorpusAsset> {
    assets
        .into_iter()
        .map(|asset| DialogueCorpusAsset {
            source_path: asset.source_path,
            stored_sha256: asset.stored_sha256,
            decoded_sha256: asset.decoded_sha256,
            fixed_cell_count: asset.fixed_cell_count,
            banks: asset
                .banks
                .into_iter()
                .map(|(selector_index, entries)| DialogueCorpusBank {
                    selector_index,
                    entries: entries
                        .into_iter()
                        .map(|entry| DialogueCorpusEntry {
                            coordinate_id: entry.coordinate_id,
                            entry_index: entry.entry_index,
                            decoded_offset: hex_offset(entry.decoded_offset),
                            runtime_address: hex_address(
                                DECODED_RUNTIME_BASE + entry.decoded_offset as u32,
                            ),
                            raw_entry_sha256: entry.raw_entry_sha256,
                            semantic_shared_coordinate_count: semantic_counts
                                [&entry.semantic_source_sha256],
                            semantic_source_sha256: entry.semantic_source_sha256,
                            raw_words: entry.raw_words.into_iter().map(hex_code).collect(),
                            alignment_padding_word_count: entry.alignment_padding_word_count,
                            source_markup: entry.source_markup,
                            tokens: entry.tokens,
                        })
                        .collect(),
                })
                .collect(),
        })
        .collect()
}
