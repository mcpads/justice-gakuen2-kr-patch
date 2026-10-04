use std::path::Path;

use crate::tim::Cell;

use super::{
    NAME_FONT_ATLAS_ROW_BYTES, NAME_GLYPH_CACHE_SLOT_COUNT, NAME_GLYPH_PACK_CELL_COUNT,
    load_name_input_keyboard, plan_name_input_runtime_atlas,
};

fn source_layout_cells() -> Vec<Cell> {
    (0..228)
        .map(|index| {
            let page = index / 84;
            let position = index % 84;
            Cell {
                x: page * 256 + position % 12 * 20,
                y: position / 12 * 20,
                width: 20,
                height: 20,
            }
        })
        .collect()
}

fn keyboard() -> super::NameInputKeyboardPlan {
    load_name_input_keyboard(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/dialogue/name-entry/keyboard.json"),
    )
    .unwrap()
}

#[test]
#[ignore = "requires assets/"]
fn runtime_atlas_keeps_pack_lookup_separate_from_the_cache_table() {
    let keyboard = keyboard();
    let layout = plan_name_input_runtime_atlas(&source_layout_cells(), &keyboard).unwrap();

    assert_eq!(layout.font_atlas_row_bytes, NAME_FONT_ATLAS_ROW_BYTES);
    assert_eq!(
        layout.pack_storage_cell_base_byte_offsets.len(),
        NAME_GLYPH_PACK_CELL_COUNT
    );
    assert_eq!(
        layout.cache_cell_base_byte_offsets.len(),
        NAME_GLYPH_CACHE_SLOT_COUNT
    );
    assert_eq!(
        layout.lookup_table_bytes.len(),
        NAME_GLYPH_PACK_CELL_COUNT * 2
    );
    for (bytes, expected) in layout
        .lookup_table_bytes
        .as_chunks::<2>()
        .0
        .iter()
        .zip(layout.pack_storage_cell_base_byte_offsets.iter())
    {
        assert_eq!(u16::from_le_bytes(*bytes), *expected);
    }
}

#[test]
#[ignore = "requires assets/"]
fn runtime_atlas_rejects_a_cell_that_cannot_be_byte_addressed() {
    let keyboard = keyboard();
    let mut cells = source_layout_cells();
    let cache_index = keyboard.cache.slots[0].atlas_layout_record_index;
    cells[cache_index].x += 1;

    let error = plan_name_input_runtime_atlas(&cells, &keyboard).unwrap_err();
    assert!(error.to_string().contains("outside the 4-bpp font atlas"));
}
