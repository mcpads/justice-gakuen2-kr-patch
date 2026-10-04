use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};
use psx_r3000a::{Instruction, Register, decode};

use super::super::model::MenuNonTextEvidence;
use super::super::scanner::candidate_codes;
use crate::pipeline::sha256_bytes;

const OVERLAY_PATH: &str = "DAT1/MINISEL.BIN";
const SOURCE_SHA256: &str = "dca00b504a885005658e860352e04ee7a4e66f774c3ce9b401e634b6b1398694";
const RUNTIME_BASE: u32 = 0x800a_2000;
const PRIMITIVE_PARSER_OFFSET: usize = 0x0cd0;
const PRIMARY_CALL_OFFSET: usize = 0x16b8;
const SELECTED_CALL_OFFSET: usize = 0x16e8;
const POINTER_TABLE_OFFSET: usize = 0x0228;
const PRIMITIVE_HALFWORDS_PER_ENTRY: usize = 11;

pub(super) const PRIMITIVE_RECORD_OFFSETS: [usize; 14] = [
    0x0010, 0x0028, 0x0040, 0x0058, 0x0070, 0x0088, 0x00a0, 0x00b8, 0x00d0, 0x0114, 0x012c, 0x0170,
    0x01b4, 0x0210,
];

const PRIMITIVE_COUNTS: [u16; 14] = [1, 1, 1, 1, 1, 1, 1, 1, 3, 1, 3, 3, 4, 1];

pub(super) const NON_TEXT_CANDIDATE_OFFSETS: [usize; 24] = [
    0x0010, 0x0022, 0x0024, 0x0028, 0x0036, 0x003a, 0x003c, 0x0040, 0x004e, 0x0052, 0x0054, 0x0058,
    0x0070, 0x0088, 0x00a0, 0x00b8, 0x00d0, 0x0114, 0x012c, 0x0170, 0x01a4, 0x01b4, 0x01e8, 0x0210,
];

pub(super) fn identify_primitive_record_candidates(
    overlay_path: &str,
    data: &[u8],
    runtime_base: Option<u32>,
    reachable_instruction_offsets: &BTreeSet<usize>,
) -> Result<BTreeMap<usize, Vec<MenuNonTextEvidence>>> {
    if overlay_path != OVERLAY_PATH {
        return Ok(BTreeMap::new());
    }
    ensure!(
        sha256_bytes(data) == SOURCE_SHA256,
        "unsupported {OVERLAY_PATH} source bytes"
    );
    ensure!(
        runtime_base == Some(RUNTIME_BASE),
        "unexpected {OVERLAY_PATH} runtime base"
    );
    validate_primitive_record_candidates(data, RUNTIME_BASE, reachable_instruction_offsets)
}

pub(super) fn validate_primitive_record_candidates(
    data: &[u8],
    runtime_base: u32,
    reachable_instruction_offsets: &BTreeSet<usize>,
) -> Result<BTreeMap<usize, Vec<MenuNonTextEvidence>>> {
    ensure!(
        runtime_base == RUNTIME_BASE,
        "MINISEL primitive records moved from their source-bound runtime base"
    );
    for required_offset in [
        PRIMITIVE_PARSER_OFFSET,
        PRIMARY_CALL_OFFSET,
        SELECTED_CALL_OFFSET,
    ] {
        ensure!(
            reachable_instruction_offsets.contains(&required_offset),
            "MINISEL primitive record consumer +0x{required_offset:04x} is not entrypoint-reachable"
        );
    }
    validate_consumer_grammar(data)?;
    validate_primitive_records(data, runtime_base)?;

    NON_TEXT_CANDIDATE_OFFSETS
        .into_iter()
        .map(|offset| {
            ensure!(
                candidate_codes(data, offset).is_some(),
                "MINISEL non-text candidate +0x{offset:04x} no longer matches the string heuristic"
            );
            ensure!(
                primitive_record_containing(offset).is_some(),
                "MINISEL non-text candidate +0x{offset:04x} escaped the primitive records"
            );
            Ok((
                offset,
                vec![MenuNonTextEvidence::MiniselPrimitiveRecordTable],
            ))
        })
        .collect()
}

fn validate_primitive_records(data: &[u8], runtime_base: u32) -> Result<()> {
    for (index, (&record_offset, &count)) in PRIMITIVE_RECORD_OFFSETS
        .iter()
        .zip(PRIMITIVE_COUNTS.iter())
        .enumerate()
    {
        let pointer = read_u32(data, POINTER_TABLE_OFFSET + index * 4)?;
        ensure!(
            pointer == runtime_base + record_offset as u32,
            "MINISEL primitive pointer {index} changed"
        );
        ensure!(
            read_u16(data, record_offset)? == count,
            "MINISEL primitive record +0x{record_offset:04x} count changed"
        );
        let record_end = record_offset
            .checked_add(2 + usize::from(count) * PRIMITIVE_HALFWORDS_PER_ENTRY * 2)
            .context("MINISEL primitive record size overflow")?;
        let next_offset = PRIMITIVE_RECORD_OFFSETS
            .get(index + 1)
            .copied()
            .unwrap_or(POINTER_TABLE_OFFSET);
        ensure!(
            record_end <= next_offset && next_offset - record_end < 4,
            "MINISEL primitive record +0x{record_offset:04x} no longer fills its bounded region"
        );
        ensure!(
            data.get(record_end..next_offset)
                .is_some_and(|padding| padding.iter().all(|byte| *byte == 0)),
            "MINISEL primitive record +0x{record_offset:04x} padding changed"
        );
    }
    Ok(())
}

fn primitive_record_containing(offset: usize) -> Option<usize> {
    PRIMITIVE_RECORD_OFFSETS
        .iter()
        .zip(PRIMITIVE_COUNTS.iter())
        .find_map(|(&record_offset, &count)| {
            let end = record_offset + 2 + usize::from(count) * PRIMITIVE_HALFWORDS_PER_ENTRY * 2;
            (record_offset <= offset && offset < end).then_some(record_offset)
        })
}

pub(super) const CONSUMER_GRAMMAR: &[(usize, Instruction)] = &[
    (
        0x0d00,
        Instruction::Lh {
            rt: Register::S5,
            base: Register::S2,
            offset: 0,
        },
    ),
    (
        0x0d04,
        Instruction::Addiu {
            rt: Register::S2,
            rs: Register::S2,
            immediate: 2,
        },
    ),
    (
        0x0d10,
        Instruction::Lh {
            rt: Register::A0,
            base: Register::S2,
            offset: 0,
        },
    ),
    (
        0x0d18,
        Instruction::Lh {
            rt: Register::A2,
            base: Register::S2,
            offset: 0,
        },
    ),
    (
        0x0d20,
        Instruction::Lh {
            rt: Register::A3,
            base: Register::S2,
            offset: 0,
        },
    ),
    (
        0x0e14,
        Instruction::Lhu {
            rt: Register::V0,
            base: Register::S2,
            offset: 0,
        },
    ),
    (
        0x0e38,
        Instruction::Addiu {
            rt: Register::S2,
            rs: Register::S2,
            immediate: 2,
        },
    ),
    (
        0x0e64,
        Instruction::Slt {
            rd: Register::V0,
            rs: Register::S3,
            rt: Register::S5,
        },
    ),
    (
        0x0e68,
        Instruction::Bne {
            rs: Register::V0,
            rt: Register::ZERO,
            target: RUNTIME_BASE + 0x0d10,
        },
    ),
    (
        0x1698,
        Instruction::Addu {
            rd: Register::S3,
            rs: Register::ZERO,
            rt: Register::ZERO,
        },
    ),
    (
        0x169c,
        Instruction::Lui {
            rt: Register::S1,
            immediate: 0x800a,
        },
    ),
    (
        0x16a0,
        Instruction::Addiu {
            rt: Register::S1,
            rs: Register::S1,
            immediate: 0x2228,
        },
    ),
    (
        0x16a8,
        Instruction::Lw {
            rt: Register::A1,
            base: Register::S1,
            offset: 0,
        },
    ),
    (
        0x16b0,
        Instruction::Addiu {
            rt: Register::S1,
            rs: Register::S1,
            immediate: 4,
        },
    ),
    (
        PRIMARY_CALL_OFFSET,
        Instruction::Jal {
            target: RUNTIME_BASE + PRIMITIVE_PARSER_OFFSET as u32,
        },
    ),
    (
        0x16bc,
        Instruction::Addiu {
            rt: Register::S3,
            rs: Register::S3,
            immediate: 1,
        },
    ),
    (
        0x16c0,
        Instruction::Slti {
            rt: Register::V0,
            rs: Register::S3,
            immediate: 8,
        },
    ),
    (
        0x16c4,
        Instruction::Bne {
            rs: Register::V0,
            rt: Register::ZERO,
            target: RUNTIME_BASE + 0x16a8,
        },
    ),
    (
        0x16cc,
        Instruction::Slti {
            rt: Register::V0,
            rs: Register::S6,
            immediate: 6,
        },
    ),
    (
        0x16d8,
        Instruction::Sll {
            rd: Register::V0,
            rt: Register::S6,
            shift: 2,
        },
    ),
    (
        0x16dc,
        Instruction::Lui {
            rt: Register::AT,
            immediate: 0x800a,
        },
    ),
    (
        0x16e0,
        Instruction::Addu {
            rd: Register::AT,
            rs: Register::AT,
            rt: Register::V0,
        },
    ),
    (
        0x16e4,
        Instruction::Lw {
            rt: Register::A1,
            base: Register::AT,
            offset: 0x2248,
        },
    ),
    (
        SELECTED_CALL_OFFSET,
        Instruction::Jal {
            target: RUNTIME_BASE + PRIMITIVE_PARSER_OFFSET as u32,
        },
    ),
];

fn validate_consumer_grammar(data: &[u8]) -> Result<()> {
    for (offset, expected_instruction) in CONSUMER_GRAMMAR {
        let instruction = decode(read_u32(data, *offset)?, RUNTIME_BASE + *offset as u32)
            .with_context(|| format!("failed to decode MINISEL at +0x{offset:04x}"))?;
        ensure!(
            instruction == *expected_instruction,
            "MINISEL primitive consumer grammar changed at +0x{offset:04x}"
        );
    }
    Ok(())
}

fn read_u16(data: &[u8], offset: usize) -> Result<u16> {
    let bytes = data
        .get(offset..offset + 2)
        .with_context(|| format!("truncated MINISEL data at +0x{offset:04x}"))?;
    Ok(u16::from_le_bytes(bytes.try_into().unwrap()))
}

fn read_u32(data: &[u8], offset: usize) -> Result<u32> {
    let bytes = data
        .get(offset..offset + 4)
        .with_context(|| format!("truncated MINISEL data at +0x{offset:04x}"))?;
    Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
}
