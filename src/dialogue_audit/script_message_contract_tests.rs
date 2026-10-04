use psx_r3000a::{Instruction, Register, encode};

use super::script_message_contract::validate_compact_message_opcode_contract;

const MGAME_RUNTIME_BASE: u32 = 0x800a_2000;

#[test]
fn compact_message_opcode_requires_typed_bank_and_entry_resolution() {
    let mut mgame = vec![0u8; 0xac00];
    write_word(&mut mgame, 0x05e8, 0x800a_ca34);
    write_word(&mut mgame, 0x05f4, 0x800a_cadc);
    for (runtime_address, instruction) in expected_instructions() {
        write_instruction(&mut mgame, runtime_address, instruction.clone());
        write_instruction(&mut mgame, runtime_address + 0xa8, instruction);
    }
    for (address, presentation) in [(0x800a_cac0, 9), (0x800a_cb68, 10)] {
        write_instruction(
            &mut mgame,
            address,
            Instruction::Addiu {
                rt: Register::A1,
                rs: Register::ZERO,
                immediate: presentation,
            },
        );
    }

    validate_compact_message_opcode_contract(&mgame).unwrap();

    let mut narration = mgame.clone();
    write_instruction(
        &mut narration,
        0x800a_cb20,
        Instruction::Lbu {
            rt: Register::V0,
            base: Register::A1,
            offset: 1,
        },
    );
    assert!(validate_compact_message_opcode_contract(&narration).is_err());

    write_instruction(
        &mut mgame,
        0x800a_ca78,
        Instruction::Lbu {
            rt: Register::V0,
            base: Register::A1,
            offset: 1,
        },
    );
    assert!(validate_compact_message_opcode_contract(&mgame).is_err());
}

fn expected_instructions() -> Vec<(u32, Instruction)> {
    vec![
        (
            0x800a_ca68,
            Instruction::Lbu {
                rt: Register::A0,
                base: Register::A1,
                offset: 1,
            },
        ),
        (
            0x800a_ca78,
            Instruction::Lbu {
                rt: Register::V0,
                base: Register::A1,
                offset: 2,
            },
        ),
        (
            0x800a_ca7c,
            Instruction::Lui {
                rt: Register::V1,
                immediate: 0x801f,
            },
        ),
        (
            0x800a_ca80,
            Instruction::Lbu {
                rt: Register::V1,
                base: Register::V1,
                offset: 0x1854,
            },
        ),
        (
            0x800a_ca84,
            Instruction::Sll {
                rd: Register::A0,
                rt: Register::A0,
                shift: 8,
            },
        ),
        (
            0x800a_ca88,
            Instruction::Sll {
                rd: Register::V1,
                rt: Register::V1,
                shift: 2,
            },
        ),
        (
            0x800a_ca8c,
            Instruction::Or {
                rd: Register::V0,
                rs: Register::V0,
                rt: Register::A0,
            },
        ),
        (
            0x800a_ca90,
            Instruction::Lui {
                rt: Register::AT,
                immediate: 0x8010,
            },
        ),
        (
            0x800a_ca94,
            Instruction::Addu {
                rd: Register::AT,
                rs: Register::V1,
                rt: Register::AT,
            },
        ),
        (
            0x800a_ca98,
            Instruction::Lw {
                rt: Register::V1,
                base: Register::AT,
                offset: 0x1000,
            },
        ),
        (
            0x800a_ca9c,
            Instruction::Sll {
                rd: Register::V0,
                rt: Register::V0,
                shift: 2,
            },
        ),
        (
            0x800a_caa0,
            Instruction::Addu {
                rd: Register::V0,
                rs: Register::V0,
                rt: Register::V1,
            },
        ),
        (
            0x800a_caa4,
            Instruction::Lw {
                rt: Register::A0,
                base: Register::V0,
                offset: 0,
            },
        ),
        (
            0x800a_cabc,
            Instruction::Jal {
                target: 0x800b_84d4,
            },
        ),
    ]
}

fn write_instruction(data: &mut [u8], runtime_address: u32, instruction: Instruction) {
    let offset = usize::try_from(runtime_address - MGAME_RUNTIME_BASE).unwrap();
    let word = encode(&instruction, runtime_address).unwrap();
    write_word(data, offset, word);
}

fn write_word(data: &mut [u8], offset: usize, word: u32) {
    data[offset..offset + 4].copy_from_slice(&word.to_le_bytes());
}
