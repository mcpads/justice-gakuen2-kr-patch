//! Static and dynamic tile-stream consumers and configuration catalogs.

use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};

use super::super::super::projection_model::*;
use super::super::footprint::{
    DecodedStaticTileStream, DynamicConfigGraph, DynamicConfigGraphLayout,
    decode_static_tile_stream, parse_dynamic_config_graph,
};
use super::super::source_evidence::{
    ValidationCounts, address_and_span, address_only, checked_end, ensure_pointer_value,
    parse_hex_u32, validate_offset_list, validate_pointer_aliases,
};

pub(super) fn validate_matrix_streams(
    overlay: &[u8],
    base: u32,
    evidence: &PracticalResultMatrixTileStreamEvidence,
    counts: &mut ValidationCounts,
) -> Result<Vec<DecodedStaticTileStream>> {
    for (role, offset, address) in [
        (
            "matrix row-axis load",
            &evidence.row_axis_load_offset,
            &evidence.row_axis_load_runtime_address,
        ),
        (
            "matrix column-axis load",
            &evidence.column_axis_load_offset,
            &evidence.column_axis_load_runtime_address,
        ),
    ] {
        address_only(overlay, base, offset, address, role, counts)?;
    }
    let pointer_table_offset = address_and_span(
        overlay,
        base,
        &evidence.pointer_table_offset,
        &evidence.pointer_table_runtime_address,
        evidence.pointer_table_size,
        &evidence.pointer_table_sha256,
        "matrix stream pointer table",
        counts,
    )?;
    let arena_offset = address_and_span(
        overlay,
        base,
        &evidence.stream_arena_offset,
        &evidence.stream_arena_runtime_address,
        evidence.stream_arena_size,
        &evidence.stream_arena_sha256,
        "matrix stream arena",
        counts,
    )?;
    let arena_end = checked_end(
        arena_offset,
        evidence.stream_arena_size,
        "matrix stream arena",
    )?;
    let selector_grid_population = evidence
        .selector_grid
        .row_count
        .checked_mul(evidence.selector_grid.column_count)
        .context("matrix selector-grid population overflow")?;
    ensure!(
        evidence.pointer_table_size.is_multiple_of(4),
        "matrix stream pointer table is not word-sized"
    );
    let source_pointer_count = evidence.pointer_table_size / 4;
    ensure!(
        source_pointer_count > 0
            && selector_grid_population == source_pointer_count
            && evidence.expected_derived_tile_id_min <= evidence.expected_derived_tile_id_max
            && matches!(
                evidence.stream_encoding,
                PracticalResultStaticStreamEncoding::TerminatedRunsWithFeRowSeparator
            ),
        "matrix stream denominator or encoding changed"
    );
    validate_offset_list(
        overlay,
        &evidence.selector_calculation_offsets,
        "selector calculation",
    )?;
    validate_offset_list(overlay, &evidence.pointer_load_offsets, "pointer load")?;
    let stream_offsets = decode_stream_pointer_offsets(
        overlay,
        base,
        pointer_table_offset,
        source_pointer_count,
        arena_offset,
        arena_end,
        "matrix stream",
    )?;
    let streams = decode_static_stream_storage(overlay, &stream_offsets, arena_end)?;
    validate_derived_tile_bounds(
        &streams,
        evidence.expected_derived_tile_id_min,
        evidence.expected_derived_tile_id_max,
        "matrix stream",
    )?;
    Ok(streams)
}

pub(super) fn validate_selector_streams(
    overlay: &[u8],
    base: u32,
    evidence: &PracticalResultSelectorTileStreamEvidence,
    counts: &mut ValidationCounts,
) -> Result<Vec<DecodedStaticTileStream>> {
    for (role, offset, address) in [
        (
            "selector-axis load",
            &evidence.selector_axis_load_offset,
            &evidence.selector_axis_load_runtime_address,
        ),
        (
            "selector scale",
            &evidence.selector_scale_offset,
            &evidence.selector_scale_runtime_address,
        ),
        (
            "stream pointer load",
            &evidence.pointer_load_offset,
            &evidence.pointer_load_runtime_address,
        ),
    ] {
        address_only(overlay, base, offset, address, role, counts)?;
    }
    let pointer_table_offset = address_and_span(
        overlay,
        base,
        &evidence.pointer_table_offset,
        &evidence.pointer_table_runtime_address,
        evidence.pointer_table_size,
        &evidence.pointer_table_sha256,
        "selector stream pointer table",
        counts,
    )?;
    let arena_offset = address_and_span(
        overlay,
        base,
        &evidence.stream_arena_offset,
        &evidence.stream_arena_runtime_address,
        evidence.stream_arena_size,
        &evidence.stream_arena_sha256,
        "selector stream arena",
        counts,
    )?;
    let arena_end = checked_end(arena_offset, evidence.stream_arena_size, "stream arena")?;
    let expected_pointer_table_size = evidence
        .streams
        .len()
        .checked_mul(4)
        .context("selector stream pointer-table size overflow")?;
    ensure!(
        !evidence.streams.is_empty()
            && evidence.pointer_table_size == expected_pointer_table_size
            && evidence.expected_derived_tile_id_min <= evidence.expected_derived_tile_id_max
            && matches!(
                evidence.stream_encoding,
                PracticalResultStaticStreamEncoding::TerminatedRunsWithFeRowSeparator
            ),
        "selector stream denominator changed"
    );
    let mut stream_offsets = Vec::with_capacity(evidence.streams.len());
    for (index, stream) in evidence.streams.iter().enumerate() {
        ensure!(
            stream.raw_selector_value == index,
            "selector stream raw-value order changed"
        );
        let offset = address_and_span(
            overlay,
            base,
            &stream.offset,
            &stream.runtime_address,
            stream.encoded_size,
            &stream.encoded_sha256,
            "encoded selector stream",
            counts,
        )?;
        ensure!(
            offset >= arena_offset
                && checked_end(offset, stream.encoded_size, "encoded stream")? <= arena_end,
            "selector stream escapes its hashed arena"
        );
        validate_pointer_aliases(
            overlay,
            pointer_table_offset,
            &[index],
            base.checked_add(u32::try_from(offset)?)
                .context("stream address overflow")?,
            "selector stream",
        )?;
        stream_offsets.push(offset);
    }
    ensure!(
        stream_offsets.first() == Some(&arena_offset)
            && stream_offsets.windows(2).all(|pair| pair[0] < pair[1]),
        "selector stream offsets no longer partition their arena"
    );
    let streams = decode_static_stream_storage(overlay, &stream_offsets, arena_end)?;
    ensure!(
        streams
            .iter()
            .zip(&evidence.streams)
            .all(|(decoded, declared)| decoded.encoded_size == declared.encoded_size),
        "selector stream encoded sizes differ from decoded terminators"
    );
    validate_derived_tile_bounds(
        &streams,
        evidence.expected_derived_tile_id_min,
        evidence.expected_derived_tile_id_max,
        "selector stream",
    )?;
    Ok(streams)
}

fn decode_stream_pointer_offsets(
    overlay: &[u8],
    base: u32,
    pointer_table_offset: usize,
    pointer_count: usize,
    arena_offset: usize,
    arena_end: usize,
    role: &str,
) -> Result<Vec<usize>> {
    ensure!(pointer_count > 0, "{role} pointer table is empty");
    let mut offsets = Vec::with_capacity(pointer_count);
    for index in 0..pointer_count {
        let pointer_offset = pointer_table_offset
            .checked_add(
                index
                    .checked_mul(4)
                    .context("stream pointer index overflow")?,
            )
            .context("stream pointer offset overflow")?;
        let pointer = u32::from_le_bytes(
            overlay[pointer_offset..pointer_offset + 4]
                .try_into()
                .expect("validated stream pointer table"),
        );
        let offset = usize::try_from(
            pointer
                .checked_sub(base)
                .with_context(|| format!("{role} pointer {index} precedes its overlay"))?,
        )?;
        ensure!(
            offset >= arena_offset && offset < arena_end,
            "{role} pointer {index} escapes its hashed stream arena"
        );
        offsets.push(offset);
    }
    ensure!(
        offsets.first() == Some(&arena_offset) && offsets.windows(2).all(|pair| pair[0] < pair[1]),
        "{role} pointers no longer partition their stream arena"
    );
    Ok(offsets)
}

fn decode_static_stream_storage(
    overlay: &[u8],
    stream_offsets: &[usize],
    arena_end: usize,
) -> Result<Vec<DecodedStaticTileStream>> {
    stream_offsets
        .iter()
        .enumerate()
        .map(|(index, offset)| {
            let end = stream_offsets.get(index + 1).copied().unwrap_or(arena_end);
            decode_static_tile_stream(&overlay[*offset..end])
                .with_context(|| format!("static source tile stream {index} is invalid"))
        })
        .collect()
}

fn validate_derived_tile_bounds(
    streams: &[DecodedStaticTileStream],
    expected_min: usize,
    expected_max: usize,
    role: &str,
) -> Result<()> {
    let tile_ids = streams
        .iter()
        .flat_map(DecodedStaticTileStream::tile_ids)
        .collect::<BTreeSet<_>>();
    ensure!(
        tile_ids.first().copied() == Some(expected_min)
            && tile_ids.last().copied() == Some(expected_max),
        "{role} derived tile bounds changed"
    );
    Ok(())
}

pub(super) fn validate_dynamic_config_graph(
    overlay: &[u8],
    base: u32,
    evidence: &PracticalResultDynamicConfigGraphEvidence,
    counts: &mut ValidationCounts,
) -> Result<DynamicConfigGraph> {
    let root_pointer_offset = address_and_span(
        overlay,
        base,
        &evidence.header_root_table_pointer_offset,
        &evidence.header_root_table_pointer_runtime_address,
        4,
        &evidence.header_root_table_pointer_sha256,
        "dynamic graph header root-table pointer",
        counts,
    )?;
    let root_table_offset = address_and_span(
        overlay,
        base,
        &evidence.root_table_offset,
        &evidence.root_table_runtime_address,
        evidence.root_table_size,
        &evidence.root_table_sha256,
        "dynamic graph root table",
        counts,
    )?;
    ensure_pointer_value(
        overlay,
        root_pointer_offset,
        parse_hex_u32(
            &evidence.header_root_table_pointer_value,
            "dynamic graph root-table pointer value",
        )?,
        "dynamic graph root-table pointer",
    )?;
    ensure!(
        evidence.header_root_table_pointer_value == evidence.root_table_runtime_address,
        "dynamic graph header pointer does not name its root table"
    );
    let graph_span_offset = address_and_span(
        overlay,
        base,
        &evidence.graph_span_offset,
        &evidence.graph_span_runtime_address,
        evidence.graph_span_size,
        &evidence.graph_span_sha256,
        "dynamic config graph",
        counts,
    )?;
    let graph_span_end = checked_end(
        graph_span_offset,
        evidence.graph_span_size,
        "dynamic config graph",
    )?;
    let root_table_size = evidence
        .root_count
        .checked_mul(4)
        .context("dynamic graph root-table size overflow")?;
    let selector_cell_count = evidence
        .selector_grid
        .row_count
        .checked_mul(evidence.selector_grid.column_count)
        .context("dynamic selector-grid population overflow")?;
    ensure!(
        evidence.root_count > 0
            && evidence.root_table_size == root_table_size
            && evidence.selector_grid.row_count > 0
            && evidence.selector_grid.column_count > 0
            && evidence.selector_grid.storage_size >= selector_cell_count
            && matches!(
                evidence.stream_encoding,
                PracticalResultDynamicStreamEncoding::CountedGroupsWithOptionalFfRowSeparator
            )
            && graph_span_offset <= root_table_offset
            && checked_end(
                root_table_offset,
                evidence.root_table_size,
                "dynamic graph root table"
            )? == graph_span_end,
        "dynamic config graph denominator or bounds changed"
    );
    for index in 0..evidence.root_count {
        let pointer_offset = root_table_offset
            .checked_add(
                index
                    .checked_mul(4)
                    .context("dynamic root index overflow")?,
            )
            .context("dynamic root pointer offset overflow")?;
        let pointer = u32::from_le_bytes(
            overlay[pointer_offset..pointer_offset + 4]
                .try_into()
                .expect("validated dynamic root pointer"),
        );
        let offset = usize::try_from(
            pointer
                .checked_sub(base)
                .with_context(|| format!("dynamic graph root {index} precedes its overlay"))?,
        )?;
        ensure!(
            offset >= graph_span_offset && offset < root_table_offset,
            "dynamic graph root {index} escapes its hashed graph objects"
        );
    }
    parse_dynamic_config_graph(
        overlay,
        DynamicConfigGraphLayout {
            runtime_base: base,
            graph_offset: graph_span_offset,
            graph_size: evidence.graph_span_size,
            root_table_offset,
            root_count: evidence.root_count,
            selector_row_count: evidence.selector_grid.row_count,
            selector_column_count: evidence.selector_grid.column_count,
            mapping_storage_size: evidence.selector_grid.storage_size,
        },
    )
    .context("dynamic source config graph is invalid")
}
