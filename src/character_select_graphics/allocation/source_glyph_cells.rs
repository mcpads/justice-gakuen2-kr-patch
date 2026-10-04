//! Canonical source glyph cells reserved from source-blank dynamic allocation.

use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};

use crate::pipeline::sha256_bytes;
use crate::tim::{cells_overlap, read_indexed_cell_in_prefix};

use super::SHARED_ATLAS_OFFSET;
use crate::character_select_graphics::consumer::SourceGlyphReference;
use crate::character_select_graphics::model::{
    CharacterSelectGlyphAllocation, CharacterSelectSourceGlyphCell,
};

pub(super) fn collect_source_glyph_cells(
    shared_source_decoded: &[u8],
    references: &[SourceGlyphReference],
) -> Result<Vec<CharacterSelectSourceGlyphCell>> {
    let unique = references
        .iter()
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
    ensure!(
        unique.len() == references.len(),
        "source glyph reference set contains duplicate scoped cells"
    );
    let mut cells = Vec::with_capacity(references.len());
    for &SourceGlyphReference {
        source_ui_id: _,
        surface,
        cell,
    } in references
    {
        ensure!(
            cell.width == 20
                && cell.height == 20
                && cell.x < 1024
                && cell.x % 256 + cell.width <= 256
                && cell.y + cell.height <= 256,
            "source glyph reference is outside a 256x256 texture page: {cell:?}"
        );
        let page = cell.x / 256;
        let texture_u = cell.x % 256;
        let texture_v = cell.y;
        let indexed = read_indexed_cell_in_prefix(shared_source_decoded, SHARED_ATLAS_OFFSET, cell)
            .with_context(|| {
                format!(
                    "failed to read source glyph cell page {page}, U {texture_u}, V {texture_v}"
                )
            })?;
        cells.push(CharacterSelectSourceGlyphCell {
            physical_cell_id: format!(
                "shared-source-glyph-page-{page}-u-{texture_u}-v-{texture_v}"
            ),
            surface,
            tim_offset: SHARED_ATLAS_OFFSET,
            texture_page_index: u8::try_from(page)?,
            texture_uv: [u8::try_from(texture_u)?, u8::try_from(texture_v)?],
            cell,
            source_indexed_sha256: sha256_bytes(&indexed),
        });
    }
    Ok(cells)
}

pub(super) fn validate_source_glyph_cell_conflicts(
    source_glyph_cells: &[CharacterSelectSourceGlyphCell],
    glyphs: &[CharacterSelectGlyphAllocation],
) -> Result<()> {
    for source_cell in source_glyph_cells {
        ensure!(
            glyphs.iter().all(|glyph| {
                glyph.source_cell_occupancy
                    != super::super::model::CharacterSelectSourceCellOccupancy::Blank
                    || !source_cell.surface.shares_physical_texture(glyph.surface)
                    || !cells_overlap(source_cell.cell, glyph.cell)
            }),
            "source glyph cell {} overlaps a source-blank dynamic glyph allocation",
            source_cell.physical_cell_id
        );
    }
    Ok(())
}

#[cfg(test)]
#[path = "source_glyph_cells_tests.rs"]
mod tests;
