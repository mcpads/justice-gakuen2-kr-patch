use super::error::SourceFootprintError;
use super::model::{DecodedStaticTileStream, TileRun};

const ROW_SEPARATOR: u8 = 0xfe;
const STREAM_TERMINATOR: u8 = 0xff;

pub(crate) fn decode_static_tile_stream(
    storage: &[u8],
) -> Result<DecodedStaticTileStream, SourceFootprintError> {
    let mut offset = 0usize;
    let mut runs = Vec::new();
    let mut row_has_run = false;

    while offset < storage.len() {
        match storage[offset] {
            STREAM_TERMINATOR => {
                if runs.is_empty() {
                    return Err(SourceFootprintError::StaticStreamHasNoRuns);
                }
                if !row_has_run {
                    return Err(SourceFootprintError::StaticTerminatorAfterRowSeparator { offset });
                }
                let encoded_size = offset + 1;
                require_zero_padding(storage, encoded_size, "static")?;
                return Ok(DecodedStaticTileStream {
                    runs,
                    encoded_size,
                    storage_size: storage.len(),
                });
            }
            ROW_SEPARATOR => {
                if !row_has_run {
                    return Err(SourceFootprintError::UnexpectedStaticRowSeparator { offset });
                }
                row_has_run = false;
                offset += 1;
            }
            start_tile_id => {
                let count_offset = offset + 1;
                let Some(&tile_count) = storage.get(count_offset) else {
                    return Err(SourceFootprintError::TruncatedTileRun {
                        encoding: "static",
                        offset,
                    });
                };
                let tile_count = usize::from(tile_count);
                if tile_count == 0 {
                    return Err(SourceFootprintError::ZeroTileCount {
                        encoding: "static",
                        offset,
                    });
                }
                let start_tile_id = usize::from(start_tile_id);
                if start_tile_id
                    .checked_add(tile_count)
                    .is_none_or(|end| end > 0x100)
                {
                    return Err(SourceFootprintError::TileRangeOverflow {
                        encoding: "static",
                        offset,
                        start_tile_id,
                        tile_count,
                    });
                }
                runs.push(TileRun {
                    start_tile_id,
                    tile_count,
                    starts_new_row: !row_has_run && !runs.is_empty(),
                });
                row_has_run = true;
                offset += 2;
            }
        }
    }

    Err(SourceFootprintError::MissingStaticTerminator)
}

pub(super) fn require_zero_padding(
    storage: &[u8],
    encoded_size: usize,
    encoding: &'static str,
) -> Result<(), SourceFootprintError> {
    if let Some((relative_offset, _)) = storage[encoded_size..]
        .iter()
        .enumerate()
        .find(|(_, byte)| **byte != 0)
    {
        return Err(SourceFootprintError::NonZeroStreamPadding {
            encoding,
            offset: encoded_size + relative_offset,
        });
    }
    Ok(())
}
