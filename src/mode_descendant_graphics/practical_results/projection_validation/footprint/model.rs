use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TileRun {
    pub(crate) start_tile_id: usize,
    pub(crate) tile_count: usize,
    pub(crate) starts_new_row: bool,
}

impl TileRun {
    pub(crate) fn tile_ids(self) -> std::ops::Range<usize> {
        self.start_tile_id..self.start_tile_id + self.tile_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DecodedStaticTileStream {
    pub(crate) runs: Vec<TileRun>,
    pub(crate) encoded_size: usize,
    pub(crate) storage_size: usize,
}

impl DecodedStaticTileStream {
    pub(crate) fn tile_ids(&self) -> BTreeSet<usize> {
        self.runs.iter().flat_map(|run| run.tile_ids()).collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DecodedCountedTileStream {
    pub(crate) header: u8,
    pub(crate) runs: Vec<TileRun>,
    pub(crate) encoded_size: usize,
    pub(crate) storage_size: usize,
}

impl DecodedCountedTileStream {
    pub(crate) fn tile_ids(&self) -> BTreeSet<usize> {
        self.runs.iter().flat_map(|run| run.tile_ids()).collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DynamicConfigGraphLayout {
    pub(crate) runtime_base: u32,
    pub(crate) graph_offset: usize,
    pub(crate) graph_size: usize,
    pub(crate) root_table_offset: usize,
    pub(crate) root_count: usize,
    pub(crate) selector_row_count: usize,
    pub(crate) selector_column_count: usize,
    pub(crate) mapping_storage_size: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DynamicStreamPointerTarget {
    pub(crate) source_offset: usize,
    pub(crate) decoded_stream: DecodedCountedTileStream,
}

impl DynamicStreamPointerTarget {
    pub(crate) fn tile_ids(&self) -> BTreeSet<usize> {
        self.decoded_stream.tile_ids()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DynamicConfig {
    pub(crate) root_offset: usize,
    pub(crate) mapping_offset: usize,
    pub(crate) selector_stream_indices: Vec<usize>,
    pub(crate) stream_pointer_targets: Vec<DynamicStreamPointerTarget>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DynamicConfigGraph {
    pub(crate) graph_offset: usize,
    pub(crate) graph_size: usize,
    pub(crate) root_table_offset: usize,
    pub(crate) configs: Vec<DynamicConfig>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DynamicStreamStateHeader {
    pub(crate) source_offset: usize,
    pub(crate) value: u8,
    pub(crate) selector_referenced: bool,
}

impl DynamicConfigGraph {
    pub(crate) fn stream_pointer_target_count(&self) -> usize {
        self.configs
            .iter()
            .map(|config| config.stream_pointer_targets.len())
            .sum()
    }

    pub(crate) fn unique_stream_pointer_target_count(&self) -> usize {
        self.unique_stream_pointer_target_offsets().len()
    }

    pub(crate) fn unique_stream_pointer_target_offsets(&self) -> BTreeSet<usize> {
        self.configs
            .iter()
            .flat_map(|config| &config.stream_pointer_targets)
            .map(|target| target.source_offset)
            .collect::<BTreeSet<_>>()
    }

    pub(crate) fn selector_referenced_unique_stream_pointer_target_count(&self) -> usize {
        self.selector_referenced_unique_stream_pointer_target_offsets()
            .len()
    }

    pub(crate) fn selector_referenced_unique_stream_pointer_target_offsets(
        &self,
    ) -> BTreeSet<usize> {
        self.configs
            .iter()
            .flat_map(|config| {
                config
                    .selector_stream_indices
                    .iter()
                    .copied()
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .map(|stream_index| config.stream_pointer_targets[stream_index].source_offset)
            })
            .collect::<BTreeSet<_>>()
    }

    pub(crate) fn decoded_tile_ids(&self) -> BTreeSet<usize> {
        self.configs
            .iter()
            .flat_map(|config| &config.stream_pointer_targets)
            .flat_map(DynamicStreamPointerTarget::tile_ids)
            .collect()
    }

    pub(crate) fn selector_referenced_tile_ids(&self) -> BTreeSet<usize> {
        self.configs
            .iter()
            .flat_map(|config| {
                config
                    .selector_stream_indices
                    .iter()
                    .copied()
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .map(|stream_index| &config.stream_pointer_targets[stream_index])
            })
            .flat_map(DynamicStreamPointerTarget::tile_ids)
            .collect()
    }

    pub(crate) fn stream_state_headers(&self) -> Vec<DynamicStreamStateHeader> {
        let selector_referenced_offsets =
            self.selector_referenced_unique_stream_pointer_target_offsets();
        self.configs
            .iter()
            .flat_map(|config| &config.stream_pointer_targets)
            .map(|target| (target.source_offset, target.decoded_stream.header))
            .collect::<BTreeMap<_, _>>()
            .into_iter()
            .map(|(source_offset, value)| DynamicStreamStateHeader {
                source_offset,
                value,
                selector_referenced: selector_referenced_offsets.contains(&source_offset),
            })
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SourceAtlasTileGeometry {
    pub(crate) atlas_width: usize,
    pub(crate) atlas_height: usize,
    pub(crate) tile_width: usize,
    pub(crate) tile_height: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct SourceAtlasRectangle {
    pub(crate) x: usize,
    pub(crate) y: usize,
    pub(crate) width: usize,
    pub(crate) height: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SourceReadSetAssessment {
    #[cfg_attr(not(test), allow(dead_code))]
    InputSetUnassessed,
    DeclaredReadSetComplete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SourceConsumerReachability {
    #[cfg_attr(not(test), allow(dead_code))]
    Unassessed,
    #[cfg_attr(not(test), allow(dead_code))]
    RuntimeRootSelectionUnresolved,
    DormantOutsideDeclaredEntrypoints,
    #[cfg_attr(not(test), allow(dead_code))]
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SourceReadFootprintDerivation {
    CatalogDescriptorFragments,
    CatalogDigitDescriptorTable,
    DirectNumericSpriteSites,
    DirectSpriteSelectorTable,
    CatalogDescriptorSelectorTable,
    NumericLookupTable,
    StaticTileStreams,
    DynamicConfigGraph,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SourceProjectionReadPopulation {
    DeclaredRectangles {
        declared_rectangle_count: usize,
        unique_rectangle_count: usize,
    },
    StaticDecodedStreams {
        decoded_stream_count: usize,
    },
    DynamicRootArrayTargets {
        pointer_target_count: usize,
        unique_pointer_target_count: usize,
        selector_referenced_unique_pointer_target_count: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SourceProjectionFootprint<ProjectionId, SourceAtlasDomainId> {
    pub(crate) projection_id: ProjectionId,
    pub(crate) source_atlas_domain_id: SourceAtlasDomainId,
    pub(crate) tile_geometry: Option<SourceAtlasTileGeometry>,
    pub(crate) derivation: SourceReadFootprintDerivation,
    pub(crate) read_population: SourceProjectionReadPopulation,
    pub(crate) decoded_tile_ids: Option<BTreeSet<usize>>,
    pub(crate) selector_referenced_tile_ids: Option<BTreeSet<usize>>,
    pub(crate) structural_stream_source_offsets: Option<BTreeSet<usize>>,
    pub(crate) selector_referenced_stream_source_offsets: Option<BTreeSet<usize>>,
    pub(crate) dynamic_stream_state_headers: Option<Vec<DynamicStreamStateHeader>>,
    pub(crate) source_read_rectangles: Vec<SourceAtlasRectangle>,
    pub(crate) selector_referenced_rectangles: Option<Vec<SourceAtlasRectangle>>,
    pub(crate) source_read_set_assessment: SourceReadSetAssessment,
    pub(crate) consumer_reachability: SourceConsumerReachability,
}
