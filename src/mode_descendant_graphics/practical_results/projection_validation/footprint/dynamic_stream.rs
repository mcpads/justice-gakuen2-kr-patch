use super::error::SourceFootprintError;
use super::model::{DecodedCountedTileStream, TileRun};
use super::static_stream::require_zero_padding;

const ROW_SEPARATOR: u8 = 0xff;

pub(crate) fn decode_counted_dynamic_tile_stream(
    storage: &[u8],
) -> Result<DecodedCountedTileStream, SourceFootprintError> {
    let Some((&header, remaining)) = storage.split_first() else {
        return Err(SourceFootprintError::TruncatedDynamicHeader);
    };
    let Some((&group_count, _)) = remaining.split_first() else {
        return Err(SourceFootprintError::TruncatedDynamicHeader);
    };
    let mut offset = 2usize;
    let mut runs = Vec::with_capacity(usize::from(group_count));
    for _ in 0..group_count {
        let starts_new_row = storage.get(offset) == Some(&ROW_SEPARATOR);
        if starts_new_row {
            offset += 1;
        }
        let Some(run_bytes) = storage.get(offset..offset + 2) else {
            return Err(SourceFootprintError::TruncatedTileRun {
                encoding: "counted dynamic",
                offset,
            });
        };
        let start_tile_id = usize::from(run_bytes[0]);
        let tile_count = usize::from(run_bytes[1]);
        if tile_count == 0 {
            return Err(SourceFootprintError::ZeroTileCount {
                encoding: "counted dynamic",
                offset,
            });
        }
        if start_tile_id
            .checked_add(tile_count)
            .is_none_or(|end| end > 0x100)
        {
            return Err(SourceFootprintError::TileRangeOverflow {
                encoding: "counted dynamic",
                offset,
                start_tile_id,
                tile_count,
            });
        }
        runs.push(TileRun {
            start_tile_id,
            tile_count,
            starts_new_row,
        });
        offset += 2;
    }
    require_zero_padding(storage, offset, "counted dynamic")?;
    Ok(DecodedCountedTileStream {
        header,
        runs,
        encoded_size: offset,
        storage_size: storage.len(),
    })
}
