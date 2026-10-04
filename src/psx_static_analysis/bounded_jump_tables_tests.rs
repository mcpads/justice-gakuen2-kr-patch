use psx_r3000a::{Instruction, Register};

use super::{BoundedJumpTable, read_bounded_jump_table as read_table};
use crate::psx_static_analysis::ExecutableDomain;

const BASE: u32 = 0x800a_2000;
const TRANSFER_OFFSET: usize = 0x20;
const TABLE_OFFSET: usize = 0x80;
const TARGETS: [u32; 2] = [BASE + 0x40, BASE + 0x50];

fn read_bounded_jump_table(
    data: &[u8],
    instruction_base: u32,
    transfer_instruction_offset: usize,
) -> Option<BoundedJumpTable> {
    let executable_domain = ExecutableDomain::full_image(data.len());
    read_table(
        data,
        instruction_base,
        &executable_domain,
        transfer_instruction_offset,
    )
}

#[test]
fn bounded_local_table_exposes_every_admissible_target() {
    let data = jump_table_fixture(TABLE_OFFSET, TARGETS);
    let executable_domain =
        ExecutableDomain::from_instruction_ranges(data.len(), std::iter::once(4..0x70))
            .expect("fixture code range is aligned and bounded");

    let table = read_table(&data, BASE, &executable_domain, TRANSFER_OFFSET).unwrap();

    assert_eq!(table.bound_instruction_offset, 4);
    assert_eq!(table.selector_register, Register::V1);
    assert_eq!(table.table_address, BASE + TABLE_OFFSET as u32);
    assert_eq!(table.table_offset, TABLE_OFFSET);
    assert_eq!(table.targets, TARGETS);
}

#[test]
fn table_lookup_without_an_unsigned_bound_is_not_admitted() {
    let mut data = jump_table_fixture(TABLE_OFFSET, TARGETS);
    write_instruction(
        &mut data,
        4,
        Instruction::Ori {
            rt: Register::V0,
            rs: Register::V1,
            immediate: 2,
        },
    );

    assert!(read_bounded_jump_table(&data, BASE, TRANSFER_OFFSET).is_none());
}

#[test]
fn truncated_table_is_not_admitted() {
    let mut data = jump_table_fixture(TABLE_OFFSET, TARGETS);
    data.truncate(TABLE_OFFSET + 4);

    assert!(read_bounded_jump_table(&data, BASE, TRANSFER_OFFSET).is_none());
}

#[test]
fn table_with_a_target_outside_the_executable_domain_is_not_admitted() {
    let mut data = jump_table_fixture(TABLE_OFFSET, [TARGETS[0], BASE + 0x100]);
    data.resize(0x104, 0);
    let executable_domain =
        ExecutableDomain::from_instruction_ranges(data.len(), std::iter::once(4..0x70))
            .expect("fixture code range is aligned and bounded");

    assert!(read_table(&data, BASE, &executable_domain, TRANSFER_OFFSET).is_none());
}

#[test]
fn signed_low_half_composes_the_table_address() {
    const SIGNED_TABLE_OFFSET: usize = 0x6010;
    let data = jump_table_fixture(SIGNED_TABLE_OFFSET, TARGETS);

    let table = read_bounded_jump_table(&data, BASE, TRANSFER_OFFSET).unwrap();

    assert_eq!(table.table_address, BASE + SIGNED_TABLE_OFFSET as u32);
    assert_eq!(table.table_offset, SIGNED_TABLE_OFFSET);
}

#[test]
fn straight_line_work_between_bound_and_branch_preserves_the_proof() {
    let data = delayed_bound_fixture();

    let table = read_bounded_jump_table(&data, BASE, 0x38).unwrap();

    assert_eq!(table.bound_instruction_offset, 0x0c);
    assert_eq!(table.selector_register, Register::A1);
    assert_eq!(table.targets, TARGETS);
}

#[test]
fn overwrite_between_bound_and_branch_invalidates_the_proof() {
    let mut data = delayed_bound_fixture();
    write_instruction(
        &mut data,
        0x18,
        Instruction::Ori {
            rt: Register::V1,
            rs: Register::ZERO,
            immediate: 1,
        },
    );

    assert!(read_bounded_jump_table(&data, BASE, 0x38).is_none());
}

#[test]
fn selector_overwrite_between_bound_and_shift_invalidates_the_proof() {
    let mut data = delayed_bound_fixture();
    write_instruction(
        &mut data,
        0x18,
        Instruction::Ori {
            rt: Register::A1,
            rs: Register::ZERO,
            immediate: 1,
        },
    );

    assert!(read_bounded_jump_table(&data, BASE, 0x38).is_none());
}

fn jump_table_fixture(table_offset: usize, targets: [u32; 2]) -> Vec<u8> {
    let table_address = BASE + table_offset as u32;
    let adjusted_high = table_address.wrapping_add(0x8000) >> 16;
    let low = table_address as i16;
    let instructions = [
        Instruction::Sltiu {
            rt: Register::V0,
            rs: Register::V1,
            immediate: 2,
        },
        Instruction::Beq {
            rs: Register::V0,
            rt: Register::ZERO,
            target: BASE + 0x38,
        },
        Instruction::Sll {
            rd: Register::V0,
            rt: Register::V1,
            shift: 2,
        },
        Instruction::Lui {
            rt: Register::AT,
            immediate: adjusted_high as u16,
        },
        Instruction::Addu {
            rd: Register::AT,
            rs: Register::AT,
            rt: Register::V0,
        },
        Instruction::Lw {
            rt: Register::V0,
            base: Register::AT,
            offset: low,
        },
        Instruction::nop(),
        Instruction::Jr { rs: Register::V0 },
        Instruction::nop(),
    ];
    let required_size = table_offset.max(0x54) + 8;
    let mut data = vec![0; required_size];
    data[..4].copy_from_slice(&(BASE + 4).to_le_bytes());
    for (index, instruction) in instructions.iter().enumerate() {
        write_instruction(&mut data, 4 + index * 4, instruction.clone());
    }
    for (index, target) in targets.into_iter().enumerate() {
        let offset = table_offset + index * 4;
        data[offset..offset + 4].copy_from_slice(&target.to_le_bytes());
    }
    data
}

fn delayed_bound_fixture() -> Vec<u8> {
    let table_address = BASE + TABLE_OFFSET as u32;
    let adjusted_high = table_address.wrapping_add(0x8000) >> 16;
    let low = table_address as i16;
    let mut data = vec![0; TABLE_OFFSET + 8];
    data[..4].copy_from_slice(&(BASE + 4).to_le_bytes());
    let instructions = [
        (
            0x0c,
            Instruction::Sltiu {
                rt: Register::V1,
                rs: Register::A1,
                immediate: 2,
            },
        ),
        (
            0x10,
            Instruction::Lui {
                rt: Register::AT,
                immediate: 0x801f,
            },
        ),
        (
            0x14,
            Instruction::Addu {
                rd: Register::AT,
                rs: Register::AT,
                rt: Register::A0,
            },
        ),
        (
            0x18,
            Instruction::Lhu {
                rt: Register::A0,
                base: Register::AT,
                offset: 0,
            },
        ),
        (
            0x1c,
            Instruction::Beq {
                rs: Register::V1,
                rt: Register::ZERO,
                target: BASE + 0x70,
            },
        ),
        (
            0x20,
            Instruction::Ori {
                rt: Register::S0,
                rs: Register::S0,
                immediate: 0x1a00,
            },
        ),
        (
            0x24,
            Instruction::Sll {
                rd: Register::V0,
                rt: Register::A1,
                shift: 2,
            },
        ),
        (
            0x28,
            Instruction::Lui {
                rt: Register::AT,
                immediate: adjusted_high as u16,
            },
        ),
        (
            0x2c,
            Instruction::Addu {
                rd: Register::AT,
                rs: Register::AT,
                rt: Register::V0,
            },
        ),
        (
            0x30,
            Instruction::Lw {
                rt: Register::V0,
                base: Register::AT,
                offset: low,
            },
        ),
        (0x34, Instruction::nop()),
        (0x38, Instruction::Jr { rs: Register::V0 }),
        (0x3c, Instruction::nop()),
    ];
    for (offset, instruction) in instructions {
        write_instruction(&mut data, offset, instruction);
    }
    for (index, target) in TARGETS.into_iter().enumerate() {
        let offset = TABLE_OFFSET + index * 4;
        data[offset..offset + 4].copy_from_slice(&target.to_le_bytes());
    }
    data
}

fn write_instruction(data: &mut [u8], offset: usize, instruction: Instruction) {
    let pc = BASE + offset as u32;
    data[offset..offset + 4]
        .copy_from_slice(&psx_r3000a::encode(&instruction, pc).unwrap().to_le_bytes());
}
