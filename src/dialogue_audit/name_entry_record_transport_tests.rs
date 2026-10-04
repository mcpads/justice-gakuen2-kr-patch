use psx_r3000a::{Instruction, Register, encode};

use super::name_entry_record_transport::audit_name_record_transport;

const RUNTIME_BASE: u32 = 0x800a_2000;

#[test]
fn accepts_the_source_shaped_in_game_name_record_transport() {
    let report = audit_name_record_transport(&fixture()).unwrap();

    assert_eq!(report.live_record_runtime_address, "0x801f1800");
    assert_eq!(report.record_byte_count, 0x200);
    assert_eq!(report.slot_stride, 0x200);
    assert_eq!(report.save_record_workspace_offset, "0x0780");
    assert_eq!(report.card_record_workspace_offset, "0x1900");
    assert_eq!(report.workspace_to_card_delta, "0x1180");
    assert_eq!(report.save_call_runtime_address, "0x800c28b0");
    assert_eq!(report.load_call_runtime_address, "0x800c2ab8");
    assert_eq!(report.fields.len(), 4);
    assert_eq!(report.fields[0].record_offset, "0x0066");
    assert_eq!(report.fields[2].name, "nickname_companion");
    assert_eq!(report.fields[3].terminator_offset, "0x009e");
    assert!(report.preserves_all_slot_bits);
    assert_eq!(report.memory_card_file_offset, None);
}

#[test]
fn rejects_a_narrowed_record_copy() {
    let mut mgame = fixture();
    write_instruction(
        &mut mgame,
        0x800c_28b4,
        Instruction::Addiu {
            rt: Register::A2,
            rs: Register::ZERO,
            immediate: 0x0100,
        },
    );

    assert!(audit_name_record_transport(&mgame).is_err());
}

#[test]
fn rejects_a_halfword_copy_store() {
    let mut mgame = fixture();
    write_instruction(
        &mut mgame,
        0x800c_9fb4,
        Instruction::Sh {
            rt: Register::V0,
            base: Register::A1,
            offset: 0,
        },
    );

    assert!(audit_name_record_transport(&mgame).is_err());
}

fn fixture() -> Vec<u8> {
    let mut mgame = vec![0u8; 0x296a4];
    for (address, instruction) in [
        (
            0x800c_9fa8,
            Instruction::Lbu {
                rt: Register::V0,
                base: Register::A0,
                offset: 0,
            },
        ),
        (
            0x800c_9fac,
            Instruction::Addiu {
                rt: Register::A0,
                rs: Register::A0,
                immediate: 1,
            },
        ),
        (
            0x800c_9fb0,
            Instruction::Addiu {
                rt: Register::A2,
                rs: Register::A2,
                immediate: -1,
            },
        ),
        (
            0x800c_9fb4,
            Instruction::Sb {
                rt: Register::V0,
                base: Register::A1,
                offset: 0,
            },
        ),
        (
            0x800c_9fb8,
            Instruction::Bgtz {
                rs: Register::A2,
                target: 0x800c_9fa8,
            },
        ),
        (
            0x800c_9fbc,
            Instruction::Addiu {
                rt: Register::A1,
                rs: Register::A1,
                immediate: 1,
            },
        ),
        (
            0x800c_2844,
            Instruction::Lui {
                rt: Register::S3,
                immediate: 0x801f,
            },
        ),
        (
            0x800c_2848,
            Instruction::Lw {
                rt: Register::S3,
                base: Register::S3,
                offset: 0x63f0,
            },
        ),
        (
            0x800c_2864,
            Instruction::Addiu {
                rt: Register::S1,
                rs: Register::S3,
                immediate: 0x0200,
            },
        ),
        (
            0x800c_289c,
            Instruction::Lui {
                rt: Register::A0,
                immediate: 0x801f,
            },
        ),
        (
            0x800c_28a0,
            Instruction::Ori {
                rt: Register::A0,
                rs: Register::A0,
                immediate: 0x1800,
            },
        ),
        (
            0x800c_28a4,
            Instruction::Sll {
                rd: Register::A1,
                rt: Register::S2,
                shift: 9,
            },
        ),
        (
            0x800c_28a8,
            Instruction::Addiu {
                rt: Register::A1,
                rs: Register::A1,
                immediate: 0x0580,
            },
        ),
        (
            0x800c_28ac,
            Instruction::Addu {
                rd: Register::A1,
                rs: Register::S1,
                rt: Register::A1,
            },
        ),
        (
            0x800c_28b0,
            Instruction::Jal {
                target: 0x800c_9f98,
            },
        ),
        (
            0x800c_28b4,
            Instruction::Addiu {
                rt: Register::A2,
                rs: Register::ZERO,
                immediate: 0x0200,
            },
        ),
        (
            0x800c_25d4,
            Instruction::Addu {
                rd: Register::A0,
                rs: Register::S0,
                rt: Register::ZERO,
            },
        ),
        (
            0x800c_25d8,
            Instruction::Addiu {
                rt: Register::A1,
                rs: Register::A0,
                immediate: 0x1180,
            },
        ),
        (
            0x800c_25dc,
            Instruction::Jal {
                target: 0x800c_9f98,
            },
        ),
        (
            0x800c_25e0,
            Instruction::Addiu {
                rt: Register::A2,
                rs: Register::ZERO,
                immediate: 0x1180,
            },
        ),
        (
            0x800c_2a80,
            Instruction::Lui {
                rt: Register::S1,
                immediate: 0x801f,
            },
        ),
        (
            0x800c_2a84,
            Instruction::Lw {
                rt: Register::S1,
                base: Register::S1,
                offset: 0x63f0,
            },
        ),
        (
            0x800c_2a8c,
            Instruction::Addu {
                rd: Register::S0,
                rs: Register::A0,
                rt: Register::ZERO,
            },
        ),
        (
            0x800c_2aa0,
            Instruction::Addiu {
                rt: Register::S1,
                rs: Register::S1,
                immediate: 0x1380,
            },
        ),
        (
            0x800c_2aa4,
            Instruction::Lui {
                rt: Register::A1,
                immediate: 0x801f,
            },
        ),
        (
            0x800c_2aa8,
            Instruction::Ori {
                rt: Register::A1,
                rs: Register::A1,
                immediate: 0x1800,
            },
        ),
        (
            0x800c_2aac,
            Instruction::Sll {
                rd: Register::S0,
                rt: Register::S0,
                shift: 9,
            },
        ),
        (
            0x800c_2ab0,
            Instruction::Addiu {
                rt: Register::S0,
                rs: Register::S0,
                immediate: 0x0580,
            },
        ),
        (
            0x800c_2ab4,
            Instruction::Addu {
                rd: Register::A0,
                rs: Register::S1,
                rt: Register::S0,
            },
        ),
        (
            0x800c_2ab8,
            Instruction::Jal {
                target: 0x800c_9f98,
            },
        ),
        (
            0x800c_2abc,
            Instruction::Addiu {
                rt: Register::A2,
                rs: Register::ZERO,
                immediate: 0x0200,
            },
        ),
    ] {
        write_instruction(&mut mgame, address, instruction);
    }
    mgame
}

fn write_instruction(mgame: &mut [u8], address: u32, instruction: Instruction) {
    let offset = usize::try_from(address - RUNTIME_BASE).unwrap();
    mgame[offset..offset + 4]
        .copy_from_slice(&encode(&instruction, address).unwrap().to_le_bytes());
}
