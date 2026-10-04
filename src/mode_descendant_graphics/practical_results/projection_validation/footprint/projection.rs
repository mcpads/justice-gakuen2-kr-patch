use std::collections::BTreeSet;

use super::error::SourceFootprintError;
use super::model::{
    DecodedStaticTileStream, DynamicConfigGraph, SourceAtlasRectangle, SourceAtlasTileGeometry,
    SourceConsumerReachability, SourceProjectionFootprint, SourceProjectionReadPopulation,
    SourceReadFootprintDerivation, SourceReadSetAssessment,
};
use super::rectangles::tile_ids_to_atlas_rectangles;

pub(crate) fn derive_static_source_footprint<ProjectionId, SourceAtlasDomainId>(
    projection_id: ProjectionId,
    source_atlas_domain_id: SourceAtlasDomainId,
    streams: &[DecodedStaticTileStream],
    geometry: SourceAtlasTileGeometry,
    consumer_reachability: SourceConsumerReachability,
) -> Result<SourceProjectionFootprint<ProjectionId, SourceAtlasDomainId>, SourceFootprintError> {
    if streams.is_empty() {
        return Err(SourceFootprintError::ProjectionHasNoReads);
    }
    let tile_ids = streams
        .iter()
        .flat_map(DecodedStaticTileStream::tile_ids)
        .collect::<BTreeSet<_>>();
    let rectangles = tile_ids_to_atlas_rectangles(&tile_ids, geometry)?;

    Ok(SourceProjectionFootprint {
        projection_id,
        source_atlas_domain_id,
        tile_geometry: Some(geometry),
        derivation: SourceReadFootprintDerivation::StaticTileStreams,
        read_population: SourceProjectionReadPopulation::StaticDecodedStreams {
            decoded_stream_count: streams.len(),
        },
        decoded_tile_ids: Some(tile_ids),
        selector_referenced_tile_ids: None,
        structural_stream_source_offsets: None,
        selector_referenced_stream_source_offsets: None,
        dynamic_stream_state_headers: None,
        source_read_rectangles: rectangles,
        selector_referenced_rectangles: None,
        source_read_set_assessment: SourceReadSetAssessment::DeclaredReadSetComplete,
        consumer_reachability,
    })
}

pub(crate) fn derive_dynamic_source_footprint<ProjectionId, SourceAtlasDomainId>(
    projection_id: ProjectionId,
    source_atlas_domain_id: SourceAtlasDomainId,
    graph: &DynamicConfigGraph,
    geometry: SourceAtlasTileGeometry,
    consumer_reachability: SourceConsumerReachability,
) -> Result<SourceProjectionFootprint<ProjectionId, SourceAtlasDomainId>, SourceFootprintError> {
    let pointer_target_count = graph.stream_pointer_target_count();
    if pointer_target_count == 0 {
        return Err(SourceFootprintError::ProjectionHasNoReads);
    }

    let decoded_tile_ids = graph.decoded_tile_ids();
    let selector_referenced_tile_ids = graph.selector_referenced_tile_ids();
    let decoded_rectangles = tile_ids_to_atlas_rectangles(&decoded_tile_ids, geometry)?;
    let selector_referenced_rectangles =
        tile_ids_to_atlas_rectangles(&selector_referenced_tile_ids, geometry)?;

    Ok(SourceProjectionFootprint {
        projection_id,
        source_atlas_domain_id,
        tile_geometry: Some(geometry),
        derivation: SourceReadFootprintDerivation::DynamicConfigGraph,
        read_population: SourceProjectionReadPopulation::DynamicRootArrayTargets {
            pointer_target_count,
            unique_pointer_target_count: graph.unique_stream_pointer_target_count(),
            selector_referenced_unique_pointer_target_count: graph
                .selector_referenced_unique_stream_pointer_target_count(),
        },
        decoded_tile_ids: Some(decoded_tile_ids),
        selector_referenced_tile_ids: Some(selector_referenced_tile_ids),
        structural_stream_source_offsets: Some(graph.unique_stream_pointer_target_offsets()),
        selector_referenced_stream_source_offsets: Some(
            graph.selector_referenced_unique_stream_pointer_target_offsets(),
        ),
        dynamic_stream_state_headers: Some(graph.stream_state_headers()),
        source_read_rectangles: decoded_rectangles,
        selector_referenced_rectangles: Some(selector_referenced_rectangles),
        source_read_set_assessment: SourceReadSetAssessment::DeclaredReadSetComplete,
        consumer_reachability,
    })
}

pub(crate) fn derive_declared_rectangle_source_footprint<ProjectionId, SourceAtlasDomainId>(
    projection_id: ProjectionId,
    source_atlas_domain_id: SourceAtlasDomainId,
    rectangles: impl IntoIterator<Item = SourceAtlasRectangle>,
    derivation: SourceReadFootprintDerivation,
    atlas_width: usize,
    atlas_height: usize,
    consumer_reachability: SourceConsumerReachability,
) -> Result<SourceProjectionFootprint<ProjectionId, SourceAtlasDomainId>, SourceFootprintError> {
    let rectangles = rectangles.into_iter().collect::<Vec<_>>();
    if rectangles.is_empty() {
        return Err(SourceFootprintError::ProjectionHasNoReads);
    }
    if rectangles.iter().any(|rectangle| {
        rectangle.width == 0
            || rectangle.height == 0
            || rectangle
                .x
                .checked_add(rectangle.width)
                .is_none_or(|end| end > atlas_width)
            || rectangle
                .y
                .checked_add(rectangle.height)
                .is_none_or(|end| end > atlas_height)
    }) {
        return Err(SourceFootprintError::InvalidSourceAtlasRectangle {
            detail: "declared source-read rectangle escapes its atlas",
        });
    }
    let declared_rectangle_count = rectangles.len();
    let source_read_rectangles = rectangles.into_iter().collect::<BTreeSet<_>>();
    let unique_rectangle_count = source_read_rectangles.len();

    Ok(SourceProjectionFootprint {
        projection_id,
        source_atlas_domain_id,
        tile_geometry: None,
        derivation,
        read_population: SourceProjectionReadPopulation::DeclaredRectangles {
            declared_rectangle_count,
            unique_rectangle_count,
        },
        decoded_tile_ids: None,
        selector_referenced_tile_ids: None,
        structural_stream_source_offsets: None,
        selector_referenced_stream_source_offsets: None,
        dynamic_stream_state_headers: None,
        source_read_rectangles: source_read_rectangles.into_iter().collect(),
        selector_referenced_rectangles: None,
        source_read_set_assessment: SourceReadSetAssessment::DeclaredReadSetComplete,
        consumer_reachability,
    })
}
