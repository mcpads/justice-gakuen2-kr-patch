use std::collections::BTreeMap;

use super::glossary::{
    entries_for_group, glossary_audit_report, term_occurs, validate_dialogue_glossary,
};
use super::glossary_model::{DialogueGlossary, DialogueGlossaryEntry, DialogueGlossaryStatus};
use super::translation_model::{
    DialogueTranslationGroup, DialogueTranslationInput, DialogueTranslationProjectStatus,
    DialogueTranslationStatus,
};

#[test]
fn katakana_term_matching_does_not_find_a_name_inside_another_word() {
    assert!(term_occurs("ランの新聞", "ラン"));
    assert!(!term_occurs("フランクフルト", "ラン"));
}

#[test]
fn glossary_is_bound_sorted_evidenced_and_source_present() {
    let source = source_input("バツのトウモロコシ");
    let glossary = glossary(vec![entry("バツ", "바츠")]);
    validate_dialogue_glossary(&glossary, &source).unwrap();

    let applicable = entries_for_group(&glossary.entries, &source.groups[0]);
    assert_eq!(applicable.len(), 1);
    assert_eq!(applicable[0].korean, "바츠");
}

#[test]
fn glossary_rejects_unbound_or_falsely_approved_terms() {
    let source = source_input("バツ");
    let mut unbound = glossary(vec![entry("ロイ", "로이")]);
    assert!(validate_dialogue_glossary(&unbound, &source).is_err());

    unbound.entries[0] = entry("バツ", "바츠");
    unbound.entries[0].status = DialogueGlossaryStatus::Approved;
    assert!(validate_dialogue_glossary(&unbound, &source).is_err());
}

#[test]
fn audit_report_keeps_candidate_terms_out_of_approval() {
    let source = source_input("バツとバツ");
    let glossary = glossary(vec![entry("バツ", "바츠")]);
    let report = glossary_audit_report(&glossary, &source, "glossary".to_string());

    assert_eq!(report.entry_count, 1);
    assert_eq!(report.candidate_entry_count, 1);
    assert_eq!(report.matched_group_count, 1);
    assert_eq!(report.matched_term_occurrence_count, 2);
    assert!(!report.all_terms_project_owner_approved);
}

fn glossary(entries: Vec<DialogueGlossaryEntry>) -> DialogueGlossary {
    DialogueGlossary {
        kind: "Justice Gakuen 2 Korean dialogue glossary".to_string(),
        source_bin_sha256: "source".to_string(),
        source_corpus_sha256: "corpus".to_string(),
        evidence_sources: BTreeMap::from([("source".to_string(), "test source".to_string())]),
        entries,
    }
}

fn entry(source: &str, korean: &str) -> DialogueGlossaryEntry {
    DialogueGlossaryEntry {
        source: source.to_string(),
        korean: korean.to_string(),
        category: "character_name".to_string(),
        status: DialogueGlossaryStatus::Candidate,
        evidence_ids: vec!["source".to_string()],
        usage_note: "test term".to_string(),
        reviewed_by: None,
        reviewed_at: None,
    }
}

fn source_input(segment: &str) -> DialogueTranslationInput {
    DialogueTranslationInput {
        kind: "test".to_string(),
        source_bin_sha256: "source".to_string(),
        codebook_sha256: "codebook".to_string(),
        source_corpus_sha256: "corpus".to_string(),
        translation_status: DialogueTranslationProjectStatus::InProgress,
        translation_review: None,
        groups: vec![DialogueTranslationGroup {
            semantic_source_sha256: "group".to_string(),
            source_segments: vec![segment.to_string()],
            controls: Vec::new(),
            coordinate_ids: vec!["coordinate".to_string()],
            korean_segments: vec![None],
            status: DialogueTranslationStatus::Untranslated,
            translator: None,
            reviewer: None,
            reviewed_at: None,
            notes: String::new(),
        }],
    }
}
