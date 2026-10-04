use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};
use serde::Serialize;

use crate::compression::decompress;
use crate::cue::CueSheet;
use crate::pipeline::{BASELINE_BIN_SHA256, sha256_bytes, sha256_file};

use super::atlas::parse_dialogue_atlas;
use super::codebook_model::{
    DialogueCodebook, DialogueCodebookAuditConfig, DialogueCodebookAuditManifest,
    DialogueCodebookEntry, DialogueCodebookStatus, DialogueGlyphMeaning,
    DialogueGlyphSourceReference, DialogueUnresolvedGlyph, ResolvedDialogueGlyph,
};
use super::format::hex_code;
use super::parser::parse_dialogue_banks;
use super::sources::{load_dialogue_runtime_image_sources, load_mgk_dialogue_sources};
use super::tokens::{DialogueTokenKind, tokenize_dialogue_message};

const CODEBOOK_KIND: &str = "Justice Gakuen 2 asset-scoped dialogue glyph codebook";

#[derive(Debug, Default)]
pub(super) struct SourceGlyphUsage {
    pub(super) occurrence_count: usize,
    pub(super) references: BTreeMap<(String, u16), usize>,
}

pub(super) fn load_resolved_dialogue_glyphs(
    codebook_path: &std::path::Path,
    source_bin_sha256: &str,
    source_pixel_hashes: &BTreeSet<String>,
    usage: &BTreeMap<String, SourceGlyphUsage>,
) -> Result<(String, BTreeMap<String, ResolvedDialogueGlyph>)> {
    let codebook_bytes = std::fs::read(codebook_path)
        .with_context(|| format!("failed to read {}", codebook_path.display()))?;
    validate_codebook_for_usage(&codebook_bytes, source_bin_sha256, usage)?;

    let glyphs = resolved_dialogue_glyphs(&codebook_bytes)?;
    let projection_sha256 = dialogue_codebook_projection_sha256(source_pixel_hashes, &glyphs)?;
    Ok((projection_sha256, glyphs))
}

pub(super) fn resolved_dialogue_glyphs(
    codebook_bytes: &[u8],
) -> Result<BTreeMap<String, ResolvedDialogueGlyph>> {
    let codebook: DialogueCodebook = serde_json::from_slice(codebook_bytes)?;
    Ok(codebook
        .entries
        .into_iter()
        .filter(|entry| entry.status.resolves_source())
        .map(|entry| {
            let (text, semantic_id) = match entry.meaning {
                DialogueGlyphMeaning::Character { text } => (text, None),
                DialogueGlyphMeaning::Symbol { id, display } => (display, Some(id)),
            };
            (
                entry.pixel_sha256,
                ResolvedDialogueGlyph {
                    text,
                    semantic_id,
                    codebook_status: entry.status.name().to_string(),
                },
            )
        })
        .collect())
}

pub(crate) fn load_verified_dialogue_pixel_texts(
    image_path: &std::path::Path,
    codebook_path: &std::path::Path,
    source_bin_sha256: &str,
) -> Result<(String, BTreeMap<String, String>)> {
    let (source_pixel_hashes, usage) = mgk_source_glyph_inventory(image_path)?;
    let (_, glyphs) = load_resolved_dialogue_glyphs(
        codebook_path,
        source_bin_sha256,
        &source_pixel_hashes,
        &usage,
    )?;
    Ok((
        sha256_file(codebook_path)?,
        glyphs
            .into_iter()
            .map(|(pixel_sha256, glyph)| (pixel_sha256, glyph.text))
            .collect(),
    ))
}

#[derive(Serialize)]
struct DialogueCodebookProjectionEntry<'a> {
    pixel_sha256: &'a str,
    text: &'a str,
    semantic_id: Option<&'a str>,
    codebook_status: &'a str,
}

pub(super) fn dialogue_codebook_projection_sha256(
    source_pixel_hashes: &BTreeSet<String>,
    glyphs: &BTreeMap<String, ResolvedDialogueGlyph>,
) -> Result<String> {
    let entries = source_pixel_hashes
        .iter()
        .filter_map(|pixel_sha256| {
            glyphs
                .get(pixel_sha256)
                .map(|glyph| DialogueCodebookProjectionEntry {
                    pixel_sha256,
                    text: &glyph.text,
                    semantic_id: glyph.semantic_id.as_deref(),
                    codebook_status: &glyph.codebook_status,
                })
        })
        .collect::<Vec<_>>();
    Ok(sha256_bytes(&serde_json::to_vec(&entries)?))
}

pub fn audit_dialogue_codebook(
    config: &DialogueCodebookAuditConfig,
) -> Result<DialogueCodebookAuditManifest> {
    let cue = CueSheet::parse(&config.cue)?;
    let source_bin_sha256 = sha256_file(&cue.image_path)?;
    ensure!(
        source_bin_sha256 == BASELINE_BIN_SHA256,
        "unsupported source BIN SHA-256: {source_bin_sha256}"
    );
    let (source_pixel_hashes, usage) = runtime_source_glyph_inventory(&cue.image_path)?;
    let codebook_bytes = std::fs::read(&config.codebook)
        .with_context(|| format!("failed to read {}", config.codebook.display()))?;
    let manifest = audit_codebook_bytes(
        &codebook_bytes,
        &source_bin_sha256,
        &source_pixel_hashes,
        &usage,
    )?;
    write_manifest(config, &manifest)?;
    Ok(manifest)
}

pub(super) fn audit_codebook_bytes(
    codebook_bytes: &[u8],
    source_bin_sha256: &str,
    source_pixel_hashes: &BTreeSet<String>,
    usage: &BTreeMap<String, SourceGlyphUsage>,
) -> Result<DialogueCodebookAuditManifest> {
    let codebook: DialogueCodebook = serde_json::from_slice(codebook_bytes)?;
    let statuses = validate_codebook(&codebook, source_bin_sha256)?;
    for entry in &codebook.entries {
        ensure!(
            source_pixel_hashes.contains(&entry.pixel_sha256),
            "codebook pixel hash {} is absent from the selected source media",
            entry.pixel_sha256
        );
    }

    let candidate_entry_count = statuses
        .values()
        .filter(|status| **status == DialogueCodebookStatus::Candidate)
        .count();
    let verified_entry_count = statuses
        .values()
        .filter(|status| status.resolves_source())
        .count();
    let mut resolved_used_source_pixel_hash_count = 0usize;
    let mut unresolved_glyph_occurrence_count = 0usize;
    let mut unresolved = Vec::new();
    for (pixel_sha256, glyph_usage) in usage {
        if statuses
            .get(pixel_sha256)
            .is_some_and(|status| status.resolves_source())
        {
            resolved_used_source_pixel_hash_count += 1;
            continue;
        }
        unresolved_glyph_occurrence_count += glyph_usage.occurrence_count;
        unresolved.push(DialogueUnresolvedGlyph {
            pixel_sha256: pixel_sha256.clone(),
            occurrence_count: glyph_usage.occurrence_count,
            source_references: glyph_usage
                .references
                .iter()
                .map(
                    |((source_asset, code), occurrence_count)| DialogueGlyphSourceReference {
                        source_asset: source_asset.clone(),
                        code: hex_code(*code),
                        occurrence_count: *occurrence_count,
                    },
                )
                .collect(),
        });
    }
    unresolved.sort_by(|left, right| {
        right
            .occurrence_count
            .cmp(&left.occurrence_count)
            .then_with(|| left.pixel_sha256.cmp(&right.pixel_sha256))
    });
    let unresolved_used_source_pixel_hash_count = unresolved.len();

    Ok(DialogueCodebookAuditManifest {
        kind: "Dialogue runtime-image codebook coverage audit".to_string(),
        implementation: "independent Rust source-pixel and codebook verification".to_string(),
        source_bin_sha256: source_bin_sha256.to_string(),
        codebook_sha256: sha256_bytes(codebook_bytes),
        codebook_entry_count: codebook.entries.len(),
        candidate_entry_count,
        verified_entry_count,
        source_pixel_hash_count: source_pixel_hashes.len(),
        used_source_pixel_hash_count: usage.len(),
        resolved_used_source_pixel_hash_count,
        unresolved_used_source_pixel_hash_count,
        unresolved_glyph_occurrence_count,
        ready_for_glyph_decode: unresolved_used_source_pixel_hash_count == 0,
        unresolved,
        limitations: vec![
            "Candidate entries remain unresolved and cannot satisfy the glyph-decode gate."
                .to_string(),
            "Glyph coverage alone does not supply runtime insertion semantics or declare the Japanese corpus complete."
                .to_string(),
            "Human-approved entries require an explicit reviewer and review date; recognition output is never promoted automatically."
                .to_string(),
        ],
    })
}

pub(super) fn validate_codebook_for_usage(
    codebook_bytes: &[u8],
    source_bin_sha256: &str,
    usage: &BTreeMap<String, SourceGlyphUsage>,
) -> Result<()> {
    let codebook: DialogueCodebook = serde_json::from_slice(codebook_bytes)?;
    let statuses = validate_codebook(&codebook, source_bin_sha256)?;
    let unresolved_count = usage
        .keys()
        .filter(|pixel_sha256| {
            !statuses
                .get(*pixel_sha256)
                .is_some_and(|status| status.resolves_source())
        })
        .count();
    ensure!(
        unresolved_count == 0,
        "dialogue codebook has {unresolved_count} unresolved used source pixel hashes"
    );
    Ok(())
}

pub(super) fn validate_codebook(
    codebook: &DialogueCodebook,
    source_bin_sha256: &str,
) -> Result<BTreeMap<String, DialogueCodebookStatus>> {
    ensure!(codebook.kind == CODEBOOK_KIND, "unexpected codebook kind");
    ensure!(
        codebook.source_bin_sha256 == source_bin_sha256,
        "codebook source BIN SHA-256 does not match the selected media"
    );
    ensure!(
        !codebook.evidence_sources.is_empty()
            && codebook
                .evidence_sources
                .iter()
                .all(|(id, description)| !id.trim().is_empty() && !description.trim().is_empty()),
        "codebook evidence_sources must contain nonempty IDs and descriptions"
    );

    let mut previous_hash = None;
    let mut statuses = BTreeMap::new();
    for entry in &codebook.entries {
        validate_entry(entry, &codebook.evidence_sources)?;
        if let Some(previous) = previous_hash {
            ensure!(
                previous < entry.pixel_sha256.as_str(),
                "codebook entries must have unique, ascending pixel_sha256 values"
            );
        }
        previous_hash = Some(entry.pixel_sha256.as_str());
        statuses.insert(entry.pixel_sha256.clone(), entry.status);
    }
    Ok(statuses)
}

pub(super) fn runtime_source_glyph_inventory(
    image_path: &std::path::Path,
) -> Result<(BTreeSet<String>, BTreeMap<String, SourceGlyphUsage>)> {
    source_glyph_inventory(load_dialogue_runtime_image_sources(image_path)?)
}

pub(super) fn mgk_source_glyph_inventory(
    image_path: &std::path::Path,
) -> Result<(BTreeSet<String>, BTreeMap<String, SourceGlyphUsage>)> {
    source_glyph_inventory(load_mgk_dialogue_sources(image_path)?)
}

fn source_glyph_inventory(
    sources: Vec<super::sources::DialogueSource>,
) -> Result<(BTreeSet<String>, BTreeMap<String, SourceGlyphUsage>)> {
    let mut source_pixel_hashes = BTreeSet::new();
    let mut usage: BTreeMap<String, SourceGlyphUsage> = BTreeMap::new();
    for source in sources {
        let decoded = decompress(&source.data, true)
            .with_context(|| format!("failed to decode {}", source.path))?;
        let atlas = parse_dialogue_atlas(&decoded)
            .with_context(|| format!("failed to parse {} dialogue atlas", source.path))?;
        source_pixel_hashes.extend(atlas.fixed_cell_sha256.iter().cloned());
        for bank in parse_dialogue_banks(&decoded)? {
            for (index, message) in bank.messages.into_iter().enumerate() {
                let raw_words: Vec<_> = message
                    .data
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
                    .collect();
                let tokenized = tokenize_dialogue_message(
                    &raw_words,
                    atlas.fixed_cell_count,
                    atlas.addressable_slot_count,
                )
                .with_context(|| {
                    format!(
                        "failed to tokenize {} bank {} entry {index}",
                        source.path, bank.selector_index
                    )
                })?;
                for token in tokenized.tokens {
                    match token.kind {
                        DialogueTokenKind::FixedGlyph => {
                            let pixel_sha256 = &atlas.fixed_cell_sha256[usize::from(token.code)];
                            let glyph_usage = usage.entry(pixel_sha256.clone()).or_default();
                            glyph_usage.occurrence_count += 1;
                            *glyph_usage
                                .references
                                .entry((source.path.clone(), token.code))
                                .or_default() += 1;
                        }
                        DialogueTokenKind::RuntimeExtensionGlyph => {
                            anyhow::bail!(
                                "{} bank {} entry {index} directly uses runtime extension code {}",
                                source.path,
                                bank.selector_index,
                                hex_code(token.code)
                            );
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    Ok((source_pixel_hashes, usage))
}

pub(super) fn validate_entry(
    entry: &DialogueCodebookEntry,
    evidence_sources: &BTreeMap<String, String>,
) -> Result<()> {
    ensure!(
        entry.pixel_sha256.len() == 64
            && entry
                .pixel_sha256
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "codebook pixel_sha256 must be 64 lowercase hexadecimal characters"
    );
    match &entry.meaning {
        DialogueGlyphMeaning::Character { text } => ensure!(
            text.chars().count() == 1 && !text.chars().next().is_some_and(char::is_control),
            "codebook character meaning must contain exactly one non-control scalar"
        ),
        DialogueGlyphMeaning::Symbol { id, display } => ensure!(
            !id.trim().is_empty() && !display.trim().is_empty(),
            "codebook symbol meaning requires a nonempty id and display"
        ),
    }
    ensure!(
        !entry.evidence.is_empty()
            && entry.evidence.iter().all(|value| !value.trim().is_empty())
            && entry.evidence.windows(2).all(|pair| pair[0] < pair[1]),
        "codebook evidence must be nonempty, unique, and ascending"
    );
    ensure!(
        entry
            .evidence
            .iter()
            .all(|evidence| evidence_sources.contains_key(evidence)),
        "codebook entry refers to an undeclared evidence source"
    );
    if entry.status == DialogueCodebookStatus::HumanApproved {
        ensure!(
            entry
                .reviewed_by
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty())
                && entry
                    .reviewed_at
                    .as_deref()
                    .is_some_and(|value| !value.trim().is_empty()),
            "human-approved codebook entries require reviewed_by and reviewed_at"
        );
    } else {
        ensure!(
            entry.reviewed_by.is_none() && entry.reviewed_at.is_none(),
            "non-approved codebook entries must not claim reviewer metadata"
        );
    }
    Ok(())
}

fn write_manifest(
    config: &DialogueCodebookAuditConfig,
    manifest: &DialogueCodebookAuditManifest,
) -> Result<()> {
    if let Some(parent) = config.output.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    std::fs::write(
        &config.output,
        format!("{}\n", serde_json::to_string_pretty(manifest)?),
    )
    .with_context(|| format!("failed to write {}", config.output.display()))
}
