use anyhow::{Result, ensure};

pub(super) const DECODED_RUNTIME_BASE: u32 = 0x800d_0000;
pub(crate) const DECODED_IMAGE_SIZE: usize = 0x51_800;
pub(super) const SELECTOR_TABLE_OFFSET: usize = 0x31_000;
pub(super) const SELECTOR_SLOT_COUNT: usize = 8;

const POINTER_TABLE_TERMINATOR: u32 = u32::MAX;

#[derive(Debug, PartialEq, Eq)]
pub(super) struct ParsedMessage {
    pub(super) decoded_offset: usize,
    pub(super) data: Vec<u8>,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct ParsedBank {
    pub(super) selector_index: usize,
    pub(super) message_data_start: usize,
    pub(super) message_data_end: usize,
    pub(super) pointer_table_offset: usize,
    pub(super) pointer_table_end: usize,
    pub(super) messages: Vec<ParsedMessage>,
}

pub(super) fn parse_dialogue_banks(decoded: &[u8]) -> Result<Vec<ParsedBank>> {
    ensure!(
        decoded.len() == DECODED_IMAGE_SIZE,
        "unexpected decoded dialogue runtime image size: 0x{:x}",
        decoded.len()
    );
    let selector_end = SELECTOR_TABLE_OFFSET + SELECTOR_SLOT_COUNT * 4;
    ensure!(selector_end <= decoded.len(), "truncated selector table");

    let mut pointer_tables = Vec::new();
    for selector_index in 0..SELECTOR_SLOT_COUNT {
        let value = read_u32(decoded, SELECTOR_TABLE_OFFSET + selector_index * 4)?;
        if value == 0 {
            continue;
        }
        pointer_tables.push((
            selector_index,
            runtime_pointer_offset(value, decoded.len())?,
        ));
    }
    ensure!(
        !pointer_tables.is_empty(),
        "dialogue runtime image has no dialogue banks"
    );

    let mut message_data_start = selector_end;
    let mut banks = Vec::with_capacity(pointer_tables.len());
    for (selector_index, pointer_table_offset) in pointer_tables {
        ensure!(
            pointer_table_offset.is_multiple_of(4),
            "bank {selector_index} pointer table is not word-aligned"
        );
        ensure!(
            pointer_table_offset > message_data_start,
            "bank {selector_index} pointer table does not follow its message data"
        );

        let (message_offsets, pointer_table_end) =
            read_pointer_table(decoded, selector_index, pointer_table_offset)?;
        ensure!(
            message_offsets.first() == Some(&message_data_start),
            "bank {selector_index} first message does not begin after the preceding table"
        );
        ensure!(
            message_offsets.windows(2).all(|pair| pair[0] < pair[1]),
            "bank {selector_index} message pointers are not strictly increasing"
        );
        ensure!(
            message_offsets
                .iter()
                .all(|offset| *offset >= message_data_start && *offset < pointer_table_offset),
            "bank {selector_index} message pointer escapes its data region"
        );

        let mut messages = Vec::with_capacity(message_offsets.len());
        for (index, &decoded_offset) in message_offsets.iter().enumerate() {
            let end = message_offsets
                .get(index + 1)
                .copied()
                .unwrap_or(pointer_table_offset);
            ensure!(
                end > decoded_offset && (end - decoded_offset).is_multiple_of(2),
                "bank {selector_index} contains an empty or odd-sized message"
            );
            messages.push(ParsedMessage {
                decoded_offset,
                data: decoded[decoded_offset..end].to_vec(),
            });
        }

        banks.push(ParsedBank {
            selector_index,
            message_data_start,
            message_data_end: pointer_table_offset,
            pointer_table_offset,
            pointer_table_end,
            messages,
        });
        message_data_start = pointer_table_end;
    }
    Ok(banks)
}

fn read_pointer_table(
    decoded: &[u8],
    selector_index: usize,
    pointer_table_offset: usize,
) -> Result<(Vec<usize>, usize)> {
    let mut cursor = pointer_table_offset;
    let mut message_offsets = Vec::new();
    loop {
        ensure!(
            cursor + 4 <= decoded.len(),
            "bank {selector_index} pointer table lacks a terminator"
        );
        let value = read_u32(decoded, cursor)?;
        cursor += 4;
        if value == POINTER_TABLE_TERMINATOR {
            break;
        }
        message_offsets.push(runtime_pointer_offset(value, decoded.len())?);
    }
    ensure!(
        !message_offsets.is_empty(),
        "bank {selector_index} pointer table is empty"
    );
    Ok((message_offsets, cursor))
}

fn runtime_pointer_offset(value: u32, decoded_size: usize) -> Result<usize> {
    ensure!(
        value >= DECODED_RUNTIME_BASE,
        "pointer 0x{value:08x} is below the decoded MGK runtime image"
    );
    let offset = usize::try_from(value - DECODED_RUNTIME_BASE)?;
    ensure!(
        offset < decoded_size,
        "pointer 0x{value:08x} is outside the decoded MGK runtime image"
    );
    Ok(offset)
}

fn read_u32(data: &[u8], offset: usize) -> Result<u32> {
    ensure!(offset + 4 <= data.len(), "truncated little-endian u32");
    Ok(u32::from_le_bytes(data[offset..offset + 4].try_into()?))
}
