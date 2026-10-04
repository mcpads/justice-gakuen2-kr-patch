use super::dialogue_development_input::classify_runtime_character_demand;
use super::dialogue_font_allocation::assess_rewritten_atlas_capacity;
use super::translation_model::DialogueTranslationControl;

#[test]
fn runtime_repertoire_keeps_known_suffixes_and_reports_unknown_name_sources() {
    let controls = [
        control("relationship_name_san_or_chan"),
        control("current_month"),
        control("current_school_name"),
        control("line_break"),
    ];

    let repertoire = classify_runtime_character_demand(&controls);

    assert_eq!(repertoire.characters, "0123456789군씨짱".chars().collect());
    assert_eq!(
        repertoire.unresolved_sources,
        [
            "localized_player_name_buffers".to_string(),
            "localized_school_name_table".to_string(),
        ]
        .into_iter()
        .collect()
    );
}

#[test]
fn name_entry_capacity_is_measured_against_the_rewritten_atlas() {
    let capacity = assess_rewritten_atlas_capacity(1_001, 773, 228);

    assert_eq!(capacity.spare_slot_count, 228);
    assert!(capacity.candidate_positions_fit);
    assert_eq!(capacity.spare_slot_count_after_candidates, 0);
}

fn control(semantic_name: &str) -> DialogueTranslationControl {
    DialogueTranslationControl {
        semantic_name: semantic_name.to_string(),
        arguments: Vec::new(),
    }
}
