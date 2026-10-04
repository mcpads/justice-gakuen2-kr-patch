use std::collections::BTreeSet;

use super::dynamic_stream::decode_counted_dynamic_tile_stream;
use super::error::SourceFootprintError;
use super::rectangles::tile_ids_to_atlas_rectangles;
use super::*;

const SOURCE_TILE_GEOMETRY: SourceAtlasTileGeometry = SourceAtlasTileGeometry {
    atlas_width: 256,
    atlas_height: 256,
    tile_width: 32,
    tile_height: 16,
};

#[test]
fn static_stream_decodes_ranges_rows_and_zero_padding() {
    let stream = decode_static_tile_stream(&[
        0x08, 0x09, 0xfe, 0x11, 0x07, 0xfe, 0x18, 0x0d, 0xff, 0x00, 0x00,
    ])
    .unwrap();

    assert_eq!(stream.encoded_size, 9);
    assert_eq!(stream.storage_size, 11);
    assert_eq!(stream.runs.len(), 3);
    assert!(!stream.runs[0].starts_new_row);
    assert!(stream.runs[1].starts_new_row);
    assert_eq!(stream.tile_ids(), (8..=36).collect());
}

#[test]
fn static_stream_rejects_missing_terminator_and_nonzero_padding() {
    assert_eq!(
        decode_static_tile_stream(&[8, 2]).unwrap_err(),
        SourceFootprintError::MissingStaticTerminator
    );
    assert_eq!(
        decode_static_tile_stream(&[8, 2, 0xff, 1]).unwrap_err(),
        SourceFootprintError::NonZeroStreamPadding {
            encoding: "static",
            offset: 3,
        }
    );
}

#[test]
fn static_projection_derives_union_instead_of_accepting_authored_tiles() {
    let streams = [
        decode_static_tile_stream(&[
            0x08, 0x09, 0xfe, 0x11, 0x07, 0xfe, 0x18, 0x0d, 0xfe, 0x25, 0x09, 0xfe, 0x2e, 0x08,
            0xff,
        ])
        .unwrap(),
        decode_static_tile_stream(&[
            0x08, 0x08, 0xfe, 0x10, 0x09, 0xfe, 0x19, 0x0c, 0xfe, 0x25, 0x0c, 0xff,
        ])
        .unwrap(),
        decode_static_tile_stream(&[
            0x08, 0x0a, 0xfe, 0x12, 0x09, 0xfe, 0x1b, 0x0b, 0xfe, 0x26, 0x0b, 0xfe, 0x31, 0x08,
            0xff,
        ])
        .unwrap(),
    ];

    let footprint: SourceProjectionFootprint<&str, &str> = derive_static_source_footprint(
        "fixture_static_projection",
        "fixture_source_atlas",
        &streams,
        SOURCE_TILE_GEOMETRY,
        SourceConsumerReachability::Closed,
    )
    .unwrap();

    assert_eq!(
        footprint.read_population,
        SourceProjectionReadPopulation::StaticDecodedStreams {
            decoded_stream_count: 3,
        }
    );
    assert_eq!(footprint.decoded_tile_ids, Some((8..=56).collect()));
    assert_eq!(footprint.selector_referenced_tile_ids, None);
    assert_eq!(
        footprint.source_read_set_assessment,
        SourceReadSetAssessment::DeclaredReadSetComplete
    );
    assert_eq!(
        footprint.consumer_reachability,
        SourceConsumerReachability::Closed
    );
    assert_eq!(
        footprint.source_read_rectangles,
        [
            SourceAtlasRectangle {
                x: 0,
                y: 16,
                width: 256,
                height: 96,
            },
            SourceAtlasRectangle {
                x: 0,
                y: 112,
                width: 32,
                height: 16,
            },
        ]
    );
}

#[test]
fn counted_dynamic_stream_honors_group_count_and_row_separator() {
    let stream = decode_counted_dynamic_tile_stream(&[7, 2, 0xff, 30, 2, 40, 1, 0]).unwrap();

    assert_eq!(stream.header, 7);
    assert_eq!(stream.encoded_size, 7);
    assert_eq!(stream.storage_size, 8);
    assert!(stream.runs[0].starts_new_row);
    assert!(!stream.runs[1].starts_new_row);
    assert_eq!(stream.tile_ids(), BTreeSet::from([30, 31, 40]));
}

#[test]
fn counted_dynamic_stream_accepts_an_empty_draw_list() {
    let stream = decode_counted_dynamic_tile_stream(&[0x27, 0, 0, 0]).unwrap();

    assert_eq!(stream.header, 0x27);
    assert!(stream.runs.is_empty());
    assert!(stream.tile_ids().is_empty());
    assert_eq!(stream.encoded_size, 2);
}

#[test]
fn dynamic_graph_recovers_mapping_and_every_declared_stream() {
    let (overlay, layout) = dynamic_graph_fixture();
    let graph = parse_dynamic_config_graph(&overlay, layout).unwrap();

    assert_eq!(graph.configs.len(), 2);
    assert_eq!(graph.stream_pointer_target_count(), 3);
    assert_eq!(graph.unique_stream_pointer_target_count(), 3);
    assert_eq!(
        graph.selector_referenced_unique_stream_pointer_target_count(),
        2
    );
    assert_eq!(graph.configs[0].selector_stream_indices, [0, 0, 0]);
    assert_eq!(graph.configs[1].selector_stream_indices, [0, 0, 0]);
    assert_eq!(
        graph.unique_stream_pointer_target_offsets(),
        BTreeSet::from([0x24, 0x28, 0x3c])
    );
    assert_eq!(
        graph.selector_referenced_unique_stream_pointer_target_offsets(),
        BTreeSet::from([0x24, 0x3c])
    );
    assert_eq!(
        graph.decoded_tile_ids(),
        BTreeSet::from([10, 11, 20, 30, 31, 40])
    );
    assert_eq!(
        graph.selector_referenced_tile_ids(),
        BTreeSet::from([10, 11, 30, 31, 40])
    );

    let footprint = derive_dynamic_source_footprint(
        "fixture_dynamic_projection",
        "fixture_source_atlas",
        &graph,
        SourceAtlasTileGeometry {
            atlas_width: 64,
            atlas_height: 64,
            tile_width: 8,
            tile_height: 8,
        },
        SourceConsumerReachability::DormantOutsideDeclaredEntrypoints,
    )
    .unwrap();
    assert_eq!(
        footprint.read_population,
        SourceProjectionReadPopulation::DynamicRootArrayTargets {
            pointer_target_count: 3,
            unique_pointer_target_count: 3,
            selector_referenced_unique_pointer_target_count: 2,
        }
    );
    assert_eq!(
        footprint.source_read_set_assessment,
        SourceReadSetAssessment::DeclaredReadSetComplete
    );
    assert_eq!(
        footprint.consumer_reachability,
        SourceConsumerReachability::DormantOutsideDeclaredEntrypoints
    );
    assert_eq!(
        footprint.source_read_rectangles,
        [
            SourceAtlasRectangle {
                x: 16,
                y: 8,
                width: 16,
                height: 8,
            },
            SourceAtlasRectangle {
                x: 32,
                y: 16,
                width: 8,
                height: 8,
            },
            SourceAtlasRectangle {
                x: 48,
                y: 24,
                width: 16,
                height: 8,
            },
            SourceAtlasRectangle {
                x: 0,
                y: 40,
                width: 8,
                height: 8,
            },
        ]
    );
    assert_eq!(
        footprint.selector_referenced_rectangles.unwrap(),
        [
            SourceAtlasRectangle {
                x: 16,
                y: 8,
                width: 16,
                height: 8,
            },
            SourceAtlasRectangle {
                x: 48,
                y: 24,
                width: 16,
                height: 8,
            },
            SourceAtlasRectangle {
                x: 0,
                y: 40,
                width: 8,
                height: 8,
            },
        ]
    );
}

#[test]
fn declared_rectangle_projection_preserves_reads_without_inventing_tiles() {
    let repeated = SourceAtlasRectangle {
        x: 8,
        y: 16,
        width: 24,
        height: 12,
    };
    let footprint = derive_declared_rectangle_source_footprint(
        "fixture_rectangle_projection",
        "fixture_source_atlas",
        [repeated, repeated],
        SourceReadFootprintDerivation::DirectSpriteSelectorTable,
        64,
        64,
        SourceConsumerReachability::Closed,
    )
    .unwrap();

    assert_eq!(
        footprint.read_population,
        SourceProjectionReadPopulation::DeclaredRectangles {
            declared_rectangle_count: 2,
            unique_rectangle_count: 1,
        }
    );
    assert_eq!(footprint.tile_geometry, None);
    assert_eq!(footprint.decoded_tile_ids, None);
    assert_eq!(footprint.source_read_rectangles, [repeated]);
}

#[test]
fn dynamic_graph_rejects_invalid_topology() {
    let (root_table_not_terminal, mut layout) = dynamic_graph_fixture();
    layout.root_table_offset -= 4;
    assert_eq!(
        parse_dynamic_config_graph(&root_table_not_terminal, layout).unwrap_err(),
        SourceFootprintError::RootTableDoesNotEndGraph
    );

    let (mut pointer_escape, layout) = dynamic_graph_fixture();
    write_pointer(
        &mut pointer_escape,
        layout.root_table_offset,
        layout.runtime_base + (layout.graph_offset + layout.graph_size) as u32,
    );
    assert!(matches!(
        parse_dynamic_config_graph(&pointer_escape, layout),
        Err(SourceFootprintError::PointerOutsideGraph { .. })
    ));

    let (mut missing_stream, layout) = dynamic_graph_fixture();
    missing_stream[layout.graph_offset] = 2;
    assert_eq!(
        parse_dynamic_config_graph(&missing_stream, layout).unwrap_err(),
        SourceFootprintError::SelectorReferencesMissingStream {
            config_index: 0,
            selector_index: 0,
            stream_index: 2,
            stream_count: 2,
        }
    );
}

#[test]
fn dynamic_graph_stream_error_names_its_config_and_absolute_offset() {
    let (mut overlay, layout) = dynamic_graph_fixture();
    overlay[0x25] = 2;

    assert_eq!(
        parse_dynamic_config_graph(&overlay, layout).unwrap_err(),
        SourceFootprintError::DynamicConfigStream {
            config_index: 0,
            stream_index: 0,
            stream_offset: 0x24,
            source: Box::new(SourceFootprintError::TruncatedTileRun {
                encoding: "counted dynamic",
                offset: 4,
            }),
        }
    );
}

#[test]
fn tile_rectangles_merge_identical_runs_vertically_in_row_major_order() {
    let tile_ids = BTreeSet::from([0, 1, 3, 4, 5, 7, 11]);

    assert_eq!(
        tile_ids_to_atlas_rectangles(
            &tile_ids,
            SourceAtlasTileGeometry {
                atlas_width: 32,
                atlas_height: 32,
                tile_width: 8,
                tile_height: 8,
            },
        )
        .unwrap(),
        [
            SourceAtlasRectangle {
                x: 0,
                y: 0,
                width: 16,
                height: 16,
            },
            SourceAtlasRectangle {
                x: 24,
                y: 0,
                width: 8,
                height: 24,
            },
        ]
    );
}

fn dynamic_graph_fixture() -> (Vec<u8>, DynamicConfigGraphLayout) {
    const BASE: u32 = 0x8000;
    const GRAPH_OFFSET: usize = 0x20;
    const MAPPING_0: usize = 0x20;
    const STREAM_0_0: usize = 0x24;
    const STREAM_0_1: usize = 0x28;
    const ROOT_0: usize = 0x2c;
    const MAPPING_1: usize = 0x38;
    const STREAM_1_0: usize = 0x3c;
    const ROOT_1: usize = 0x44;
    const ROOT_TABLE: usize = 0x4c;
    const GRAPH_END: usize = 0x54;

    let mut overlay = vec![0; GRAPH_END];
    overlay[MAPPING_0..MAPPING_0 + 4].copy_from_slice(&[0, 0, 0, 0]);
    overlay[STREAM_0_0..STREAM_0_0 + 4].copy_from_slice(&[0, 1, 10, 2]);
    overlay[STREAM_0_1..STREAM_0_1 + 4].copy_from_slice(&[1, 1, 20, 1]);
    write_pointer(&mut overlay, ROOT_0, BASE + MAPPING_0 as u32);
    write_pointer(&mut overlay, ROOT_0 + 4, BASE + STREAM_0_0 as u32);
    write_pointer(&mut overlay, ROOT_0 + 8, BASE + STREAM_0_1 as u32);

    overlay[MAPPING_1..MAPPING_1 + 4].copy_from_slice(&[0, 0, 0, 0]);
    overlay[STREAM_1_0..STREAM_1_0 + 8].copy_from_slice(&[2, 2, 0xff, 30, 2, 40, 1, 0]);
    write_pointer(&mut overlay, ROOT_1, BASE + MAPPING_1 as u32);
    write_pointer(&mut overlay, ROOT_1 + 4, BASE + STREAM_1_0 as u32);

    write_pointer(&mut overlay, ROOT_TABLE, BASE + ROOT_0 as u32);
    write_pointer(&mut overlay, ROOT_TABLE + 4, BASE + ROOT_1 as u32);

    (
        overlay,
        DynamicConfigGraphLayout {
            runtime_base: BASE,
            graph_offset: GRAPH_OFFSET,
            graph_size: GRAPH_END - GRAPH_OFFSET,
            root_table_offset: ROOT_TABLE,
            root_count: 2,
            selector_row_count: 1,
            selector_column_count: 3,
            mapping_storage_size: 4,
        },
    )
}

fn write_pointer(bytes: &mut [u8], offset: usize, pointer: u32) {
    bytes[offset..offset + 4].copy_from_slice(&pointer.to_le_bytes());
}
