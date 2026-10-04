use std::collections::BTreeMap;

use anyhow::{Context, Result, ensure};

use crate::text::read_length_prefixed_codes;

use super::source::OVERLAY_RUNTIME_BASE;

const SOURCE_RECORD_ARENA_START: usize = 0x036c;
const SOURCE_RECORD_ARENA_END: usize = 0x0b90;
const POINTER_TABLE_START: usize = 0x0b90;
const POINTER_TABLE_END: usize = 0x0de0;
const PLACEMENT_TABLES: [(usize, usize); 3] = [(0x0090, 16), (0x0100, 12), (0x0300, 7)];
const LARGE_TEXT_SCALE_CODE: u8 = 0x06;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct NewoptPointerRecord {
    pub(super) source_offset: usize,
    pub(super) pointer_offsets: Vec<usize>,
    pub(super) source_codes: Vec<u16>,
}

#[derive(Debug)]
pub(super) struct NewoptSourceCatalog {
    pub(super) pointer_slot_count: usize,
    pub(super) records: Vec<NewoptPointerRecord>,
}

pub(super) fn catalog_newopt_pointer_records(overlay: &[u8]) -> Result<NewoptSourceCatalog> {
    ensure!(
        POINTER_TABLE_END <= overlay.len(),
        "NEWOPT pointer table is truncated"
    );
    let mut records = BTreeMap::<usize, NewoptPointerRecord>::new();
    let mut pointer_slot_count = 0usize;
    for pointer_offset in (POINTER_TABLE_START..POINTER_TABLE_END).step_by(4) {
        pointer_slot_count += 1;
        let pointer = read_u32(overlay, pointer_offset)?;
        let source_offset = pointer
            .checked_sub(OVERLAY_RUNTIME_BASE)
            .and_then(|offset| usize::try_from(offset).ok())
            .with_context(|| {
                format!("NEWOPT pointer +0x{pointer_offset:04x} leaves the overlay")
            })?;
        ensure!(
            (SOURCE_RECORD_ARENA_START..SOURCE_RECORD_ARENA_END).contains(&source_offset),
            "NEWOPT pointer +0x{pointer_offset:04x} leaves the string arena"
        );
        let source_codes = read_length_prefixed_codes(overlay, source_offset)?;
        let record_end = source_offset
            .checked_add(2 + source_codes.len() * 2)
            .context("NEWOPT source record length overflow")?;
        ensure!(
            record_end <= SOURCE_RECORD_ARENA_END,
            "NEWOPT source record +0x{source_offset:04x} overlaps its pointer table"
        );
        let record = records
            .entry(source_offset)
            .or_insert_with(|| NewoptPointerRecord {
                source_offset,
                pointer_offsets: Vec::new(),
                source_codes: source_codes.clone(),
            });
        ensure!(
            record.source_codes == source_codes,
            "NEWOPT source record +0x{source_offset:04x} changed while cataloging pointers"
        );
        record.pointer_offsets.push(pointer_offset);
    }

    let records = records.into_values().collect::<Vec<_>>();
    for pair in records.windows(2) {
        let left = &pair[0];
        let right = &pair[1];
        let left_end = left.source_offset + 2 + left.source_codes.len() * 2;
        ensure!(
            left_end <= right.source_offset,
            "NEWOPT source records +0x{:04x} and +0x{:04x} overlap",
            left.source_offset,
            right.source_offset
        );
    }
    Ok(NewoptSourceCatalog {
        pointer_slot_count,
        records,
    })
}

pub(super) fn catalog_large_placement_text_offsets(overlay: &[u8]) -> Result<Vec<usize>> {
    let mut offsets = Vec::new();
    for (table_offset, record_count) in PLACEMENT_TABLES {
        let table_end = table_offset + record_count * 4;
        ensure!(
            table_end <= overlay.len(),
            "NEWOPT placement table is truncated"
        );
        for record_offset in (table_offset..table_end).step_by(4) {
            let scale_code = overlay[record_offset + 2];
            if scale_code != LARGE_TEXT_SCALE_CODE {
                continue;
            }
            let pointer_index = usize::from(overlay[record_offset + 3]);
            let pointer_offset = POINTER_TABLE_START + pointer_index * 4;
            ensure!(
                pointer_offset < POINTER_TABLE_END,
                "NEWOPT placement record selects a pointer outside the table"
            );
            let pointer = read_u32(overlay, pointer_offset)?;
            let source_offset = pointer
                .checked_sub(OVERLAY_RUNTIME_BASE)
                .and_then(|offset| usize::try_from(offset).ok())
                .context("NEWOPT placement pointer leaves the overlay")?;
            offsets.push(source_offset);
        }
    }
    offsets.sort_unstable();
    offsets.dedup();
    ensure!(
        offsets == [0x0778, 0x078c, 0x08b8],
        "NEWOPT large placement text population changed"
    );
    Ok(offsets)
}

fn read_u32(data: &[u8], offset: usize) -> Result<u32> {
    let bytes = data
        .get(offset..offset + 4)
        .with_context(|| format!("truncated NEWOPT pointer at +0x{offset:04x}"))?;
    Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
}
