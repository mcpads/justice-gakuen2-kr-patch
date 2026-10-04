use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};
use psx_r3000a::{Instruction, Register, decode};

use super::super::model::MenuNonTextEvidence;
use super::super::scanner::candidate_codes;
use crate::pipeline::sha256_bytes;

const OVERLAY_PATH: &str = "DAT1/KANRI.BIN";
const SOURCE_SHA256: &str = "be317e6eb86ad7d15c68a1c6a3bf8237009a73c1b8edc4e4cdc5350576c20627";
const RUNTIME_BASE: u32 = 0x800a_2000;
const RECORD_HALFWORD_COUNT: usize = 11;
const OBJECT_RECORD_PARSER_OFFSET: usize = 0x6880;

#[derive(Clone, Copy)]
pub(super) struct CountedObjectRecordTable {
    pub(super) offset: usize,
    pub(super) count: u16,
    pub(super) next_offset: usize,
    pub(super) pointer_storage_offset: usize,
}

pub(super) const RECORD_TABLES: [CountedObjectRecordTable; 3] = [
    CountedObjectRecordTable {
        offset: 0x01a0,
        count: 7,
        next_offset: 0x023c,
        pointer_storage_offset: 0x023c,
    },
    CountedObjectRecordTable {
        offset: 0x0fd0,
        count: 2,
        next_offset: 0x1000,
        pointer_storage_offset: 0x1030,
    },
    CountedObjectRecordTable {
        offset: 0x1000,
        count: 2,
        next_offset: 0x1030,
        pointer_storage_offset: 0x1034,
    },
];

pub(super) const REQUIRED_REACHABLE_OFFSETS: [usize; 17] = [
    0x4da0,
    0x4da4,
    0x4db0,
    0x4db4,
    0x4db8,
    0x596c,
    0x5970,
    0x5980,
    0x5984,
    0x5988,
    0x6678,
    0x667c,
    0x6680,
    0x6818,
    0x681c,
    0x6820,
    OBJECT_RECORD_PARSER_OFFSET,
];

pub(super) fn identify_counted_object_record_candidates(
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
    validate_counted_object_record_candidates(data, RUNTIME_BASE, reachable_instruction_offsets)
}

pub(super) fn validate_counted_object_record_candidates(
    data: &[u8],
    runtime_base: u32,
    reachable_instruction_offsets: &BTreeSet<usize>,
) -> Result<BTreeMap<usize, Vec<MenuNonTextEvidence>>> {
    ensure!(
        runtime_base == RUNTIME_BASE,
        "KANRI counted object records moved from their source-bound runtime base"
    );
    for required_offset in REQUIRED_REACHABLE_OFFSETS {
        ensure!(
            reachable_instruction_offsets.contains(&required_offset),
            "KANRI counted object record consumer +0x{required_offset:04x} is not entrypoint-reachable"
        );
    }
    validate_instructions(data, &caller_grammar(), "caller")?;
    validate_instructions(data, &parser_grammar(), "parser")?;
    validate_record_tables(data, runtime_base)?;

    RECORD_TABLES
        .into_iter()
        .map(|table| {
            ensure!(
                candidate_codes(data, table.offset).is_some(),
                "KANRI non-text candidate +0x{:04x} no longer matches the string heuristic",
                table.offset
            );
            Ok((
                table.offset,
                vec![MenuNonTextEvidence::KanriCountedObjectRecordTable],
            ))
        })
        .collect()
}

fn validate_record_tables(data: &[u8], runtime_base: u32) -> Result<()> {
    for table in RECORD_TABLES {
        ensure!(
            read_u32(data, table.pointer_storage_offset)? == runtime_base + table.offset as u32,
            "KANRI counted object record pointer for +0x{:04x} changed",
            table.offset
        );
        ensure!(
            read_u16(data, table.offset)? == table.count,
            "KANRI counted object record +0x{:04x} count changed",
            table.offset
        );
        let record_end = table
            .offset
            .checked_add(2 + usize::from(table.count) * RECORD_HALFWORD_COUNT * 2)
            .context("KANRI counted object record size overflow")?;
        ensure!(
            record_end <= table.next_offset && table.next_offset - record_end < 4,
            "KANRI counted object record +0x{:04x} no longer fills its bounded region",
            table.offset
        );
        ensure!(
            data.get(record_end..table.next_offset)
                .is_some_and(|padding| padding.iter().all(|byte| *byte == 0)),
            "KANRI counted object record +0x{:04x} padding changed",
            table.offset
        );
    }
    Ok(())
}

pub(super) fn caller_grammar() -> Vec<(usize, Instruction)> {
    let mut grammar = Vec::new();

    push_pointer_load(&mut grammar, 0x4da0, 0x4da4, 0x3034);
    grammar.push((
        0x4da8,
        Instruction::J {
            target: RUNTIME_BASE + 0x4db8,
        },
    ));
    push_pointer_load(&mut grammar, 0x4db0, 0x4db4, 0x3030);
    push_parser_call(&mut grammar, 0x4db8, 9);

    push_pointer_load(&mut grammar, 0x596c, 0x5970, 0x3034);
    grammar.push((
        0x5974,
        Instruction::J {
            target: RUNTIME_BASE + 0x5988,
        },
    ));
    push_pointer_load(&mut grammar, 0x5980, 0x5984, 0x3030);
    push_parser_call(&mut grammar, 0x5988, 9);

    push_pointer_load(&mut grammar, 0x6678, 0x667c, 0x223c);
    push_parser_call(&mut grammar, 0x6680, 0x0422);
    push_pointer_load(&mut grammar, 0x6818, 0x681c, 0x223c);
    push_parser_call(&mut grammar, 0x6820, 0x0422);

    grammar
}

fn push_pointer_load(
    grammar: &mut Vec<(usize, Instruction)>,
    seed_offset: usize,
    load_offset: usize,
    storage_offset: i16,
) {
    grammar.extend([
        (
            seed_offset,
            Instruction::Lui {
                rt: Register::A1,
                immediate: 0x800a,
            },
        ),
        (
            load_offset,
            Instruction::Lw {
                rt: Register::A1,
                base: Register::A1,
                offset: storage_offset,
            },
        ),
    ]);
}

fn push_parser_call(grammar: &mut Vec<(usize, Instruction)>, call_offset: usize, mode: i16) {
    grammar.extend([
        (
            call_offset,
            Instruction::Jal {
                target: RUNTIME_BASE + OBJECT_RECORD_PARSER_OFFSET as u32,
            },
        ),
        (
            call_offset + 4,
            Instruction::Addiu {
                rt: Register::A2,
                rs: Register::ZERO,
                immediate: mode,
            },
        ),
    ]);
}

pub(super) fn parser_grammar() -> Vec<(usize, Instruction)> {
    let mut grammar = vec![
        (
            0x6890,
            Instruction::Addu {
                rd: Register::S2,
                rs: Register::A1,
                rt: Register::ZERO,
            },
        ),
        (
            0x68b0,
            Instruction::Lh {
                rt: Register::S5,
                base: Register::S2,
                offset: 0,
            },
        ),
        (0x68b4, advance_record_cursor()),
    ];

    for (load_offset, advance_offset, register, signed) in [
        (0x68c0, 0x68c4, Register::A0, true),
        (0x68c8, 0x68cc, Register::A2, true),
        (0x68d0, 0x68d4, Register::A3, true),
        (0x6950, 0x6954, Register::A0, true),
        (0x6960, 0x6970, Register::A1, true),
        (0x6988, 0x698c, Register::V0, false),
        (0x6994, 0x6998, Register::V0, false),
        (0x69a0, 0x69a4, Register::V0, false),
        (0x69ac, 0x69b0, Register::V0, false),
        (0x69b8, 0x69bc, Register::V0, false),
        (0x69c4, 0x69e8, Register::V0, false),
    ] {
        let load = if signed {
            Instruction::Lh {
                rt: register,
                base: Register::S2,
                offset: 0,
            }
        } else {
            Instruction::Lhu {
                rt: register,
                base: Register::S2,
                offset: 0,
            }
        };
        grammar.extend([
            (load_offset, load),
            (advance_offset, advance_record_cursor()),
        ]);
    }

    grammar.extend([
        (
            0x68e4,
            Instruction::Addiu {
                rt: Register::S3,
                rs: Register::S3,
                immediate: 1,
            },
        ),
        (
            0x6974,
            Instruction::Sh {
                rt: Register::V0,
                base: Register::S0,
                offset: 22,
            },
        ),
        (
            0x6990,
            Instruction::Sb {
                rt: Register::V0,
                base: Register::S0,
                offset: 20,
            },
        ),
        (
            0x69cc,
            Instruction::Sh {
                rt: Register::V0,
                base: Register::S0,
                offset: 18,
            },
        ),
        (
            0x6a14,
            Instruction::Slt {
                rd: Register::V0,
                rs: Register::S3,
                rt: Register::S5,
            },
        ),
        (
            0x6a18,
            Instruction::Bne {
                rs: Register::V0,
                rt: Register::ZERO,
                target: RUNTIME_BASE + 0x68c0,
            },
        ),
    ]);
    grammar
}

fn advance_record_cursor() -> Instruction {
    Instruction::Addiu {
        rt: Register::S2,
        rs: Register::S2,
        immediate: 2,
    }
}

fn validate_instructions(data: &[u8], expected: &[(usize, Instruction)], role: &str) -> Result<()> {
    for (offset, expected_instruction) in expected {
        let instruction = decode(read_u32(data, *offset)?, RUNTIME_BASE + *offset as u32)
            .with_context(|| format!("failed to decode KANRI at +0x{offset:04x}"))?;
        ensure!(
            instruction == *expected_instruction,
            "KANRI counted object record {role} grammar changed at +0x{offset:04x}"
        );
    }
    Ok(())
}

fn read_u16(data: &[u8], offset: usize) -> Result<u16> {
    let bytes = data
        .get(offset..offset + 2)
        .with_context(|| format!("truncated KANRI data at +0x{offset:04x}"))?;
    Ok(u16::from_le_bytes(bytes.try_into().unwrap()))
}

fn read_u32(data: &[u8], offset: usize) -> Result<u32> {
    let bytes = data
        .get(offset..offset + 4)
        .with_context(|| format!("truncated KANRI data at +0x{offset:04x}"))?;
    Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
}
