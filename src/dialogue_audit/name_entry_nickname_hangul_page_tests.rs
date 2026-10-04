use psx_r3000a::{Instruction, Register, decode, encode};

use super::name_entry::OVERLAY_RUNTIME_BASE;
use super::name_entry_nickname_hangul_page::install_name_entry_nickname_hangul_page;

const PAGE_CYCLE_BRANCH_OFFSET: usize = 0x1ce8;
const PAGE_CYCLE_CONTINUE_OFFSET: usize = 0x1d00;
const NICKNAME_INITIAL_PAGE_STORE_OFFSET: usize = 0x1fa8;

#[test]
fn nickname_starts_on_and_cycles_through_the_hangul_page() {
    let source = source_fixture();
    let mut patched = source.clone();

    let report = install_name_entry_nickname_hangul_page(&source, &mut patched).unwrap();

    assert_eq!(report.page_cycle_file_offset, "0x1ce8");
    assert_eq!(report.page_cycle_runtime_address, "0x8017bce8");
    assert_eq!(report.nickname_initial_page_store_file_offset, "0x1fa8");
    assert_eq!(
        report.nickname_initial_page_store_runtime_address,
        "0x8017bfa8"
    );
    assert_eq!(report.overwritten_byte_count, 8);
    assert_eq!(report.typed_instruction_count, 2);
    assert!(report.source_instructions_verified);
    assert_eq!(report.nickname_initial_page_index, 0);
    assert!(report.nickname_page_cycle_includes_hangul);
    assert!(report.installed);
    assert!(!report.runtime_execution_verified);
    assert_eq!(
        decode_word(&patched, PAGE_CYCLE_BRANCH_OFFSET),
        Instruction::J {
            target: runtime_address(PAGE_CYCLE_CONTINUE_OFFSET),
        }
    );
    assert_eq!(
        decode_word(&patched, NICKNAME_INITIAL_PAGE_STORE_OFFSET),
        Instruction::Sb {
            rt: Register::ZERO,
            base: Register::S0,
            offset: 9,
        }
    );
}

#[test]
fn changed_source_or_an_existing_writer_is_rejected() {
    let source = source_fixture();
    let mut changed_source = source.clone();
    write_instruction(
        &mut changed_source,
        PAGE_CYCLE_BRANCH_OFFSET,
        &Instruction::Beq {
            rs: Register::V1,
            rt: Register::V0,
            target: runtime_address(PAGE_CYCLE_CONTINUE_OFFSET),
        },
    );
    assert!(
        install_name_entry_nickname_hangul_page(&changed_source, &mut changed_source.clone())
            .is_err()
    );

    let mut changed_destination = source.clone();
    write_instruction(
        &mut changed_destination,
        NICKNAME_INITIAL_PAGE_STORE_OFFSET,
        &Instruction::Sb {
            rt: Register::A0,
            base: Register::S0,
            offset: 9,
        },
    );
    assert!(install_name_entry_nickname_hangul_page(&source, &mut changed_destination).is_err());
}

fn source_fixture() -> Vec<u8> {
    let mut source = vec![0_u8; 0x7c28];
    write_instruction(
        &mut source,
        PAGE_CYCLE_BRANCH_OFFSET,
        &Instruction::Bne {
            rs: Register::V1,
            rt: Register::V0,
            target: runtime_address(PAGE_CYCLE_CONTINUE_OFFSET),
        },
    );
    write_instruction(
        &mut source,
        NICKNAME_INITIAL_PAGE_STORE_OFFSET,
        &Instruction::Sb {
            rt: Register::V0,
            base: Register::S0,
            offset: 9,
        },
    );
    write_instruction(
        &mut source,
        0x7bf4,
        &Instruction::Beq {
            rs: Register::V1,
            rt: Register::V0,
            target: runtime_address(0x7c14),
        },
    );
    source
}

fn write_instruction(bytes: &mut [u8], offset: usize, instruction: &Instruction) {
    bytes[offset..offset + 4].copy_from_slice(
        &encode(instruction, runtime_address(offset))
            .unwrap()
            .to_le_bytes(),
    );
}

fn decode_word(bytes: &[u8], offset: usize) -> Instruction {
    decode(
        u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()),
        runtime_address(offset),
    )
    .unwrap()
}

const fn runtime_address(offset: usize) -> u32 {
    OVERLAY_RUNTIME_BASE + offset as u32
}
