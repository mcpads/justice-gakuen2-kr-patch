use std::collections::{BTreeMap, BTreeSet};

use super::error::SourceFootprintError;
use super::model::{SourceAtlasRectangle, SourceAtlasTileGeometry};

pub(crate) fn tile_ids_to_atlas_rectangles(
    tile_ids: &BTreeSet<usize>,
    geometry: SourceAtlasTileGeometry,
) -> Result<Vec<SourceAtlasRectangle>, SourceFootprintError> {
    let (column_count, row_count, tile_capacity) = validate_geometry(geometry)?;
    let mut columns_by_row: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for &tile_id in tile_ids {
        if tile_id >= tile_capacity {
            return Err(SourceFootprintError::TileOutsideAtlas {
                tile_id,
                tile_capacity,
            });
        }
        columns_by_row
            .entry(tile_id / column_count)
            .or_default()
            .push(tile_id % column_count);
    }

    let mut rectangles: Vec<SourceAtlasRectangle> = Vec::new();
    let mut active: BTreeMap<(usize, usize), usize> = BTreeMap::new();
    let mut previous_row = None;
    for (row, columns) in columns_by_row {
        debug_assert!(row < row_count);
        if previous_row.is_none_or(|previous| previous + 1 != row) {
            active.clear();
        }
        let mut next_active = BTreeMap::new();
        for (start_column, end_column) in horizontal_runs(&columns) {
            let key = (start_column, end_column);
            let rectangle_index = if let Some(&index) = active.get(&key) {
                rectangles[index].height = rectangles[index]
                    .height
                    .checked_add(geometry.tile_height)
                    .ok_or(SourceFootprintError::CoordinateOverflow)?;
                index
            } else {
                let x = start_column
                    .checked_mul(geometry.tile_width)
                    .ok_or(SourceFootprintError::CoordinateOverflow)?;
                let y = row
                    .checked_mul(geometry.tile_height)
                    .ok_or(SourceFootprintError::CoordinateOverflow)?;
                let run_width = end_column - start_column + 1;
                let width = run_width
                    .checked_mul(geometry.tile_width)
                    .ok_or(SourceFootprintError::CoordinateOverflow)?;
                rectangles.push(SourceAtlasRectangle {
                    x,
                    y,
                    width,
                    height: geometry.tile_height,
                });
                rectangles.len() - 1
            };
            next_active.insert(key, rectangle_index);
        }
        active = next_active;
        previous_row = Some(row);
    }
    rectangles
        .sort_by_key(|rectangle| (rectangle.y, rectangle.x, rectangle.width, rectangle.height));
    Ok(rectangles)
}

fn validate_geometry(
    geometry: SourceAtlasTileGeometry,
) -> Result<(usize, usize, usize), SourceFootprintError> {
    if geometry.atlas_width == 0
        || geometry.atlas_height == 0
        || geometry.tile_width == 0
        || geometry.tile_height == 0
    {
        return Err(SourceFootprintError::InvalidSourceAtlasTileGeometry {
            detail: "dimensions must be nonzero",
        });
    }
    if !geometry.atlas_width.is_multiple_of(geometry.tile_width)
        || !geometry.atlas_height.is_multiple_of(geometry.tile_height)
    {
        return Err(SourceFootprintError::InvalidSourceAtlasTileGeometry {
            detail: "atlas dimensions must be divisible by tile dimensions",
        });
    }
    let column_count = geometry.atlas_width / geometry.tile_width;
    let row_count = geometry.atlas_height / geometry.tile_height;
    let tile_capacity = column_count.checked_mul(row_count).ok_or(
        SourceFootprintError::InvalidSourceAtlasTileGeometry {
            detail: "tile capacity overflows",
        },
    )?;
    Ok((column_count, row_count, tile_capacity))
}

fn horizontal_runs(columns: &[usize]) -> Vec<(usize, usize)> {
    let mut runs = Vec::new();
    let Some(&first) = columns.first() else {
        return runs;
    };
    let mut start = first;
    let mut end = first;
    for &column in &columns[1..] {
        if column == end + 1 {
            end = column;
        } else {
            runs.push((start, end));
            start = column;
            end = column;
        }
    }
    runs.push((start, end));
    runs
}
