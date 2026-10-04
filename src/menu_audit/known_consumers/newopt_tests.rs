use std::collections::BTreeSet;

use psx_r3000a::{Instruction, Register, encode};

use super::newopt::{
    STRING_LOAD_OFFSET, STRING_LOAD_SEED_OFFSET, STRING_WRITER_CALL_OFFSET, STRING_WRITER_OFFSET,
    validate_direct_string_writer_consumer,
};

const BASE: u32 = 0x800a_2000;
const STRING_OFFSET: usize = 0x04a8;

#[test]
fn admits_the_loaded_string_passed_to_the_reachable_writer() {
    let (data, reachable) = fixture();

    let consumers = validate_direct_string_writer_consumer(&data, BASE, &reachable).unwrap();

    assert_eq!(
        consumers.keys().copied().collect::<Vec<_>>(),
        [STRING_OFFSET]
    );
}

#[test]
fn rejects_an_unreachable_direct_string_call() {
    let (data, mut reachable) = fixture();
    reachable.remove(&STRING_WRITER_CALL_OFFSET);

    let error = validate_direct_string_writer_consumer(&data, BASE, &reachable).unwrap_err();

    assert!(error.to_string().contains("not entrypoint-reachable"));
}

#[test]
fn rejects_a_writer_that_no_longer_reads_the_string_length() {
    let (mut data, reachable) = fixture();
    write_instruction(&mut data, 0x2fec, nop());

    let error = validate_direct_string_writer_consumer(&data, BASE, &reachable).unwrap_err();

    assert!(error.to_string().contains("string writer grammar changed"));
}

fn fixture() -> (Vec<u8>, BTreeSet<usize>) {
    let mut data = vec![0u8; 0x3060];
    data[STRING_OFFSET..STRING_OFFSET + 2].copy_from_slice(&1u16.to_le_bytes());
    data[STRING_OFFSET + 2..STRING_OFFSET + 4].copy_from_slice(&0x0130u16.to_le_bytes());
    data[0x0be4..0x0be8].copy_from_slice(&(BASE + STRING_OFFSET as u32).to_le_bytes());

    write_instruction(
        &mut data,
        STRING_LOAD_SEED_OFFSET,
        Instruction::Lui {
            rt: Register::A3,
            immediate: 0x800a,
        },
    );
    write_instruction(
        &mut data,
        STRING_LOAD_OFFSET,
        Instruction::Lw {
            rt: Register::A3,
            base: Register::A3,
            offset: 0x2be4,
        },
    );
    write_instruction(
        &mut data,
        STRING_WRITER_CALL_OFFSET,
        Instruction::Jal {
            target: BASE + STRING_WRITER_OFFSET as u32,
        },
    );
    write_instruction(
        &mut data,
        STRING_WRITER_CALL_OFFSET + 4,
        Instruction::Addu {
            rd: Register::A2,
            rs: Register::S3,
            rt: Register::ZERO,
        },
    );
    for (offset, instruction) in [
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
    ] {
        write_instruction(&mut data, offset, instruction);
    }
    (
        data,
        BTreeSet::from([
            STRING_LOAD_SEED_OFFSET,
            STRING_LOAD_OFFSET,
            STRING_WRITER_CALL_OFFSET,
            STRING_WRITER_CALL_OFFSET + 4,
            STRING_WRITER_OFFSET,
        ]),
    )
}

fn nop() -> Instruction {
    Instruction::Sll {
        rd: Register::ZERO,
        rt: Register::ZERO,
        shift: 0,
    }
}

fn write_instruction(data: &mut [u8], offset: usize, instruction: Instruction) {
    let word = encode(&instruction, BASE + offset as u32).unwrap();
    data[offset..offset + 4].copy_from_slice(&word.to_le_bytes());
}
