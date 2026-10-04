use anyhow::{Result, ensure};

use std::collections::BTreeSet;

use super::model::{
    NameField, NameGlyphCachePlan, NameGlyphCacheSlot, NameGlyphPackCell, NameGlyphPackStoragePlan,
    NameInputPagePlan,
};

pub const NAME_GLYPH_CODE_START: u16 = 0x0305;
pub const NAME_GLYPH_CODE_COUNT: usize = 228;
pub const NAME_GLYPH_CACHE_SLOT_COUNT: usize = 16;
pub const NAME_GLYPH_CELL_BYTES: usize = 200;
pub const NAME_GLYPH_PACK_CELL_COUNT: usize = 95;
pub const NAME_GLYPH_PACK_STORAGE_BYTES: usize = NAME_GLYPH_PACK_CELL_COUNT * NAME_GLYPH_CELL_BYTES;

const CODE_LOOKUP_PREFIX_ENTRY_COUNT: usize = 2;
const CACHE_SOURCE_PAGE_INDEX: usize = 0;

pub(super) fn plan_name_glyph_cache(pages: &[NameInputPagePlan]) -> Result<NameGlyphCachePlan> {
    ensure!(
        pages.len() == 3,
        "name glyph cache requires the three physical source pages"
    );
    ensure!(
        pages
            .iter()
            .map(|page| page.source_selectable_capacity)
            .sum::<usize>()
            == NAME_GLYPH_CODE_COUNT,
        "name glyph code domain no longer matches the source lookup table"
    );

    let page = &pages[CACHE_SOURCE_PAGE_INDEX];
    ensure!(
        page.role == "hangul",
        "name glyph cache must follow the Hangul key range"
    );
    ensure!(
        page.active_key_count == page.assignments.len(),
        "Hangul page active-key count differs from its assignments"
    );
    let first_sequence_position = page.active_key_count;
    ensure!(
        first_sequence_position + NAME_GLYPH_CACHE_SLOT_COUNT <= page.source_selectable_capacity,
        "Hangul page has too few inactive positions for the persistent name cache"
    );

    let mut slots = Vec::with_capacity(NAME_GLYPH_CACHE_SLOT_COUNT);
    for field in [
        NameField::FamilyName,
        NameField::GivenName,
        NameField::Nickname,
    ] {
        for field_slot in 0..field.visible_glyph_capacity() {
            let sequence_position = first_sequence_position + slots.len();
            slots.push(NameGlyphCacheSlot {
                field,
                field_slot,
                source_page: page.source_page.clone(),
                source_page_position: sequence_position,
                selectable_sequence_position: sequence_position,
                code_lookup_entry_index: CODE_LOOKUP_PREFIX_ENTRY_COUNT + sequence_position,
                atlas_layout_record_index: sequence_position,
                cache_code: NAME_GLYPH_CODE_START + u16::try_from(sequence_position)?,
            });
        }
    }
    ensure!(
        slots.len() == NAME_GLYPH_CACHE_SLOT_COUNT,
        "persistent name fields no longer total sixteen visible slots"
    );

    Ok(NameGlyphCachePlan {
        source_page: page.source_page.clone(),
        first_source_page_position: first_sequence_position,
        slot_count: slots.len(),
        slots,
    })
}

pub(super) fn plan_name_glyph_pack_storage(
    pages: &[NameInputPagePlan],
    cache: &NameGlyphCachePlan,
) -> Result<NameGlyphPackStoragePlan> {
    let cache_cells = cache
        .slots
        .iter()
        .map(|slot| (slot.source_page.as_str(), slot.source_page_position))
        .collect::<BTreeSet<_>>();
    let mut cells = Vec::new();
    let mut page_start = 0_usize;
    for page in pages {
        for source_page_position in page.active_key_count..page.source_selectable_capacity {
            if cache_cells.contains(&(page.source_page.as_str(), source_page_position)) {
                continue;
            }
            let selectable_sequence_position = page_start + source_page_position;
            cells.push(NameGlyphPackCell {
                source_page: page.source_page.clone(),
                source_page_position,
                selectable_sequence_position,
                atlas_layout_record_index: selectable_sequence_position,
            });
        }
        page_start += page.source_selectable_capacity;
    }
    ensure!(
        cells.len() == NAME_GLYPH_PACK_CELL_COUNT,
        "name glyph pack storage no longer owns {NAME_GLYPH_PACK_CELL_COUNT} inactive cells"
    );
    Ok(NameGlyphPackStoragePlan {
        cell_count: cells.len(),
        bytes_per_cell: NAME_GLYPH_CELL_BYTES,
        byte_capacity: cells.len() * NAME_GLYPH_CELL_BYTES,
        cells,
    })
}
