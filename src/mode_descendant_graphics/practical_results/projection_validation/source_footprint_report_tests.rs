use crate::tim::Cell;

use super::intersect_cells;

#[test]
fn disjoint_source_regions_do_not_underflow_when_classifying_a_footprint() {
    assert_eq!(
        intersect_cells(
            Cell {
                x: 0,
                y: 0,
                width: 32,
                height: 16,
            },
            Cell {
                x: 200,
                y: 72,
                width: 56,
                height: 32,
            },
        )
        .unwrap(),
        None
    );
    assert!(
        intersect_cells(
            Cell {
                x: usize::MAX,
                y: 0,
                width: 1,
                height: 1,
            },
            Cell {
                x: 0,
                y: 0,
                width: 1,
                height: 1,
            },
        )
        .is_err()
    );
}
