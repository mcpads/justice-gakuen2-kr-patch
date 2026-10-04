use std::collections::BTreeMap;

use anyhow::{Result, ensure};

use super::corpus_digest::source_corpus_sha256;
use super::corpus_model::{DialogueCorpusEntry, DialogueCorpusToken, DialogueSourceCorpus};
use super::translation_model::{
    DialogueTranslationAuditConfig, DialogueTranslationControl, DialogueTranslationGroup,
    DialogueTranslationInitConfig, DialogueTranslationInput, DialogueTranslationProjectStatus,
    DialogueTranslationStatus,
};
use super::translation_workspace::initialize_translation_workspace;
use super::translation_workspace_audit::audit_translation_workspace;
use super::translation_workspace_model::{
    DialogueTranslationWorkspaceAuditReport, DialogueTranslationWorkspaceManifest,
};

pub(super) const TRANSLATION_KIND: &str = "Justice Gakuen 2 Korean main-dialogue translation";

pub fn initialize_dialogue_translation(
    config: &DialogueTranslationInitConfig,
) -> Result<DialogueTranslationWorkspaceManifest> {
    initialize_translation_workspace(config)
}

pub fn audit_dialogue_translation(
    config: &DialogueTranslationAuditConfig,
) -> Result<DialogueTranslationWorkspaceAuditReport> {
    audit_translation_workspace(config)
}

pub(super) fn expected_translation_input(
    corpus: &DialogueSourceCorpus,
) -> Result<DialogueTranslationInput> {
    let source_corpus_sha256 = source_corpus_sha256(corpus)?;
    expected_translation_input_with_source_corpus_sha256(corpus, source_corpus_sha256)
}

pub(super) fn expected_translation_input_with_source_corpus_sha256(
    corpus: &DialogueSourceCorpus,
    source_corpus_sha256: String,
) -> Result<DialogueTranslationInput> {
    ensure!(
        source_corpus_sha256.len() == 64
            && source_corpus_sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit()),
        "source corpus SHA-256 is invalid"
    );
    let mut groups: BTreeMap<String, DialogueTranslationGroup> = BTreeMap::new();
    for entry in corpus
        .assets
        .iter()
        .flat_map(|asset| asset.banks.iter())
        .flat_map(|bank| bank.entries.iter())
    {
        let (source_segments, controls) = translation_shape(entry);
        let group = groups
            .entry(entry.semantic_source_sha256.clone())
            .or_insert_with(|| initial_group(entry, source_segments.clone(), controls.clone()));
        ensure!(
            group.source_segments == source_segments && group.controls == controls,
            "semantic group {} contains different decoded shapes",
            entry.semantic_source_sha256
        );
        if group.coordinate_ids.last() != Some(&entry.coordinate_id) {
            group.coordinate_ids.push(entry.coordinate_id.clone());
        }
    }
    let coordinate_count: usize = groups
        .values()
        .map(|group| group.coordinate_ids.len())
        .sum();
    ensure!(
        groups.len() == corpus.semantic_shared_group_count,
        "translation group denominator changed"
    );
    ensure!(
        coordinate_count == corpus.coordinate_count,
        "translation coordinate denominator changed"
    );
    Ok(DialogueTranslationInput {
        kind: TRANSLATION_KIND.to_string(),
        source_bin_sha256: corpus.source_bin_sha256.clone(),
        codebook_sha256: corpus.codebook_sha256.clone(),
        source_corpus_sha256,
        translation_status: DialogueTranslationProjectStatus::InProgress,
        translation_review: None,
        groups: groups.into_values().collect(),
    })
}

fn initial_group(
    entry: &DialogueCorpusEntry,
    source_segments: Vec<String>,
    controls: Vec<DialogueTranslationControl>,
) -> DialogueTranslationGroup {
    let protected_only = source_segments.iter().all(String::is_empty);
    DialogueTranslationGroup {
        semantic_source_sha256: entry.semantic_source_sha256.clone(),
        korean_segments: if protected_only {
            source_segments
                .iter()
                .map(|_| Some(String::new()))
                .collect()
        } else {
            source_segments.iter().map(|_| None).collect()
        },
        status: if protected_only {
            DialogueTranslationStatus::ProtectedOnly
        } else {
            DialogueTranslationStatus::Untranslated
        },
        source_segments,
        controls,
        coordinate_ids: vec![entry.coordinate_id.clone()],
        translator: None,
        reviewer: None,
        reviewed_at: None,
        notes: String::new(),
    }
}

fn translation_shape(
    entry: &DialogueCorpusEntry,
) -> (Vec<String>, Vec<DialogueTranslationControl>) {
    let mut source_segments = vec![String::new()];
    let mut controls = Vec::new();
    for token in &entry.tokens {
        match token {
            DialogueCorpusToken::Glyph { text, .. } => {
                source_segments.last_mut().unwrap().push_str(text)
            }
            DialogueCorpusToken::Control {
                semantic_name,
                arguments,
                ..
            } => {
                controls.push(DialogueTranslationControl {
                    semantic_name: semantic_name.clone(),
                    arguments: arguments.clone(),
                });
                source_segments.push(String::new());
            }
        }
    }
    (source_segments, controls)
}
