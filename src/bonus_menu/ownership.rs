use crate::tim::{Cell, cells_overlap};

#[derive(Clone, Copy)]
pub(super) struct OwnedCell {
    pub(super) tim_offset: usize,
    pub(super) bits_per_pixel: u8,
    pub(super) cell: Cell,
}

pub(super) fn cells_are_unique_and_non_overlapping(cells: &[OwnedCell]) -> bool {
    cells.iter().enumerate().all(|(index, left)| {
        cells[index + 1..].iter().all(|right| {
            left.tim_offset != right.tim_offset
                || left.bits_per_pixel != right.bits_per_pixel
                || !cells_overlap(left.cell, right.cell)
        })
    })
}
