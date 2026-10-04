use std::collections::BTreeSet;
use std::path::Path;

use crate::name_input::{NAME_GLYPH_CODE_START, load_name_input_keyboard};

use super::name_entry::{PADDING_CODE, PAGE_SPECS, SELECTABLE_CODE_SEQUENCE_OFFSET};
use super::name_entry_keyboard_build::patch_korean_name_keyboard_overlay;
use super::name_entry_test_fixture::name_entry_overlay;

#[test]
#[ignore = "requires assets/"]
fn keyboard_overlay_separates_keys_cache_and_pack_storage() {
    let keyboard = load_name_input_keyboard(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/dialogue/name-entry/keyboard.json"),
    )
    .unwrap();
    let patched = patch_korean_name_keyboard_overlay(&name_entry_overlay(), &keyboard).unwrap();

    assert_eq!(patched.report.active_key_count, 117);
    assert_eq!(patched.report.cache_code_count, 16);
    assert_eq!(patched.report.pack_storage_cell_count, 95);
    assert_eq!(patched.report.code_lookup_entry_count, 228);
    assert_eq!(
        patched
            .report
            .pages
            .iter()
            .map(|page| page.active_key_count)
            .collect::<Vec<_>>(),
        [40, 52, 25]
    );
    assert!(patched.report.runtime_navigation_consumer_installed);
    assert_eq!(patched.report.navigation_map_ids.len(), 5);
    assert_eq!(patched.navigation_maps.len(), 5);
    assert_eq!(
        patched
            .active_positions_by_page
            .iter()
            .map(BTreeSet::len)
            .collect::<Vec<_>>(),
        [40, 52, 25]
    );

    for ((_, page_offset, physical_cell_count, _), page_report) in
        PAGE_SPECS.into_iter().zip(&patched.report.pages)
    {
        let codes = (0..physical_cell_count)
            .map(|position| read_u16(&patched.bytes, page_offset + position * 2))
            .collect::<Vec<_>>();
        assert_eq!(
            codes.iter().filter(|code| **code != PADDING_CODE).count(),
            page_report.active_key_count
        );
    }

    for slot in &keyboard.cache.slots {
        assert_eq!(
            read_u16(
                &patched.bytes,
                SELECTABLE_CODE_SEQUENCE_OFFSET + slot.selectable_sequence_position * 2,
            ),
            slot.cache_code
        );
    }
    for cell in &keyboard.glyph_pack_storage.cells {
        assert_eq!(
            read_u16(
                &patched.bytes,
                SELECTABLE_CODE_SEQUENCE_OFFSET + cell.selectable_sequence_position * 2,
            ),
            PADDING_CODE
        );
    }
}

#[test]
#[ignore = "requires assets/"]
fn every_generated_navigation_edge_reaches_an_active_key_or_action() {
    let keyboard = load_name_input_keyboard(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/dialogue/name-entry/keyboard.json"),
    )
    .unwrap();
    let patched = patch_korean_name_keyboard_overlay(&name_entry_overlay(), &keyboard).unwrap();

    for (spec, navigation) in super::name_entry_input::NAME_NAVIGATION_SOURCE_SPECS
        .iter()
        .zip(&patched.navigation_maps)
    {
        let active = &patched.active_positions_by_page[spec.source_page_index];
        assert!(
            navigation
                .bytes
                .iter()
                .all(|position| usize::from(*position) >= 90 || active.contains(position))
        );
    }
}

#[test]
#[ignore = "requires assets/"]
fn active_lookup_codes_retain_their_layout_aligned_sequence_positions() {
    let keyboard = load_name_input_keyboard(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/dialogue/name-entry/keyboard.json"),
    )
    .unwrap();
    let patched = patch_korean_name_keyboard_overlay(&name_entry_overlay(), &keyboard).unwrap();
    let page_starts = [0_usize, 84, 166];

    for (page, page_start) in keyboard.pages.iter().zip(page_starts) {
        for assignment in &page.assignments {
            let sequence_position = page_start + assignment.page_position;
            assert_eq!(
                read_u16(
                    &patched.bytes,
                    SELECTABLE_CODE_SEQUENCE_OFFSET + sequence_position * 2,
                ),
                NAME_GLYPH_CODE_START + u16::try_from(sequence_position).unwrap()
            );
        }
    }
}

fn read_u16(data: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(data[offset..offset + 2].try_into().unwrap())
}
