use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

use crate::tim::Cell;

use super::{NAME_GLYPH_CACHE_SLOT_COUNT, NAME_GLYPH_PACK_CELL_COUNT, NameInputKeyboardPlan};

pub const NAME_FONT_ATLAS_ROW_BYTES: usize = 384;
const NAME_FONT_ATLAS_PIXEL_WIDTH: usize = 768;
const NAME_FONT_ATLAS_HEIGHT: usize = 256;
const NAME_GLYPH_CELL_WIDTH: usize = 20;
const NAME_GLYPH_CELL_HEIGHT: usize = 20;

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct NameInputRuntimeAtlasLayout {
    pub font_atlas_row_bytes: usize,
    pub glyph_cell_width: usize,
    pub glyph_cell_height: usize,
    pub pack_storage_cell_base_byte_offsets: Vec<u16>,
    pub cache_cell_base_byte_offsets: Vec<u16>,
    pub lookup_table_bytes: Vec<u8>,
}

pub fn plan_name_input_runtime_atlas(
    cells: &[Cell],
    keyboard: &NameInputKeyboardPlan,
) -> Result<NameInputRuntimeAtlasLayout> {
    let pack_storage_cell_base_byte_offsets = keyboard
        .glyph_pack_storage
        .cells
        .iter()
        .map(|storage| {
            let cell = cells
                .get(storage.atlas_layout_record_index)
                .context("name glyph pack runtime layout index is out of range")?;
            cell_base_byte_offset(*cell)
        })
        .collect::<Result<Vec<_>>>()?;
    let cache_cell_base_byte_offsets = keyboard
        .cache
        .slots
        .iter()
        .map(|slot| {
            let cell = cells
                .get(slot.atlas_layout_record_index)
                .context("name glyph cache runtime layout index is out of range")?;
            cell_base_byte_offset(*cell)
        })
        .collect::<Result<Vec<_>>>()?;

    ensure!(
        pack_storage_cell_base_byte_offsets.len() == NAME_GLYPH_PACK_CELL_COUNT,
        "name glyph pack runtime layout has the wrong cell count"
    );
    ensure!(
        cache_cell_base_byte_offsets.len() == NAME_GLYPH_CACHE_SLOT_COUNT,
        "name glyph cache runtime layout has the wrong cell count"
    );
    let pack_offsets = pack_storage_cell_base_byte_offsets
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let cache_offsets = cache_cell_base_byte_offsets
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    ensure!(
        pack_offsets.len() == pack_storage_cell_base_byte_offsets.len()
            && cache_offsets.len() == cache_cell_base_byte_offsets.len(),
        "name-input runtime atlas assigns one physical cell more than once"
    );
    ensure!(
        pack_offsets.is_disjoint(&cache_offsets),
        "name glyph pack storage overlaps the persistent glyph cache"
    );

    let lookup_table_bytes = pack_storage_cell_base_byte_offsets
        .iter()
        .flat_map(|offset| offset.to_le_bytes())
        .collect::<Vec<_>>();

    Ok(NameInputRuntimeAtlasLayout {
        font_atlas_row_bytes: NAME_FONT_ATLAS_ROW_BYTES,
        glyph_cell_width: NAME_GLYPH_CELL_WIDTH,
        glyph_cell_height: NAME_GLYPH_CELL_HEIGHT,
        pack_storage_cell_base_byte_offsets,
        cache_cell_base_byte_offsets,
        lookup_table_bytes,
    })
}

fn cell_base_byte_offset(cell: Cell) -> Result<u16> {
    ensure!(
        cell.width == NAME_GLYPH_CELL_WIDTH && cell.height == NAME_GLYPH_CELL_HEIGHT,
        "name-input runtime atlas requires 20x20 glyph cells"
    );
    ensure!(
        cell.x.is_multiple_of(2)
            && cell.x + cell.width <= NAME_FONT_ATLAS_PIXEL_WIDTH
            && cell.y + cell.height <= NAME_FONT_ATLAS_HEIGHT,
        "name-input runtime glyph cell is outside the 4-bpp font atlas"
    );
    let byte_offset = cell
        .y
        .checked_mul(NAME_FONT_ATLAS_ROW_BYTES)
        .and_then(|row| row.checked_add(cell.x / 2))
        .context("name-input runtime glyph address overflow")?;
    Ok(u16::try_from(byte_offset)?)
}

/// A selectable direct key's canonical low code and physical MA_ENT cell.
#[derive(Clone, Debug)]
pub(crate) struct NameInputDirectGlyphSource {
    pub legacy_code: u16,
    pub character: u8,
    pub pixel_byte_offset: u16,
}
