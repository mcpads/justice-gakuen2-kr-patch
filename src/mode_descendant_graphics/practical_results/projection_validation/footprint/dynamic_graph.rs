use std::ops::Range;

use super::dynamic_stream::decode_counted_dynamic_tile_stream;
use super::error::SourceFootprintError;
use super::model::{
    DynamicConfig, DynamicConfigGraph, DynamicConfigGraphLayout, DynamicStreamPointerTarget,
};

const POINTER_SIZE: usize = 4;

pub(crate) fn parse_dynamic_config_graph(
    overlay: &[u8],
    layout: DynamicConfigGraphLayout,
) -> Result<DynamicConfigGraph, SourceFootprintError> {
    let graph_end = layout
        .graph_offset
        .checked_add(layout.graph_size)
        .ok_or(SourceFootprintError::InvalidGraphSpan)?;
    if layout.graph_size == 0 || graph_end > overlay.len() {
        return Err(SourceFootprintError::InvalidGraphSpan);
    }
    if layout.root_count == 0 {
        return Err(SourceFootprintError::InvalidGraphLayout {
            config_index: None,
            detail: "root table is empty",
        });
    }
    let selector_cell_count = layout
        .selector_row_count
        .checked_mul(layout.selector_column_count)
        .ok_or(SourceFootprintError::InvalidSelectorGrid)?;
    if selector_cell_count == 0 || selector_cell_count > layout.mapping_storage_size {
        return Err(SourceFootprintError::InvalidSelectorGrid);
    }
    let root_table_size = layout
        .root_count
        .checked_mul(POINTER_SIZE)
        .ok_or(SourceFootprintError::InvalidGraphSpan)?;
    let root_table_end = layout
        .root_table_offset
        .checked_add(root_table_size)
        .ok_or(SourceFootprintError::InvalidGraphSpan)?;
    if layout.root_table_offset < layout.graph_offset || root_table_end != graph_end {
        return Err(SourceFootprintError::RootTableDoesNotEndGraph);
    }

    let graph_range = layout.graph_offset..graph_end;
    let root_offsets = pointer_offsets(
        overlay,
        layout.root_table_offset,
        layout.root_count,
        layout.runtime_base,
        &graph_range,
    )?;
    require_strictly_increasing(
        &root_offsets,
        None,
        "config roots are not strictly increasing",
    )?;
    if root_offsets
        .iter()
        .any(|offset| *offset >= layout.root_table_offset)
    {
        return Err(SourceFootprintError::InvalidGraphLayout {
            config_index: None,
            detail: "config root overlaps the root table",
        });
    }

    let mut mapping_offsets = Vec::with_capacity(root_offsets.len());
    for &root_offset in &root_offsets {
        let mapping_pointer = read_u32(overlay, root_offset)?;
        mapping_offsets.push(pointer_offset(
            mapping_pointer,
            layout.runtime_base,
            &graph_range,
        )?);
    }
    require_strictly_increasing(
        &mapping_offsets,
        None,
        "config mappings are not strictly increasing",
    )?;
    if mapping_offsets.first() != Some(&layout.graph_offset) {
        return Err(SourceFootprintError::InvalidGraphLayout {
            config_index: None,
            detail: "first config mapping does not begin the graph span",
        });
    }

    let mut configs = Vec::with_capacity(layout.root_count);
    for config_index in 0..layout.root_count {
        let mapping_offset = mapping_offsets[config_index];
        let root_offset = root_offsets[config_index];
        let root_bound = mapping_offsets
            .get(config_index + 1)
            .copied()
            .unwrap_or(layout.root_table_offset);
        let mapping_end = mapping_offset
            .checked_add(layout.mapping_storage_size)
            .ok_or(SourceFootprintError::InvalidGraphSpan)?;
        if mapping_end > root_offset {
            return Err(SourceFootprintError::InvalidGraphLayout {
                config_index: Some(config_index),
                detail: "mapping storage overlaps its stream storage or root",
            });
        }
        let root_storage_size = root_bound.checked_sub(root_offset).ok_or(
            SourceFootprintError::InvalidGraphLayout {
                config_index: Some(config_index),
                detail: "root appears after its storage boundary",
            },
        )?;
        if root_storage_size < POINTER_SIZE * 2 || root_storage_size % POINTER_SIZE != 0 {
            return Err(SourceFootprintError::InvalidGraphLayout {
                config_index: Some(config_index),
                detail: "root pointer array does not contain a mapping and at least one stream",
            });
        }
        let pointer_count = root_storage_size / POINTER_SIZE;
        let pointers = pointer_offsets(
            overlay,
            root_offset,
            pointer_count,
            layout.runtime_base,
            &graph_range,
        )?;
        if pointers[0] != mapping_offset {
            return Err(SourceFootprintError::InvalidGraphLayout {
                config_index: Some(config_index),
                detail: "root first pointer does not name its mapping",
            });
        }
        let stream_offsets = &pointers[1..];
        require_strictly_increasing(
            stream_offsets,
            Some(config_index),
            "stream pointers are not strictly increasing",
        )?;
        if stream_offsets.first() != Some(&mapping_end) {
            return Err(SourceFootprintError::InvalidGraphLayout {
                config_index: Some(config_index),
                detail: "first stream does not immediately follow mapping storage",
            });
        }
        if stream_offsets.iter().any(|offset| *offset >= root_offset) {
            return Err(SourceFootprintError::InvalidGraphLayout {
                config_index: Some(config_index),
                detail: "stream pointer does not precede its root",
            });
        }

        let mapping = &overlay[mapping_offset..mapping_end];
        let selector_stream_indices = mapping[..selector_cell_count]
            .iter()
            .map(|value| usize::from(*value))
            .collect::<Vec<_>>();
        if let Some((relative_offset, _)) = mapping[selector_cell_count..]
            .iter()
            .enumerate()
            .find(|(_, byte)| **byte != 0)
        {
            return Err(SourceFootprintError::NonZeroMappingPadding {
                config_index,
                offset: mapping_offset + selector_cell_count + relative_offset,
            });
        }

        let mut stream_pointer_targets = Vec::with_capacity(stream_offsets.len());
        for (stream_index, &stream_offset) in stream_offsets.iter().enumerate() {
            let stream_end = stream_offsets
                .get(stream_index + 1)
                .copied()
                .unwrap_or(root_offset);
            stream_pointer_targets.push(DynamicStreamPointerTarget {
                source_offset: stream_offset,
                decoded_stream: decode_counted_dynamic_tile_stream(
                    &overlay[stream_offset..stream_end],
                )
                .map_err(|source| SourceFootprintError::DynamicConfigStream {
                    config_index,
                    stream_index,
                    stream_offset,
                    source: Box::new(source),
                })?,
            });
        }
        for (selector_index, &stream_index) in selector_stream_indices.iter().enumerate() {
            if stream_index >= stream_pointer_targets.len() {
                return Err(SourceFootprintError::SelectorReferencesMissingStream {
                    config_index,
                    selector_index,
                    stream_index,
                    stream_count: stream_pointer_targets.len(),
                });
            }
        }
        configs.push(DynamicConfig {
            root_offset,
            mapping_offset,
            selector_stream_indices,
            stream_pointer_targets,
        });
    }

    Ok(DynamicConfigGraph {
        graph_offset: layout.graph_offset,
        graph_size: layout.graph_size,
        root_table_offset: layout.root_table_offset,
        configs,
    })
}

fn pointer_offsets(
    overlay: &[u8],
    table_offset: usize,
    pointer_count: usize,
    runtime_base: u32,
    graph_range: &Range<usize>,
) -> Result<Vec<usize>, SourceFootprintError> {
    (0..pointer_count)
        .map(|index| {
            let offset = table_offset
                .checked_add(index * POINTER_SIZE)
                .ok_or(SourceFootprintError::InvalidGraphSpan)?;
            pointer_offset(read_u32(overlay, offset)?, runtime_base, graph_range)
        })
        .collect()
}

fn pointer_offset(
    pointer: u32,
    runtime_base: u32,
    graph_range: &Range<usize>,
) -> Result<usize, SourceFootprintError> {
    let relative =
        pointer
            .checked_sub(runtime_base)
            .ok_or(SourceFootprintError::PointerBelowRuntimeBase {
                pointer,
                runtime_base,
            })?;
    let offset =
        usize::try_from(relative).map_err(|_| SourceFootprintError::PointerOutsideGraph {
            pointer,
            graph_offset: graph_range.start,
            graph_end: graph_range.end,
        })?;
    if offset % POINTER_SIZE != 0 {
        return Err(SourceFootprintError::UnalignedGraphPointer { pointer });
    }
    if !graph_range.contains(&offset) {
        return Err(SourceFootprintError::PointerOutsideGraph {
            pointer,
            graph_offset: graph_range.start,
            graph_end: graph_range.end,
        });
    }
    Ok(offset)
}

fn read_u32(overlay: &[u8], offset: usize) -> Result<u32, SourceFootprintError> {
    let bytes = overlay
        .get(offset..offset + POINTER_SIZE)
        .ok_or(SourceFootprintError::TruncatedGraphWord { offset })?;
    Ok(u32::from_le_bytes(
        bytes.try_into().expect("four-byte slice"),
    ))
}

fn require_strictly_increasing(
    offsets: &[usize],
    config_index: Option<usize>,
    detail: &'static str,
) -> Result<(), SourceFootprintError> {
    if offsets.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(SourceFootprintError::InvalidGraphLayout {
            config_index,
            detail,
        });
    }
    Ok(())
}
