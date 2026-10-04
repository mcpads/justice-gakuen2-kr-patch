use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};

use crate::pipeline::sha256_bytes;
use crate::source_disc::SupportedSourceDisc;

use super::corpus::{
    extract_all_dialogue_source_corpus_from_source, extract_dialogue_source_corpus_from_source,
};
use super::corpus_digest::source_corpus_sha256;
use super::runtime_insertion_messages::runtime_insertion_korean_text;
use super::script_source::load_mgame_from_source;
use super::selector_consumer_spec::validate_adopted_selector_consumers;
use super::selector_consumers_model::DialogueSelectorConsumerReport;
use super::selector_translation_model::{
    DialogueSelectorConsumerEvidence, DialogueSelectorDevelopmentResolution,
    DialogueSelectorTranslationContextEntry, DialogueSelectorTranslationScope,
    DialogueSelectorTranslationSourceGroup,
};
use super::translation::expected_translation_input_with_source_corpus_sha256;

const FIRST_SELECTOR_TRANSLATION_BANK: usize = 2;

pub(super) struct SelectorTranslationSource {
    pub(super) source_bin_sha256: String,
    pub(super) codebook_sha256: String,
    pub(super) source_corpus_sha256: String,
    pub(super) selector_consumer_audit_sha256: String,
    pub(super) scope: DialogueSelectorTranslationScope,
    pub(super) source_asset_count: usize,
    pub(super) groups_by_selector: BTreeMap<usize, Vec<DialogueSelectorTranslationSourceGroup>>,
    pub(super) contexts_by_selector: BTreeMap<usize, Vec<DialogueSelectorTranslationContextEntry>>,
}

pub(super) fn extract_selector_translation_source(
    cue_path: &std::path::Path,
    codebook_path: &std::path::Path,
    scope: DialogueSelectorTranslationScope,
) -> Result<SelectorTranslationSource> {
    let source = SupportedSourceDisc::open(cue_path)?;
    extract_selector_translation_source_from_source(&source, codebook_path, scope)
}

pub(super) fn extract_selector_translation_source_from_source(
    source: &SupportedSourceDisc,
    codebook_path: &std::path::Path,
    scope: DialogueSelectorTranslationScope,
) -> Result<SelectorTranslationSource> {
    let corpus = match scope {
        DialogueSelectorTranslationScope::MgkDevelopment => {
            extract_dialogue_source_corpus_from_source(source, codebook_path)?
        }
        DialogueSelectorTranslationScope::AllRuntimeImages => {
            extract_all_dialogue_source_corpus_from_source(source, codebook_path)?
        }
    };
    let source_corpus_sha256 = source_corpus_sha256(&corpus)?;
    extract_selector_translation_source_from_corpus_and_source(
        source,
        &corpus,
        source_corpus_sha256,
        scope,
    )
}

pub(super) fn extract_selector_translation_source_from_source_with_bound_corpus_sha256(
    source: &SupportedSourceDisc,
    codebook_path: &std::path::Path,
    scope: DialogueSelectorTranslationScope,
    bound_source_corpus_sha256: &str,
) -> Result<SelectorTranslationSource> {
    let corpus = match scope {
        DialogueSelectorTranslationScope::MgkDevelopment => {
            extract_dialogue_source_corpus_from_source(source, codebook_path)?
        }
        DialogueSelectorTranslationScope::AllRuntimeImages => {
            extract_all_dialogue_source_corpus_from_source(source, codebook_path)?
        }
    };
    extract_selector_translation_source_from_corpus_and_source(
        source,
        &corpus,
        bound_source_corpus_sha256.to_string(),
        scope,
    )
}

pub(super) fn extract_selector_translation_source_from_corpus_and_source(
    source: &SupportedSourceDisc,
    corpus: &super::corpus_model::DialogueSourceCorpus,
    source_corpus_sha256: String,
    scope: DialogueSelectorTranslationScope,
) -> Result<SelectorTranslationSource> {
    ensure!(
        corpus.assets.len() == scope.source_asset_count(),
        "selector translation source asset population changed"
    );
    let codebook_sha256 = corpus.codebook_sha256.clone();
    let consumer_report = validate_adopted_selector_consumers(&load_mgame_from_source(source)?)?;
    let selector_consumer_audit_sha256 = sha256_bytes(&serde_json::to_vec(&consumer_report)?);
    let source_groups =
        expected_translation_input_with_source_corpus_sha256(corpus, source_corpus_sha256.clone())?
            .groups
            .into_iter()
            .map(|group| (group.semantic_source_sha256.clone(), group))
            .collect::<BTreeMap<_, _>>();

    let mut contexts_by_hash = BTreeMap::<String, Vec<_>>::new();
    for asset in &corpus.assets {
        for bank in &asset.banks {
            if bank.selector_index < FIRST_SELECTOR_TRANSLATION_BANK {
                continue;
            }
            for entry in &bank.entries {
                let (consumer_evidence, evidence_basis) =
                    occurrence_evidence(&consumer_report, bank.selector_index, entry.entry_index);
                let context = DialogueSelectorTranslationContextEntry {
                    occurrence_id: entry.coordinate_id.clone(),
                    source_path: asset.source_path.clone(),
                    selector_index: bank.selector_index,
                    entry_index: entry.entry_index,
                    decoded_offset: entry.decoded_offset.clone(),
                    runtime_address: entry.runtime_address.clone(),
                    coordinate_id: entry.coordinate_id.clone(),
                    semantic_source_sha256: entry.semantic_source_sha256.clone(),
                    consumer_evidence,
                    evidence_basis,
                };
                contexts_by_hash
                    .entry(entry.semantic_source_sha256.clone())
                    .or_default()
                    .push(context);
            }
        }
    }

    let mut groups_by_selector = BTreeMap::<usize, Vec<_>>::new();
    let mut contexts_by_selector = BTreeMap::<usize, Vec<_>>::new();
    for (semantic_source_sha256, mut contexts) in contexts_by_hash {
        contexts.sort_by(|left, right| left.coordinate_id.cmp(&right.coordinate_id));
        let canonical_selector = contexts
            .iter()
            .map(|context| context.selector_index)
            .min()
            .context("selector translation group has no coordinate")?;
        let target_selectors = contexts
            .iter()
            .map(|context| context.selector_index)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let consumer_evidence = contexts
            .iter()
            .map(|context| context.consumer_evidence)
            .max()
            .context("selector translation group has no consumer evidence")?;
        let evidence_basis = contexts
            .iter()
            .filter(|context| context.consumer_evidence == consumer_evidence)
            .flat_map(|context| context.evidence_basis.iter().cloned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let expected = source_groups
            .get(&semantic_source_sha256)
            .context("selector translation semantic group disappeared")?;
        let runtime_korean_segments = runtime_korean_segments(corpus, &contexts)?;
        let development_resolution = if runtime_korean_segments.is_some() {
            DialogueSelectorDevelopmentResolution::RuntimeInsertionRewrite
        } else {
            DialogueSelectorDevelopmentResolution::AuthoredTranslation
        };
        let coordinate_count = contexts.len();
        let coordinate_ids = match scope {
            DialogueSelectorTranslationScope::MgkDevelopment => contexts
                .iter()
                .map(|context| context.coordinate_id.clone())
                .collect(),
            DialogueSelectorTranslationScope::AllRuntimeImages => Vec::new(),
        };
        groups_by_selector
            .entry(canonical_selector)
            .or_default()
            .push(DialogueSelectorTranslationSourceGroup {
                semantic_source_sha256,
                source_segments: expected.source_segments.clone(),
                controls: expected.controls.clone(),
                target_selectors,
                coordinate_count,
                coordinate_ids,
                consumer_evidence,
                evidence_basis,
                development_resolution,
                development_korean_segments: runtime_korean_segments,
            });
        contexts_by_selector
            .entry(canonical_selector)
            .or_default()
            .append(&mut contexts);
    }
    for groups in groups_by_selector.values_mut() {
        groups.sort_by(|left, right| {
            left.semantic_source_sha256
                .cmp(&right.semantic_source_sha256)
        });
    }
    for contexts in contexts_by_selector.values_mut() {
        contexts.sort_by(|left, right| left.coordinate_id.cmp(&right.coordinate_id));
    }

    validate_denominator(scope, &groups_by_selector, &contexts_by_selector)?;
    let source_asset_count = corpus.assets.len();
    Ok(SelectorTranslationSource {
        source_bin_sha256: corpus.source_bin_sha256.clone(),
        codebook_sha256,
        source_corpus_sha256,
        selector_consumer_audit_sha256,
        scope,
        source_asset_count,
        groups_by_selector,
        contexts_by_selector,
    })
}

fn runtime_korean_segments(
    corpus: &super::corpus_model::DialogueSourceCorpus,
    contexts: &[DialogueSelectorTranslationContextEntry],
) -> Result<Option<Vec<String>>> {
    let mut korean_text = None;
    for context in contexts {
        let asset = corpus
            .assets
            .iter()
            .find(|asset| asset.source_path == context.source_path)
            .context("selector translation context asset disappeared")?;
        let bank = asset
            .banks
            .iter()
            .find(|bank| bank.selector_index == context.selector_index)
            .context("selector translation context bank disappeared")?;
        let entry = bank
            .entries
            .get(context.entry_index)
            .context("selector translation context entry disappeared")?;
        if let Some(text) = runtime_insertion_korean_text(context.selector_index, entry)? {
            if let Some(existing) = korean_text {
                ensure!(
                    existing == text,
                    "one semantic group has conflicting runtime insertion text"
                );
            }
            korean_text = Some(text);
        }
    }
    Ok(korean_text.map(|text| vec![text.to_string(), String::new()]))
}

fn occurrence_evidence(
    report: &DialogueSelectorConsumerReport,
    selector_index: usize,
    entry_index: usize,
) -> (DialogueSelectorConsumerEvidence, Vec<String>) {
    if selector_index == 2 && (5..=8).contains(&entry_index) {
        return (
            DialogueSelectorConsumerEvidence::RuntimeInsertionRewrite,
            vec![format!(
                "source_bound_runtime_insertion_selector_{selector_index}_entry_{entry_index}"
            )],
        );
    }
    let entry_loads = report
        .consumers
        .iter()
        .filter(|consumer| consumer.selector_index == selector_index)
        .flat_map(|consumer| &consumer.entry_pointer_loads)
        .filter(|entry| entry.entry_index == entry_index)
        .map(|entry| {
            format!(
                "direct_entry_load_{}",
                entry.load_instruction_runtime_address
            )
        })
        .collect::<BTreeSet<_>>();
    if !entry_loads.is_empty() {
        return (
            DialogueSelectorConsumerEvidence::DirectEntryLoad,
            entry_loads.into_iter().collect(),
        );
    }
    let selector_loads = report
        .consumers
        .iter()
        .filter(|consumer| consumer.selector_index == selector_index)
        .map(|consumer| {
            format!(
                "direct_selector_load_{}",
                consumer.load_instruction_runtime_address
            )
        })
        .collect::<BTreeSet<_>>();
    if !selector_loads.is_empty() {
        return (
            DialogueSelectorConsumerEvidence::DirectSelectorLoad,
            selector_loads.into_iter().collect(),
        );
    }
    (
        DialogueSelectorConsumerEvidence::ConsumerPending,
        vec!["consumer_not_yet_resolved".to_string()],
    )
}

fn validate_denominator(
    scope: DialogueSelectorTranslationScope,
    groups_by_selector: &BTreeMap<usize, Vec<DialogueSelectorTranslationSourceGroup>>,
    contexts_by_selector: &BTreeMap<usize, Vec<DialogueSelectorTranslationContextEntry>>,
) -> Result<()> {
    let groups = groups_by_selector.values().flatten().collect::<Vec<_>>();
    let contexts = contexts_by_selector.values().flatten().collect::<Vec<_>>();
    let semantic_hashes = groups
        .iter()
        .map(|group| group.semantic_source_sha256.as_str())
        .collect::<BTreeSet<_>>();
    let coordinate_ids = contexts
        .iter()
        .map(|context| context.coordinate_id.as_str())
        .collect::<BTreeSet<_>>();
    let count = |evidence| {
        groups
            .iter()
            .filter(|group| group.consumer_evidence == evidence)
            .count()
    };
    let (group_count, coordinate_count, direct_selector_count) = match scope {
        DialogueSelectorTranslationScope::MgkDevelopment => (496, 3_070, 431),
        DialogueSelectorTranslationScope::AllRuntimeImages => (550, 22_500, 485),
    };
    ensure!(
        groups_by_selector.keys().copied().collect::<Vec<_>>() == vec![2, 3, 4, 5, 6]
            && groups.len() == group_count
            && semantic_hashes.len() == groups.len()
            && contexts.len() == coordinate_count
            && coordinate_ids.len() == contexts.len()
            && count(DialogueSelectorConsumerEvidence::RuntimeInsertionRewrite) == 4
            && count(DialogueSelectorConsumerEvidence::DirectEntryLoad) == 1
            && count(DialogueSelectorConsumerEvidence::DirectSelectorLoad) == direct_selector_count
            && count(DialogueSelectorConsumerEvidence::ConsumerPending) == 60,
        "selector translation source denominator changed for {}: groups={}, coordinates={}, runtime={}, direct_entry={}, direct_selector={}, pending={}",
        scope.target_scope(),
        groups.len(),
        contexts.len(),
        count(DialogueSelectorConsumerEvidence::RuntimeInsertionRewrite),
        count(DialogueSelectorConsumerEvidence::DirectEntryLoad),
        count(DialogueSelectorConsumerEvidence::DirectSelectorLoad),
        count(DialogueSelectorConsumerEvidence::ConsumerPending),
    );
    Ok(())
}

#[cfg(test)]
pub(super) fn classify_test_evidence(
    report: &DialogueSelectorConsumerReport,
    selector_index: usize,
    entry_index: usize,
) -> DialogueSelectorConsumerEvidence {
    occurrence_evidence(report, selector_index, entry_index).0
}
