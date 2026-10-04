use std::collections::BTreeSet;
use std::path::Path;

use super::{
    NAME_GLYPH_CACHE_SLOT_COUNT, NAME_GLYPH_CODE_COUNT, NAME_GLYPH_PACK_CELL_COUNT, NameField,
    load_name_input_keyboard, plan_name_glyph_consumer_layout,
};

fn tracked_layout() -> super::NameGlyphConsumerLayout {
    let keyboard = load_name_input_keyboard(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/dialogue/name-entry/keyboard.json"),
    )
    .unwrap();
    plan_name_glyph_consumer_layout(&keyboard).unwrap()
}

#[test]
#[ignore = "requires assets/"]
fn active_keys_cache_slots_and_pack_cells_partition_one_shared_code_domain() {
    let layout = tracked_layout();
    let all_codes = layout
        .active_glyphs
        .iter()
        .map(|cell| cell.code)
        .chain(layout.cache_slots.iter().map(|slot| slot.cache_code))
        .chain(layout.pack_cells.iter().map(|cell| cell.code))
        .collect::<BTreeSet<_>>();

    assert_eq!(layout.active_glyphs.len(), 117);
    assert_eq!(layout.cache_slots.len(), NAME_GLYPH_CACHE_SLOT_COUNT);
    assert_eq!(layout.pack_cells.len(), NAME_GLYPH_PACK_CELL_COUNT);
    assert_eq!(all_codes.len(), NAME_GLYPH_CODE_COUNT);
    assert_eq!(all_codes.first().copied(), Some(0x0305));
    assert_eq!(all_codes.last().copied(), Some(0x03e8));
}

#[test]
#[ignore = "requires assets/"]
fn every_consumer_uses_the_same_field_slot_cache_codes() {
    let layout = tracked_layout();

    assert_eq!(
        layout.cache_codes_for_field(NameField::FamilyName).unwrap(),
        (0x032d..=0x0332).collect::<Vec<_>>()
    );
    assert_eq!(
        layout.cache_codes_for_field(NameField::GivenName).unwrap(),
        (0x0333..=0x0338).collect::<Vec<_>>()
    );
    assert_eq!(
        layout.cache_codes_for_field(NameField::Nickname).unwrap(),
        (0x0339..=0x033c).collect::<Vec<_>>()
    );
}

#[test]
#[ignore = "requires assets/"]
fn direct_ascii_codes_come_from_the_same_active_key_cells() {
    let layout = tracked_layout();

    assert_eq!(layout.code_for_active_character('A'), Some(0x0359));
    assert_eq!(layout.code_for_active_character('z'), Some(0x038c));
    assert_eq!(layout.code_for_active_character('0'), Some(0x03ab));
    assert!(layout.code_for_active_character('가').is_none());
}
