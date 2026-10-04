use std::collections::BTreeSet;

use super::scanner::{AddressFlowReference, CandidateString, LoadedWordReference};

#[derive(Debug, Default)]
pub(super) struct ReachableCandidateReferences {
    pub(super) address_materializations: Vec<AddressFlowReference>,
    pub(super) memory_accesses: Vec<AddressFlowReference>,
    pub(super) loaded_words: Vec<LoadedWordReference>,
}

impl ReachableCandidateReferences {
    pub(super) fn is_empty(&self) -> bool {
        self.address_materializations.is_empty()
            && self.memory_accesses.is_empty()
            && self.loaded_words.is_empty()
    }
}

pub(super) fn bind_reachable_candidate_references(
    candidate: &CandidateString,
    reachable_instruction_offsets: &BTreeSet<usize>,
    reachable_lui_seed_offsets: &BTreeSet<usize>,
) -> ReachableCandidateReferences {
    let address_materializations = candidate
        .address_materialization_references
        .iter()
        .filter(|reference| {
            address_flow_reference_is_reachable(
                reference,
                reachable_instruction_offsets,
                reachable_lui_seed_offsets,
            )
        })
        .cloned()
        .collect();
    let memory_accesses = candidate
        .memory_access_references
        .iter()
        .filter(|reference| {
            address_flow_reference_is_reachable(
                reference,
                reachable_instruction_offsets,
                reachable_lui_seed_offsets,
            )
        })
        .cloned()
        .collect();
    let loaded_words = candidate
        .loaded_word_references
        .iter()
        .filter(|reference| {
            reachable_lui_seed_offsets.contains(&reference.seed_offset)
                && reachable_instruction_offsets.contains(&reference.load_instruction_offset)
                && reachable_instruction_offsets.contains(&reference.instruction_offset)
        })
        .cloned()
        .collect();
    ReachableCandidateReferences {
        address_materializations,
        memory_accesses,
        loaded_words,
    }
}

fn address_flow_reference_is_reachable(
    reference: &AddressFlowReference,
    reachable_instruction_offsets: &BTreeSet<usize>,
    reachable_lui_seed_offsets: &BTreeSet<usize>,
) -> bool {
    reachable_lui_seed_offsets.contains(&reference.seed_offset)
        && reachable_instruction_offsets.contains(&reference.instruction_offset)
}

#[cfg(test)]
#[path = "reachable_references_tests.rs"]
mod tests;
