use std::ops::Range;

use anyhow::{Result, ensure};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TzzMember {
    pub index: usize,
    pub offset: usize,
    pub compressed_size: usize,
    pub slot_size: usize,
}

impl TzzMember {
    pub fn compressed_range(self) -> Range<usize> {
        self.offset..self.offset + self.compressed_size
    }

    pub fn slot_range(self) -> Range<usize> {
        self.offset..self.offset + self.slot_size
    }
}

pub fn parse_tzz(data: &[u8]) -> Result<Vec<TzzMember>> {
    ensure!(data.len() >= 8, "truncated TZZ archive");
    let mut entries = Vec::new();
    let mut table_offset = 0usize;
    loop {
        ensure!(table_offset + 8 <= data.len(), "unterminated TZZ table");
        let offset = u32_le(data, table_offset)? as usize;
        let compressed_size = u32_le(data, table_offset + 4)? as usize;
        table_offset += 8;
        if offset == 0 && compressed_size == 0 {
            break;
        }
        ensure!(
            offset > 0 && compressed_size > 0,
            "invalid TZZ member entry"
        );
        entries.push((offset, compressed_size));
    }
    ensure!(!entries.is_empty(), "TZZ archive has no members");
    ensure!(
        entries[0].0 >= table_offset,
        "TZZ member data overlaps its table"
    );

    let mut members = Vec::with_capacity(entries.len());
    for (index, &(offset, compressed_size)) in entries.iter().enumerate() {
        let slot_end = entries
            .get(index + 1)
            .map_or(data.len(), |(next_offset, _)| *next_offset);
        ensure!(
            offset.is_multiple_of(0x800),
            "TZZ member {index} offset is not sector-aligned"
        );
        ensure!(slot_end > offset, "TZZ member offsets are not increasing");
        ensure!(
            offset + compressed_size <= slot_end,
            "TZZ member {index} exceeds its slot"
        );
        ensure!(slot_end <= data.len(), "TZZ member {index} exceeds archive");
        members.push(TzzMember {
            index,
            offset,
            compressed_size,
            slot_size: slot_end - offset,
        });
    }
    Ok(members)
}

fn u32_le(data: &[u8], offset: usize) -> Result<u32> {
    ensure!(offset + 4 <= data.len(), "truncated TZZ u32");
    Ok(u32::from_le_bytes(data[offset..offset + 4].try_into()?))
}

#[cfg(test)]
#[path = "tzz_tests.rs"]
mod tests;
