use std::path::Path;

use anyhow::{Context, Result, ensure};

use crate::pipeline::sha256_file;

use super::corpus::extract_dialogue_source_corpus;
use super::glossary_model::{
    DialogueGlossary, DialogueGlossaryAuditConfig, DialogueGlossaryAuditReport,
    DialogueGlossaryEntry, DialogueGlossaryStatus,
};
use super::translation::expected_translation_input;
use super::translation_model::{DialogueTranslationGroup, DialogueTranslationInput};
use super::translation_validation::{contains_japanese_source_character, valid_date};

const GLOSSARY_KIND: &str = "Justice Gakuen 2 Korean dialogue glossary";

pub fn audit_dialogue_glossary(
    config: &DialogueGlossaryAuditConfig,
) -> Result<DialogueGlossaryAuditReport> {
    let corpus = extract_dialogue_source_corpus(&config.cue, &config.codebook)?;
    let source = expected_translation_input(&corpus)?;
    let glossary = load_dialogue_glossary(&config.glossary, &source)?;
    let report = glossary_audit_report(&glossary, &source, sha256_file(&config.glossary)?);
    if let Some(parent) = config.output.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    std::fs::write(
        &config.output,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )
    .with_context(|| format!("failed to write glossary audit {}", config.output.display()))?;
    Ok(report)
}

pub(super) fn glossary_audit_report(
    glossary: &DialogueGlossary,
    source: &DialogueTranslationInput,
    glossary_sha256: String,
) -> DialogueGlossaryAuditReport {
    let candidate_entry_count = glossary
        .entries
        .iter()
        .filter(|entry| entry.status == DialogueGlossaryStatus::Candidate)
        .count();
    let approved_entry_count = glossary.entries.len() - candidate_entry_count;
    DialogueGlossaryAuditReport {
        kind: "Justice Gakuen 2 Korean dialogue glossary audit".to_string(),
        source_bin_sha256: source.source_bin_sha256.clone(),
        source_corpus_sha256: source.source_corpus_sha256.clone(),
        glossary_sha256,
        entry_count: glossary.entries.len(),
        candidate_entry_count,
        approved_entry_count,
        matched_group_count: source
            .groups
            .iter()
            .filter(|group| !entries_for_group(&glossary.entries, group).is_empty())
            .count(),
        matched_term_occurrence_count: glossary
            .entries
            .iter()
            .map(|entry| {
                source
                    .groups
                    .iter()
                    .flat_map(|group| &group.source_segments)
                    .map(|segment| term_occurrence_count(segment, &entry.source))
                    .sum::<usize>()
            })
            .sum(),
        all_terms_project_owner_approved: candidate_entry_count == 0,
    }
}

pub(super) fn load_dialogue_glossary(
    path: &Path,
    source: &DialogueTranslationInput,
) -> Result<DialogueGlossary> {
    let bytes = std::fs::read(path)
        .with_context(|| format!("failed to read dialogue glossary {}", path.display()))?;
    let glossary: DialogueGlossary = serde_json::from_slice(&bytes)
        .with_context(|| format!("failed to parse dialogue glossary {}", path.display()))?;
    validate_dialogue_glossary(&glossary, source)?;
    Ok(glossary)
}

pub(super) fn validate_dialogue_glossary(
    glossary: &DialogueGlossary,
    source: &DialogueTranslationInput,
) -> Result<()> {
    ensure!(glossary.kind == GLOSSARY_KIND, "unexpected glossary kind");
    ensure!(
        glossary.source_bin_sha256 == source.source_bin_sha256,
        "glossary source BIN identity changed"
    );
    ensure!(
        glossary.source_corpus_sha256 == source.source_corpus_sha256,
        "glossary source corpus identity changed"
    );
    ensure!(
        glossary
            .entries
            .windows(2)
            .all(|pair| pair[0].source < pair[1].source),
        "glossary entries must have unique ascending source terms"
    );
    for entry in &glossary.entries {
        validate_entry(entry, glossary, &source.groups)?;
    }
    Ok(())
}

fn validate_entry(
    entry: &DialogueGlossaryEntry,
    glossary: &DialogueGlossary,
    groups: &[DialogueTranslationGroup],
) -> Result<()> {
    ensure!(
        !entry.source.trim().is_empty()
            && !entry.korean.trim().is_empty()
            && !entry.category.trim().is_empty()
            && !entry.usage_note.trim().is_empty(),
        "glossary entry has an empty required field"
    );
    ensure!(
        !contains_japanese_source_character(&entry.korean),
        "glossary Korean term retains Japanese source characters: {}",
        entry.source
    );
    ensure!(
        entry
            .evidence_ids
            .iter()
            .all(|id| { !id.trim().is_empty() && glossary.evidence_sources.contains_key(id) })
            && !entry.evidence_ids.is_empty(),
        "glossary term {} has missing or undeclared evidence",
        entry.source
    );
    ensure!(
        groups.iter().any(|group| {
            group
                .source_segments
                .iter()
                .any(|segment| term_occurs(segment, &entry.source))
        }),
        "glossary term does not occur in the bound source: {}",
        entry.source
    );
    match entry.status {
        DialogueGlossaryStatus::Candidate => ensure!(
            entry.reviewed_by.is_none() && entry.reviewed_at.is_none(),
            "candidate glossary term claims review: {}",
            entry.source
        ),
        DialogueGlossaryStatus::Approved => ensure!(
            entry.reviewed_by.as_deref() == Some("project_owner")
                && valid_date(entry.reviewed_at.as_deref()),
            "approved glossary term lacks dated project-owner review: {}",
            entry.source
        ),
    }
    Ok(())
}

pub(super) fn entries_for_group(
    entries: &[DialogueGlossaryEntry],
    group: &DialogueTranslationGroup,
) -> Vec<DialogueGlossaryEntry> {
    entries
        .iter()
        .filter(|entry| {
            group
                .source_segments
                .iter()
                .any(|segment| term_occurs(segment, &entry.source))
        })
        .cloned()
        .collect()
}

pub(super) fn term_occurs(text: &str, term: &str) -> bool {
    term_occurrence_count(text, term) > 0
}

fn term_occurrence_count(text: &str, term: &str) -> usize {
    text.match_indices(term)
        .filter(|(start, _)| {
            let before = text[..*start].chars().next_back();
            let after = text[*start + term.len()..].chars().next();
            before.is_none_or(|character| !is_katakana(character))
                && after.is_none_or(|character| !is_katakana(character))
        })
        .count()
}

fn is_katakana(character: char) -> bool {
    matches!(
        character,
        '\u{30a0}'..='\u{30ff}' | '\u{31f0}'..='\u{31ff}' | '\u{ff65}'..='\u{ff9f}'
    )
}
