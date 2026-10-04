//! Derives source-atlas occupancy from encoded consumer data.
//!
//! These types describe JP read-side evidence only. They do not allocate a
//! Korean destination cell or authorize a write.

mod dynamic_graph;
mod dynamic_stream;
mod error;
mod model;
mod projection;
mod rectangles;
mod static_stream;

pub(super) use dynamic_graph::parse_dynamic_config_graph;
pub(super) use model::{
    DecodedStaticTileStream, DynamicConfigGraph, DynamicConfigGraphLayout, SourceAtlasRectangle,
    SourceAtlasTileGeometry, SourceConsumerReachability, SourceProjectionFootprint,
    SourceProjectionReadPopulation, SourceReadFootprintDerivation, SourceReadSetAssessment,
};
pub(super) use projection::{
    derive_declared_rectangle_source_footprint, derive_dynamic_source_footprint,
    derive_static_source_footprint,
};
pub(super) use static_stream::decode_static_tile_stream;

#[cfg(test)]
mod tests;
