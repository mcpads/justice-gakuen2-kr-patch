//! Propagates the canonical result-atlas edits into every exact SIKEN20 member.

use anyhow::{Result, ensure};

use crate::decoded_record_write_plan::DecodedDataClaim;
use crate::pipeline::difference_ranges;
use crate::tim::{
    Cell, cells_overlap, read_4bpp_palette_words_in_prefix, read_indexed_cell_in_prefix,
    write_indexed_cell_in_prefix_with_report,
};
use crate::write_scope::changed_ranges_are_within;

use super::super::catalog::PRACTICAL_1999_RESULT_ARCHIVE;

const RESULT_TIM_OFFSET: usize = 0;
const STATIC_SHARED_RESULT_CELLS: [Cell; 4] = [
    Cell {
        x: 0,
        y: 0,
        width: 240,
        height: 20,
    },
    Cell {
        x: 0,
        y: 20,
        width: 40,
        height: 20,
    },
    Cell {
        x: 140,
        y: 40,
        width: 20,
        height: 20,
    },
    Cell {
        x: 160,
        y: 40,
        width: 20,
        height: 20,
    },
];

pub(in crate::mode_descendant_graphics) struct IndexedResultMirrorBuild {
    pub(in crate::mode_descendant_graphics) decoded: Vec<u8>,
    pub(in crate::mode_descendant_graphics) claims: Vec<DecodedDataClaim>,
    pub(in crate::mode_descendant_graphics) member_count: usize,
    pub(in crate::mode_descendant_graphics) mirrored_cell_count: usize,
}

pub(in crate::mode_descendant_graphics) fn mirror_canonical_result_cells_into_indexed_members(
    immutable_canonical: &[u8],
    patched_canonical: &[u8],
    immutable_indexed_results: &[u8],
    indexed_results_base: &[u8],
    active_title_cache_cells: &[Cell],
) -> Result<IndexedResultMirrorBuild> {
    ensure!(
        immutable_canonical.len() == patched_canonical.len()
            && immutable_indexed_results.len() == indexed_results_base.len()
            && !active_title_cache_cells.is_empty(),
        "indexed result mirror inputs are empty or changed extent"
    );
    let cells = STATIC_SHARED_RESULT_CELLS
        .into_iter()
        .chain(active_title_cache_cells.iter().copied())
        .collect::<Vec<_>>();
    ensure!(
        cells.iter().all(|cell| {
            cell.width > 0
                && cell.height > 0
                && cell.x + cell.width <= 256
                && cell.y + cell.height <= 256
        }) && cells.iter().enumerate().all(|(index, cell)| {
            cells[index + 1..]
                .iter()
                .all(|other| !cells_overlap(*cell, *other))
        }),
        "indexed result mirror cells are invalid or overlapping"
    );

    let canonical_palette =
        read_4bpp_palette_words_in_prefix(immutable_canonical, RESULT_TIM_OFFSET, 0)?;
    let mut patched = indexed_results_base.to_vec();
    let mut claims = Vec::new();
    let mut member_base = 0usize;
    let mut mirrored_cell_count = 0usize;

    for member in PRACTICAL_1999_RESULT_ARCHIVE.members {
        ensure!(
            member.index < PRACTICAL_1999_RESULT_ARCHIVE.members.len()
                && member_base + member.decoded_size <= immutable_indexed_results.len()
                && read_4bpp_palette_words_in_prefix(
                    immutable_indexed_results,
                    member_base + RESULT_TIM_OFFSET,
                    0,
                )? == canonical_palette,
            "indexed result member {} no longer aliases the canonical result palette",
            member.id
        );
        let member_before = patched.clone();
        let mut member_allowed_ranges = Vec::new();
        for cell in &cells {
            let immutable_source =
                read_indexed_cell_in_prefix(immutable_canonical, RESULT_TIM_OFFSET, *cell)?;
            let immutable_target = read_indexed_cell_in_prefix(
                immutable_indexed_results,
                member_base + RESULT_TIM_OFFSET,
                *cell,
            )?;
            ensure!(
                immutable_target == immutable_source,
                "indexed result member {} cell {cell:?} no longer aliases the canonical source",
                member.id
            );
            ensure!(
                read_indexed_cell_in_prefix(
                    indexed_results_base,
                    member_base + RESULT_TIM_OFFSET,
                    *cell,
                )? == immutable_target,
                "indexed result member {} cell {cell:?} overlaps an earlier compositor",
                member.id
            );
            let replacement =
                read_indexed_cell_in_prefix(patched_canonical, RESULT_TIM_OFFSET, *cell)?;
            if replacement == immutable_source {
                continue;
            }
            let write = write_indexed_cell_in_prefix_with_report(
                &mut patched,
                member_base + RESULT_TIM_OFFSET,
                *cell,
                &replacement,
            )?;
            ensure!(
                write.changed_byte_count > 0,
                "indexed result member {} cell {cell:?} changed no pixels",
                member.id
            );
            member_allowed_ranges.extend(write.allowed_ranges);
            mirrored_cell_count += 1;
        }
        ensure!(
            !member_allowed_ranges.is_empty()
                && changed_ranges_are_within(
                    &difference_ranges(&member_before, &patched),
                    &member_allowed_ranges,
                ),
            "indexed result member {} mirror escaped its finite source cells",
            member.id
        );
        claims.extend(DecodedDataClaim::from_effective_ranges(
            &format!(
                "mode-descendant:practical-result:{}:canonical-cell-mirror",
                member.id
            ),
            "mirror canonical result-atlas edits into an exact indexed result member",
            immutable_indexed_results,
            &patched,
            member_allowed_ranges,
        )?);
        member_base += member.decoded_size;
    }

    ensure!(
        member_base == immutable_indexed_results.len()
            && !claims.is_empty()
            && mirrored_cell_count
                >= STATIC_SHARED_RESULT_CELLS.len() * PRACTICAL_1999_RESULT_ARCHIVE.members.len(),
        "indexed result mirror omitted a member or every shared source cell"
    );
    Ok(IndexedResultMirrorBuild {
        decoded: patched,
        claims,
        member_count: PRACTICAL_1999_RESULT_ARCHIVE.members.len(),
        mirrored_cell_count,
    })
}
