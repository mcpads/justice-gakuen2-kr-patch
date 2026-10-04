use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};
use psx_r3000a::{Instruction, Register, decode};

use super::super::model::MenuNonTextEvidence;
use super::super::scanner::candidate_codes;
use crate::pipeline::sha256_bytes;

const OVERLAY_PATH: &str = "DAT1/PLSEL5.BIN";
const SOURCE_SHA256: &str = "3461b0ed24bb41acbbf44546d2c65ded2ac1d4d295423ed355d3f6b8352eca11";
const RUNTIME_BASE: u32 = 0x800a_2000;
const PARAMETER_CONSUMER_OFFSET: usize = 0xbc30;

#[derive(Clone, Copy)]
pub(super) struct CountedParameterPointerTable {
    pub(super) offset: usize,
    pub(super) targets: &'static [usize],
    pub(super) next_offset: usize,
    pub(super) pointer_storage_offset: usize,
    pub(super) initializer_offset: usize,
    count_store_offset: usize,
    pub(super) selector_offset: usize,
    selector_index_register: Register,
    jumps_to_common_call: bool,
}

pub(super) const TABLES: [CountedParameterPointerTable; 6] = [
    delayed_count_store_table(0x3420, &[0x21e8], 0x3428, 0x3534, 0x5c74, 0x5cf8),
    direct_table(0x34a0, &[0x3064, 0x30bc], 0x34ac, 0x3564, 0x5e34, 0x5e58),
    table(0x34ac, &[0x3170], 0x34b4, 0x3568, 0x56b4, 0x5720),
    table(0x34b4, &[0x31dc], 0x34bc, 0x356c, 0x5824, 0x5890),
    table(0x34bc, &[0x3248], 0x34c4, 0x3570, 0x5994, 0x5a00),
    table(0x34c4, &[0x32b4], 0x34cc, 0x3574, 0x5b04, 0x5b70),
];

const fn table(
    offset: usize,
    targets: &'static [usize],
    next_offset: usize,
    pointer_storage_offset: usize,
    initializer_offset: usize,
    selector_offset: usize,
) -> CountedParameterPointerTable {
    CountedParameterPointerTable {
        offset,
        targets,
        next_offset,
        pointer_storage_offset,
        initializer_offset,
        count_store_offset: initializer_offset + 28,
        selector_offset,
        selector_index_register: Register::V1,
        jumps_to_common_call: true,
    }
}

const fn delayed_count_store_table(
    offset: usize,
    targets: &'static [usize],
    next_offset: usize,
    pointer_storage_offset: usize,
    initializer_offset: usize,
    selector_offset: usize,
) -> CountedParameterPointerTable {
    CountedParameterPointerTable {
        count_store_offset: initializer_offset + 36,
        ..table(
            offset,
            targets,
            next_offset,
            pointer_storage_offset,
            initializer_offset,
            selector_offset,
        )
    }
}

const fn direct_table(
    offset: usize,
    targets: &'static [usize],
    next_offset: usize,
    pointer_storage_offset: usize,
    initializer_offset: usize,
    selector_offset: usize,
) -> CountedParameterPointerTable {
    CountedParameterPointerTable {
        offset,
        targets,
        next_offset,
        pointer_storage_offset,
        initializer_offset,
        count_store_offset: initializer_offset + 28,
        selector_offset,
        selector_index_register: Register::V0,
        jumps_to_common_call: false,
    }
}

pub(super) fn identify_counted_parameter_pointer_candidates(
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
    validate_counted_parameter_pointer_candidates(data, RUNTIME_BASE, reachable_instruction_offsets)
}

pub(super) fn validate_counted_parameter_pointer_candidates(
    data: &[u8],
    runtime_base: u32,
    reachable_instruction_offsets: &BTreeSet<usize>,
) -> Result<BTreeMap<usize, Vec<MenuNonTextEvidence>>> {
    ensure!(
        runtime_base == RUNTIME_BASE,
        "PLSEL5 parameter pointer tables moved from their source-bound runtime base"
    );
    validate_reachability(reachable_instruction_offsets)?;
    validate_tables(data, runtime_base)?;
    validate_instructions(data, &consumer_grammar(), "consumer")?;

    TABLES
        .into_iter()
        .map(|table| {
            ensure!(
                candidate_codes(data, table.offset).is_some(),
                "PLSEL5 non-text candidate +0x{:04x} no longer matches the string heuristic",
                table.offset
            );
            Ok((
                table.offset,
                vec![MenuNonTextEvidence::Plsel5CountedParameterPointerTable],
            ))
        })
        .collect()
}

fn validate_reachability(reachable_instruction_offsets: &BTreeSet<usize>) -> Result<()> {
    for required_offset in TABLES
        .iter()
        .flat_map(|table| [table.initializer_offset, table.selector_offset])
        .chain([0x5e74, PARAMETER_CONSUMER_OFFSET, 0xbc40, 0xbc98])
    {
        ensure!(
            reachable_instruction_offsets.contains(&required_offset),
            "PLSEL5 parameter pointer consumer +0x{required_offset:04x} is not entrypoint-reachable"
        );
    }
    Ok(())
}

fn validate_tables(data: &[u8], runtime_base: u32) -> Result<()> {
    for table in TABLES {
        ensure!(
            read_u32(data, table.pointer_storage_offset)? == runtime_base + table.offset as u32,
            "PLSEL5 parameter table pointer for +0x{:04x} changed",
            table.offset
        );
        ensure!(
            read_u32(data, table.offset)? == table.targets.len() as u32,
            "PLSEL5 parameter pointer table +0x{:04x} count changed",
            table.offset
        );
        let table_end = table
            .offset
            .checked_add(4 + table.targets.len() * 4)
            .context("PLSEL5 parameter pointer table size overflow")?;
        ensure!(
            table_end == table.next_offset,
            "PLSEL5 parameter pointer table +0x{:04x} span changed",
            table.offset
        );
        for (index, target_offset) in table.targets.iter().copied().enumerate() {
            ensure!(
                read_u32(data, table.offset + 4 + index * 4)?
                    == runtime_base + target_offset as u32,
                "PLSEL5 parameter pointer table +0x{:04x} target {index} changed",
                table.offset
            );
            read_u16(data, target_offset).with_context(|| {
                format!(
                    "PLSEL5 parameter pointer table +0x{:04x} target {index} is outside the image",
                    table.offset
                )
            })?;
        }
    }
    Ok(())
}

pub(super) fn consumer_grammar() -> Vec<(usize, Instruction)> {
    let mut grammar = Vec::new();
    for table in TABLES {
        push_initializer_grammar(&mut grammar, table);
        push_selector_grammar(&mut grammar, table);
    }
    grammar.extend([
        (
            0x5e74,
            Instruction::Jal {
                target: RUNTIME_BASE + PARAMETER_CONSUMER_OFFSET as u32,
            },
        ),
        (
            0x5e78,
            Instruction::Addu {
                rd: Register::A2,
                rs: Register::ZERO,
                rt: Register::ZERO,
            },
        ),
        (
            PARAMETER_CONSUMER_OFFSET,
            Instruction::Addiu {
                rt: Register::SP,
                rs: Register::SP,
                immediate: -64,
            },
        ),
        (
            0xbc40,
            Instruction::Addu {
                rd: Register::S1,
                rs: Register::A1,
                rt: Register::ZERO,
            },
        ),
        (
            0xbc98,
            Instruction::Lhu {
                rt: Register::A2,
                base: Register::S1,
                offset: 0,
            },
        ),
    ]);
    grammar
}

fn push_initializer_grammar(
    grammar: &mut Vec<(usize, Instruction)>,
    table: CountedParameterPointerTable,
) {
    let offset = table.initializer_offset;
    grammar.extend([
        (
            offset,
            Instruction::Lui {
                rt: Register::V1,
                immediate: 0x800a,
            },
        ),
        (
            offset + 4,
            Instruction::Lw {
                rt: Register::V1,
                base: Register::V1,
                offset: runtime_low_half(table.pointer_storage_offset),
            },
        ),
        (
            offset + 12,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::V1,
                immediate: 4,
            },
        ),
        (
            offset + 16,
            Instruction::Sw {
                rt: Register::V0,
                base: Register::S0,
                offset: 0x4280,
            },
        ),
        (
            offset + 20,
            Instruction::Lw {
                rt: Register::A1,
                base: Register::V1,
                offset: 0,
            },
        ),
        (
            table.count_store_offset,
            Instruction::Sb {
                rt: Register::A1,
                base: Register::S0,
                offset: 0x427f,
            },
        ),
    ]);
}

fn runtime_low_half(file_offset: usize) -> i16 {
    (RUNTIME_BASE.wrapping_add(file_offset as u32) & 0xffff) as u16 as i16
}

fn push_selector_grammar(
    grammar: &mut Vec<(usize, Instruction)>,
    table: CountedParameterPointerTable,
) {
    let offset = table.selector_offset;
    let table_register = if table.jumps_to_common_call {
        Register::A1
    } else {
        Register::V1
    };
    grammar.extend([
        (
            offset,
            Instruction::Lw {
                rt: table_register,
                base: Register::S0,
                offset: 0x4280,
            },
        ),
        (
            offset + 4,
            Instruction::Lbu {
                rt: Register::A3,
                base: Register::S0,
                offset: 0x427f,
            },
        ),
    ]);

    if table.jumps_to_common_call {
        grammar.extend([
            (
                offset + 20,
                Instruction::Sll {
                    rd: Register::V1,
                    rt: table.selector_index_register,
                    shift: 2,
                },
            ),
            (
                offset + 24,
                Instruction::Addu {
                    rd: Register::V1,
                    rs: Register::V1,
                    rt: table_register,
                },
            ),
            (
                offset + 32,
                Instruction::Lw {
                    rt: Register::A1,
                    base: Register::V1,
                    offset: 0,
                },
            ),
            (
                offset + 36,
                Instruction::J {
                    target: RUNTIME_BASE + 0x5e74,
                },
            ),
            (
                offset + 40,
                Instruction::Addu {
                    rd: Register::A0,
                    rs: Register::S0,
                    rt: Register::ZERO,
                },
            ),
        ]);
    } else {
        grammar.extend([
            (
                offset + 8,
                Instruction::Addu {
                    rd: Register::A0,
                    rs: Register::S0,
                    rt: Register::ZERO,
                },
            ),
            (
                offset + 16,
                Instruction::Sll {
                    rd: Register::V0,
                    rt: table.selector_index_register,
                    shift: 2,
                },
            ),
            (
                offset + 20,
                Instruction::Addu {
                    rd: Register::V0,
                    rs: Register::V0,
                    rt: table_register,
                },
            ),
            (
                offset + 24,
                Instruction::Lw {
                    rt: Register::A1,
                    base: Register::V0,
                    offset: 0,
                },
            ),
        ]);
    }
}

fn validate_instructions(data: &[u8], expected: &[(usize, Instruction)], role: &str) -> Result<()> {
    for (offset, expected_instruction) in expected {
        let instruction = decode(read_u32(data, *offset)?, RUNTIME_BASE + *offset as u32)
            .with_context(|| format!("failed to decode PLSEL5 at +0x{offset:04x}"))?;
        ensure!(
            instruction == *expected_instruction,
            "PLSEL5 parameter pointer {role} grammar changed at +0x{offset:04x}"
        );
    }
    Ok(())
}

fn read_u16(data: &[u8], offset: usize) -> Result<u16> {
    let bytes = data
        .get(offset..offset + 2)
        .with_context(|| format!("truncated PLSEL5 data at +0x{offset:04x}"))?;
    Ok(u16::from_le_bytes(bytes.try_into().unwrap()))
}

fn read_u32(data: &[u8], offset: usize) -> Result<u32> {
    let bytes = data
        .get(offset..offset + 4)
        .with_context(|| format!("truncated PLSEL5 data at +0x{offset:04x}"))?;
    Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
}
