//! One source-bound TESTMJ allocation shared by the counter producer and packets.
use crate::tim::Cell;

pub(crate) const MEMBER: usize = 31;
pub(crate) const LABEL_CELLS: [Cell; 5] = [
    Cell {
        x: 0,
        y: 128,
        width: 16,
        height: 16,
    },
    Cell {
        x: 16,
        y: 128,
        width: 16,
        height: 16,
    },
    Cell {
        x: 32,
        y: 128,
        width: 16,
        height: 16,
    },
    Cell {
        x: 48,
        y: 128,
        width: 16,
        height: 16,
    },
    Cell {
        x: 64,
        y: 128,
        width: 16,
        height: 16,
    },
];
pub(crate) const OWNED_TILES: [usize; 3] = [64, 65, 66];
pub(crate) fn label_index(row: usize, fragment: usize) -> usize {
    assert!(row < 3 && fragment < 4);
    match fragment {
        0 => row,
        1 | 3 => 3,
        2 => 4,
        _ => unreachable!(),
    }
}
