use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};
use psx_r3000a::{Instruction, Register, decode};

use super::super::model::MenuConsumerEvidence;
use super::super::scanner::candidate_codes;
use crate::consumer_analysis::profiles::MGTIT_MENU_TEXT_RENDERER_ADDRESS;
use crate::pipeline::sha256_bytes;
use crate::psx_static_analysis::value_flow::ResolvedDirectCallArgument;

const OVERLAY_PATH: &str = "DAT1/MGTIT.BIN";
const SOURCE_SHA256: &str = "859515ec9f58a2326708dbbf46516e8601cb59d158c0da3f2e214311762a07d1";
const RUNTIME_BASE: u32 = 0x800a_2000;
const PLACEMENT_TABLE_OFFSET: usize = 0x02a0;
const PLACEMENT_RECORD_SIZE: usize = 12;
const PLACEMENT_RECORD_COUNT: usize = 18;
pub(super) const RENDERER_OFFSET: usize = 0x30b4;
pub(super) const RENDERER_ADDRESS: u32 = MGTIT_MENU_TEXT_RENDERER_ADDRESS;

pub(super) fn identify_placement_record_consumers(
    overlay_path: &str,
    data: &[u8],
    runtime_base: Option<u32>,
    reachable_instruction_offsets: &BTreeSet<usize>,
    reachable_lui_seed_offsets: &BTreeSet<usize>,
    direct_call_arguments: &[ResolvedDirectCallArgument],
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
    validate_placement_record_consumers(
        data,
        RUNTIME_BASE,
        reachable_instruction_offsets,
        reachable_lui_seed_offsets,
        direct_call_arguments,
    )
}

pub(super) fn validate_placement_record_consumers(
    data: &[u8],
    runtime_base: u32,
    reachable_instruction_offsets: &BTreeSet<usize>,
    reachable_lui_seed_offsets: &BTreeSet<usize>,
    direct_call_arguments: &[ResolvedDirectCallArgument],
) -> Result<BTreeMap<usize, Vec<MenuConsumerEvidence>>> {
    ensure!(
        runtime_base == RUNTIME_BASE,
        "MGTIT placement renderer moved from its source-bound runtime base"
    );
    ensure!(
        reachable_instruction_offsets.contains(&RENDERER_OFFSET),
        "MGTIT placement renderer is not entrypoint-reachable"
    );
    validate_mgtit_placement_renderer(data)?;

    let records = placement_records(data, runtime_base)?;
    let expected_record_addresses = records
        .iter()
        .map(|record| record.runtime_address)
        .collect::<BTreeSet<_>>();
    let observed_record_addresses = direct_call_arguments
        .iter()
        .filter(|argument| {
            argument.target == RENDERER_ADDRESS
                && argument.argument_register == Register::A1
                && reachable_lui_seed_offsets.contains(&argument.seed_offset)
                && reachable_instruction_offsets.contains(&argument.instruction_offset)
        })
        .map(|argument| argument.value)
        .collect::<BTreeSet<_>>();
    let missing_record_addresses = expected_record_addresses
        .difference(&observed_record_addresses)
        .map(|address| format!("0x{address:08x}"))
        .collect::<Vec<_>>();
    let unexpected_record_addresses = observed_record_addresses
        .difference(&expected_record_addresses)
        .map(|address| format!("0x{address:08x}"))
        .collect::<Vec<_>>();
    let unreachable_argument_contexts = direct_call_arguments
        .iter()
        .filter(|argument| {
            argument.target == RENDERER_ADDRESS
                && argument.argument_register == Register::A1
                && expected_record_addresses.contains(&argument.value)
                && !observed_record_addresses.contains(&argument.value)
        })
        .map(|argument| {
            format!(
                "value=0x{:08x}/seed=0x{:04x}/call=0x{:04x}/seed_reachable={}/call_reachable={}",
                argument.value,
                argument.seed_offset,
                argument.instruction_offset,
                reachable_lui_seed_offsets.contains(&argument.seed_offset),
                reachable_instruction_offsets.contains(&argument.instruction_offset),
            )
        })
        .collect::<Vec<_>>();
    ensure!(
        observed_record_addresses == expected_record_addresses,
        "MGTIT renderer call sites do not cover the complete placement table; missing: {}; unexpected: {}; excluded arguments: {}",
        missing_record_addresses.join(", "),
        unexpected_record_addresses.join(", "),
        unreachable_argument_contexts.join(", ")
    );

    let mut consumers = BTreeMap::new();
    for record in records {
        ensure!(
            consumers
                .insert(
                    record.string_offset,
                    vec![MenuConsumerEvidence::MgtitPlacementRecordTable],
                )
                .is_none(),
            "MGTIT placement table repeats a string target"
        );
    }
    Ok(consumers)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PlacementRecord {
    runtime_address: u32,
    string_offset: usize,
}

fn placement_records(data: &[u8], runtime_base: u32) -> Result<Vec<PlacementRecord>> {
    let mut records = Vec::with_capacity(PLACEMENT_RECORD_COUNT);
    for index in 0..PLACEMENT_RECORD_COUNT {
        let offset = PLACEMENT_TABLE_OFFSET + index * PLACEMENT_RECORD_SIZE;
        let pointer = read_u32(data, offset + 4)?;
        ensure!(
            read_u32(data, offset + 8)? == 0,
            "MGTIT placement record {index} reserved word is not zero"
        );
        let string_offset = pointer
            .checked_sub(runtime_base)
            .and_then(|value| usize::try_from(value).ok())
            .with_context(|| {
                format!("MGTIT placement record {index} points outside the overlay")
            })?;
        ensure!(
            candidate_codes(data, string_offset).is_some(),
            "MGTIT placement record {index} does not point to an admissible menu string"
        );
        records.push(PlacementRecord {
            runtime_address: runtime_base
                .checked_add(u32::try_from(offset)?)
                .context("MGTIT placement record address overflow")?,
            string_offset,
        });
    }
    Ok(records)
}

pub(crate) fn validate_mgtit_placement_renderer(data: &[u8]) -> Result<()> {
    let expected = [
        (
            0x30e8,
            Instruction::Lh {
                rt: Register::S4,
                base: Register::A1,
                offset: 0,
            },
        ),
        (
            0x30ec,
            Instruction::Lw {
                rt: Register::S5,
                base: Register::A1,
                offset: 4,
            },
        ),
        (
            0x30f0,
            Instruction::Lh {
                rt: Register::T0,
                base: Register::A1,
                offset: 2,
            },
        ),
        (
            0x3100,
            Instruction::Lhu {
                rt: Register::T0,
                base: Register::S5,
                offset: 0,
            },
        ),
        (
            0x3114,
            Instruction::Addiu {
                rt: Register::S5,
                rs: Register::S5,
                immediate: 2,
            },
        ),
        (
            0x316c,
            Instruction::Lhu {
                rt: Register::V1,
                base: Register::S5,
                offset: 0,
            },
        ),
        (
            0x3170,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::ZERO,
                immediate: 0x0fff,
            },
        ),
        (
            0x3174,
            Instruction::Andi {
                rt: Register::V1,
                rs: Register::V1,
                immediate: 0x0fff,
            },
        ),
        (
            0x317c,
            Instruction::Addiu {
                rt: Register::S5,
                rs: Register::S5,
                immediate: 2,
            },
        ),
        (
            0x3198,
            Instruction::Andi {
                rt: Register::V0,
                rs: Register::V1,
                immediate: 0x000f,
            },
        ),
        (
            0x319c,
            Instruction::Sll {
                rd: Register::S2,
                rt: Register::V0,
                shift: 2,
            },
        ),
        (
            0x31a0,
            Instruction::Addu {
                rd: Register::S2,
                rs: Register::S2,
                rt: Register::V0,
            },
        ),
        (
            0x31a4,
            Instruction::Sll {
                rd: Register::S2,
                rt: Register::S2,
                shift: 2,
            },
        ),
        (
            0x31a8,
            Instruction::Sra {
                rd: Register::V0,
                rt: Register::V1,
                shift: 4,
            },
        ),
        (
            0x31ac,
            Instruction::Andi {
                rt: Register::V0,
                rs: Register::V0,
                immediate: 0x000f,
            },
        ),
        (
            0x31b0,
            Instruction::Sll {
                rd: Register::S1,
                rt: Register::V0,
                shift: 2,
            },
        ),
        (
            0x31b4,
            Instruction::Addu {
                rd: Register::S1,
                rs: Register::S1,
                rt: Register::V0,
            },
        ),
        (
            0x31c0,
            Instruction::Sll {
                rd: Register::S1,
                rt: Register::S1,
                shift: 2,
            },
        ),
        (
            0x3258,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::ZERO,
                immediate: 20,
            },
        ),
        (
            0x32dc,
            Instruction::Slt {
                rd: Register::V0,
                rs: Register::S6,
                rt: Register::T0,
            },
        ),
    ];
    for (offset, expected_instruction) in expected {
        let pc = RUNTIME_BASE + offset as u32;
        let instruction = decode(read_u32(data, offset)?, pc)
            .with_context(|| format!("failed to decode MGTIT renderer at +0x{offset:04x}"))?;
        ensure!(
            instruction == expected_instruction,
            "MGTIT renderer grammar changed at +0x{offset:04x}"
        );
    }
    Ok(())
}

fn read_u32(data: &[u8], offset: usize) -> Result<u32> {
    let bytes = data
        .get(offset..offset + 4)
        .with_context(|| format!("truncated MGTIT data at +0x{offset:04x}"))?;
    Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
}
