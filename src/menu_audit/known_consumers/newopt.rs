use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};
use psx_r3000a::{Instruction, Register, decode};

use super::super::model::MenuConsumerEvidence;
use super::super::scanner::candidate_codes;
use crate::consumer_analysis::profiles::NEWOPT_DIRECT_STRING_WRITER_ADDRESS;
use crate::pipeline::sha256_bytes;

const OVERLAY_PATH: &str = "DAT1/NEWOPT.BIN";
const SOURCE_SHA256: &str = "221f6eb284509304c4bf3c68c68eb944378f93026864bf33cf66a04aeca95187";
const RUNTIME_BASE: u32 = 0x800a_2000;
const STRING_OFFSET: usize = 0x04a8;
const STRING_POINTER_STORAGE_OFFSET: usize = 0x0be4;
pub(super) const STRING_LOAD_SEED_OFFSET: usize = 0x1b70;
pub(super) const STRING_LOAD_OFFSET: usize = 0x1b74;
pub(super) const STRING_WRITER_CALL_OFFSET: usize = 0x1b78;
pub(super) const STRING_WRITER_OFFSET: usize = 0x2fe0;
const STRING_WRITER_ADDRESS: u32 = NEWOPT_DIRECT_STRING_WRITER_ADDRESS;

pub(super) fn identify_direct_string_writer_consumer(
    overlay_path: &str,
    data: &[u8],
    runtime_base: Option<u32>,
    reachable_instruction_offsets: &BTreeSet<usize>,
) -> Result<BTreeMap<usize, Vec<MenuConsumerEvidence>>> {
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
    validate_direct_string_writer_consumer(data, RUNTIME_BASE, reachable_instruction_offsets)
}

pub(super) fn validate_direct_string_writer_consumer(
    data: &[u8],
    runtime_base: u32,
    reachable_instruction_offsets: &BTreeSet<usize>,
) -> Result<BTreeMap<usize, Vec<MenuConsumerEvidence>>> {
    ensure!(
        runtime_base == RUNTIME_BASE,
        "NEWOPT string writer moved from its source-bound runtime base"
    );
    for offset in [
        STRING_LOAD_SEED_OFFSET,
        STRING_LOAD_OFFSET,
        STRING_WRITER_CALL_OFFSET,
        STRING_WRITER_CALL_OFFSET + 4,
        STRING_WRITER_OFFSET,
    ] {
        ensure!(
            reachable_instruction_offsets.contains(&offset),
            "NEWOPT direct string writer path is not entrypoint-reachable at +0x{offset:04x}"
        );
    }
    validate_call_grammar(data)?;
    validate_writer_grammar(data)?;

    let target = read_u32(data, STRING_POINTER_STORAGE_OFFSET)?;
    let target_offset = target
        .checked_sub(runtime_base)
        .and_then(|offset| usize::try_from(offset).ok())
        .context("NEWOPT direct string pointer leaves the overlay")?;
    ensure!(
        target_offset == STRING_OFFSET && candidate_codes(data, target_offset).is_some(),
        "NEWOPT direct string pointer target changed"
    );
    Ok(BTreeMap::from([(
        target_offset,
        vec![MenuConsumerEvidence::NewoptDirectStringWriterCall],
    )]))
}

fn validate_call_grammar(data: &[u8]) -> Result<()> {
    let expected = [
        (
            STRING_LOAD_SEED_OFFSET,
            Instruction::Lui {
                rt: Register::A3,
                immediate: 0x800a,
            },
        ),
        (
            STRING_LOAD_OFFSET,
            Instruction::Lw {
                rt: Register::A3,
                base: Register::A3,
                offset: 0x2be4,
            },
        ),
        (
            STRING_WRITER_CALL_OFFSET,
            Instruction::Jal {
                target: STRING_WRITER_ADDRESS,
            },
        ),
        (
            STRING_WRITER_CALL_OFFSET + 4,
            Instruction::Addu {
                rd: Register::A2,
                rs: Register::S3,
                rt: Register::ZERO,
            },
        ),
    ];
    validate_instructions(data, &expected, "direct string call")
}

fn validate_writer_grammar(data: &[u8]) -> Result<()> {
    let expected = [
        (
            0x2fec,
            Instruction::Lhu {
                rt: Register::T1,
                base: Register::A3,
                offset: 0,
            },
        ),
        (
            0x2ff0,
            Instruction::Addiu {
                rt: Register::A3,
                rs: Register::A3,
                immediate: 2,
            },
        ),
        (
            0x3030,
            Instruction::Lhu {
                rt: Register::V0,
                base: Register::A3,
                offset: 0,
            },
        ),
        (
            0x3034,
            Instruction::Addiu {
                rt: Register::A3,
                rs: Register::A3,
                immediate: 2,
            },
        ),
        (
            0x303c,
            Instruction::Addiu {
                rt: Register::T0,
                rs: Register::T0,
                immediate: 1,
            },
        ),
        (
            0x3048,
            Instruction::Slt {
                rd: Register::V0,
                rs: Register::T0,
                rt: Register::T1,
            },
        ),
    ];
    validate_instructions(data, &expected, "string writer")
}

fn validate_instructions(data: &[u8], expected: &[(usize, Instruction)], role: &str) -> Result<()> {
    for (offset, expected_instruction) in expected {
        let pc = RUNTIME_BASE + *offset as u32;
        let instruction = decode(read_u32(data, *offset)?, pc)
            .with_context(|| format!("failed to decode NEWOPT {role} at +0x{offset:04x}"))?;
        ensure!(
            instruction == *expected_instruction,
            "NEWOPT {role} grammar changed at +0x{offset:04x}"
        );
    }
    Ok(())
}

fn read_u32(data: &[u8], offset: usize) -> Result<u32> {
    let bytes = data
        .get(offset..offset + 4)
        .with_context(|| format!("truncated NEWOPT data at +0x{offset:04x}"))?;
    Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
}
