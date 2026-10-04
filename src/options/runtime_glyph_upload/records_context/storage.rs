use anyhow::{Context, Result, ensure};

use crate::tim::{parse_4bpp_prefix, parse_4bpp_without_clut_prefix};

use super::super::assembly::{
    BOOT_NOTICE_PROGRAM_CAPACITY, BOOT_NOTICE_PROGRAM_OFFSET,
    RECORDS_CARD_OPERATION_REFRESH_PROGRAM_BYTE_CAPACITY,
    RECORDS_CARD_OPERATION_REFRESH_PROGRAM_OFFSET, RECORDS_ENTRY_PROGRAM_BYTE_CAPACITY,
    RECORDS_ENTRY_PROGRAM_OFFSET, RECORDS_EXIT_RESTORE_PROGRAM_BYTE_CAPACITY,
    RECORDS_EXIT_RESTORE_PROGRAM_OFFSET,
};
use super::super::model::DESCRIPTOR_BYTE_COUNT;

const RECORDS_TIM_BYTE_COUNT: usize = 0x1840;
const RECORDS_TIM_SLOT_START: usize = 0x83800;
const RECORDS_TIM_SLOT_STRIDE: usize = 0x2000;

pub(crate) const RECORDS_STORAGE_SLOT_COUNT: usize = 14;
pub(crate) const RECORDS_FIRST_STORAGE_START: usize = 0x85040;
pub(crate) const RECORDS_DESCRIPTOR_OFFSET: usize =
    BOOT_NOTICE_PROGRAM_OFFSET + BOOT_NOTICE_PROGRAM_CAPACITY;
pub(crate) const RECORDS_STORAGE_END: usize =
    RECORDS_TIM_SLOT_START + RECORDS_STORAGE_SLOT_COUNT * RECORDS_TIM_SLOT_STRIDE;

pub(crate) struct RecordsStorageLayout {
    pub(crate) entry_descriptor_offset: usize,
    pub(crate) exit_restore_descriptor_offset: usize,
    pub(crate) entry_payload_offsets: Vec<usize>,
    pub(crate) exit_restore_payload_offsets: Vec<usize>,
    pub(crate) data_write_ranges: Vec<[usize; 2]>,
}

impl RecordsStorageLayout {
    pub(crate) fn entry_payload_ranges(&self, sizes: &[usize]) -> Vec<[usize; 2]> {
        coalesce_payload_ranges(&self.entry_payload_offsets, sizes)
    }

    pub(crate) fn exit_restore_payload_ranges(&self, sizes: &[usize]) -> Vec<[usize; 2]> {
        coalesce_payload_ranges(&self.exit_restore_payload_offsets, sizes)
    }
}

pub(crate) fn validate_source_storage(source_menu: &[u8]) -> Result<()> {
    ensure!(
        source_menu.len() == RECORDS_STORAGE_END,
        "Records MENU storage extent changed"
    );
    for [tim_offset, storage_start, storage_end] in storage_slot_profiles() {
        let tim = parse_4bpp_prefix(
            source_menu
                .get(tim_offset..)
                .context("Records storage TIM offset is outside MENU")?,
        )?;
        ensure!(
            tim.total_size == RECORDS_TIM_BYTE_COUNT
                && tim.image_x == 640
                && tim.image_y == 0
                && tim.pixel_width() == 128
                && tim.image_height == 96,
            "Records storage TIM source profile changed at {tim_offset:#x}"
        );
        ensure!(
            tim_offset + tim.total_size == storage_start,
            "Records storage no longer starts at the source TIM boundary"
        );
        ensure!(
            source_menu
                .get(storage_start..storage_end)
                .context("Records source storage slot is truncated")?
                .iter()
                .all(|byte| *byte == 0),
            "Records source storage padding is no longer zero at {storage_start:#x}"
        );
    }
    let background = parse_4bpp_prefix(&source_menu[0x22800..])?;
    ensure!(
        background.total_size + 0x22800 == super::super::RECORDS_PAYLOAD_STORAGE_START
            && source_menu[super::super::RECORDS_PAYLOAD_STORAGE_START..0x4b800]
                .iter()
                .all(|b| *b == 0),
        "Records background-tail storage is not source-zero TIM padding"
    );
    for (index, [start, end]) in additional_storage_slots().into_iter().skip(1).enumerate() {
        let tim_offset = if index < 14 {
            0x4b800 + index * 0x2800
        } else {
            0x6e800 + (index - 14) * 0x1800
        };
        let total_size = if index < 14 {
            let tim = parse_4bpp_without_clut_prefix(&source_menu[tim_offset..])?;
            ensure!(
                tim.image_x == 832
                    && tim.image_y == 256
                    && tim.image_word_width == 44
                    && tim.image_height == 112,
                "Records supplemental TIM geometry changed"
            );
            tim.total_size
        } else {
            let tim = parse_4bpp_prefix(&source_menu[tim_offset..])?;
            ensure!(
                tim.image_x == 512
                    && tim.image_y == 0
                    && tim.pixel_width() == 100
                    && tim.image_height == 114,
                "Records supplemental portrait TIM geometry changed"
            );
            tim.total_size
        };
        ensure!(
            tim_offset + total_size == start && source_menu[start..end].iter().all(|b| *b == 0),
            "Records supplemental storage is not source-zero TIM padding"
        );
    }
    ensure!(
        RECORDS_ENTRY_PROGRAM_OFFSET == RECORDS_FIRST_STORAGE_START
            && RECORDS_ENTRY_PROGRAM_OFFSET + RECORDS_ENTRY_PROGRAM_BYTE_CAPACITY
                == RECORDS_EXIT_RESTORE_PROGRAM_OFFSET
            && BOOT_NOTICE_PROGRAM_OFFSET
                == RECORDS_CARD_OPERATION_REFRESH_PROGRAM_OFFSET
                    + RECORDS_CARD_OPERATION_REFRESH_PROGRAM_BYTE_CAPACITY
            && RECORDS_DESCRIPTOR_OFFSET
                == BOOT_NOTICE_PROGRAM_OFFSET + BOOT_NOTICE_PROGRAM_CAPACITY
            && RECORDS_CARD_OPERATION_REFRESH_PROGRAM_OFFSET
                == RECORDS_EXIT_RESTORE_PROGRAM_OFFSET + RECORDS_EXIT_RESTORE_PROGRAM_BYTE_CAPACITY,
        "Records program and descriptor storage layout changed"
    );
    Ok(())
}

pub(crate) fn allocate_storage(
    entry_payload_sizes: &[usize],
    exit_restore_payload_sizes: &[usize],
) -> Result<RecordsStorageLayout> {
    ensure!(
        !entry_payload_sizes.is_empty()
            && entry_payload_sizes.len() == exit_restore_payload_sizes.len()
            && entry_payload_sizes.iter().all(|size| *size > 0)
            && exit_restore_payload_sizes.iter().all(|size| *size > 0),
        "Records entry and restore payload populations are malformed"
    );
    let entry_descriptor_offset = RECORDS_DESCRIPTOR_OFFSET;
    let entry_end = entry_descriptor_offset
        .checked_add(entry_payload_sizes.len() * DESCRIPTOR_BYTE_COUNT)
        .context("Records entry descriptor overflow")?;
    let first_end = storage_slots()[0][1];
    ensure!(
        entry_end <= first_end,
        "Records entry descriptors exceed their source-zero slot"
    );
    let exit_size = exit_restore_payload_sizes.len() * DESCRIPTOR_BYTE_COUNT;
    let exit_restore_descriptor_offset = if entry_end + exit_size <= first_end {
        entry_end
    } else {
        storage_slots()[1][0]
    };
    let exit_end = exit_restore_descriptor_offset + exit_size;
    ensure!(
        storage_slots()
            .iter()
            .any(|[a, b]| *a <= exit_restore_descriptor_offset && exit_end <= *b),
        "Records restore descriptors exceed their source-zero slot"
    );
    let reserved = [
        [RECORDS_FIRST_STORAGE_START, entry_end],
        [exit_restore_descriptor_offset, exit_end],
    ];
    let mut payload_sizes =
        Vec::with_capacity(entry_payload_sizes.len() + exit_restore_payload_sizes.len());
    payload_sizes.extend_from_slice(entry_payload_sizes);
    payload_sizes.extend_from_slice(exit_restore_payload_sizes);
    let payload_offsets = allocate_payload_offsets(&reserved, &payload_sizes)?;
    let (entry_payload_offsets, exit_restore_payload_offsets) =
        payload_offsets.split_at(entry_payload_sizes.len());

    let mut data_write_ranges = vec![
        [
            entry_descriptor_offset,
            entry_descriptor_offset + entry_payload_sizes.len() * DESCRIPTOR_BYTE_COUNT,
        ],
        [
            exit_restore_descriptor_offset,
            exit_restore_descriptor_offset
                + exit_restore_payload_sizes.len() * DESCRIPTOR_BYTE_COUNT,
        ],
    ];
    data_write_ranges.extend(coalesce_payload_ranges(&payload_offsets, &payload_sizes));
    data_write_ranges.sort_unstable();
    data_write_ranges = coalesce_ranges(&data_write_ranges);

    Ok(RecordsStorageLayout {
        entry_descriptor_offset,
        exit_restore_descriptor_offset,
        entry_payload_offsets: entry_payload_offsets.to_vec(),
        exit_restore_payload_offsets: exit_restore_payload_offsets.to_vec(),
        data_write_ranges,
    })
}

fn allocate_payload_offsets(
    reserved: &[[usize; 2]],
    payload_sizes: &[usize],
) -> Result<Vec<usize>> {
    let mut slots = storage_slots();
    slots.extend(additional_storage_slots());
    let mut cursors = slots
        .iter()
        .map(|[start, end]| {
            reserved
                .iter()
                .filter(|[a, b]| *a < *end && *b > *start)
                .map(|[_, b]| *b)
                .max()
                .unwrap_or(*start)
        })
        .collect::<Vec<_>>();
    let mut offsets = vec![0; payload_sizes.len()];
    let mut order = (0..payload_sizes.len()).collect::<Vec<_>>();
    order.sort_by_key(|index| std::cmp::Reverse(payload_sizes[*index]));
    for payload_index in order {
        let size = &payload_sizes[payload_index];
        let (slot_index, cursor) = cursors
            .iter()
            .copied()
            .enumerate()
            .find(|(index, cursor)| {
                cursor
                    .checked_add(*size)
                    .is_some_and(|end| end <= slots[*index][1])
            })
            .with_context(|| format!("Records contextual glyph storage exhausted: {} payloads, {} bytes, next {} bytes", payload_sizes.len(), payload_sizes.iter().sum::<usize>(), size))?;
        offsets[payload_index] = cursor;
        cursors[slot_index] = cursor + size;
    }
    Ok(offsets)
}

fn storage_slot_profiles() -> Vec<[usize; 3]> {
    (0..RECORDS_STORAGE_SLOT_COUNT)
        .map(|index| {
            let tim_offset = RECORDS_TIM_SLOT_START + index * RECORDS_TIM_SLOT_STRIDE;
            [
                tim_offset,
                tim_offset + RECORDS_TIM_BYTE_COUNT,
                tim_offset + RECORDS_TIM_SLOT_STRIDE,
            ]
        })
        .collect()
}

fn storage_slots() -> Vec<[usize; 2]> {
    storage_slot_profiles()
        .into_iter()
        .map(|[_, start, end]| [start, end])
        .collect()
}

fn additional_storage_slots() -> Vec<[usize; 2]> {
    std::iter::once([super::super::RECORDS_PAYLOAD_STORAGE_START, 0x4b800])
        .chain(
            (0..14)
                .map(|i| [0x4b800 + i * 0x2800 + 0x2694, 0x4b800 + (i + 1) * 0x2800])
                .chain(
                    (0..14).map(|i| [0x6e800 + i * 0x1800 + 0x1684, 0x6e800 + (i + 1) * 0x1800]),
                ),
        )
        .collect()
}

fn coalesce_payload_ranges(offsets: &[usize], sizes: &[usize]) -> Vec<[usize; 2]> {
    let ranges = offsets
        .iter()
        .zip(sizes)
        .map(|(offset, size)| [*offset, *offset + *size])
        .collect::<Vec<_>>();
    coalesce_ranges(&ranges)
}

fn coalesce_ranges(ranges: &[[usize; 2]]) -> Vec<[usize; 2]> {
    let mut ranges = ranges.to_vec();
    ranges.sort_unstable();
    let mut coalesced = Vec::<[usize; 2]>::new();
    for range in &ranges {
        if let Some(previous) = coalesced.last_mut()
            && previous[1] == range[0]
        {
            previous[1] = range[1];
        } else {
            coalesced.push(*range);
        }
    }
    coalesced
}

#[cfg(test)]
mod expanded_storage_tests {
    use super::*;

    #[test]
    fn larger_records_population_splits_descriptors_and_preserves_tim_bodies() {
        let mut sizes = vec![200; 68];
        sizes.extend([800, 800]);
        let layout = allocate_storage(&sizes, &sizes).unwrap();
        assert_eq!(layout.entry_payload_offsets.len(), sizes.len());
        assert_eq!(layout.exit_restore_payload_offsets.len(), sizes.len());
        assert!(layout.exit_restore_descriptor_offset >= storage_slots()[1][0]);
        let mut allowed = storage_slots();
        allowed.extend(additional_storage_slots());
        for [a, b] in &layout.data_write_ranges {
            assert!(allowed.iter().any(|[start, end]| start <= a && b <= end));
        }
        assert!(
            layout
                .entry_payload_offsets
                .iter()
                .chain(&layout.exit_restore_payload_offsets)
                .any(|offset| *offset < RECORDS_FIRST_STORAGE_START)
        );
        let mut regions = vec![
            [
                RECORDS_FIRST_STORAGE_START,
                layout.entry_descriptor_offset + sizes.len() * DESCRIPTOR_BYTE_COUNT,
            ],
            [
                layout.exit_restore_descriptor_offset,
                layout.exit_restore_descriptor_offset + sizes.len() * DESCRIPTOR_BYTE_COUNT,
            ],
        ];
        regions.extend(
            layout
                .entry_payload_offsets
                .iter()
                .chain(&layout.exit_restore_payload_offsets)
                .zip(sizes.iter().chain(&sizes))
                .map(|(a, size)| [*a, *a + size]),
        );
        regions.sort_unstable();
        assert!(regions.windows(2).all(|pair| pair[0][1] <= pair[1][0]));
    }
}
