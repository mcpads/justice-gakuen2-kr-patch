use super::dialogue_disc_finalization::contiguous_lba_ranges;

#[test]
fn changed_sectors_are_reported_as_compact_end_exclusive_ranges() {
    assert_eq!(
        contiguous_lba_ranges(&[4, 5, 6, 9, 12, 13]),
        vec![[4, 7], [9, 10], [12, 14]]
    );
    assert!(contiguous_lba_ranges(&[]).is_empty());
}
