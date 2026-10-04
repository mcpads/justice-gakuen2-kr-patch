use super::{
    DIGIT_KEYS, HANGUL_CONSONANT_KEYS, HANGUL_VOWEL_KEYS, LATIN_KEYS, NameInputKey, NameInputPage,
    SYMBOL_KEYS, keyboard_keys,
};

#[test]
fn pages_cycle_through_hangul_latin_and_digits_with_symbols() {
    assert_eq!(NameInputPage::Hangul.next(), NameInputPage::Latin);
    assert_eq!(NameInputPage::Latin.next(), NameInputPage::DigitsAndSymbols);
    assert_eq!(
        NameInputPage::DigitsAndSymbols.next(),
        NameInputPage::Hangul
    );
}

#[test]
fn hangul_page_places_consonants_and_vowels_together() {
    let keys = keyboard_keys(NameInputPage::Hangul);

    assert_eq!(keys.len(), 40);
    assert_eq!(
        keys[..HANGUL_CONSONANT_KEYS.len()],
        HANGUL_CONSONANT_KEYS.map(NameInputKey::Consonant)
    );
    assert_eq!(
        keys[HANGUL_CONSONANT_KEYS.len()..],
        HANGUL_VOWEL_KEYS.map(NameInputKey::Vowel)
    );
}

#[test]
fn direct_input_pages_keep_letter_case_and_separate_symbols_from_digits() {
    assert_eq!(keyboard_keys(NameInputPage::Latin).len(), 52);
    assert_eq!(
        LATIN_KEYS,
        "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz"
    );

    let digit_and_symbol_keys = keyboard_keys(NameInputPage::DigitsAndSymbols);
    assert_eq!(
        digit_and_symbol_keys.len(),
        DIGIT_KEYS.chars().count() + SYMBOL_KEYS.chars().count()
    );
    assert_eq!(digit_and_symbol_keys[0], NameInputKey::Direct('0'));
    assert!(digit_and_symbol_keys.contains(&NameInputKey::Direct('@')));
}

#[test]
fn candidate_keys_fit_the_original_three_page_capacities() {
    let original_selectable_capacities = [84, 82, 62];
    for (page, capacity) in [
        NameInputPage::Hangul,
        NameInputPage::Latin,
        NameInputPage::DigitsAndSymbols,
    ]
    .into_iter()
    .zip(original_selectable_capacities)
    {
        assert!(keyboard_keys(page).len() <= capacity);
    }
}
