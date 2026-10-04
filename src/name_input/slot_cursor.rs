use anyhow::{Result, ensure};

use super::{EMPTY_NAME_SLOT, NameField, NameSlotCode};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NameRecordSlotSelection {
    AdvancedToSlot(usize),
    RedirectedToConfirm,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NameRecordSlotCursor {
    field: NameField,
    slot: usize,
    words: Vec<u16>,
}

impl NameRecordSlotCursor {
    pub fn empty(field: NameField) -> Self {
        Self {
            field,
            slot: 0,
            words: vec![EMPTY_NAME_SLOT; field.visible_glyph_capacity()],
        }
    }

    pub fn from_words(field: NameField, slot: usize, words: &[u16]) -> Result<Self> {
        ensure!(
            words.len() == field.visible_glyph_capacity(),
            "name record cursor has the wrong slot count for its field"
        );
        ensure!(
            slot < words.len(),
            "name record cursor points outside its visible field"
        );
        for word in words {
            NameSlotCode::decode(*word)?;
        }
        Ok(Self {
            field,
            slot,
            words: words.to_vec(),
        })
    }

    pub fn select(&mut self, code: NameSlotCode) -> Result<NameRecordSlotSelection> {
        ensure!(
            code != NameSlotCode::Empty,
            "name record selection cannot write the empty marker"
        );
        self.words[self.slot] = code.encode()?;
        if self.slot + 1 < self.words.len() {
            self.slot += 1;
            Ok(NameRecordSlotSelection::AdvancedToSlot(self.slot))
        } else {
            Ok(NameRecordSlotSelection::RedirectedToConfirm)
        }
    }

    pub fn delete(&mut self) -> usize {
        self.words[self.slot] = EMPTY_NAME_SLOT;
        self.slot = self.slot.saturating_sub(1);
        self.slot
    }

    pub fn advance(&mut self) -> usize {
        if self.slot + 1 < self.words.len() {
            self.slot += 1;
        }
        self.slot
    }

    pub fn field(&self) -> NameField {
        self.field
    }

    pub fn slot(&self) -> usize {
        self.slot
    }

    pub fn words(&self) -> &[u16] {
        &self.words
    }
}
