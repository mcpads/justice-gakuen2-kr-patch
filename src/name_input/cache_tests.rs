use std::collections::BTreeSet;
use std::path::Path;

use super::{
    NAME_GLYPH_CACHE_SLOT_COUNT, NAME_GLYPH_PACK_CELL_COUNT, NAME_GLYPH_PACK_STORAGE_BYTES,
    NameField, load_name_input_keyboard,
};

#[test]
#[ignore = "requires assets/"]
fn tracked_keyboard_reserves_one_unique_cache_cell_for_every_visible_name_slot() {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/dialogue/name-entry/keyboard.json");

    let plan = load_name_input_keyboard(&path).unwrap();
    let cache = plan.cache;

    assert_eq!(cache.source_page, "hiragana");
    assert_eq!(cache.first_source_page_position, 40);
    assert_eq!(cache.slot_count, NAME_GLYPH_CACHE_SLOT_COUNT);
    assert_eq!(cache.slots.first().unwrap().cache_code, 0x032d);
    assert_eq!(cache.slots.last().unwrap().cache_code, 0x033c);
    assert_eq!(cache.slots.first().unwrap().code_lookup_entry_index, 42);
    assert_eq!(cache.slots.last().unwrap().code_lookup_entry_index, 57);
    assert_eq!(
        cache
            .slots
            .iter()
            .map(|slot| slot.field)
            .collect::<Vec<_>>(),
        [
            vec![NameField::FamilyName; 6],
            vec![NameField::GivenName; 6],
            vec![NameField::Nickname; 4],
        ]
        .concat()
    );
    assert_eq!(
        cache
            .slots
            .iter()
            .map(|slot| slot.cache_code)
            .collect::<BTreeSet<_>>()
            .len(),
        NAME_GLYPH_CACHE_SLOT_COUNT
    );
    assert!(
        cache
            .slots
            .iter()
            .all(|slot| slot.source_page_position >= plan.pages[0].active_key_count)
    );
}

#[test]
#[ignore = "requires assets/"]
fn every_non_key_non_cache_cell_is_reserved_for_the_component_pack() {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/dialogue/name-entry/keyboard.json");

    let plan = load_name_input_keyboard(&path).unwrap();
    let storage = &plan.glyph_pack_storage;
    let cache_positions = plan
        .cache
        .slots
        .iter()
        .map(|slot| slot.atlas_layout_record_index)
        .collect::<BTreeSet<_>>();
    let pack_positions = storage
        .cells
        .iter()
        .map(|cell| cell.atlas_layout_record_index)
        .collect::<BTreeSet<_>>();

    assert_eq!(storage.cell_count, NAME_GLYPH_PACK_CELL_COUNT);
    assert_eq!(storage.byte_capacity, NAME_GLYPH_PACK_STORAGE_BYTES);
    assert!(cache_positions.is_disjoint(&pack_positions));
    assert_eq!(pack_positions.len(), NAME_GLYPH_PACK_CELL_COUNT);
    assert!(storage.cells.iter().all(|cell| {
        let page = plan
            .pages
            .iter()
            .find(|page| page.source_page == cell.source_page)
            .unwrap();
        cell.source_page_position >= page.active_key_count
    }));
}
