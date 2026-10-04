use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SourceFootprintError {
    ProjectionHasNoReads,
    StaticStreamHasNoRuns,
    MissingStaticTerminator,
    StaticTerminatorAfterRowSeparator {
        offset: usize,
    },
    UnexpectedStaticRowSeparator {
        offset: usize,
    },
    TruncatedTileRun {
        encoding: &'static str,
        offset: usize,
    },
    ZeroTileCount {
        encoding: &'static str,
        offset: usize,
    },
    TileRangeOverflow {
        encoding: &'static str,
        offset: usize,
        start_tile_id: usize,
        tile_count: usize,
    },
    NonZeroStreamPadding {
        encoding: &'static str,
        offset: usize,
    },
    TruncatedDynamicHeader,
    InvalidSourceAtlasTileGeometry {
        detail: &'static str,
    },
    InvalidSourceAtlasRectangle {
        detail: &'static str,
    },
    TileOutsideAtlas {
        tile_id: usize,
        tile_capacity: usize,
    },
    CoordinateOverflow,
    InvalidGraphSpan,
    RootTableDoesNotEndGraph,
    InvalidSelectorGrid,
    TruncatedGraphWord {
        offset: usize,
    },
    PointerBelowRuntimeBase {
        pointer: u32,
        runtime_base: u32,
    },
    UnalignedGraphPointer {
        pointer: u32,
    },
    PointerOutsideGraph {
        pointer: u32,
        graph_offset: usize,
        graph_end: usize,
    },
    InvalidGraphLayout {
        config_index: Option<usize>,
        detail: &'static str,
    },
    NonZeroMappingPadding {
        config_index: usize,
        offset: usize,
    },
    SelectorReferencesMissingStream {
        config_index: usize,
        selector_index: usize,
        stream_index: usize,
        stream_count: usize,
    },
    DynamicConfigStream {
        config_index: usize,
        stream_index: usize,
        stream_offset: usize,
        source: Box<SourceFootprintError>,
    },
}

impl fmt::Display for SourceFootprintError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProjectionHasNoReads => {
                formatter.write_str("projection footprint has no source reads")
            }
            Self::StaticStreamHasNoRuns => {
                formatter.write_str("static tile stream has no tile runs")
            }
            Self::MissingStaticTerminator => {
                formatter.write_str("static tile stream has no 0xff terminator")
            }
            Self::StaticTerminatorAfterRowSeparator { offset } => write!(
                formatter,
                "static tile stream terminates immediately after a row separator at byte {offset:#x}"
            ),
            Self::UnexpectedStaticRowSeparator { offset } => write!(
                formatter,
                "static tile stream has an unexpected 0xfe row separator at byte {offset:#x}"
            ),
            Self::TruncatedTileRun { encoding, offset } => write!(
                formatter,
                "{encoding} tile run is truncated at byte {offset:#x}"
            ),
            Self::ZeroTileCount { encoding, offset } => write!(
                formatter,
                "{encoding} tile run has zero tiles at byte {offset:#x}"
            ),
            Self::TileRangeOverflow {
                encoding,
                offset,
                start_tile_id,
                tile_count,
            } => write!(
                formatter,
                "{encoding} tile run at byte {offset:#x} exceeds the u8 tile-ID space: start {start_tile_id}, count {tile_count}"
            ),
            Self::NonZeroStreamPadding { encoding, offset } => write!(
                formatter,
                "{encoding} tile stream has nonzero padding at byte {offset:#x}"
            ),
            Self::TruncatedDynamicHeader => {
                formatter.write_str("counted dynamic tile stream has a truncated header")
            }
            Self::InvalidSourceAtlasTileGeometry { detail } => {
                write!(formatter, "invalid atlas tile geometry: {detail}")
            }
            Self::InvalidSourceAtlasRectangle { detail } => {
                write!(formatter, "invalid source-atlas rectangle: {detail}")
            }
            Self::TileOutsideAtlas {
                tile_id,
                tile_capacity,
            } => write!(
                formatter,
                "tile ID {tile_id} escapes atlas capacity {tile_capacity}"
            ),
            Self::CoordinateOverflow => formatter.write_str("atlas rectangle coordinate overflow"),
            Self::InvalidGraphSpan => {
                formatter.write_str("dynamic config graph span escapes the overlay")
            }
            Self::RootTableDoesNotEndGraph => formatter
                .write_str("dynamic config root table does not end at the declared graph boundary"),
            Self::InvalidSelectorGrid => {
                formatter.write_str("dynamic config selector grid is empty or overflows")
            }
            Self::TruncatedGraphWord { offset } => {
                write!(
                    formatter,
                    "dynamic config graph word is truncated at {offset:#x}"
                )
            }
            Self::PointerBelowRuntimeBase {
                pointer,
                runtime_base,
            } => write!(
                formatter,
                "dynamic config pointer {pointer:#010x} is below runtime base {runtime_base:#010x}"
            ),
            Self::UnalignedGraphPointer { pointer } => write!(
                formatter,
                "dynamic config pointer {pointer:#010x} is not word-aligned"
            ),
            Self::PointerOutsideGraph {
                pointer,
                graph_offset,
                graph_end,
            } => write!(
                formatter,
                "dynamic config pointer {pointer:#010x} escapes graph {graph_offset:#x}..{graph_end:#x}"
            ),
            Self::InvalidGraphLayout {
                config_index,
                detail,
            } => match config_index {
                Some(index) => write!(
                    formatter,
                    "dynamic config {index} has invalid graph layout: {detail}"
                ),
                None => write!(formatter, "invalid dynamic config graph layout: {detail}"),
            },
            Self::NonZeroMappingPadding {
                config_index,
                offset,
            } => write!(
                formatter,
                "dynamic config {config_index} has nonzero mapping padding at {offset:#x}"
            ),
            Self::SelectorReferencesMissingStream {
                config_index,
                selector_index,
                stream_index,
                stream_count,
            } => write!(
                formatter,
                "dynamic config {config_index} selector {selector_index} references stream {stream_index}, but the config has {stream_count} streams"
            ),
            Self::DynamicConfigStream {
                config_index,
                stream_index,
                stream_offset,
                source,
            } => write!(
                formatter,
                "dynamic config {config_index} stream {stream_index} at {stream_offset:#x} is invalid: {source}"
            ),
        }
    }
}

impl std::error::Error for SourceFootprintError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::DynamicConfigStream { source, .. } => Some(source.as_ref()),
            _ => None,
        }
    }
}
