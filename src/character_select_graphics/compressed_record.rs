//! Locates and rebuilds one compressed stream without claiming its surrounding record.

use anyhow::{Context, Result, ensure};

use crate::compression::decompress;
use crate::pipeline::difference_ranges;
use crate::write_scope::changed_ranges_are_within;

const DISC_READ_ALIGNMENT: usize = 0x800;

pub(crate) fn decode_built_record_stream(
    stored: &[u8],
    stream_offset: usize,
    slot_capacity: usize,
) -> Result<Vec<u8>> {
    let slot = stored
        .get(stream_offset..stream_offset + slot_capacity)
        .context("built compressed stream slot exceeds its record")?;
    decompress(slot, true)
}

pub(crate) fn decode_built_record_streams(
    stored: &[u8],
    streams: impl IntoIterator<Item = (usize, usize)>,
) -> Result<Vec<u8>> {
    let mut decoded = Vec::new();
    for (stream_offset, slot_capacity) in streams {
        decoded.extend(decode_built_record_stream(
            stored,
            stream_offset,
            slot_capacity,
        )?);
    }
    ensure!(
        !decoded.is_empty(),
        "compressed record has no decoded streams"
    );
    Ok(decoded)
}

#[derive(Debug, Clone, Copy)]
pub(super) struct CompressedTextureStream {
    pub(super) stream_index: usize,
    pub(super) decoded_range: [usize; 2],
    pub(super) storage: CompressedStreamStorage,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct CompressedStreamStorage {
    stream_offset: usize,
    slot_capacity: usize,
    declared_stream_size: Option<usize>,
    declared_size_field_offset: Option<usize>,
}

impl CompressedStreamStorage {
    pub(super) fn direct(record_size: usize) -> Result<Self> {
        ensure!(record_size > 0, "compressed record is empty");
        Ok(Self {
            stream_offset: 0,
            slot_capacity: record_size,
            declared_stream_size: None,
            declared_size_field_offset: None,
        })
    }

    pub(super) fn indexed(
        stored: &[u8],
        header_size: usize,
        stream_count: usize,
        selected_stream_index: usize,
    ) -> Result<Self> {
        ensure!(
            stream_count > 0 && selected_stream_index < stream_count,
            "indexed compressed record has an invalid selected stream"
        );
        ensure!(
            header_size >= stream_count * 8
                && header_size <= stored.len()
                && header_size.is_multiple_of(DISC_READ_ALIGNMENT),
            "indexed compressed record has an invalid header extent"
        );

        let mut entries = Vec::with_capacity(stream_count);
        for index in 0..stream_count {
            let descriptor_offset = index * 8;
            let stream_offset = read_u32(stored, descriptor_offset)? as usize;
            let stream_size = read_u32(stored, descriptor_offset + 4)? as usize;
            ensure!(
                stream_offset >= header_size
                    && stream_offset < stored.len()
                    && stream_offset.is_multiple_of(DISC_READ_ALIGNMENT)
                    && stream_size > 0,
                "indexed compressed stream {index} has an invalid descriptor"
            );
            if let Some((previous_offset, _)) = entries.last() {
                ensure!(
                    *previous_offset < stream_offset,
                    "indexed compressed stream offsets are not strictly increasing"
                );
            }
            entries.push((stream_offset, stream_size));
        }

        for (index, &(stream_offset, stream_size)) in entries.iter().enumerate() {
            let slot_end = entries
                .get(index + 1)
                .map_or(stored.len(), |(next_offset, _)| *next_offset);
            ensure!(
                slot_end.is_multiple_of(DISC_READ_ALIGNMENT)
                    && stream_offset
                        .checked_add(stream_size)
                        .is_some_and(|stream_end| stream_end <= slot_end),
                "indexed compressed stream {index} exceeds its storage slot"
            );
            ensure!(
                stored[stream_offset + stream_size..slot_end]
                    .iter()
                    .all(|byte| *byte == 0),
                "indexed compressed stream {index} has non-padding bytes after its declared size"
            );
        }

        let (stream_offset, declared_stream_size) = entries[selected_stream_index];
        let slot_end = entries
            .get(selected_stream_index + 1)
            .map_or(stored.len(), |(next_offset, _)| *next_offset);
        Ok(Self {
            stream_offset,
            slot_capacity: slot_end - stream_offset,
            declared_stream_size: Some(declared_stream_size),
            declared_size_field_offset: Some(selected_stream_index * 8 + 4),
        })
    }

    pub(super) fn stream_offset(self) -> usize {
        self.stream_offset
    }

    pub(super) fn slot_capacity(self) -> usize {
        self.slot_capacity
    }

    pub(super) fn owned_stored_ranges(self) -> Vec<[usize; 2]> {
        let mut ranges = vec![[self.stream_offset, self.stream_offset + self.slot_capacity]];
        if let Some(size_field_offset) = self.declared_size_field_offset {
            ranges.push([size_field_offset, size_field_offset + 4]);
        }
        ranges
    }

    pub(super) fn source_encoded(self, stored: &[u8]) -> Result<&[u8]> {
        let slot = self.source_slot(stored)?;
        Ok(match self.declared_stream_size {
            Some(size) => slot
                .get(..size)
                .context("declared compressed stream exceeds its storage slot")?,
            None => slot,
        })
    }

    pub(super) fn source_slot(self, stored: &[u8]) -> Result<&[u8]> {
        stored
            .get(self.stream_offset..self.stream_offset + self.slot_capacity)
            .context("compressed stream slot exceeds its source record")
    }

    pub(super) fn validate_stream_size(self, stream_size: usize) -> Result<()> {
        ensure!(
            stream_size <= self.slot_capacity,
            "compressed stream terminator exceeds its storage slot"
        );
        if let Some(declared_stream_size) = self.declared_stream_size {
            ensure!(
                stream_size == declared_stream_size,
                "indexed compressed stream size does not match its descriptor"
            );
        }
        Ok(())
    }

    pub(super) fn rebuild(self, source: &[u8], reencoded: &[u8]) -> Result<Vec<u8>> {
        ensure!(
            reencoded.len() <= self.slot_capacity,
            "rebuilt compressed stream exceeds its storage slot"
        );
        let mut rebuilt = source.to_vec();
        let slot = rebuilt
            .get_mut(self.stream_offset..self.stream_offset + self.slot_capacity)
            .context("compressed stream slot exceeds its source record")?;
        slot.fill(0);
        slot[..reencoded.len()].copy_from_slice(reencoded);

        let mut allowed_ranges =
            vec![[self.stream_offset, self.stream_offset + self.slot_capacity]];
        if let Some(size_field_offset) = self.declared_size_field_offset {
            let source_size = read_u32(source, size_field_offset)? as usize;
            ensure!(
                Some(source_size) == self.declared_stream_size,
                "indexed compressed stream descriptor changed before rebuild"
            );
            let rebuilt_size = u32::try_from(reencoded.len())
                .context("rebuilt compressed stream size does not fit its descriptor")?;
            rebuilt[size_field_offset..size_field_offset + 4]
                .copy_from_slice(&rebuilt_size.to_le_bytes());
            allowed_ranges.push([size_field_offset, size_field_offset + 4]);
        }

        let changed_ranges = difference_ranges(source, &rebuilt);
        ensure!(
            changed_ranges_are_within(&changed_ranges, &allowed_ranges),
            "rebuilt compressed stream changed bytes outside its owned storage ranges"
        );
        Ok(rebuilt)
    }
}

fn read_u32(data: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        data.get(offset..offset + 4)
            .context("indexed compressed stream descriptor is truncated")?
            .try_into()?,
    ))
}

#[cfg(test)]
mod tests {
    use super::CompressedStreamStorage;

    fn indexed_two_stream_record() -> Vec<u8> {
        let mut stored = vec![0u8; 0x1800];
        stored[0..4].copy_from_slice(&0x800_u32.to_le_bytes());
        stored[4..8].copy_from_slice(&4_u32.to_le_bytes());
        stored[8..12].copy_from_slice(&0x1000_u32.to_le_bytes());
        stored[12..16].copy_from_slice(&5_u32.to_le_bytes());
        stored[0x800..0x804].copy_from_slice(&[1, 2, 3, 4]);
        stored[0x1000..0x1005].copy_from_slice(&[5, 6, 7, 8, 9]);
        stored
    }

    #[test]
    fn indexed_stream_rebuild_preserves_sibling_descriptor_and_slot() {
        let source = indexed_two_stream_record();
        let first = CompressedStreamStorage::indexed(&source, 0x800, 2, 0).unwrap();
        let rebuilt = first.rebuild(&source, &[10, 11, 12]).unwrap();

        assert_eq!(&rebuilt[0..4], &source[0..4]);
        assert_eq!(u32::from_le_bytes(rebuilt[4..8].try_into().unwrap()), 3);
        assert_eq!(&rebuilt[8..0x800], &source[8..0x800]);
        assert_eq!(&rebuilt[0x800..0x803], &[10, 11, 12]);
        assert!(rebuilt[0x803..0x1000].iter().all(|byte| *byte == 0));
        assert_eq!(&rebuilt[0x1000..], &source[0x1000..]);
    }

    #[test]
    fn sequential_indexed_stream_rebuilds_keep_each_owned_result() {
        let source = indexed_two_stream_record();
        let first = CompressedStreamStorage::indexed(&source, 0x800, 2, 0).unwrap();
        let second = CompressedStreamStorage::indexed(&source, 0x800, 2, 1).unwrap();
        let rebuilt_first = first.rebuild(&source, &[10, 11, 12]).unwrap();
        let rebuilt_both = second
            .rebuild(&rebuilt_first, &[20, 21, 22, 23, 24, 25])
            .unwrap();

        assert_eq!(
            u32::from_le_bytes(rebuilt_both[4..8].try_into().unwrap()),
            3
        );
        assert_eq!(
            u32::from_le_bytes(rebuilt_both[12..16].try_into().unwrap()),
            6
        );
        assert_eq!(&rebuilt_both[0x800..0x803], &[10, 11, 12]);
        assert_eq!(&rebuilt_both[0x1000..0x1006], &[20, 21, 22, 23, 24, 25]);
        assert!(rebuilt_both[0x1006..].iter().all(|byte| *byte == 0));
    }
}
