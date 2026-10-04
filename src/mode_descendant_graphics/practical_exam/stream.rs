use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};

use crate::pipeline::{difference_ranges, sha256_bytes};
use crate::tim::Cell;
use crate::write_scope::changed_ranges_are_within;

use super::descriptor::{PracticalExamConsumer, PracticalExamConsumerAudit};

const OVERLAY_RUNTIME_BASE: u32 = 0x800a_2000;
const TEXTURE_PAGE_WIDTH: usize = 256;
const TEXTURE_WIDTH: usize = 768;
const TEXTURE_HEIGHT: usize = 256;
const NORMAL_GLYPH_HEIGHT: usize = 20;
const TITLE_GLYPH_HEIGHT: usize = 27;
const MAX_INLINE_GLYPH_WIDTH: usize = 32;
const COORDINATE_ENTRY_SIZE: usize = 3;

const HINT_COMPOSITE_CELL: Cell = Cell {
    x: 512,
    y: 64,
    width: 104,
    height: 64,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DescriptorGlyphStream {
    pub(super) descriptor_index: usize,
    pub(super) codes: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct GlyphCoordinate {
    pub(super) cell: Cell,
    pub(super) title_height: bool,
}

pub(super) struct PracticalExamStreamPack {
    pub(super) patched_overlay: Vec<u8>,
    pub(super) coordinate_table_runtime_address: u32,
    pub(super) changed_ranges: Vec<[usize; 2]>,
    pub(super) owned_ranges: Vec<[usize; 2]>,
    pub(super) stream_size: usize,
    pub(super) coordinate_table_size: usize,
}

#[derive(Debug, Clone, Copy)]
struct StreamLayout {
    consumer: PracticalExamConsumer,
    source_sha256: &'static str,
    descriptor_arena_start: usize,
    descriptor_arena_end: usize,
    descriptor_arena_sha256: &'static str,
    pointer_table_offset: usize,
    descriptor_count: usize,
    pointer_table_sha256: &'static str,
}

const BASICS_LAYOUT: StreamLayout = StreamLayout {
    consumer: PracticalExamConsumer::BasicsReview,
    source_sha256: "18f31dbd4eab7f7011fc0e168e94b7fb123f149cfb3f68a9d4875d8cee6283d9",
    descriptor_arena_start: 0x00a4,
    descriptor_arena_end: 0x0420,
    descriptor_arena_sha256: "9b3554de840eb0b8a6109793645c477e3ed7fa7134ff30688e1ed852ecd15b03",
    pointer_table_offset: 0x0420,
    descriptor_count: 49,
    pointer_table_sha256: "b3375d17687e64de361dc6efd6660f46bb116a9126188ce8e605ad6998997425",
};

const EXAM_1999_LAYOUT: StreamLayout = StreamLayout {
    consumer: PracticalExamConsumer::Exam1999,
    source_sha256: "0b23fd0dc314143ad13c16bb97816384d64cda15f7e205863bed42f091190f10",
    descriptor_arena_start: 0x0158,
    descriptor_arena_end: 0x0210,
    descriptor_arena_sha256: "5050be74ecc65ecffb0609d632e41f48b970770b2552d3e3f9c7e8109b4ace43",
    pointer_table_offset: 0x0210,
    descriptor_count: 8,
    pointer_table_sha256: "f83999abc42b002579cb4f60f7dda055b3d0eaa5578fccade145f57991cf863e",
};

pub(super) fn pack_practical_exam_glyph_streams(
    consumer: PracticalExamConsumer,
    source_overlay: &[u8],
    source_audit: &PracticalExamConsumerAudit,
    descriptor_streams: &[DescriptorGlyphStream],
    coordinates: &[GlyphCoordinate],
) -> Result<PracticalExamStreamPack> {
    let layout = layout(consumer);
    ensure_source_matches_audit(source_overlay, source_audit, layout)?;

    ensure!(
        !coordinates.is_empty() && coordinates.len() <= usize::from(u8::MAX) + 1,
        "{consumer:?} practical-exam coordinate table must contain 1..=256 entries"
    );
    let encoded_coordinates = encode_coordinates(coordinates)?;
    let coordinate_table_size = encoded_coordinates.len();
    let coordinate_table_offset = layout
        .pointer_table_offset
        .checked_sub(coordinate_table_size)
        .context("practical-exam coordinate table does not fit before its pointer table")?;
    ensure!(
        coordinate_table_offset >= layout.descriptor_arena_start,
        "{consumer:?} practical-exam coordinate table begins before its descriptor arena"
    );

    let ordered_streams = order_and_validate_streams(
        descriptor_streams,
        layout.descriptor_count,
        coordinates.len(),
        consumer,
    )?;
    let stream_size = ordered_streams.iter().try_fold(0usize, |size, stream| {
        size.checked_add(1 + stream.codes.len())
            .context("practical-exam glyph stream size overflow")
    })?;
    let stream_end = layout
        .descriptor_arena_start
        .checked_add(stream_size)
        .context("practical-exam glyph stream end overflow")?;
    ensure!(
        stream_end <= coordinate_table_offset,
        "{consumer:?} practical-exam glyph streams ({stream_size} bytes) and coordinate table ({coordinate_table_size} bytes) exceed the {}-byte primary arena",
        layout.descriptor_arena_end - layout.descriptor_arena_start
    );

    let pointer_table_end = layout
        .pointer_table_offset
        .checked_add(layout.descriptor_count * 4)
        .context("practical-exam pointer table end overflow")?;
    ensure!(
        pointer_table_end <= source_overlay.len(),
        "{consumer:?} practical-exam pointer table is truncated"
    );
    let owned_ranges = vec![[layout.descriptor_arena_start, pointer_table_end]];
    let mut patched_overlay = source_overlay.to_vec();
    patched_overlay[layout.descriptor_arena_start..layout.descriptor_arena_end].fill(0);

    let mut stream_offset = layout.descriptor_arena_start;
    let mut expected_stream_offsets = Vec::with_capacity(layout.descriptor_count);
    for stream in &ordered_streams {
        expected_stream_offsets.push(stream_offset);
        let stream_end = stream_offset + 1 + stream.codes.len();
        let bytes = &mut patched_overlay[stream_offset..stream_end];
        bytes[0] = u8::try_from(stream.codes.len())?;
        bytes[1..].copy_from_slice(&stream.codes);

        let pointer_offset = layout.pointer_table_offset + stream.descriptor_index * 4;
        let runtime_address = runtime_address(stream_offset)?;
        patched_overlay[pointer_offset..pointer_offset + 4]
            .copy_from_slice(&runtime_address.to_le_bytes());
        stream_offset = stream_end;
    }
    ensure!(
        stream_offset == stream_end,
        "{consumer:?} practical-exam stream packing did not consume its measured size"
    );
    patched_overlay[coordinate_table_offset..layout.pointer_table_offset]
        .copy_from_slice(&encoded_coordinates);

    verify_packed_streams(
        &patched_overlay,
        layout,
        &ordered_streams,
        &expected_stream_offsets,
        coordinates,
        coordinate_table_offset,
    )?;

    let changed_ranges = difference_ranges(source_overlay, &patched_overlay);
    ensure!(
        !changed_ranges.is_empty() && changed_ranges_are_within(&changed_ranges, &owned_ranges),
        "{consumer:?} practical-exam stream packing changed bytes outside its primary descriptor arena and pointer table"
    );

    Ok(PracticalExamStreamPack {
        patched_overlay,
        coordinate_table_runtime_address: runtime_address(coordinate_table_offset)?,
        changed_ranges,
        owned_ranges,
        stream_size,
        coordinate_table_size,
    })
}

fn ensure_source_matches_audit(
    source_overlay: &[u8],
    source_audit: &PracticalExamConsumerAudit,
    layout: &StreamLayout,
) -> Result<()> {
    ensure!(
        layout.consumer == source_audit.consumer,
        "practical-exam stream layout and audited consumer do not match"
    );
    ensure!(
        sha256_bytes(source_overlay) == layout.source_sha256,
        "{:?} practical-exam stream source changed",
        layout.consumer
    );
    ensure_guarded_region(
        source_overlay,
        layout.descriptor_arena_start,
        layout.descriptor_arena_end - layout.descriptor_arena_start,
        layout.descriptor_arena_sha256,
        "primary descriptor arena",
    )?;
    ensure_guarded_region(
        source_overlay,
        layout.pointer_table_offset,
        layout.descriptor_count * 4,
        layout.pointer_table_sha256,
        "primary descriptor pointer table",
    )?;
    ensure!(
        source_audit.descriptors.len() == layout.descriptor_count,
        "{:?} practical-exam audit has the wrong primary descriptor count",
        layout.consumer
    );
    for (index, descriptor) in source_audit.descriptors.iter().enumerate() {
        ensure!(
            descriptor.index == index
                && (layout.descriptor_arena_start..layout.descriptor_arena_end)
                    .contains(&descriptor.offset),
            "{:?} practical-exam audit descriptor {index} has the wrong identity",
            layout.consumer
        );
        let bytes = source_overlay
            .get(descriptor.offset..descriptor.offset + descriptor.capacity)
            .with_context(|| {
                format!(
                    "{:?} practical-exam audit descriptor {index} is truncated",
                    layout.consumer
                )
            })?;
        ensure!(
            sha256_bytes(bytes) == descriptor.source_sha256,
            "{:?} practical-exam audit descriptor {index} no longer matches its source",
            layout.consumer
        );
        let pointer_offset = layout.pointer_table_offset + index * 4;
        let found_pointer = u32::from_le_bytes(
            source_overlay[pointer_offset..pointer_offset + 4]
                .try_into()
                .context("truncated practical-exam source pointer")?,
        );
        ensure!(
            found_pointer == runtime_address(descriptor.offset)?,
            "{:?} practical-exam audit descriptor {index} points at another source record",
            layout.consumer
        );
    }
    Ok(())
}

fn order_and_validate_streams(
    streams: &[DescriptorGlyphStream],
    descriptor_count: usize,
    coordinate_count: usize,
    consumer: PracticalExamConsumer,
) -> Result<Vec<&DescriptorGlyphStream>> {
    ensure!(
        streams.len() == descriptor_count,
        "{consumer:?} practical-exam requires exactly {descriptor_count} primary descriptor streams, found {}",
        streams.len()
    );
    let mut referenced_codes = BTreeSet::new();
    let mut ordered = vec![None; descriptor_count];
    for stream in streams {
        ensure!(
            stream.descriptor_index < descriptor_count,
            "{consumer:?} practical-exam descriptor stream {} is outside its pointer table",
            stream.descriptor_index
        );
        ensure!(
            !stream.codes.is_empty() && stream.codes.len() <= usize::from(u8::MAX),
            "{consumer:?} practical-exam descriptor {} must contain 1..=255 glyph codes",
            stream.descriptor_index
        );
        ensure!(
            ordered[stream.descriptor_index].replace(stream).is_none(),
            "{consumer:?} practical-exam descriptor {} was supplied more than once",
            stream.descriptor_index
        );
        for code in &stream.codes {
            ensure!(
                usize::from(*code) < coordinate_count,
                "{consumer:?} practical-exam descriptor {} references missing glyph code {code}",
                stream.descriptor_index
            );
            referenced_codes.insert(*code);
        }
    }
    ensure!(
        referenced_codes.len() == coordinate_count,
        "{consumer:?} practical-exam coordinate table contains unreferenced entries"
    );
    ordered
        .into_iter()
        .enumerate()
        .map(|(index, stream)| {
            stream.with_context(|| {
                format!("{consumer:?} practical-exam descriptor {index} stream is missing")
            })
        })
        .collect()
}

fn encode_coordinates(coordinates: &[GlyphCoordinate]) -> Result<Vec<u8>> {
    let mut encoded = Vec::with_capacity(coordinates.len() * COORDINATE_ENTRY_SIZE);
    let mut seen = BTreeSet::new();
    let mut hint_count = 0usize;
    for (code, coordinate) in coordinates.iter().copied().enumerate() {
        ensure!(
            seen.insert((
                coordinate.cell.x,
                coordinate.cell.y,
                coordinate.cell.width,
                coordinate.cell.height,
                coordinate.title_height,
            )),
            "practical-exam glyph code {code} duplicates another coordinate"
        );
        let right = coordinate
            .cell
            .x
            .checked_add(coordinate.cell.width)
            .with_context(|| format!("practical-exam glyph code {code} width overflows"))?;
        let bottom = coordinate
            .cell
            .y
            .checked_add(coordinate.cell.height)
            .with_context(|| format!("practical-exam glyph code {code} height overflows"))?;
        let page_index = coordinate.cell.x / TEXTURE_PAGE_WIDTH;
        ensure!(
            page_index < 3
                && coordinate.cell.width > 0
                && coordinate.cell.height > 0
                && right <= TEXTURE_WIDTH
                && bottom <= TEXTURE_HEIGHT
                && (right - 1) / TEXTURE_PAGE_WIDTH == page_index,
            "practical-exam glyph code {code} has invalid texture geometry"
        );
        let meta = if coordinate.cell == HINT_COMPOSITE_CELL && !coordinate.title_height {
            ensure!(
                page_index == 2,
                "practical-exam glyph code {code} moved the guarded hint outside p14"
            );
            hint_count += 1;
            u8::try_from(page_index << 5)?
        } else {
            let required_height = if coordinate.title_height {
                TITLE_GLYPH_HEIGHT
            } else {
                NORMAL_GLYPH_HEIGHT
            };
            ensure!(
                coordinate.cell.height == required_height
                    && coordinate.cell.width <= MAX_INLINE_GLYPH_WIDTH,
                "practical-exam glyph code {code} must be 1..=32x{required_height}"
            );
            let width_bits = u8::try_from(coordinate.cell.width - 1)?;
            let page_bits = u8::try_from(page_index << 5)?;
            let meta = width_bits | page_bits | if coordinate.title_height { 0x80 } else { 0 };
            ensure!(
                meta != 0x40,
                "practical-exam glyph code {code} conflicts with the exact p14 hint marker"
            );
            meta
        };
        encoded.extend_from_slice(&[
            meta,
            u8::try_from(coordinate.cell.x % TEXTURE_PAGE_WIDTH)?,
            u8::try_from(coordinate.cell.y)?,
        ]);
    }
    ensure!(
        hint_count == 1,
        "practical-exam coordinate table must contain the exact p14 hint composite once"
    );
    Ok(encoded)
}

fn verify_packed_streams(
    patched: &[u8],
    layout: &StreamLayout,
    streams: &[&DescriptorGlyphStream],
    expected_offsets: &[usize],
    coordinates: &[GlyphCoordinate],
    coordinate_table_offset: usize,
) -> Result<()> {
    for ((stream, expected_offset), index) in streams
        .iter()
        .zip(expected_offsets)
        .zip(0..layout.descriptor_count)
    {
        ensure!(
            stream.descriptor_index == index,
            "{:?} practical-exam ordered stream index drifted",
            layout.consumer
        );
        let pointer_offset = layout.pointer_table_offset + index * 4;
        let pointer = u32::from_le_bytes(
            patched[pointer_offset..pointer_offset + 4]
                .try_into()
                .context("truncated practical-exam rewritten pointer")?,
        );
        ensure!(
            pointer == runtime_address(*expected_offset)?,
            "{:?} practical-exam descriptor {index} pointer roundtrip failed",
            layout.consumer
        );
        let count = usize::from(patched[*expected_offset]);
        ensure!(
            count == stream.codes.len()
                && patched[*expected_offset + 1..*expected_offset + 1 + count] == stream.codes,
            "{:?} practical-exam descriptor {index} glyph stream roundtrip failed",
            layout.consumer
        );
    }

    for (code, expected) in coordinates.iter().copied().enumerate() {
        let entry_offset = coordinate_table_offset + code * COORDINATE_ENTRY_SIZE;
        let entry: [u8; COORDINATE_ENTRY_SIZE] = patched
            [entry_offset..entry_offset + COORDINATE_ENTRY_SIZE]
            .try_into()
            .context("truncated practical-exam coordinate table entry")?;
        let decoded = decode_coordinate(entry)?;
        ensure!(
            decoded == expected,
            "{:?} practical-exam glyph coordinate {code} roundtrip failed",
            layout.consumer
        );
    }
    Ok(())
}

fn decode_coordinate(entry: [u8; COORDINATE_ENTRY_SIZE]) -> Result<GlyphCoordinate> {
    let meta = entry[0];
    let page_index = usize::from((meta >> 5) & 0x03);
    ensure!(
        page_index < 3,
        "practical-exam coordinate entry names an unknown texture page"
    );
    if meta == 0x40 {
        let cell = Cell {
            x: page_index * TEXTURE_PAGE_WIDTH + usize::from(entry[1]),
            y: usize::from(entry[2]),
            width: HINT_COMPOSITE_CELL.width,
            height: HINT_COMPOSITE_CELL.height,
        };
        ensure!(
            cell == HINT_COMPOSITE_CELL,
            "practical-exam exact hint marker does not name its guarded composite"
        );
        return Ok(GlyphCoordinate {
            cell,
            title_height: false,
        });
    }

    let title_height = meta & 0x80 != 0;
    Ok(GlyphCoordinate {
        cell: Cell {
            x: page_index * TEXTURE_PAGE_WIDTH + usize::from(entry[1]),
            y: usize::from(entry[2]),
            width: usize::from(meta & 0x1f) + 1,
            height: if title_height {
                TITLE_GLYPH_HEIGHT
            } else {
                NORMAL_GLYPH_HEIGHT
            },
        },
        title_height,
    })
}

fn ensure_guarded_region(
    source: &[u8],
    offset: usize,
    size: usize,
    expected_sha256: &str,
    role: &str,
) -> Result<()> {
    let bytes = source
        .get(offset..offset + size)
        .with_context(|| format!("practical-exam {role} is truncated"))?;
    let found_sha256 = sha256_bytes(bytes);
    ensure!(
        found_sha256 == expected_sha256,
        "practical-exam {role} changed: found {found_sha256}"
    );
    Ok(())
}

fn runtime_address(offset: usize) -> Result<u32> {
    OVERLAY_RUNTIME_BASE
        .checked_add(u32::try_from(offset)?)
        .context("practical-exam overlay runtime address overflow")
}

fn layout(consumer: PracticalExamConsumer) -> &'static StreamLayout {
    match consumer {
        PracticalExamConsumer::BasicsReview => &BASICS_LAYOUT,
        PracticalExamConsumer::Exam1999 => &EXAM_1999_LAYOUT,
    }
}
