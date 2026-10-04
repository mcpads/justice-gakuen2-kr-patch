use crate::tim::Cell;

use super::ownership::{OwnedCell, cells_are_unique_and_non_overlapping};

fn owned(tim_offset: usize, bits_per_pixel: u8, cell: Cell) -> OwnedCell {
    OwnedCell {
        tim_offset,
        bits_per_pixel,
        cell,
    }
}

#[test]
fn cells_with_the_same_coordinates_belong_to_distinct_tim_surfaces() {
    let cell = Cell {
        x: 0,
        y: 0,
        width: 32,
        height: 32,
    };
    assert!(cells_are_unique_and_non_overlapping(&[
        owned(0, 8, cell),
        owned(0x3c800, 4, cell),
    ]));
}

#[test]
fn ownership_checks_nonadjacent_pairs_for_overlap() {
    let cells = [
        owned(
            0x3c800,
            4,
            Cell {
                x: 0,
                y: 0,
                width: 32,
                height: 32,
            },
        ),
        owned(
            0x3c800,
            4,
            Cell {
                x: 64,
                y: 0,
                width: 32,
                height: 32,
            },
        ),
        owned(
            0x3c800,
            4,
            Cell {
                x: 16,
                y: 16,
                width: 32,
                height: 32,
            },
        ),
    ];

    assert!(!cells_are_unique_and_non_overlapping(&cells));
}
