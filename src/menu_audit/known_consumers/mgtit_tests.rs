use std::collections::BTreeSet;

use psx_r3000a::{Instruction, Register, encode};

use super::mgtit::{RENDERER_ADDRESS, RENDERER_OFFSET, validate_placement_record_consumers};
use crate::psx_static_analysis::value_flow::ResolvedDirectCallArgument;

const BASE: u32 = 0x800a_2000;
const RECORD_TABLE_OFFSET: usize = 0x02a0;
const RECORD_SIZE: usize = 12;
const RECORD_COUNT: usize = 18;
const STRING_OFFSETS: [usize; RECORD_COUNT] = [
    0x0010, 0x001c, 0x0030, 0x0054, 0x0064, 0x008c, 0x00a8, 0x00cc, 0x00ec, 0x0114, 0x012c, 0x014c,
    0x0170, 0x0184, 0x019c, 0x01b4, 0x01cc, 0x01ec,
];

#[test]
fn admits_every_record_only_when_reachable_calls_cover_the_table() {
    let (data, reachable_instructions, reachable_seeds, arguments) = fixture();

    let consumers = validate_placement_record_consumers(
        &data,
        BASE,
        &reachable_instructions,
        &reachable_seeds,
        &arguments,
    )
    .unwrap();

    assert_eq!(
        consumers.keys().copied().collect::<Vec<_>>(),
        STRING_OFFSETS
    );
}

#[test]
fn rejects_a_placement_record_without_a_reachable_renderer_call() {
    let (data, reachable_instructions, reachable_seeds, mut arguments) = fixture();
    arguments.pop();

    let error = validate_placement_record_consumers(
        &data,
        BASE,
        &reachable_instructions,
        &reachable_seeds,
        &arguments,
    )
    .unwrap_err();

    assert!(error.to_string().contains("complete placement table"));
}

#[test]
fn rejects_a_renderer_that_no_longer_reads_the_record_string_pointer() {
    let (mut data, reachable_instructions, reachable_seeds, arguments) = fixture();
    write_instruction(
        &mut data,
        0x30ec,
        Instruction::Sll {
            rd: Register::ZERO,
            rt: Register::ZERO,
            shift: 0,
        },
    );

    let error = validate_placement_record_consumers(
        &data,
        BASE,
        &reachable_instructions,
        &reachable_seeds,
        &arguments,
    )
    .unwrap_err();

    assert!(error.to_string().contains("renderer grammar changed"));
}

fn fixture() -> (
    Vec<u8>,
    BTreeSet<usize>,
    BTreeSet<usize>,
    Vec<ResolvedDirectCallArgument>,
) {
    let mut data = vec![0u8; 0x3300];
    for (index, string_offset) in STRING_OFFSETS.into_iter().enumerate() {
        data[string_offset..string_offset + 2].copy_from_slice(&1u16.to_le_bytes());
        data[string_offset + 2..string_offset + 4].copy_from_slice(&1u16.to_le_bytes());
        let record_offset = RECORD_TABLE_OFFSET + index * RECORD_SIZE;
        data[record_offset..record_offset + 2].copy_from_slice(&(index as i16).to_le_bytes());
        data[record_offset + 2..record_offset + 4].copy_from_slice(&(index as i16).to_le_bytes());
        data[record_offset + 4..record_offset + 8]
            .copy_from_slice(&(BASE + string_offset as u32).to_le_bytes());
    }
    write_renderer_grammar(&mut data);

    let mut reachable_instructions = BTreeSet::from([RENDERER_OFFSET]);
    let mut reachable_seeds = BTreeSet::new();
    let arguments = (0..RECORD_COUNT)
        .map(|index| {
            let seed_offset = 0x0400 + index * 8;
            let instruction_offset = seed_offset + 4;
            reachable_seeds.insert(seed_offset);
            reachable_instructions.insert(instruction_offset);
            ResolvedDirectCallArgument {
                seed_offset,
                instruction_offset,
                target: RENDERER_ADDRESS,
                argument_register: Register::A1,
                value: BASE + (RECORD_TABLE_OFFSET + index * RECORD_SIZE) as u32,
            }
        })
        .collect();
    (data, reachable_instructions, reachable_seeds, arguments)
}

fn write_renderer_grammar(data: &mut [u8]) {
    let instructions = [
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
    for (offset, instruction) in instructions {
        write_instruction(data, offset, instruction);
    }
}

fn write_instruction(data: &mut [u8], offset: usize, instruction: Instruction) {
    let word = encode(&instruction, BASE + offset as u32).unwrap();
    data[offset..offset + 4].copy_from_slice(&word.to_le_bytes());
}
