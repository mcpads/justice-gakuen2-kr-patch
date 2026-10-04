use psx_r3000a::{Instruction, Register, encode};

use super::name_entry_save_transport::audit_name_save_transport;

const RUNTIME_BASE: u32 = 0x8001_0000;

#[test]
fn accepts_byte_exact_bidirectional_save_transport() {
    let text = fixture();
    let report = audit_name_save_transport("SLPS_021.20", &text, RUNTIME_BASE).unwrap();

    assert_eq!(report.copied_byte_count, 0x400);
    assert!(report.preserves_all_slot_bits);
    assert_eq!(report.save.direction, "staging_to_card_buffer");
    assert_eq!(report.load.direction, "card_buffer_to_staging");
    assert_eq!(report.name_record_staging_offset, None);
}

#[test]
fn rejects_a_halfword_store_in_the_byte_copy_loop() {
    let mut text = fixture();
    write_instruction(
        &mut text,
        0x8006_00a4,
        Instruction::Sh {
            rt: Register::V0,
            base: Register::A1,
            offset: 0,
        },
    );

    assert!(audit_name_save_transport("SLPS_021.20", &text, RUNTIME_BASE).is_err());
}

fn fixture() -> Vec<u8> {
    let mut text = vec![0u8; 0x500b0];
    for (address, instruction) in [
        (
            0x8006_0098,
            Instruction::Lbu {
                rt: Register::V0,
                base: Register::A0,
                offset: 0,
            },
        ),
        (
            0x8006_009c,
            Instruction::Addiu {
                rt: Register::A0,
                rs: Register::A0,
                immediate: 1,
            },
        ),
        (
            0x8006_00a0,
            Instruction::Addiu {
                rt: Register::A2,
                rs: Register::A2,
                immediate: -1,
            },
        ),
        (
            0x8006_00a4,
            Instruction::Sb {
                rt: Register::V0,
                base: Register::A1,
                offset: 0,
            },
        ),
        (
            0x8006_00a8,
            Instruction::Bgtz {
                rs: Register::A2,
                target: 0x8006_0098,
            },
        ),
        (
            0x8006_00ac,
            Instruction::Addiu {
                rt: Register::A1,
                rs: Register::A1,
                immediate: 1,
            },
        ),
        (
            0x8001_d524,
            Instruction::Addiu {
                rt: Register::A0,
                rs: Register::S0,
                immediate: 0x0180,
            },
        ),
        (
            0x8001_d528,
            Instruction::Lui {
                rt: Register::A1,
                immediate: 0x801f,
            },
        ),
        (
            0x8001_d52c,
            Instruction::Ori {
                rt: Register::A1,
                rs: Register::A1,
                immediate: 0x5800,
            },
        ),
        (
            0x8001_d530,
            Instruction::Jal {
                target: 0x8006_0088,
            },
        ),
        (
            0x8001_d534,
            Instruction::Addiu {
                rt: Register::A2,
                rs: Register::ZERO,
                immediate: 0x0400,
            },
        ),
        (
            0x8001_d9f0,
            Instruction::Lui {
                rt: Register::A0,
                immediate: 0x801f,
            },
        ),
        (
            0x8001_d9f4,
            Instruction::Ori {
                rt: Register::A0,
                rs: Register::A0,
                immediate: 0x5800,
            },
        ),
        (
            0x8001_d9f8,
            Instruction::Addiu {
                rt: Register::A1,
                rs: Register::S1,
                immediate: 0x0380,
            },
        ),
        (
            0x8001_d9fc,
            Instruction::Jal {
                target: 0x8006_0088,
            },
        ),
        (
            0x8001_da00,
            Instruction::Addiu {
                rt: Register::A2,
                rs: Register::ZERO,
                immediate: 0x0400,
            },
        ),
    ] {
        write_instruction(&mut text, address, instruction);
    }
    text
}

fn write_instruction(text: &mut [u8], address: u32, instruction: Instruction) {
    let offset = usize::try_from(address - RUNTIME_BASE).unwrap();
    text[offset..offset + 4].copy_from_slice(&encode(&instruction, address).unwrap().to_le_bytes());
}
