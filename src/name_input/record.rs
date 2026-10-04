use anyhow::{Result, ensure};

use super::keyboard::{DIGIT_KEYS, LATIN_KEYS, SYMBOL_KEYS};
use super::model::NameField;
use super::repertoire::is_name_input_hangul;

pub const EMPTY_NAME_SLOT: u16 = 0x0fff;
pub const ASCII_NAME_TAG: u16 = 0x4000;
pub const HANGUL_NAME_TAG: u16 = 0x8000;
pub const NICKNAME_COMPANION_TO_NICKNAME_BYTE_DISPLACEMENT: i16 = 0x0010;

const ASCII_PAYLOAD_MASK: u16 = 0x007f;
const HANGUL_SYLLABLE_START: u32 = 0xac00;
const HANGUL_SYLLABLE_END: u32 = 0xd7a3;
const HANGUL_SYLLABLE_COUNT: u16 = 11_172;
const LEGACY_GLYPH_CODE_END: u16 = 0x0ffe;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NameSlotCode {
    Hangul(char),
    Ascii(char),
    LegacyGlyph(u16),
    Empty,
}

impl NameSlotCode {
    pub fn encode(self) -> Result<u16> {
        match self {
            Self::Hangul(character) => encode_hangul(character),
            Self::Ascii(character) => encode_ascii(character),
            Self::LegacyGlyph(code) => {
                ensure!(
                    code <= LEGACY_GLYPH_CODE_END,
                    "legacy name glyph code is outside the original low-code domain"
                );
                Ok(code)
            }
            Self::Empty => Ok(EMPTY_NAME_SLOT),
        }
    }

    pub fn decode(word: u16) -> Result<Self> {
        if word == EMPTY_NAME_SLOT {
            return Ok(Self::Empty);
        }
        if word & 0xc000 == HANGUL_NAME_TAG {
            let index = word & 0x3fff;
            ensure!(
                index < HANGUL_SYLLABLE_COUNT,
                "tagged Hangul name slot is outside the modern syllable range"
            );
            let character = char::from_u32(HANGUL_SYLLABLE_START + u32::from(index))
                .expect("validated modern Hangul index is a Unicode scalar");
            ensure!(
                is_name_input_hangul(character),
                "tagged Hangul name slot is outside the supported name repertoire"
            );
            return Ok(Self::Hangul(character));
        }
        if word & 0xff80 == ASCII_NAME_TAG {
            let character = char::from_u32(u32::from(word & ASCII_PAYLOAD_MASK))
                .expect("seven-bit ASCII payload is a Unicode scalar");
            ensure!(
                is_name_ascii(character),
                "tagged ASCII name slot is not admitted by the keyboard"
            );
            return Ok(Self::Ascii(character));
        }
        ensure!(
            word <= LEGACY_GLYPH_CODE_END,
            "name slot has an unknown tagged representation"
        );
        Ok(Self::LegacyGlyph(word))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalNameRecord {
    field: NameField,
    slots: Vec<NameSlotCode>,
}

impl CanonicalNameRecord {
    pub fn from_text(field: NameField, text: &str) -> Result<Self> {
        let slots = text
            .chars()
            .map(|character| {
                if (char::from_u32(HANGUL_SYLLABLE_START).unwrap()
                    ..=char::from_u32(HANGUL_SYLLABLE_END).unwrap())
                    .contains(&character)
                {
                    Ok(NameSlotCode::Hangul(character))
                } else if is_name_ascii(character) {
                    Ok(NameSlotCode::Ascii(character))
                } else {
                    anyhow::bail!("name contains a character absent from the admitted keyboard")
                }
            })
            .collect::<Result<Vec<_>>>()?;
        Self::from_slots(field, slots)
    }

    pub fn from_words(field: NameField, words: &[u16]) -> Result<Self> {
        ensure!(
            words.len() == field.visible_glyph_capacity(),
            "canonical name record has the wrong slot count for its field"
        );
        let decoded = words
            .iter()
            .copied()
            .map(NameSlotCode::decode)
            .collect::<Result<Vec<_>>>()?;
        let occupied = decoded
            .iter()
            .position(|slot| *slot == NameSlotCode::Empty)
            .unwrap_or(decoded.len());
        ensure!(
            decoded[occupied..]
                .iter()
                .all(|slot| *slot == NameSlotCode::Empty),
            "canonical name record contains data after an empty slot"
        );
        Self::from_slots(field, decoded[..occupied].to_vec())
    }

    pub fn to_words(&self) -> Result<Vec<u16>> {
        let mut words = vec![EMPTY_NAME_SLOT; self.field.visible_glyph_capacity()];
        for (word, slot) in words.iter_mut().zip(&self.slots) {
            *word = slot.encode()?;
        }
        Ok(words)
    }

    pub fn field(&self) -> NameField {
        self.field
    }

    pub fn slots(&self) -> &[NameSlotCode] {
        &self.slots
    }

    fn from_slots(field: NameField, slots: Vec<NameSlotCode>) -> Result<Self> {
        ensure!(!slots.is_empty(), "canonical name record cannot be empty");
        ensure!(
            slots.len() <= field.visible_glyph_capacity(),
            "canonical name record exceeds its field capacity"
        );
        ensure!(
            slots.iter().all(|slot| *slot != NameSlotCode::Empty),
            "occupied canonical name slots cannot contain the empty marker"
        );
        for slot in &slots {
            slot.encode()?;
        }
        Ok(Self { field, slots })
    }
}

fn encode_hangul(character: char) -> Result<u16> {
    let scalar = u32::from(character);
    ensure!(
        (HANGUL_SYLLABLE_START..=HANGUL_SYLLABLE_END).contains(&scalar),
        "tagged Hangul name character is not a modern precomposed syllable"
    );
    ensure!(
        is_name_input_hangul(character),
        "tagged Hangul name character is outside the supported name repertoire"
    );
    Ok(HANGUL_NAME_TAG | u16::try_from(scalar - HANGUL_SYLLABLE_START)?)
}

fn encode_ascii(character: char) -> Result<u16> {
    ensure!(
        is_name_ascii(character),
        "ASCII name character is absent from the admitted keyboard"
    );
    Ok(ASCII_NAME_TAG | u16::try_from(u32::from(character))?)
}

fn is_name_ascii(character: char) -> bool {
    LATIN_KEYS.contains(character)
        || DIGIT_KEYS.contains(character)
        || SYMBOL_KEYS.contains(character)
}
