use anyhow::{Result, ensure};

use super::keyboard::{HANGUL_CONSONANT_KEYS, HANGUL_VOWEL_KEYS, page_contains_key};
use super::model::{HangulCompositionState, NameInputEditor, NameInputKey, NameInputPage};
use super::repertoire::is_name_input_hangul;

const FINAL_CONSONANTS: [char; 27] = [
    'ㄱ', 'ㄲ', 'ㄳ', 'ㄴ', 'ㄵ', 'ㄶ', 'ㄷ', 'ㄹ', 'ㄺ', 'ㄻ', 'ㄼ', 'ㄽ', 'ㄾ', 'ㄿ', 'ㅀ', 'ㅁ',
    'ㅂ', 'ㅄ', 'ㅅ', 'ㅆ', 'ㅇ', 'ㅈ', 'ㅊ', 'ㅋ', 'ㅌ', 'ㅍ', 'ㅎ',
];
const HANGUL_SYLLABLE_BASE: u32 = 0xac00;
const MEDIAL_COUNT: usize = 21;
const FINAL_COUNT_WITH_NONE: usize = 28;

impl NameInputEditor {
    pub fn select_key(&mut self, key: NameInputKey) -> Result<()> {
        ensure!(
            page_contains_key(self.page, key),
            "selected key is not present on the active name-input page"
        );
        let mut next = self.clone();
        match key {
            NameInputKey::Consonant(consonant) => next.select_consonant(consonant)?,
            NameInputKey::Vowel(vowel) => next.select_vowel(vowel)?,
            NameInputKey::Direct(character) => next.select_direct(character)?,
        }
        next.require_visible_capacity()?;
        *self = next;
        Ok(())
    }

    pub fn switch_page(&mut self) -> Result<NameInputPage> {
        let mut next = self.clone();
        if !matches!(next.composition, HangulCompositionState::Initial { .. }) {
            next.commit_complete_syllable()?;
        }
        next.page = next.page.next();
        *self = next;
        Ok(self.page)
    }

    pub fn backspace(&mut self) -> bool {
        match self.composition {
            HangulCompositionState::Empty => self.committed.pop().is_some(),
            HangulCompositionState::Initial { .. } => {
                self.composition = HangulCompositionState::Empty;
                true
            }
            HangulCompositionState::Syllable {
                initial,
                medial,
                final_consonant: Some(final_consonant),
            } => {
                self.composition = HangulCompositionState::Syllable {
                    initial,
                    medial,
                    final_consonant: split_compound_final(final_consonant).map(|(first, _)| first),
                };
                true
            }
            HangulCompositionState::Syllable {
                initial,
                medial,
                final_consonant: None,
            } => {
                self.composition = match super::medial::backspace(medial) {
                    Some(medial) => HangulCompositionState::Syllable {
                        initial,
                        medial,
                        final_consonant: None,
                    },
                    None => HangulCompositionState::Initial { consonant: initial },
                };
                true
            }
        }
    }

    pub fn visible_text(&self) -> String {
        let mut characters = self.committed.clone();
        if let HangulCompositionState::Initial { consonant } = self.composition {
            characters.push(consonant);
        } else if let Some(character) = active_syllable(self.composition) {
            characters.push(character);
        }
        characters.into_iter().collect()
    }

    pub fn finish(&self) -> Result<String> {
        let mut completed = self.clone();
        completed.commit_complete_syllable()?;
        ensure!(
            !completed.committed.is_empty(),
            "name input cannot finish with an empty name"
        );
        Ok(completed.committed.into_iter().collect())
    }

    fn select_consonant(&mut self, consonant: char) -> Result<()> {
        ensure!(
            HANGUL_CONSONANT_KEYS.contains(&consonant),
            "unknown Hangul consonant key"
        );
        match self.composition {
            HangulCompositionState::Empty => {
                self.composition = HangulCompositionState::Initial { consonant };
            }
            HangulCompositionState::Initial { .. } => {
                self.composition = HangulCompositionState::Initial { consonant };
            }
            HangulCompositionState::Syllable {
                initial,
                medial,
                final_consonant: None,
            } => {
                if FINAL_CONSONANTS.contains(&consonant)
                    && compose_syllable(initial, medial, Some(consonant))
                        .is_some_and(is_name_input_hangul)
                {
                    self.composition = HangulCompositionState::Syllable {
                        initial,
                        medial,
                        final_consonant: Some(consonant),
                    };
                } else {
                    self.commit_complete_syllable()?;
                    self.composition = HangulCompositionState::Initial { consonant };
                }
            }
            HangulCompositionState::Syllable {
                initial,
                medial,
                final_consonant: Some(final_consonant),
            } => {
                if let Some(compound) =
                    combine_final(final_consonant, consonant).filter(|&final_consonant| {
                        compose_syllable(initial, medial, Some(final_consonant))
                            .is_some_and(is_name_input_hangul)
                    })
                {
                    self.composition = HangulCompositionState::Syllable {
                        initial,
                        medial,
                        final_consonant: Some(compound),
                    };
                } else {
                    self.commit_complete_syllable()?;
                    self.composition = HangulCompositionState::Initial { consonant };
                }
            }
        }
        Ok(())
    }

    fn select_vowel(&mut self, vowel: char) -> Result<()> {
        ensure!(
            HANGUL_VOWEL_KEYS.contains(&vowel),
            "unknown Hangul vowel key"
        );
        match self.composition {
            HangulCompositionState::Empty => {
                anyhow::bail!("an initial consonant is required before a vowel")
            }
            HangulCompositionState::Initial { consonant } => {
                self.composition = HangulCompositionState::Syllable {
                    initial: consonant,
                    medial: vowel,
                    final_consonant: None,
                };
            }
            HangulCompositionState::Syllable {
                initial,
                medial,
                final_consonant: None,
            } => {
                let next = super::medial::select(medial, vowel);
                ensure!(
                    compose_syllable(initial, next, None).is_some_and(is_name_input_hangul),
                    "selected Hangul syllable is outside the supported name repertoire"
                );
                self.composition = HangulCompositionState::Syllable {
                    initial,
                    medial: next,
                    final_consonant: None,
                };
            }
            HangulCompositionState::Syllable {
                initial,
                medial,
                final_consonant: Some(final_consonant),
            } => {
                let (retained_final, moved_initial) = split_final(final_consonant);
                let completed = compose_syllable(initial, medial, retained_final)
                    .expect("validated composition state must form a Hangul syllable");
                ensure!(
                    is_name_input_hangul(completed),
                    "completed Hangul syllable is outside the supported name repertoire"
                );
                self.committed.push(completed);
                self.composition = HangulCompositionState::Syllable {
                    initial: moved_initial,
                    medial: vowel,
                    final_consonant: None,
                };
            }
        }
        Ok(())
    }

    fn select_direct(&mut self, character: char) -> Result<()> {
        ensure!(
            matches!(
                self.page,
                NameInputPage::Latin | NameInputPage::DigitsAndSymbols
            ),
            "direct name input is only valid on the Latin or digit-and-symbol page"
        );
        self.commit_complete_syllable()?;
        self.committed.push(character);
        Ok(())
    }

    fn commit_complete_syllable(&mut self) -> Result<()> {
        match self.composition {
            HangulCompositionState::Empty => Ok(()),
            HangulCompositionState::Initial { .. } => {
                anyhow::bail!("name input contains an incomplete Hangul syllable")
            }
            composition @ HangulCompositionState::Syllable { .. } => {
                let completed = active_syllable(composition)
                    .expect("complete Hangul composition must form one syllable");
                ensure!(
                    is_name_input_hangul(completed),
                    "completed Hangul syllable is outside the supported name repertoire"
                );
                self.committed.push(completed);
                self.composition = HangulCompositionState::Empty;
                Ok(())
            }
        }
    }

    fn require_visible_capacity(&self) -> Result<()> {
        let visible_count = self.committed.len()
            + usize::from(!matches!(self.composition, HangulCompositionState::Empty));
        ensure!(
            visible_count <= self.field.visible_glyph_capacity(),
            "name input exceeds the field's visible glyph capacity"
        );
        Ok(())
    }
}

fn active_syllable(composition: HangulCompositionState) -> Option<char> {
    let HangulCompositionState::Syllable {
        initial,
        medial,
        final_consonant,
    } = composition
    else {
        return None;
    };
    compose_syllable(initial, medial, final_consonant)
}

fn compose_syllable(initial: char, medial: char, final_consonant: Option<char>) -> Option<char> {
    let initial_index = HANGUL_CONSONANT_KEYS
        .iter()
        .position(|candidate| *candidate == initial)?;
    let medial_index = HANGUL_VOWEL_KEYS
        .iter()
        .position(|candidate| *candidate == medial)?;
    let final_index = final_consonant
        .map(|final_consonant| {
            FINAL_CONSONANTS
                .iter()
                .position(|candidate| *candidate == final_consonant)
                .map(|index| index + 1)
        })
        .unwrap_or(Some(0))?;
    let syllable_index = initial_index * MEDIAL_COUNT * FINAL_COUNT_WITH_NONE
        + medial_index * FINAL_COUNT_WITH_NONE
        + final_index;
    char::from_u32(HANGUL_SYLLABLE_BASE + u32::try_from(syllable_index).ok()?)
}

fn combine_final(first: char, second: char) -> Option<char> {
    match (first, second) {
        ('ㄱ', 'ㅅ') => Some('ㄳ'),
        ('ㄴ', 'ㅈ') => Some('ㄵ'),
        ('ㄴ', 'ㅎ') => Some('ㄶ'),
        ('ㄹ', 'ㄱ') => Some('ㄺ'),
        ('ㄹ', 'ㅁ') => Some('ㄻ'),
        ('ㄹ', 'ㅂ') => Some('ㄼ'),
        ('ㄹ', 'ㅅ') => Some('ㄽ'),
        ('ㄹ', 'ㅌ') => Some('ㄾ'),
        ('ㄹ', 'ㅍ') => Some('ㄿ'),
        ('ㄹ', 'ㅎ') => Some('ㅀ'),
        ('ㅂ', 'ㅅ') => Some('ㅄ'),
        _ => None,
    }
}

fn split_compound_final(final_consonant: char) -> Option<(char, char)> {
    match final_consonant {
        'ㄳ' => Some(('ㄱ', 'ㅅ')),
        'ㄵ' => Some(('ㄴ', 'ㅈ')),
        'ㄶ' => Some(('ㄴ', 'ㅎ')),
        'ㄺ' => Some(('ㄹ', 'ㄱ')),
        'ㄻ' => Some(('ㄹ', 'ㅁ')),
        'ㄼ' => Some(('ㄹ', 'ㅂ')),
        'ㄽ' => Some(('ㄹ', 'ㅅ')),
        'ㄾ' => Some(('ㄹ', 'ㅌ')),
        'ㄿ' => Some(('ㄹ', 'ㅍ')),
        'ㅀ' => Some(('ㄹ', 'ㅎ')),
        'ㅄ' => Some(('ㅂ', 'ㅅ')),
        _ => None,
    }
}

fn split_final(final_consonant: char) -> (Option<char>, char) {
    split_compound_final(final_consonant)
        .map(|(first, second)| (Some(first), second))
        .unwrap_or((None, final_consonant))
}
