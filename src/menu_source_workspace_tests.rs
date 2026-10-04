use std::collections::BTreeMap;

use super::model::{MenuSourceAdmission, MenuSourceCandidate};
use super::writer::{
    MAX_MENU_SOURCE_SHARD_BYTES, MAX_MENU_SOURCE_SHARD_ENTRIES, MenuSourceShardIdentity,
    partition_entries,
};
use super::{
    MenuSourceTokenKind, compact_loaded_word_references, newopt_placement_reference_count,
    source_candidate, source_token,
};
use crate::menu_audit::{
    LoadedWordReferenceAudit, MenuNonTextEvidence, ReachableLoadedWordReferenceAudit,
    StringCandidate,
};
use crate::menu_glyph_audit::MenuGlyphCellAudit;

#[test]
fn source_token_preserves_control_nibble_and_exact_pixel_text() {
    let cell = MenuGlyphCellAudit {
        code: "0x0026".to_string(),
        page: 0,
        column: 6,
        row: 2,
        physical_x: 120,
        physical_y: 40,
        pixel_sha256: "pixel-hash".to_string(),
        exact_dialogue_pixel_text: Some("U".to_string()),
    };
    let glyphs = BTreeMap::from([(cell.code.as_str(), &cell)]);

    let token = source_token("0xa026", &glyphs).unwrap();

    assert_eq!(token.kind, MenuSourceTokenKind::Glyph);
    assert_eq!(token.normalized_code, "0x0026");
    assert_eq!(token.control_nibble, 0x0a);
    assert_eq!(token.exact_dialogue_pixel_text.as_deref(), Some("U"));
}

#[test]
fn skip_token_never_inherits_a_pixel() {
    let token = source_token("0xbfff", &BTreeMap::new()).unwrap();

    assert_eq!(token.kind, MenuSourceTokenKind::Skip);
    assert_eq!(token.control_nibble, 0x0b);
    assert!(token.pixel_sha256.is_none());
    assert!(token.exact_dialogue_pixel_text.is_none());
}

#[test]
fn source_candidates_are_partitioned_before_they_become_god_files() {
    let entries = (0..65)
        .map(|index| MenuSourceCandidate {
            candidate_id: format!("DAT1/TEST.BIN@0x{index:04x}"),
            target_offset: format!("0x{index:04x}"),
            admission: MenuSourceAdmission::StaticReferenceCandidate,
            consumer_confirmed: false,
            consumer_evidence: Vec::new(),
            consumer_reference_count: 0,
            non_text_evidence: Vec::new(),
            fully_exact_text: Some("A".to_string()),
            exact_decoded_glyph_count: 1,
            unresolved_glyph_count: 0,
            tokens: Vec::new(),
            pointer_offsets: Vec::new(),
            address_materialization_offsets: Vec::new(),
            memory_access_offsets: Vec::new(),
            loaded_word_references: Vec::new(),
            entrypoint_reachable_address_materialization_references: Vec::new(),
            entrypoint_reachable_memory_access_references: Vec::new(),
            shared_references: Vec::new(),
        })
        .collect();
    let identity = MenuSourceShardIdentity {
        source_bin_sha256: "source",
        dialogue_codebook_sha256: "codebook",
        menu_code_analysis_sha256: "code-analysis",
        menu_glyph_analysis_sha256: "glyph-analysis",
    };

    let chunks = partition_entries("DAT1/TEST.BIN", entries, &identity).unwrap();

    assert!(chunks.len() > 1);
    assert_eq!(chunks.iter().map(Vec::len).sum::<usize>(), 65);
    for chunk in chunks {
        assert!(chunk.len() <= MAX_MENU_SOURCE_SHARD_ENTRIES);
        assert!(serde_json::to_vec_pretty(&chunk).unwrap().len() < MAX_MENU_SOURCE_SHARD_BYTES);
    }
}

#[test]
fn only_the_source_bound_newopt_placement_table_confirms_consumers() {
    assert_eq!(
        newopt_placement_reference_count(
            "DAT1/NEWOPT.BIN",
            &["0x0b90".to_string(), "0x0bcc".to_string()]
        )
        .unwrap(),
        2
    );
    assert_eq!(
        newopt_placement_reference_count("DAT1/NEWOPT.BIN", &["0x0bd0".to_string()]).unwrap(),
        0
    );
    assert_eq!(
        newopt_placement_reference_count("DAT1/MODESEL.BIN", &["0x0b90".to_string()]).unwrap(),
        0
    );
}

#[test]
fn confirmed_non_text_structure_remains_evidence_without_becoming_a_translation_target() {
    let candidate = StringCandidate {
        target_offset: "0x0010".to_string(),
        consumer_evidence: Vec::new(),
        consumer_reference_count: 0,
        non_text_evidence: vec![MenuNonTextEvidence::MiniselPrimitiveRecordTable],
        pointer_offsets: Vec::new(),
        address_materialization_offsets: Vec::new(),
        memory_access_offsets: Vec::new(),
        loaded_word_references: Vec::new(),
        entrypoint_reachable_address_materialization_references: Vec::new(),
        entrypoint_reachable_memory_access_references: Vec::new(),
        entrypoint_reachable_loaded_word_references: Vec::new(),
        entrypoint_reachable_direct_pointer_load_references: Vec::new(),
        shared_references: Vec::new(),
        length: 1,
        raw_codes: vec!["0x0fff".to_string()],
    };

    let entry = source_candidate("DAT1/MINISEL.BIN", candidate, &BTreeMap::new()).unwrap();

    assert_eq!(
        entry.admission,
        MenuSourceAdmission::ConfirmedNonTextStructure
    );
    assert!(!entry.consumer_confirmed);
    assert_eq!(entry.non_text_evidence.len(), 1);
}

#[test]
fn loaded_word_evidence_is_normalized_without_losing_reachable_seeds() {
    let loaded_word = LoadedWordReferenceAudit {
        instruction_offset: "0x0018".to_string(),
        load_instruction_offset: "0x0014".to_string(),
        storage_address: "0x800a2020".to_string(),
    };
    let reachable = |seed: &str| ReachableLoadedWordReferenceAudit {
        seed_offset: seed.to_string(),
        instruction_offset: loaded_word.instruction_offset.clone(),
        load_instruction_offset: loaded_word.load_instruction_offset.clone(),
        storage_address: loaded_word.storage_address.clone(),
    };
    let reachable_loaded_words = vec![reachable("0x0010"), reachable("0x0030")];
    let direct_pointer_loads = vec![reachable("0x0010")];
    let candidate = StringCandidate {
        target_offset: "0x0040".to_string(),
        consumer_evidence: Vec::new(),
        consumer_reference_count: 0,
        non_text_evidence: Vec::new(),
        pointer_offsets: vec!["0x0020".to_string()],
        address_materialization_offsets: Vec::new(),
        memory_access_offsets: Vec::new(),
        loaded_word_references: vec![loaded_word],
        entrypoint_reachable_address_materialization_references: Vec::new(),
        entrypoint_reachable_memory_access_references: Vec::new(),
        entrypoint_reachable_loaded_word_references: reachable_loaded_words,
        entrypoint_reachable_direct_pointer_load_references: direct_pointer_loads,
        shared_references: Vec::new(),
        length: 1,
        raw_codes: vec!["0x0047".to_string()],
    };

    let compact = compact_loaded_word_references(&candidate).unwrap();

    assert_eq!(compact.len(), 1);
    assert_eq!(
        compact[0].entrypoint_reachable_seed_offsets,
        ["0x0010", "0x0030"]
    );
    assert_eq!(
        compact[0].entrypoint_reachable_direct_pointer_load_seed_offsets,
        ["0x0010"]
    );
}
