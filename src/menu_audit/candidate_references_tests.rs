use std::collections::BTreeSet;

use super::{CandidateClassificationEvidence, audit_string_candidate};
use crate::menu_audit::scanner::{CandidateString, LoadedWordReference, OVERLAY_BASE};

#[test]
fn direct_pointer_load_requires_reachable_storage_owned_by_the_candidate() {
    let candidate = CandidateString {
        pointer_offsets: vec![0x20],
        address_materialization_references: Vec::new(),
        memory_access_references: Vec::new(),
        loaded_word_references: vec![
            LoadedWordReference {
                seed_offset: 0x10,
                load_instruction_offset: 0x14,
                instruction_offset: 0x18,
                storage_address: OVERLAY_BASE + 0x20,
            },
            LoadedWordReference {
                seed_offset: 0x10,
                load_instruction_offset: 0x14,
                instruction_offset: 0x1c,
                storage_address: OVERLAY_BASE + 0x24,
            },
        ],
        shared_pointer_offsets: Vec::new(),
        shared_address_materialization_references: Vec::new(),
        shared_memory_access_references: Vec::new(),
        shared_loaded_word_references: Vec::new(),
        raw_codes: vec![0x0047],
    };

    let audited = audit_string_candidate(
        0x40,
        candidate,
        CandidateClassificationEvidence {
            consumer: Vec::new(),
            non_text: Vec::new(),
        },
        "SLPS_021.20",
        Some(OVERLAY_BASE),
        &BTreeSet::from([0x10, 0x14, 0x18, 0x1c]),
        &BTreeSet::from([0x10]),
    );

    assert_eq!(
        audited
            .candidate
            .entrypoint_reachable_loaded_word_references
            .len(),
        2
    );
    assert_eq!(
        audited
            .candidate
            .entrypoint_reachable_direct_pointer_load_references
            .len(),
        1
    );
    assert_eq!(
        audited
            .candidate
            .entrypoint_reachable_direct_pointer_load_references[0]
            .storage_address,
        "0x800a2020"
    );
}
