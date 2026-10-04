use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};
use psx_r3000a::{Instruction, Register, decode};

use super::super::model::MenuNonTextEvidence;
use super::super::scanner::candidate_codes;
use crate::pipeline::sha256_bytes;

const OVERLAY_PATH: &str = "DAT1/KOUBAI2.BIN";
const SOURCE_SHA256: &str = "9c1e0b2f226e3c16dc4622068477ff77ab97ecc738db1273680d391f3fa4e393";
const RUNTIME_BASE: u32 = 0x800a_2000;
const PRIMARY_CONSUMER_OFFSET: usize = 0x986c;
const SECONDARY_CONSUMER_OFFSET: usize = 0x9374;

#[derive(Clone, Copy)]
pub(super) struct CommandByteSequence {
    pub(super) offset: usize,
    pub(super) terminator_offset: usize,
    pub(super) pointer_storage_offset: usize,
}

pub(super) const SEQUENCES: [CommandByteSequence; 6] = [
    sequence(0x0044, 0x0059, 0x11e8),
    sequence(0x0610, 0x0625, 0x12f8),
    sequence(0x0628, 0x063d, 0x12fc),
    sequence(0x0640, 0x0655, 0x1300),
    sequence(0x0658, 0x066d, 0x1304),
    sequence(0x1034, 0x104c, 0x13f8),
];

const fn sequence(
    offset: usize,
    terminator_offset: usize,
    pointer_storage_offset: usize,
) -> CommandByteSequence {
    CommandByteSequence {
        offset,
        terminator_offset,
        pointer_storage_offset,
    }
}

pub(super) fn identify_command_byte_sequence_candidates(
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
    validate_command_byte_sequence_candidates(data, RUNTIME_BASE, reachable_instruction_offsets)
}

pub(super) fn validate_command_byte_sequence_candidates(
    data: &[u8],
    runtime_base: u32,
    reachable_instruction_offsets: &BTreeSet<usize>,
) -> Result<BTreeMap<usize, Vec<MenuNonTextEvidence>>> {
    ensure!(
        runtime_base == RUNTIME_BASE,
        "KOUBAI2 command sequences moved from their source-bound runtime base"
    );
    for required_offset in [
        0x6f04,
        0x6fdc,
        0x6fe0,
        0x7680,
        0x76dc,
        0x76e0,
        0xd35c,
        0xd368,
        0xd370,
        0xd420,
        0xd424,
        0xd42c,
        PRIMARY_CONSUMER_OFFSET,
        0x9874,
        0x98a8,
        SECONDARY_CONSUMER_OFFSET,
        0x937c,
        0x93b4,
    ] {
        ensure!(
            reachable_instruction_offsets.contains(&required_offset),
            "KOUBAI2 command-sequence consumer +0x{required_offset:04x} is not entrypoint-reachable"
        );
    }
    validate_sequences(data, runtime_base)?;
    validate_instructions(data, &consumer_grammar())?;

    SEQUENCES
        .into_iter()
        .map(|sequence| {
            ensure!(
                candidate_codes(data, sequence.offset).is_some(),
                "KOUBAI2 non-text candidate +0x{:04x} no longer matches the string heuristic",
                sequence.offset
            );
            Ok((
                sequence.offset,
                vec![MenuNonTextEvidence::Koubai2CommandByteSequence],
            ))
        })
        .collect()
}

fn validate_sequences(data: &[u8], runtime_base: u32) -> Result<()> {
    for sequence in SEQUENCES {
        ensure!(
            read_u32(data, sequence.pointer_storage_offset)?
                == runtime_base + sequence.offset as u32,
            "KOUBAI2 command-sequence pointer for +0x{:04x} changed",
            sequence.offset
        );
        let bytes = data
            .get(sequence.offset..=sequence.terminator_offset)
            .with_context(|| {
                format!(
                    "KOUBAI2 command sequence +0x{:04x} is truncated",
                    sequence.offset
                )
            })?;
        ensure!(
            bytes.last() == Some(&0x81) && !bytes[..bytes.len() - 1].contains(&0x81),
            "KOUBAI2 command sequence +0x{:04x} terminator changed",
            sequence.offset
        );
        let next_offset = align_word(sequence.terminator_offset + 1);
        ensure!(
            read_u32(data, sequence.pointer_storage_offset + 4)?
                == runtime_base + next_offset as u32,
            "KOUBAI2 command sequence +0x{:04x} boundary changed",
            sequence.offset
        );
    }
    Ok(())
}

pub(super) fn consumer_grammar() -> Vec<(usize, Instruction)> {
    let mut grammar = Vec::new();
    push_indexed_caller(&mut grammar, 0x6f04, 0x6fc8, 0x6fdc, 0x6fe0);
    push_indexed_caller(&mut grammar, 0x7680, 0x76d0, 0x76dc, 0x76e0);
    push_secondary_callers(&mut grammar);
    push_parser_grammar(&mut grammar, ParserSpec::primary());
    push_parser_grammar(&mut grammar, ParserSpec::secondary());
    grammar
}

fn push_indexed_caller(
    grammar: &mut Vec<(usize, Instruction)>,
    seed_offset: usize,
    index_offset: usize,
    load_offset: usize,
    call_offset: usize,
) {
    grammar.extend([
        (
            seed_offset,
            Instruction::Lui {
                rt: Register::S5,
                immediate: 0x800a,
            },
        ),
        (
            seed_offset + 4,
            Instruction::Addiu {
                rt: Register::S5,
                rs: Register::S5,
                immediate: 0x31e0,
            },
        ),
        (
            index_offset,
            Instruction::Sll {
                rd: Register::V0,
                rt: Register::A1,
                shift: 2,
            },
        ),
        (
            index_offset + 4,
            Instruction::Addu {
                rd: Register::V0,
                rs: Register::V0,
                rt: Register::S5,
            },
        ),
        (
            load_offset,
            Instruction::Lw {
                rt: Register::A1,
                base: Register::V0,
                offset: 0,
            },
        ),
        (
            call_offset,
            Instruction::Jal {
                target: RUNTIME_BASE + PRIMARY_CONSUMER_OFFSET as u32,
            },
        ),
    ]);
}

fn push_secondary_callers(grammar: &mut Vec<(usize, Instruction)>) {
    grammar.extend([
        (
            0xd35c,
            Instruction::Lui {
                rt: Register::V0,
                immediate: 0x800a,
            },
        ),
        (
            0xd360,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::V0,
                immediate: 0x33f8,
            },
        ),
        (
            0xd368,
            Instruction::Lw {
                rt: Register::A1,
                base: Register::V0,
                offset: 0,
            },
        ),
        (0xd370, secondary_call()),
        (
            0xd420,
            Instruction::Lui {
                rt: Register::A1,
                immediate: 0x800a,
            },
        ),
        (
            0xd424,
            Instruction::Lw {
                rt: Register::A1,
                base: Register::A1,
                offset: 0x33f8,
            },
        ),
        (0xd42c, secondary_call()),
    ]);
}

fn secondary_call() -> Instruction {
    Instruction::Jal {
        target: RUNTIME_BASE + SECONDARY_CONSUMER_OFFSET as u32,
    }
}

#[derive(Clone, Copy)]
struct ParserSpec {
    entry_offset: usize,
    stack_size: i16,
    cursor_copy_offset: usize,
    terminal_constant_offset: usize,
    opcode_read_offset: usize,
    opcode_mask_offset: usize,
    terminal_branch_offset: usize,
    terminal_target_offset: usize,
    advances: &'static [(usize, i16)],
    operand_reads: &'static [usize],
    loop_read_offset: usize,
    loop_branch_offset: usize,
    loop_target_offset: usize,
}

impl ParserSpec {
    const fn primary() -> Self {
        Self {
            entry_offset: 0x986c,
            stack_size: -56,
            cursor_copy_offset: 0x9874,
            terminal_constant_offset: 0x9890,
            opcode_read_offset: 0x98a8,
            opcode_mask_offset: 0x98b0,
            terminal_branch_offset: 0x98b4,
            terminal_target_offset: 0x9a60,
            advances: &[
                (0x98fc, 1),
                (0x9910, 3),
                (0x9914, 1),
                (0x99c8, 1),
                (0x9a14, 1),
            ],
            operand_reads: &[0x99c4, 0x99e4],
            loop_read_offset: 0x9a4c,
            loop_branch_offset: 0x9a58,
            loop_target_offset: 0x98e8,
        }
    }

    const fn secondary() -> Self {
        Self {
            entry_offset: 0x9374,
            stack_size: -64,
            cursor_copy_offset: 0x937c,
            terminal_constant_offset: 0x9394,
            opcode_read_offset: 0x93b4,
            opcode_mask_offset: 0x93bc,
            terminal_branch_offset: 0x93c0,
            terminal_target_offset: 0x9578,
            advances: &[
                (0x93e8, 1),
                (0x93fc, 3),
                (0x9400, 1),
                (0x94e0, 1),
                (0x952c, 1),
            ],
            operand_reads: &[0x94dc, 0x94fc],
            loop_read_offset: 0x9564,
            loop_branch_offset: 0x9570,
            loop_target_offset: 0x93d0,
        }
    }
}

fn push_parser_grammar(grammar: &mut Vec<(usize, Instruction)>, parser: ParserSpec) {
    grammar.extend([
        (
            parser.entry_offset,
            Instruction::Addiu {
                rt: Register::SP,
                rs: Register::SP,
                immediate: parser.stack_size,
            },
        ),
        (
            parser.cursor_copy_offset,
            Instruction::Addu {
                rd: Register::S2,
                rs: Register::A1,
                rt: Register::ZERO,
            },
        ),
        (
            parser.terminal_constant_offset,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::ZERO,
                immediate: 0x81,
            },
        ),
        (parser.opcode_read_offset, read_cursor(Register::A2)),
        (
            parser.opcode_mask_offset,
            Instruction::Andi {
                rt: Register::V1,
                rs: Register::A2,
                immediate: 0xff,
            },
        ),
        (
            parser.terminal_branch_offset,
            Instruction::Beq {
                rs: Register::V1,
                rt: Register::V0,
                target: RUNTIME_BASE + parser.terminal_target_offset as u32,
            },
        ),
    ]);
    grammar.extend(
        parser
            .advances
            .iter()
            .map(|&(offset, amount)| (offset, advance_cursor(amount))),
    );
    grammar.extend(
        parser
            .operand_reads
            .iter()
            .map(|&offset| (offset, read_cursor(Register::V1))),
    );
    grammar.extend([
        (parser.loop_read_offset, read_cursor(Register::A2)),
        (
            parser.loop_branch_offset,
            Instruction::Bne {
                rs: Register::V1,
                rt: Register::V0,
                target: RUNTIME_BASE + parser.loop_target_offset as u32,
            },
        ),
    ]);
}

fn read_cursor(register: Register) -> Instruction {
    Instruction::Lbu {
        rt: register,
        base: Register::S2,
        offset: 0,
    }
}

fn advance_cursor(amount: i16) -> Instruction {
    Instruction::Addiu {
        rt: Register::S2,
        rs: Register::S2,
        immediate: amount,
    }
}

fn align_word(offset: usize) -> usize {
    (offset + 3) & !3
}

fn validate_instructions(data: &[u8], expected: &[(usize, Instruction)]) -> Result<()> {
    for (offset, expected_instruction) in expected {
        let instruction = decode(read_u32(data, *offset)?, RUNTIME_BASE + *offset as u32)
            .with_context(|| format!("failed to decode KOUBAI2 at +0x{offset:04x}"))?;
        ensure!(
            instruction == *expected_instruction,
            "KOUBAI2 command-sequence consumer grammar changed at +0x{offset:04x}: expected {expected_instruction:?}, found {instruction:?}"
        );
    }
    Ok(())
}

fn read_u32(data: &[u8], offset: usize) -> Result<u32> {
    let bytes = data
        .get(offset..offset + 4)
        .with_context(|| format!("truncated KOUBAI2 data at +0x{offset:04x}"))?;
    Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
}
