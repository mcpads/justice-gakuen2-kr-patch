use std::collections::BTreeSet;

use super::bind_reachable_candidate_references;
use crate::menu_audit::scanner::{AddressFlowReference, CandidateString, LoadedWordReference};

#[test]
fn binds_only_references_whose_seed_and_consumer_are_reachable() {
    let candidate = candidate_with_references();
    let reachable_instructions = BTreeSet::from([0x10, 0x14, 0x18, 0x20, 0x24]);
    let reachable_lui_seeds = BTreeSet::from([0x10, 0x20]);

    let reachable = bind_reachable_candidate_references(
        &candidate,
        &reachable_instructions,
        &reachable_lui_seeds,
    );

    assert_eq!(reachable.address_materializations.len(), 1);
    assert_eq!(reachable.address_materializations[0].seed_offset, 0x10);
    assert_eq!(reachable.memory_accesses.len(), 1);
    assert_eq!(reachable.memory_accesses[0].instruction_offset, 0x24);
    assert_eq!(reachable.loaded_words.len(), 1);
    assert_eq!(reachable.loaded_words[0].instruction_offset, 0x18);
}

#[test]
fn rejects_a_loaded_word_when_its_load_instruction_is_unreachable() {
    let candidate = CandidateString {
        pointer_offsets: Vec::new(),
        address_materialization_references: Vec::new(),
        memory_access_references: Vec::new(),
        loaded_word_references: vec![LoadedWordReference {
            seed_offset: 0x10,
            load_instruction_offset: 0x14,
            instruction_offset: 0x18,
            storage_address: 0x800a_2080,
        }],
        shared_pointer_offsets: Vec::new(),
        shared_address_materialization_references: Vec::new(),
        shared_memory_access_references: Vec::new(),
        shared_loaded_word_references: Vec::new(),
        raw_codes: vec![0x0047],
    };

    let reachable = bind_reachable_candidate_references(
        &candidate,
        &BTreeSet::from([0x10, 0x18]),
        &BTreeSet::from([0x10]),
    );

    assert!(reachable.is_empty());
}

#[test]
fn raw_pointer_storage_is_not_entrypoint_reachable_code_evidence() {
    let candidate = CandidateString {
        pointer_offsets: vec![0x40],
        address_materialization_references: Vec::new(),
        memory_access_references: Vec::new(),
        loaded_word_references: Vec::new(),
        shared_pointer_offsets: Vec::new(),
        shared_address_materialization_references: Vec::new(),
        shared_memory_access_references: Vec::new(),
        shared_loaded_word_references: Vec::new(),
        raw_codes: vec![0x0047],
    };

    let reachable =
        bind_reachable_candidate_references(&candidate, &BTreeSet::new(), &BTreeSet::new());

    assert!(reachable.is_empty());
}

fn candidate_with_references() -> CandidateString {
    CandidateString {
        pointer_offsets: vec![0x40],
        address_materialization_references: vec![
            AddressFlowReference {
                seed_offset: 0x10,
                instruction_offset: 0x14,
            },
            AddressFlowReference {
                seed_offset: 0x30,
                instruction_offset: 0x34,
            },
        ],
        memory_access_references: vec![
            AddressFlowReference {
                seed_offset: 0x20,
                instruction_offset: 0x24,
            },
            AddressFlowReference {
                seed_offset: 0x28,
                instruction_offset: 0x2c,
            },
        ],
        loaded_word_references: vec![
            LoadedWordReference {
                seed_offset: 0x10,
                load_instruction_offset: 0x14,
                instruction_offset: 0x18,
                storage_address: 0x800a_2080,
            },
            LoadedWordReference {
                seed_offset: 0x30,
                load_instruction_offset: 0x34,
                instruction_offset: 0x38,
                storage_address: 0x800a_2084,
            },
        ],
        shared_pointer_offsets: Vec::new(),
        shared_address_materialization_references: Vec::new(),
        shared_memory_access_references: Vec::new(),
        shared_loaded_word_references: Vec::new(),
        raw_codes: vec![0x0047],
    }
}
