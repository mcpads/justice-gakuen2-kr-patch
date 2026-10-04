use std::collections::BTreeSet;

use super::dialogue_code_allocation_model::DialogueCharacterCodeAssignment;
use super::dialogue_fixed_code_consumers::{
    StoredDialogueCoordinate, classify_stored_coordinates,
    runtime_decimal_codes_remain_source_glyphs,
};

#[test]
fn coordinate_ownership_separates_rewrites_from_unreachable_primary_storage() {
    let coordinates = [
        coordinate(0, "primary", "translated-primary"),
        coordinate(1, "inactive", "untranslated"),
        coordinate(2, "selector", "translated-selector"),
        coordinate(2, "runtime", "runtime-insertion"),
    ];
    let counts = classify_stored_coordinates(
        &coordinates,
        &BTreeSet::from(["primary".to_string()]),
        &BTreeSet::from([
            "translated-primary".to_string(),
            "translated-selector".to_string(),
        ]),
        &BTreeSet::from(["runtime-insertion".to_string()]),
        false,
    )
    .unwrap();

    assert_eq!(counts.stored_coordinate_count, 4);
    assert_eq!(counts.primary_referenced_coordinate_rewrite_count, 1);
    assert_eq!(counts.selector_stored_coordinate_count, 2);
    assert_eq!(counts.selector_authored_coordinate_rewrite_count, 1);
    assert_eq!(counts.runtime_insertion_coordinate_rewrite_count, 1);
    assert_eq!(counts.preserved_primary_unreferenced_coordinate_count, 1);
    assert_eq!(counts.preserved_selector_coordinate_count, 0);
}

#[test]
fn execution_referenced_primary_storage_cannot_be_preserved() {
    let result = classify_stored_coordinates(
        &[coordinate(0, "primary", "untranslated")],
        &BTreeSet::from(["primary".to_string()]),
        &BTreeSet::new(),
        &BTreeSet::new(),
        false,
    );

    assert!(result.is_err());
}

#[test]
fn authored_subset_keeps_untranslated_primary_storage_visible() {
    let counts = classify_stored_coordinates(
        &[coordinate(0, "primary", "untranslated")],
        &BTreeSet::from(["primary".to_string()]),
        &BTreeSet::new(),
        &BTreeSet::new(),
        true,
    )
    .unwrap();

    assert_eq!(counts.preserved_primary_untranslated_coordinate_count, 1);
    assert_eq!(counts.preserved_primary_unreferenced_coordinate_count, 0);
}

#[test]
fn preserved_selector_storage_remains_visible_to_the_completion_gate() {
    let counts = classify_stored_coordinates(
        &[coordinate(6, "selector", "untranslated")],
        &BTreeSet::new(),
        &BTreeSet::new(),
        &BTreeSet::new(),
        false,
    )
    .unwrap();

    assert_eq!(counts.preserved_selector_coordinate_count, 1);
}

#[test]
fn decimal_renderer_codes_must_keep_their_source_glyphs() {
    let mut assignments = (0..=9)
        .map(|digit| {
            let character = char::from_digit(digit, 10).unwrap();
            let code = if digit == 0 { 0x000a } else { digit as u16 };
            assignment(character, code, true)
        })
        .collect::<Vec<_>>();
    assert!(runtime_decimal_codes_remain_source_glyphs(&assignments).unwrap());

    assignments[4].source_glyph_reused = false;
    assert!(!runtime_decimal_codes_remain_source_glyphs(&assignments).unwrap());
}

fn coordinate<'a>(
    selector_index: usize,
    coordinate_id: &'a str,
    semantic_source_sha256: &'a str,
) -> StoredDialogueCoordinate<'a> {
    StoredDialogueCoordinate {
        selector_index,
        coordinate_id,
        semantic_source_sha256,
    }
}

fn assignment(
    character: char,
    code: u16,
    source_glyph_reused: bool,
) -> DialogueCharacterCodeAssignment {
    DialogueCharacterCodeAssignment {
        character: character.to_string(),
        code: format!("0x{code:04x}"),
        source_glyph_reused,
        global_name_glyph_reused: false,
        requires_glyph_install: !source_glyph_reused,
        target_was_fixed_source_cell: true,
        target_source_glyph_protected: false,
        target_source_cell_sha256: None,
    }
}
