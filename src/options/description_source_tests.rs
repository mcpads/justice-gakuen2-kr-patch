use super::description_source::{
    ITEM_COUNT, ITEM_SLOT_SIZE, ITEM_STATE_COUNTS, ITEM_TABLE_OFFSETS, ROOT_TABLE_OFFSET,
    STREAM_ARENA_END,
};

#[test]
fn description_tables_fit_between_the_stream_arena_and_root_table() {
    assert_eq!(ITEM_COUNT, ITEM_STATE_COUNTS.len());
    assert_eq!(ITEM_COUNT, ITEM_TABLE_OFFSETS.len());
    assert_eq!(ITEM_SLOT_SIZE, 0x8800);
    assert_eq!(STREAM_ARENA_END, ITEM_TABLE_OFFSETS[0]);
    for item_index in 0..ITEM_COUNT {
        let end = ITEM_TABLE_OFFSETS[item_index] + (ITEM_STATE_COUNTS[item_index] + 1) * 4;
        let expected_end = ITEM_TABLE_OFFSETS
            .get(item_index + 1)
            .copied()
            .unwrap_or(ROOT_TABLE_OFFSET);
        assert_eq!(end, expected_end);
    }
}
