use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};
use serde::Serialize;

use super::{
    NAME_GLYPH_CODE_COUNT, NAME_GLYPH_CODE_START, NameField, NameGlyphCacheSlot,
    NameInputKeyboardPlan,
};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, serde::Deserialize)]
pub struct NameGlyphConsumerLayout {
    pub code_start: u16,
    pub code_count: usize,
    pub active_glyphs: Vec<NameGlyphActiveCell>,
    pub cache_slots: Vec<NameGlyphCacheSlot>,
    pub pack_cells: Vec<NameGlyphPackCodeCell>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, serde::Deserialize)]
pub struct NameGlyphActiveCell {
    pub source_page: String,
    pub source_page_position: usize,
    pub selectable_sequence_position: usize,
    pub character: String,
    pub code: u16,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, serde::Deserialize)]
pub struct NameGlyphPackCodeCell {
    pub pack_cell_index: usize,
    pub source_page: String,
    pub source_page_position: usize,
    pub selectable_sequence_position: usize,
    pub code: u16,
}

impl NameGlyphConsumerLayout {
    pub fn cache_codes_for_field(&self, field: NameField) -> Result<Vec<u16>> {
        let codes = self
            .cache_slots
            .iter()
            .filter(|slot| slot.field == field)
            .map(|slot| slot.cache_code)
            .collect::<Vec<_>>();
        ensure!(
            codes.len() == field.visible_glyph_capacity(),
            "name glyph consumer layout does not cover every {field:?} slot"
        );
        ensure!(
            codes.windows(2).all(|pair| pair[1] == pair[0] + 1),
            "name glyph consumer cache codes are not consecutive within {field:?}"
        );
        Ok(codes)
    }

    pub fn code_for_active_character(&self, character: char) -> Option<u16> {
        self.active_glyphs.iter().find_map(|cell| {
            let mut characters = cell.character.chars();
            (characters.next() == Some(character) && characters.next().is_none())
                .then_some(cell.code)
        })
    }
}

pub fn plan_name_glyph_consumer_layout(
    keyboard: &NameInputKeyboardPlan,
) -> Result<NameGlyphConsumerLayout> {
    let mut active_glyphs = Vec::new();
    let mut page_start = 0_usize;
    for page in &keyboard.pages {
        for assignment in &page.assignments {
            ensure!(
                assignment.page_position < page.active_key_count,
                "active name key lies outside its page population"
            );
            let selectable_sequence_position = page_start
                .checked_add(assignment.page_position)
                .context("active name-key sequence position overflow")?;
            active_glyphs.push(NameGlyphActiveCell {
                source_page: page.source_page.clone(),
                source_page_position: assignment.page_position,
                selectable_sequence_position,
                character: assignment.character.clone(),
                code: code_for_sequence_position(selectable_sequence_position)?,
            });
        }
        page_start = page_start
            .checked_add(page.source_selectable_capacity)
            .context("name glyph page sequence overflow")?;
    }
    ensure!(
        page_start == NAME_GLYPH_CODE_COUNT,
        "name glyph consumer pages no longer span the shared code domain"
    );

    let pack_cells = keyboard
        .glyph_pack_storage
        .cells
        .iter()
        .enumerate()
        .map(|(pack_cell_index, cell)| {
            Ok(NameGlyphPackCodeCell {
                pack_cell_index,
                source_page: cell.source_page.clone(),
                source_page_position: cell.source_page_position,
                selectable_sequence_position: cell.selectable_sequence_position,
                code: code_for_sequence_position(cell.selectable_sequence_position)?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let cache_slots = keyboard.cache.slots.clone();

    let active_codes = active_glyphs
        .iter()
        .map(|cell| cell.code)
        .collect::<BTreeSet<_>>();
    let cache_codes = cache_slots
        .iter()
        .map(|slot| slot.cache_code)
        .collect::<BTreeSet<_>>();
    let pack_codes = pack_cells
        .iter()
        .map(|cell| cell.code)
        .collect::<BTreeSet<_>>();
    ensure!(
        active_codes.len() == active_glyphs.len()
            && cache_codes.len() == cache_slots.len()
            && pack_codes.len() == pack_cells.len(),
        "name glyph consumer layout assigns one code more than once within a role"
    );
    ensure!(
        active_codes.is_disjoint(&cache_codes)
            && active_codes.is_disjoint(&pack_codes)
            && cache_codes.is_disjoint(&pack_codes),
        "name glyph consumer code ownership overlaps"
    );
    ensure!(
        active_codes.len() + cache_codes.len() + pack_codes.len() == NAME_GLYPH_CODE_COUNT,
        "name glyph consumer roles do not partition the shared code domain"
    );

    let layout = NameGlyphConsumerLayout {
        code_start: NAME_GLYPH_CODE_START,
        code_count: NAME_GLYPH_CODE_COUNT,
        active_glyphs,
        cache_slots,
        pack_cells,
    };
    for field in [
        NameField::FamilyName,
        NameField::GivenName,
        NameField::Nickname,
    ] {
        layout.cache_codes_for_field(field)?;
    }
    Ok(layout)
}

fn code_for_sequence_position(sequence_position: usize) -> Result<u16> {
    ensure!(
        sequence_position < NAME_GLYPH_CODE_COUNT,
        "name glyph sequence position is outside the shared code domain"
    );
    NAME_GLYPH_CODE_START
        .checked_add(u16::try_from(sequence_position)?)
        .context("name glyph code overflow")
}
