use super::*;

#[test]
fn merges_adjacent_difference_offsets() {
    assert_eq!(
        difference_ranges(&[0, 0, 0, 0, 0], &[0, 1, 2, 0, 3]),
        vec![[1, 3], [4, 5]]
    );
}
