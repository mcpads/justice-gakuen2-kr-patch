use std::path::Path;

use super::{HANGUL_CONSONANT_KEYS, HANGUL_VOWEL_KEYS, load_name_input_keyboard};
use crate::name_input::spec::parse_name_input_keyboard_fixture;

#[test]
#[ignore = "requires assets/"]
fn tracked_keyboard_spec_binds_three_role_pages_to_source_capacities() {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/dialogue/name-entry/keyboard.json");

    let plan = load_name_input_keyboard(&path).unwrap();

    assert_eq!(
        plan.pages
            .iter()
            .map(|page| page.role.as_str())
            .collect::<Vec<_>>(),
        ["hangul", "latin", "digits_and_symbols"]
    );
    assert_eq!(
        plan.pages
            .iter()
            .map(|page| page.active_key_count)
            .collect::<Vec<_>>(),
        [
            HANGUL_CONSONANT_KEYS.len() + HANGUL_VOWEL_KEYS.len(),
            52,
            25
        ]
    );
    assert_eq!(
        plan.pages
            .iter()
            .map(|page| page.source_selectable_capacity)
            .collect::<Vec<_>>(),
        [84, 82, 62]
    );
}

#[test]
#[ignore = "requires assets/"]
fn duplicate_key_character_is_rejected_across_pages() {
    let document = crate::test_input::read_str("assets/dialogue/name-entry/keyboard.json")
        .replace("0123456789", "A123456789");

    let error = parse_name_input_keyboard_fixture(document.as_bytes()).unwrap_err();

    assert!(error.to_string().contains("'A' is duplicated"));
}

#[test]
#[ignore = "requires assets/"]
fn changed_hangul_key_behavior_is_rejected() {
    let document = crate::test_input::read_str("assets/dialogue/name-entry/keyboard.json").replace(
        r#""id": "vowels",
          "input": "vowel""#,
        r#""id": "vowels",
          "input": "direct""#,
    );

    let error = parse_name_input_keyboard_fixture(document.as_bytes()).unwrap_err();

    assert!(error.to_string().contains("vowel repertoire changed"));
}
