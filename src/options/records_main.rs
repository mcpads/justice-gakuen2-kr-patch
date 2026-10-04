use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};

use crate::pipeline::difference_ranges;
use crate::source_disc::{MAIN_TEXT_RUNTIME_BASE, PSX_EXE_HEADER_SIZE};
use crate::write_scope::changed_ranges_are_within;

pub(super) const UNIT_IDS: [&str; 4] = [
    "records_heading",
    "records_load",
    "records_save",
    "records_automatic",
];
pub(super) const MAIN_EXECUTABLE_UNIT_IDS: [&str; 7] = [
    "records_heading",
    "records_load",
    "records_save",
    "records_automatic",
    "disabled",
    "enabled",
    "exit",
];

pub(super) const HEADING_GLYPHS: [(char, u16); 2] = [('기', 0x0368), ('록', 0x036a)];

const POOL_START: usize = 0x05f0;
const POOL_END: usize = 0x0610;

const MAIN_EXECUTABLE_POOL_START: usize = 0x0807e0;
const MAIN_EXECUTABLE_POOL_END: usize = 0x080800;
const MAIN_EXECUTABLE_SOURCE_POOL: [u8; 0x20] = [
    0x03, 0x00, 0x68, 0x03, 0xff, 0x0f, 0x6a, 0x03, 0x03, 0x00, 0x56, 0x01, 0x71, 0x00, 0x71, 0x01,
    0x03, 0x00, 0x31, 0x01, 0x71, 0x00, 0x74, 0x01, 0x03, 0x00, 0x24, 0x01, 0x71, 0x00, 0x37, 0x01,
];
const MAIN_EXECUTABLE_SOURCE_OFFSETS: [(&str, usize); 4] = [
    ("records_heading", 0x0807e0),
    ("records_load", 0x0807e8),
    ("records_save", 0x0807f0),
    ("records_automatic", 0x0807f8),
];
const MAIN_EXECUTABLE_POINTER_OFFSETS: [(&str, usize); 4] = [
    ("records_heading", 0x080e28),
    ("records_load", 0x080e2c),
    ("records_save", 0x080e30),
    ("records_automatic", 0x080e34),
];
const MAIN_EXECUTABLE_IN_PLACE_RECORDS: [(&str, usize, usize, usize); 3] = [
    ("disabled", 0x080678, 0x080680, 0x080dc4),
    ("enabled", 0x080680, 0x080686, 0x080dc8),
    ("exit", 0x080c00, 0x080c0c, 0x080f40),
];
const MAIN_EXECUTABLE_DISABLED_SOURCE: [u8; 8] = [0x03, 0x00, 0x83, 0x00, 0x90, 0x00, 0x75, 0x00];
const MAIN_EXECUTABLE_ENABLED_SOURCE: [u8; 6] = [0x02, 0x00, 0x84, 0x00, 0xa8, 0x00];
const MAIN_EXECUTABLE_EXIT_SOURCE: [u8; 12] = [
    0x04, 0x00, 0x12, 0x00, 0x29, 0x00, 0x16, 0x00, 0x25, 0x00, 0x00, 0x00,
];

pub(super) fn contains_unit(id: &str) -> bool {
    UNIT_IDS.contains(&id)
}

pub(super) fn contains_main_executable_unit(id: &str) -> bool {
    MAIN_EXECUTABLE_UNIT_IDS.contains(&id)
}

pub(super) fn rebuild_pool<'a>(
    output: &mut [u8],
    records: &[(&'a str, &[u16])],
) -> Result<BTreeMap<&'a str, usize>> {
    let (bytes, relative_offsets) = pack_records(records, POOL_END - POOL_START)?;
    output
        .get_mut(POOL_START..POOL_END)
        .context("records main pool is outside NEWOPT")?
        .copy_from_slice(&bytes);
    Ok(relative_offsets
        .into_iter()
        .map(|(id, offset)| (id, POOL_START + offset))
        .collect())
}

pub(super) const fn expected_write_range() -> [usize; 2] {
    [POOL_START, POOL_END]
}

pub(super) fn rebuild_main_executable(
    source: &[u8],
    records: &[(&str, &[u16])],
) -> Result<Vec<u8>> {
    ensure!(
        source.starts_with(b"PS-X EXE"),
        "records main source is not a PS-X EXE"
    );
    ensure!(
        source.get(MAIN_EXECUTABLE_POOL_START..MAIN_EXECUTABLE_POOL_END)
            == Some(MAIN_EXECUTABLE_SOURCE_POOL.as_slice()),
        "records main source pool changed in the main executable"
    );
    for ((id, pointer_offset), (source_id, source_offset)) in MAIN_EXECUTABLE_POINTER_OFFSETS
        .into_iter()
        .zip(MAIN_EXECUTABLE_SOURCE_OFFSETS)
    {
        ensure!(
            id == source_id,
            "records main source and pointer order changed"
        );
        let pointer = read_u32(source, pointer_offset)?;
        ensure!(
            pointer == runtime_address(source_offset)?,
            "records main {id} source pointer changed"
        );
    }
    for (id, start, end, pointer_offset) in MAIN_EXECUTABLE_IN_PLACE_RECORDS {
        let expected_source = match id {
            "disabled" => MAIN_EXECUTABLE_DISABLED_SOURCE.as_slice(),
            "enabled" => MAIN_EXECUTABLE_ENABLED_SOURCE.as_slice(),
            "exit" => MAIN_EXECUTABLE_EXIT_SOURCE.as_slice(),
            _ => unreachable!("closed records-main in-place record set"),
        };
        ensure!(
            source.get(start..end) == Some(expected_source),
            "records main {id} source record changed in the main executable"
        );
        ensure!(
            read_u32(source, pointer_offset)? == runtime_address(start)?,
            "records main {id} source pointer changed"
        );
    }

    ensure!(
        records.iter().map(|(id, _)| *id).collect::<BTreeSet<_>>()
            == MAIN_EXECUTABLE_UNIT_IDS.into_iter().collect(),
        "records main main-executable membership changed"
    );
    let pooled_records = records
        .iter()
        .filter(|(id, _)| contains_unit(id))
        .copied()
        .collect::<Vec<_>>();
    let (bytes, relative_offsets) = pack_records(
        &pooled_records,
        MAIN_EXECUTABLE_POOL_END - MAIN_EXECUTABLE_POOL_START,
    )?;
    let mut output = source.to_vec();
    output[MAIN_EXECUTABLE_POOL_START..MAIN_EXECUTABLE_POOL_END].copy_from_slice(&bytes);
    for (id, pointer_offset) in MAIN_EXECUTABLE_POINTER_OFFSETS {
        let relative_offset = *relative_offsets
            .get(id)
            .with_context(|| format!("records main main-executable pool is missing {id}"))?;
        write_u32(
            &mut output,
            pointer_offset,
            runtime_address(MAIN_EXECUTABLE_POOL_START + relative_offset)?,
        )?;
    }
    for (id, start, end, _) in MAIN_EXECUTABLE_IN_PLACE_RECORDS {
        let codes = records
            .iter()
            .find(|(record_id, _)| *record_id == id)
            .map(|(_, codes)| *codes)
            .with_context(|| format!("records main main executable is missing {id}"))?;
        write_record(&mut output[start..end], codes, id)?;
    }
    ensure!(
        changed_ranges_are_within(
            &difference_ranges(source, &output),
            &main_executable_expected_write_ranges(),
        ),
        "records main patch escaped its main-executable Expected Writes"
    );
    Ok(output)
}

pub(super) const fn main_executable_expected_write_ranges() -> [[usize; 2]; 4] {
    [
        [0x080678, 0x080686],
        [MAIN_EXECUTABLE_POOL_START, MAIN_EXECUTABLE_POOL_END],
        [0x080c00, 0x080c0c],
        [0x080e28, 0x080e38],
    ]
}

fn pack_records<'a>(
    records: &[(&'a str, &[u16])],
    capacity: usize,
) -> Result<(Vec<u8>, BTreeMap<&'a str, usize>)> {
    ensure!(
        records.iter().map(|(id, _)| *id).collect::<BTreeSet<_>>()
            == UNIT_IDS.into_iter().collect(),
        "records main pool membership changed"
    );
    let mut bytes = Vec::with_capacity(capacity);
    let mut offsets = BTreeMap::new();
    for id in UNIT_IDS {
        let (record_id, codes) = records
            .iter()
            .find(|(record_id, _)| *record_id == id)
            .map(|(record_id, codes)| (*record_id, *codes))
            .with_context(|| format!("records main pool is missing {id}"))?;
        ensure!(
            codes.len() <= usize::from(u16::MAX),
            "records main {id} code count exceeds u16"
        );
        offsets.insert(record_id, bytes.len());
        bytes.extend_from_slice(&(codes.len() as u16).to_le_bytes());
        for code in codes {
            bytes.extend_from_slice(&code.to_le_bytes());
        }
    }
    ensure!(
        bytes.len() <= capacity,
        "records main strings need {} bytes but their pool owns {capacity}",
        bytes.len()
    );
    bytes.resize(capacity, 0);
    Ok((bytes, offsets))
}

fn runtime_address(file_offset: usize) -> Result<u32> {
    let text_offset = file_offset
        .checked_sub(PSX_EXE_HEADER_SIZE)
        .context("records main file offset precedes the PS-X EXE header")?;
    MAIN_TEXT_RUNTIME_BASE
        .checked_add(u32::try_from(text_offset)?)
        .context("records main runtime address overflow")
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    let value = bytes
        .get(offset..offset + 4)
        .context("records main pointer is outside the main executable")?;
    Ok(u32::from_le_bytes(value.try_into().unwrap()))
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) -> Result<()> {
    bytes
        .get_mut(offset..offset + 4)
        .context("records main pointer is outside the main executable")?
        .copy_from_slice(&value.to_le_bytes());
    Ok(())
}

fn write_record(output: &mut [u8], codes: &[u16], id: &str) -> Result<()> {
    let required = 2 + codes.len() * 2;
    ensure!(
        required <= output.len(),
        "records main {id} needs {required} bytes but owns {}",
        output.len()
    );
    output.fill(0);
    output[..2].copy_from_slice(&(codes.len() as u16).to_le_bytes());
    for (index, code) in codes.iter().enumerate() {
        let start = 2 + index * 2;
        output[start..start + 2].copy_from_slice(&code.to_le_bytes());
    }
    Ok(())
}
