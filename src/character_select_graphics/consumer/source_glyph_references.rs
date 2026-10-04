//! Canonical 20px source cells reached by typed PLSEL consumers.

use std::collections::BTreeSet;

use anyhow::{Result, ensure};

use crate::tim::Cell;

use super::map::{DescriptorEncoding, OVERLAYS};
use super::stage_selector;
use crate::character_select_graphics::model::{
    CharacterSelectLocalizedSource, CharacterSelectTextureSurface,
};

const GLYPH_SIZE: usize = 20;

#[derive(Clone, Copy)]
pub(in crate::character_select_graphics) struct SourceGlyphReference {
    pub(in crate::character_select_graphics) source_ui_id: &'static str,
    pub(in crate::character_select_graphics) surface: CharacterSelectTextureSurface,
    pub(in crate::character_select_graphics) cell: Cell,
}

pub(in crate::character_select_graphics) fn source_referenced_20px_cells()
-> Result<Vec<SourceGlyphReference>> {
    let mut references = stage_selector::source_glyph_cells()?
        .into_iter()
        .map(|(source_ui_id, cell)| SourceGlyphReference {
            source_ui_id,
            surface: CharacterSelectTextureSurface::StageLabelAtlas,
            cell,
        })
        .collect::<Vec<_>>();
    for descriptor in OVERLAYS.iter().flat_map(|overlay| overlay.descriptors) {
        let mut cells = Vec::new();
        match descriptor.encoding {
            DescriptorEncoding::DirectLabelGrid => {
                append_grid_pairs(&mut cells, descriptor.expected, 0)?;
            }
            DescriptorEncoding::CountedLabelGrid => {
                let count = usize::from(descriptor.expected[0]);
                let end = 1 + count * 2;
                ensure!(
                    end <= descriptor.expected.len(),
                    "{} source count exceeds its label-grid descriptor",
                    descriptor.source_ui_id
                );
                append_grid_pairs(&mut cells, &descriptor.expected[1..end], 0)?;
            }
            DescriptorEncoding::ParticipantRuns => {
                let run_count = usize::from(descriptor.expected[0]);
                ensure!(
                    descriptor.expected.len() == 1 + run_count * 3,
                    "{} source participant runs changed shape",
                    descriptor.source_ui_id
                );
                for run in descriptor.expected[1..].as_chunks::<3>().0 {
                    for column in run[0]..run[0] + run[2] {
                        cells.push(grid_cell(0, column, run[1])?);
                    }
                }
            }
            DescriptorEncoding::DirectUv => {
                ensure!(
                    descriptor.expected.len() % 2 == 0,
                    "{} source direct-UV descriptor changed shape",
                    descriptor.source_ui_id
                );
                for pair in descriptor.expected.as_chunks::<2>().0 {
                    let cell = Cell {
                        x: 2 * 256 + usize::from(pair[0]),
                        y: usize::from(pair[1]),
                        width: GLYPH_SIZE,
                        height: GLYPH_SIZE,
                    };
                    ensure!(
                        cell.x + cell.width <= 3 * 256 && cell.y + cell.height <= 256,
                        "{} source direct-UV cell escaped page 2",
                        descriptor.source_ui_id
                    );
                    cells.push(cell);
                }
            }
            DescriptorEncoding::ColumnRow
            | DescriptorEncoding::DirectURow
            | DescriptorEncoding::SelectionHelp => {}
        }
        references.extend(cells.into_iter().map(|cell| SourceGlyphReference {
            source_ui_id: descriptor.source_ui_id,
            surface: descriptor.surface,
            cell,
        }));
    }
    references.push(SourceGlyphReference {
        source_ui_id: "team_marker_h",
        surface: CharacterSelectTextureSurface::StageLabelAtlas,
        cell: Cell {
            x: 220,
            y: 0,
            width: GLYPH_SIZE,
            height: GLYPH_SIZE,
        },
    });
    Ok(references)
}

pub(in crate::character_select_graphics) fn retained_source_referenced_20px_cells(
    localized_sources: &[CharacterSelectLocalizedSource],
) -> Result<Vec<SourceGlyphReference>> {
    let replaced_source_ui_ids = localized_sources
        .iter()
        .map(|source| source.source_ui_id.as_str())
        .collect::<BTreeSet<_>>();
    let unique = source_referenced_20px_cells()?
        .into_iter()
        .filter(|reference| !replaced_source_ui_ids.contains(reference.source_ui_id))
        .map(|reference| {
            (
                reference.surface,
                reference.cell.x,
                reference.cell.y,
                reference.cell.width,
                reference.cell.height,
            )
        })
        .collect::<BTreeSet<_>>();
    Ok(unique
        .into_iter()
        .map(|(surface, x, y, width, height)| SourceGlyphReference {
            source_ui_id: "retained-typed-consumer",
            surface,
            cell: Cell {
                x,
                y,
                width,
                height,
            },
        })
        .collect())
}

pub(in crate::character_select_graphics) fn reclaimable_stage_label_source_cells(
    localized_sources: &[CharacterSelectLocalizedSource],
) -> Result<Vec<Cell>> {
    let replaced_source_ui_ids = localized_sources
        .iter()
        .map(|source| source.source_ui_id.as_str())
        .collect::<BTreeSet<_>>();
    let translated_source_cells = stage_selector::source_glyph_cells()?
        .into_iter()
        .map(|(_, cell)| (cell.x, cell.y, cell.width, cell.height))
        .collect::<BTreeSet<_>>();
    let other_references = source_referenced_20px_cells()?
        .into_iter()
        .filter(|reference| !replaced_source_ui_ids.contains(reference.source_ui_id))
        .collect::<Vec<_>>();
    Ok(translated_source_cells
        .into_iter()
        .map(|(x, y, width, height)| Cell {
            x,
            y,
            width,
            height,
        })
        .filter(|translated_cell| {
            other_references.iter().all(|reference| {
                !reference
                    .surface
                    .shares_physical_texture(CharacterSelectTextureSurface::StageLabelAtlas)
                    || !crate::tim::cells_overlap(reference.cell, *translated_cell)
            })
        })
        .collect())
}

fn append_grid_pairs(cells: &mut Vec<Cell>, bytes: &[u8], page: u8) -> Result<()> {
    ensure!(
        bytes.len().is_multiple_of(2),
        "source label-grid descriptor has an odd byte count"
    );
    for pair in bytes.as_chunks::<2>().0 {
        cells.push(grid_cell(page, pair[0], pair[1])?);
    }
    Ok(())
}

fn grid_cell(page: u8, column: u8, row: u8) -> Result<Cell> {
    ensure!(
        page < 4 && column < 12 && row < 12,
        "source label-grid cell escaped the canonical 20px grid"
    );
    Ok(Cell {
        x: usize::from(page) * 256 + usize::from(column) * GLYPH_SIZE,
        y: usize::from(row) * GLYPH_SIZE,
        width: GLYPH_SIZE,
        height: GLYPH_SIZE,
    })
}
