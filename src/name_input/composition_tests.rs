use super::{HangulCompositionState, NameField, NameInputEditor, NameInputKey, NameInputPage};

fn select(editor: &mut NameInputEditor, characters: &str) {
    for character in characters.chars() {
        let key = if ('ㄱ'..='ㅎ').contains(&character) {
            NameInputKey::Consonant(character)
        } else {
            NameInputKey::Vowel(character)
        };
        editor.select_key(key).unwrap();
    }
}

#[test]
fn unsupported_final_starts_the_next_initial_without_losing_the_key() {
    let mut editor = NameInputEditor::new(NameField::GivenName);
    select(&mut editor, "ㄱㅏㄲ");

    assert_eq!(editor.visible_text(), "가ㄲ");
    assert!(editor.finish().is_err());
    assert_eq!(editor.committed_characters(), &['가']);
    select(&mut editor, "ㅏ");
    assert_eq!(editor.finish().unwrap(), "가까");
}

#[test]
fn consonants_and_vowels_compose_a_name_without_stage_pages() {
    let mut editor = NameInputEditor::new(NameField::GivenName);

    select(&mut editor, "ㅎㅏㄴㄱㅡㄹ");

    assert_eq!(editor.visible_text(), "한글");
    assert_eq!(editor.finish().unwrap(), "한글");
}

#[test]
fn vowel_after_a_final_moves_that_consonant_to_the_next_syllable() {
    let mut editor = NameInputEditor::new(NameField::GivenName);

    select(&mut editor, "ㄱㅏㄴㅏ");

    assert_eq!(editor.visible_text(), "가나");
}

#[test]
fn compound_final_splits_when_the_next_vowel_is_selected() {
    let mut editor = NameInputEditor::new(NameField::GivenName);

    select(&mut editor, "ㄷㅏㄹㄱㅏ");

    assert_eq!(editor.visible_text(), "달가");
}

#[test]
fn backspace_reverses_compound_final_then_final_then_medial_then_initial() {
    let mut editor = NameInputEditor::new(NameField::GivenName);
    select(&mut editor, "ㄷㅏㄹㄱ");

    assert!(editor.backspace());
    assert_eq!(editor.visible_text(), "달");
    assert!(editor.backspace());
    assert_eq!(editor.visible_text(), "다");
    assert!(editor.backspace());
    assert_eq!(
        editor.composition(),
        HangulCompositionState::Initial { consonant: 'ㄷ' }
    );
    assert!(editor.backspace());
    assert_eq!(editor.composition(), HangulCompositionState::Empty);
}

#[test]
fn page_switch_commits_a_complete_syllable_and_direct_pages_append_characters() {
    let mut editor = NameInputEditor::new(NameField::GivenName);
    select(&mut editor, "ㅎㅏㄴ");

    assert_eq!(editor.switch_page().unwrap(), NameInputPage::Latin);
    editor.select_key(NameInputKey::Direct('A')).unwrap();
    assert_eq!(
        editor.switch_page().unwrap(),
        NameInputPage::DigitsAndSymbols
    );
    editor.select_key(NameInputKey::Direct('2')).unwrap();
    editor.select_key(NameInputKey::Direct('@')).unwrap();

    assert_eq!(editor.finish().unwrap(), "한A2@");
}

#[test]
fn incomplete_syllables_and_wrong_page_keys_fail_without_mutation() {
    let mut editor = NameInputEditor::new(NameField::GivenName);
    editor.select_key(NameInputKey::Consonant('ㄱ')).unwrap();
    let before = editor.clone();

    assert!(editor.select_key(NameInputKey::Direct('A')).is_err());
    assert_eq!(editor, before);
    assert_eq!(editor.switch_page().unwrap(), NameInputPage::Latin);
    let before = editor.clone();
    assert!(editor.select_key(NameInputKey::Direct('A')).is_err());
    assert_eq!(editor, before);
    assert!(editor.finish().is_err());
}

#[test]
fn nickname_capacity_rejects_a_fifth_visible_character_transactionally() {
    let mut editor = NameInputEditor::new(NameField::Nickname);
    editor.switch_page().unwrap();
    for character in "AbCd".chars() {
        editor.select_key(NameInputKey::Direct(character)).unwrap();
    }
    let before = editor.clone();

    assert!(editor.select_key(NameInputKey::Direct('E')).is_err());
    assert_eq!(editor, before);
    assert_eq!(editor.finish().unwrap(), "AbCd");
}

#[test]
fn medial_selection_combines_sequences_and_accepts_explicit_replacement() {
    for (keys, expected) in [
        ("ㄱㅗㅏ", "과"),
        ("ㄱㅗㅏㅣ", "괘"),
        ("ㄱㅜㅓㅣ", "궤"),
        ("ㄱㅗㅘ", "과"),
        ("ㄱㅏㅓ", "거"),
    ] {
        let mut editor = NameInputEditor::new(NameField::Nickname);
        select(&mut editor, keys);
        assert_eq!(editor.visible_text(), expected);
        assert_eq!(editor.finish().unwrap(), expected);
    }
}

#[test]
fn backspace_reverses_each_compound_medial_before_removing_the_initial() {
    for (input, stages) in [
        ("ㄱㅗㅏㅣ", vec!["과", "고", "ㄱ", ""]),
        ("ㄱㅜㅓㅣ", vec!["궈", "구", "ㄱ", ""]),
        ("ㄱㅚ", vec!["고", "ㄱ", ""]),
        ("ㄱㅟ", vec!["구", "ㄱ", ""]),
        ("ㄱㅢ", vec!["그", "ㄱ", ""]),
    ] {
        let mut editor = NameInputEditor::new(NameField::GivenName);
        select(&mut editor, input);
        for expected in stages {
            assert!(editor.backspace());
            assert_eq!(editor.visible_text(), expected, "{input}");
        }
        assert!(!editor.backspace());
        assert_eq!(editor.visible_text(), "");
    }
}

#[test]
fn an_unfinished_initial_survives_pages_and_can_be_replaced_then_completed() {
    let mut editor = NameInputEditor::new(NameField::Nickname);
    select(&mut editor, "ㄱㄴ");
    assert_eq!(editor.visible_text(), "ㄴ");
    editor.switch_page().unwrap();
    assert!(editor.select_key(NameInputKey::Direct('A')).is_err());
    editor.switch_page().unwrap();
    assert!(editor.select_key(NameInputKey::Direct('0')).is_err());
    editor.switch_page().unwrap();
    assert_eq!(editor.visible_text(), "ㄴ");
    select(&mut editor, "ㅏ");
    assert_eq!(editor.finish().unwrap(), "나");
}
