use super::model::{NameInputKey, NameInputPage};

pub const HANGUL_CONSONANT_KEYS: [char; 19] = [
    'ㄱ', 'ㄲ', 'ㄴ', 'ㄷ', 'ㄸ', 'ㄹ', 'ㅁ', 'ㅂ', 'ㅃ', 'ㅅ', 'ㅆ', 'ㅇ', 'ㅈ', 'ㅉ', 'ㅊ', 'ㅋ',
    'ㅌ', 'ㅍ', 'ㅎ',
];

pub const HANGUL_VOWEL_KEYS: [char; 21] = [
    'ㅏ', 'ㅐ', 'ㅑ', 'ㅒ', 'ㅓ', 'ㅔ', 'ㅕ', 'ㅖ', 'ㅗ', 'ㅘ', 'ㅙ', 'ㅚ', 'ㅛ', 'ㅜ', 'ㅝ', 'ㅞ',
    'ㅟ', 'ㅠ', 'ㅡ', 'ㅢ', 'ㅣ',
];

pub const LATIN_KEYS: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
pub const DIGIT_KEYS: &str = "0123456789";
pub const SYMBOL_KEYS: &str = " .-_!?@#%&+/:'\"";

pub fn keyboard_keys(page: NameInputPage) -> Vec<NameInputKey> {
    match page {
        NameInputPage::Hangul => HANGUL_CONSONANT_KEYS
            .into_iter()
            .map(NameInputKey::Consonant)
            .chain(HANGUL_VOWEL_KEYS.into_iter().map(NameInputKey::Vowel))
            .collect(),
        NameInputPage::Latin => LATIN_KEYS.chars().map(NameInputKey::Direct).collect(),
        NameInputPage::DigitsAndSymbols => DIGIT_KEYS
            .chars()
            .chain(SYMBOL_KEYS.chars())
            .map(NameInputKey::Direct)
            .collect(),
    }
}

pub(super) fn page_contains_key(page: NameInputPage, key: NameInputKey) -> bool {
    match (page, key) {
        (NameInputPage::Hangul, NameInputKey::Consonant(character)) => {
            HANGUL_CONSONANT_KEYS.contains(&character)
        }
        (NameInputPage::Hangul, NameInputKey::Vowel(character)) => {
            HANGUL_VOWEL_KEYS.contains(&character)
        }
        (NameInputPage::Latin, NameInputKey::Direct(character)) => LATIN_KEYS.contains(character),
        (NameInputPage::DigitsAndSymbols, NameInputKey::Direct(character)) => {
            DIGIT_KEYS.contains(character) || SYMBOL_KEYS.contains(character)
        }
        _ => false,
    }
}
