#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NameInputPage {
    Hangul,
    Latin,
    DigitsAndSymbols,
}

impl NameInputPage {
    pub fn next(self) -> Self {
        match self {
            Self::Hangul => Self::Latin,
            Self::Latin => Self::DigitsAndSymbols,
            Self::DigitsAndSymbols => Self::Hangul,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NameInputKey {
    Consonant(char),
    Vowel(char),
    Direct(char),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NameField {
    FamilyName,
    GivenName,
    Nickname,
}

impl NameField {
    pub fn visible_glyph_capacity(self) -> usize {
        match self {
            Self::FamilyName | Self::GivenName => 6,
            Self::Nickname => 4,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HangulCompositionState {
    Empty,
    Initial {
        consonant: char,
    },
    Syllable {
        initial: char,
        medial: char,
        final_consonant: Option<char>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NameInputEditor {
    pub(super) field: NameField,
    pub(super) page: NameInputPage,
    pub(super) committed: Vec<char>,
    pub(super) composition: HangulCompositionState,
}

impl NameInputEditor {
    pub fn new(field: NameField) -> Self {
        Self {
            field,
            page: NameInputPage::Hangul,
            committed: Vec::with_capacity(field.visible_glyph_capacity()),
            composition: HangulCompositionState::Empty,
        }
    }

    pub fn field(&self) -> NameField {
        self.field
    }

    pub fn page(&self) -> NameInputPage {
        self.page
    }

    pub fn composition(&self) -> HangulCompositionState {
        self.composition
    }

    pub fn committed_characters(&self) -> &[char] {
        &self.committed
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct NameInputKeyboardPlan {
    pub kind: String,
    pub source_overlay_sha256: String,
    pub spec_path: String,
    pub spec_sha256: String,
    pub pages: Vec<NameInputPagePlan>,
    pub cache: NameGlyphCachePlan,
    pub glyph_pack_storage: NameGlyphPackStoragePlan,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct NameGlyphPackStoragePlan {
    pub cell_count: usize,
    pub bytes_per_cell: usize,
    pub byte_capacity: usize,
    pub cells: Vec<NameGlyphPackCell>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct NameGlyphPackCell {
    pub source_page: String,
    pub source_page_position: usize,
    pub selectable_sequence_position: usize,
    pub atlas_layout_record_index: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct NameGlyphCachePlan {
    pub source_page: String,
    pub first_source_page_position: usize,
    pub slot_count: usize,
    pub slots: Vec<NameGlyphCacheSlot>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, serde::Deserialize)]
pub struct NameGlyphCacheSlot {
    pub field: NameField,
    pub field_slot: usize,
    pub source_page: String,
    pub source_page_position: usize,
    pub selectable_sequence_position: usize,
    pub code_lookup_entry_index: usize,
    pub atlas_layout_record_index: usize,
    pub cache_code: u16,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct NameInputPagePlan {
    pub source_page: String,
    pub role: String,
    pub label: String,
    pub source_selectable_capacity: usize,
    pub active_key_count: usize,
    pub inactive_position_count: usize,
    pub assignments: Vec<NameInputKeyAssignment>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct NameInputKeyAssignment {
    pub page_position: usize,
    pub group: String,
    pub input: String,
    pub character: String,
}
use serde::Serialize;
