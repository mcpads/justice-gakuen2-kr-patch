use std::collections::BTreeSet;

use super::model::{
    AddressFlowReferenceAudit, LoadedWordReferenceAudit, MenuConsumerEvidence, MenuNonTextEvidence,
    ReachableLoadedWordReferenceAudit, SharedStringReferences, StringCandidate,
};
use super::reachable_references::bind_reachable_candidate_references;
use super::scanner::{AddressFlowReference, CandidateString, LoadedWordReference};

#[derive(Debug, Default)]
pub(super) struct CandidateReferenceCounts {
    pub(super) pointer: usize,
    pub(super) address_materialization: usize,
    pub(super) memory_access: usize,
    pub(super) loaded_word: usize,
    pub(super) entrypoint_reachable_string: usize,
    pub(super) entrypoint_reachable_address_materialization: usize,
    pub(super) entrypoint_reachable_memory_access: usize,
    pub(super) entrypoint_reachable_loaded_word: usize,
    pub(super) entrypoint_reachable_direct_pointer_load_string: usize,
    pub(super) entrypoint_reachable_direct_pointer_load: usize,
    pub(super) shared_pointer: usize,
    pub(super) shared_address_materialization: usize,
    pub(super) shared_memory_access: usize,
    pub(super) shared_loaded_word: usize,
}

pub(super) struct AuditedStringCandidate {
    pub(super) candidate: StringCandidate,
    pub(super) reference_counts: CandidateReferenceCounts,
}

pub(super) struct CandidateClassificationEvidence {
    pub(super) consumer: Vec<MenuConsumerEvidence>,
    pub(super) non_text: Vec<MenuNonTextEvidence>,
}

pub(super) fn audit_string_candidate(
    target_offset: usize,
    candidate: CandidateString,
    classification: CandidateClassificationEvidence,
    shared_source_path: &str,
    runtime_base: Option<u32>,
    reachable_instruction_offsets: &BTreeSet<usize>,
    reachable_lui_seed_offsets: &BTreeSet<usize>,
) -> AuditedStringCandidate {
    let consumer_reference_count = classification.consumer.len();
    let reachable_references = bind_reachable_candidate_references(
        &candidate,
        reachable_instruction_offsets,
        reachable_lui_seed_offsets,
    );
    let pointer_offsets = candidate
        .pointer_offsets
        .iter()
        .copied()
        .map(hex_offset)
        .collect::<Vec<_>>();
    let address_materialization_offsets =
        unique_address_materialization_offsets(candidate.address_materialization_references.iter());
    let memory_access_offsets =
        unique_memory_access_offsets(candidate.memory_access_references.iter());
    let loaded_word_references =
        loaded_word_reference_audits(candidate.loaded_word_references.iter());
    let entrypoint_reachable_address_materialization_references = reachable_references
        .address_materializations
        .iter()
        .map(address_flow_reference_audit)
        .collect::<Vec<_>>();
    let entrypoint_reachable_memory_access_references = reachable_references
        .memory_accesses
        .iter()
        .map(address_flow_reference_audit)
        .collect::<Vec<_>>();
    let entrypoint_reachable_loaded_word_references = reachable_references
        .loaded_words
        .iter()
        .map(reachable_loaded_word_reference_audit)
        .collect::<Vec<_>>();
    let pointer_offset_set = candidate
        .pointer_offsets
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let entrypoint_reachable_direct_pointer_load_references = runtime_base
        .map(|runtime_base| {
            reachable_references
                .loaded_words
                .iter()
                .filter(|reference| {
                    reference
                        .storage_address
                        .checked_sub(runtime_base)
                        .and_then(|offset| usize::try_from(offset).ok())
                        .is_some_and(|offset| pointer_offset_set.contains(&offset))
                })
                .map(reachable_loaded_word_reference_audit)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let shared_references = (!candidate.shared_pointer_offsets.is_empty()
        || !candidate
            .shared_address_materialization_references
            .is_empty()
        || !candidate.shared_memory_access_references.is_empty()
        || !candidate.shared_loaded_word_references.is_empty())
    .then(|| SharedStringReferences {
        source_path: shared_source_path.to_string(),
        pointer_offsets: candidate
            .shared_pointer_offsets
            .iter()
            .copied()
            .map(hex_offset)
            .collect(),
        address_materialization_offsets: unique_address_materialization_offsets(
            candidate.shared_address_materialization_references.iter(),
        ),
        memory_access_offsets: unique_memory_access_offsets(
            candidate.shared_memory_access_references.iter(),
        ),
        loaded_word_references: loaded_word_reference_audits(
            candidate.shared_loaded_word_references.iter(),
        ),
    })
    .into_iter()
    .collect::<Vec<_>>();
    let reference_counts = CandidateReferenceCounts {
        pointer: pointer_offsets.len(),
        address_materialization: address_materialization_offsets.len(),
        memory_access: memory_access_offsets.len(),
        loaded_word: loaded_word_references.len(),
        entrypoint_reachable_string: usize::from(!reachable_references.is_empty()),
        entrypoint_reachable_address_materialization:
            entrypoint_reachable_address_materialization_references.len(),
        entrypoint_reachable_memory_access: entrypoint_reachable_memory_access_references.len(),
        entrypoint_reachable_loaded_word: entrypoint_reachable_loaded_word_references.len(),
        entrypoint_reachable_direct_pointer_load_string: usize::from(
            !entrypoint_reachable_direct_pointer_load_references.is_empty(),
        ),
        entrypoint_reachable_direct_pointer_load:
            entrypoint_reachable_direct_pointer_load_references.len(),
        shared_pointer: shared_references
            .iter()
            .map(|references| references.pointer_offsets.len())
            .sum(),
        shared_address_materialization: shared_references
            .iter()
            .map(|references| references.address_materialization_offsets.len())
            .sum(),
        shared_memory_access: shared_references
            .iter()
            .map(|references| references.memory_access_offsets.len())
            .sum(),
        shared_loaded_word: shared_references
            .iter()
            .map(|references| references.loaded_word_references.len())
            .sum(),
    };
    AuditedStringCandidate {
        candidate: StringCandidate {
            target_offset: hex_offset(target_offset),
            consumer_evidence: classification.consumer,
            consumer_reference_count,
            non_text_evidence: classification.non_text,
            pointer_offsets,
            address_materialization_offsets,
            memory_access_offsets,
            loaded_word_references,
            entrypoint_reachable_address_materialization_references,
            entrypoint_reachable_memory_access_references,
            entrypoint_reachable_loaded_word_references,
            entrypoint_reachable_direct_pointer_load_references,
            shared_references,
            length: candidate.raw_codes.len(),
            raw_codes: candidate.raw_codes.into_iter().map(hex_code).collect(),
        },
        reference_counts,
    }
}

pub(super) fn address_materialization_reference_count(
    candidate: &CandidateString,
    shared: bool,
) -> usize {
    if shared {
        unique_address_materialization_offsets(
            candidate.shared_address_materialization_references.iter(),
        )
        .len()
    } else {
        unique_address_materialization_offsets(candidate.address_materialization_references.iter())
            .len()
    }
}

pub(super) fn memory_access_reference_count(candidate: &CandidateString, shared: bool) -> usize {
    if shared {
        unique_memory_access_offsets(candidate.shared_memory_access_references.iter()).len()
    } else {
        unique_memory_access_offsets(candidate.memory_access_references.iter()).len()
    }
}

pub(super) fn loaded_word_reference_count(references: &[LoadedWordReference]) -> usize {
    loaded_word_reference_audits(references.iter()).len()
}

fn unique_address_materialization_offsets<'a>(
    references: impl Iterator<Item = &'a AddressFlowReference>,
) -> Vec<String> {
    references
        .map(|reference| reference.seed_offset)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(hex_offset)
        .collect()
}

fn unique_memory_access_offsets<'a>(
    references: impl Iterator<Item = &'a AddressFlowReference>,
) -> Vec<String> {
    references
        .map(|reference| reference.instruction_offset)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(hex_offset)
        .collect()
}

fn address_flow_reference_audit(reference: &AddressFlowReference) -> AddressFlowReferenceAudit {
    AddressFlowReferenceAudit {
        seed_offset: hex_offset(reference.seed_offset),
        instruction_offset: hex_offset(reference.instruction_offset),
    }
}

fn loaded_word_reference_audits<'a>(
    references: impl Iterator<Item = &'a LoadedWordReference>,
) -> Vec<LoadedWordReferenceAudit> {
    references
        .map(|reference| {
            (
                reference.instruction_offset,
                reference.load_instruction_offset,
                reference.storage_address,
            )
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(
            |(instruction_offset, load_instruction_offset, storage_address)| {
                LoadedWordReferenceAudit {
                    instruction_offset: hex_offset(instruction_offset),
                    load_instruction_offset: hex_offset(load_instruction_offset),
                    storage_address: format!("0x{storage_address:08x}"),
                }
            },
        )
        .collect()
}

fn reachable_loaded_word_reference_audit(
    reference: &LoadedWordReference,
) -> ReachableLoadedWordReferenceAudit {
    ReachableLoadedWordReferenceAudit {
        seed_offset: hex_offset(reference.seed_offset),
        instruction_offset: hex_offset(reference.instruction_offset),
        load_instruction_offset: hex_offset(reference.load_instruction_offset),
        storage_address: format!("0x{:08x}", reference.storage_address),
    }
}

fn hex_offset(offset: usize) -> String {
    format!("0x{offset:04x}")
}

fn hex_code(code: u16) -> String {
    format!("0x{code:04x}")
}

#[cfg(test)]
#[path = "candidate_references_tests.rs"]
mod tests;
