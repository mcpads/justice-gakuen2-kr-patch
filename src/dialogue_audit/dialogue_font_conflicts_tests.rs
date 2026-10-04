use std::collections::BTreeMap;

use super::dialogue_font_conflict_attribution::{
    ConflictGlyphOccurrence, attribute_conflict_occurrences,
};

#[test]
fn conflict_attribution_requires_every_preserved_semantic_owner() {
    let targets = BTreeMap::from([(7, "한".to_string()), (9, "글".to_string())]);
    let occurrences = vec![
        occurrence("group-a", "asset#0", 7, "日"),
        occurrence("group-a", "asset#1", 7, "日"),
        occurrence("group-b", "asset#2", 7, "日"),
        occurrence("group-a", "asset#0", 9, "本"),
    ];

    let conflicts = attribute_conflict_occurrences(&targets, &occurrences).unwrap();

    assert_eq!(conflicts.len(), 2);
    assert_eq!(
        conflicts[0].required_semantic_hashes,
        vec!["group-a".to_string(), "group-b".to_string()]
    );
    assert_eq!(conflicts[0].preserved_coordinate_count, 3);
    assert_eq!(conflicts[0].preserved_glyph_occurrence_count, 3);
    assert_eq!(conflicts[0].required_semantic_hashes.len(), 2);
    assert_eq!(conflicts[1].required_semantic_hashes, vec!["group-a"]);
}

#[test]
fn every_conflicting_code_must_have_a_preserved_owner() {
    let targets = BTreeMap::from([(7, "한".to_string())]);

    assert!(attribute_conflict_occurrences(&targets, &[]).is_err());
}

fn occurrence(
    semantic_source_sha256: &str,
    coordinate_id: &str,
    code: u16,
    source_character: &str,
) -> ConflictGlyphOccurrence {
    ConflictGlyphOccurrence {
        semantic_source_sha256: semantic_source_sha256.to_string(),
        coordinate_id: coordinate_id.to_string(),
        code,
        source_character: source_character.to_string(),
    }
}
