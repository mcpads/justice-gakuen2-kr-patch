use psx_r3000a::{Instruction, Register, encode};

use super::diary_save_slot_marker_flow::validate_diary_save_slot_marker_flow;

const BASE: u32 = 0x800a_2000;

#[test]
fn binds_the_clear_prompt_to_the_selected_slot_marker_writer() {
    let flow = validate_diary_save_slot_marker_flow(&fixture()).unwrap();

    assert_eq!(flow.selector_index, 5);
    assert_eq!(flow.clear_prompt_entry_index, 11);
    assert_eq!(flow.clear_prompt_action_row_index, 25);
    assert_eq!(flow.clear_prompt_flag_runtime_address, "0x801f1952");
    assert_eq!(flow.sentinel_state_runtime_address, "0x801f18ec");
    assert_eq!(flow.sentinel_state_value, 0xff);
    assert_eq!(flow.save_buffer_slot_marker_start_offset, "0x203");
    assert_eq!(flow.slot_marker_count, 5);
    assert_eq!(flow.marker_value, 1);
    assert_eq!(flow.marker_producer_runtime_address, "0x800c282c");
    assert_eq!(flow.marker_producer_call_runtime_addresses, ["0x800c19dc"]);
    assert!(flow.evidence_scope.contains("runtime gate"));
}

#[test]
fn rejects_a_clear_prompt_row_that_selects_another_message() {
    let mut data = fixture();
    data[0x25c4] = 10;

    let error = validate_diary_save_slot_marker_flow(&data).unwrap_err();

    assert!(error.to_string().contains("prompt action row"));
}

#[test]
fn rejects_a_selected_slot_marker_with_a_different_offset() {
    let mut data = fixture();
    write_instruction(
        &mut data,
        0x20870,
        Instruction::Sb {
            rt: Register::V1,
            base: Register::V0,
            offset: 4,
        },
    );

    let error = validate_diary_save_slot_marker_flow(&data).unwrap_err();

    assert!(error.to_string().contains("marker producer grammar"));
}

#[test]
fn rejects_a_different_natural_route_sentinel() {
    let mut data = fixture();
    write_instruction(
        &mut data,
        0x05578,
        Instruction::Addiu {
            rt: Register::V0,
            rs: Register::ZERO,
            immediate: 0x7f,
        },
    );

    let error = validate_diary_save_slot_marker_flow(&data).unwrap_err();

    assert!(error.to_string().contains("prompt flag path"));
}

#[test]
fn rejects_an_unaccounted_marker_producer_caller() {
    let mut data = fixture();
    write_instruction(
        &mut data,
        0x0100,
        Instruction::Jal {
            target: 0x800c_282c,
        },
    );

    let error = validate_diary_save_slot_marker_flow(&data).unwrap_err();

    assert!(error.to_string().contains("caller population"));
}

fn fixture() -> Vec<u8> {
    let mut data = vec![0u8; 0x20a00];
    data[0x25c4..0x25cc].copy_from_slice(&[11, 0, 2, 2, 0, 0, 0, 0]);
    for (offset, instruction) in [
        (
            0x05214,
            Instruction::Lui {
                rt: Register::V0,
                immediate: 0x801f,
            },
        ),
        (
            0x05218,
            Instruction::Lbu {
                rt: Register::V0,
                base: Register::V0,
                offset: 0x18f4,
            },
        ),
        (
            0x05220,
            Instruction::Beq {
                rs: Register::V0,
                rt: Register::ZERO,
                target: 0x800a_7570,
            },
        ),
        (
            0x05570,
            Instruction::Lui {
                rt: Register::V1,
                immediate: 0x801f,
            },
        ),
        (
            0x05574,
            Instruction::Lbu {
                rt: Register::V1,
                base: Register::V1,
                offset: 0x18ec,
            },
        ),
        (
            0x05578,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::ZERO,
                immediate: 0xff,
            },
        ),
        (
            0x0557c,
            Instruction::Beq {
                rs: Register::V1,
                rt: Register::V0,
                target: 0x800a_759c,
            },
        ),
        (
            0x0559c,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::ZERO,
                immediate: 1,
            },
        ),
        (
            0x055a0,
            Instruction::Addiu {
                rt: Register::V1,
                rs: Register::ZERO,
                immediate: 0x3c,
            },
        ),
        (
            0x055a4,
            Instruction::Lui {
                rt: Register::AT,
                immediate: 0x801f,
            },
        ),
        (
            0x055a8,
            Instruction::Sb {
                rt: Register::V1,
                base: Register::AT,
                offset: 0x1b01,
            },
        ),
        (
            0x055ac,
            Instruction::Addiu {
                rt: Register::V1,
                rs: Register::ZERO,
                immediate: 1,
            },
        ),
        (
            0x055b0,
            Instruction::Lui {
                rt: Register::AT,
                immediate: 0x801f,
            },
        ),
        (
            0x055b4,
            Instruction::Sb {
                rt: Register::ZERO,
                base: Register::AT,
                offset: 0x1b02,
            },
        ),
        (
            0x055b8,
            Instruction::Lui {
                rt: Register::AT,
                immediate: 0x801f,
            },
        ),
        (
            0x055bc,
            Instruction::Sb {
                rt: Register::V1,
                base: Register::AT,
                offset: 0x1952,
            },
        ),
        (
            0x055c0,
            Instruction::Lui {
                rt: Register::AT,
                immediate: 0x801f,
            },
        ),
        (
            0x055c4,
            Instruction::Sb {
                rt: Register::V1,
                base: Register::AT,
                offset: 0x1954,
            },
        ),
        (
            0x1f164,
            Instruction::Lui {
                rt: Register::V0,
                immediate: 0x801f,
            },
        ),
        (
            0x1f168,
            Instruction::Lbu {
                rt: Register::V0,
                base: Register::V0,
                offset: 0x1952,
            },
        ),
        (
            0x1f174,
            Instruction::Beq {
                rs: Register::V0,
                rt: Register::ZERO,
                target: 0x800c_1194,
            },
        ),
        (
            0x1f178,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::ZERO,
                immediate: 0x14,
            },
        ),
        (
            0x1f17c,
            Instruction::Sb {
                rt: Register::V0,
                base: Register::S0,
                offset: 6,
            },
        ),
        (
            0x1f180,
            Instruction::Addu {
                rd: Register::A0,
                rs: Register::S0,
                rt: Register::ZERO,
            },
        ),
        (
            0x1f184,
            Instruction::Jal {
                target: 0x800c_2400,
            },
        ),
        (
            0x1f188,
            Instruction::Addiu {
                rt: Register::A1,
                rs: Register::ZERO,
                immediate: 25,
            },
        ),
        (
            0x1f9d4,
            Instruction::Lui {
                rt: Register::A0,
                immediate: 0x801f,
            },
        ),
        (
            0x1f9d8,
            Instruction::Lbu {
                rt: Register::A0,
                base: Register::A0,
                offset: 0x1950,
            },
        ),
        (
            0x1f9dc,
            Instruction::Jal {
                target: 0x800c_282c,
            },
        ),
        (
            0x20404,
            Instruction::Andi {
                rt: Register::A1,
                rs: Register::A1,
                immediate: 0xff,
            },
        ),
        (
            0x2040c,
            Instruction::Sll {
                rd: Register::S1,
                rt: Register::A1,
                shift: 3,
            },
        ),
        (
            0x20418,
            Instruction::Lui {
                rt: Register::AT,
                immediate: 0x800a,
            },
        ),
        (
            0x2041c,
            Instruction::Addu {
                rd: Register::AT,
                rs: Register::AT,
                rt: Register::S1,
            },
        ),
        (
            0x20420,
            Instruction::Lbu {
                rt: Register::V0,
                base: Register::AT,
                offset: 0x44fc,
            },
        ),
        (
            0x20424,
            Instruction::Lui {
                rt: Register::V1,
                immediate: 0x8010,
            },
        ),
        (
            0x20428,
            Instruction::Lw {
                rt: Register::V1,
                base: Register::V1,
                offset: 0x1014,
            },
        ),
        (
            0x2042c,
            Instruction::Sll {
                rd: Register::V0,
                rt: Register::V0,
                shift: 2,
            },
        ),
        (
            0x20430,
            Instruction::Addu {
                rd: Register::V0,
                rs: Register::V0,
                rt: Register::V1,
            },
        ),
        (
            0x20434,
            Instruction::Lw {
                rt: Register::A1,
                base: Register::V0,
                offset: 0,
            },
        ),
        (
            0x20834,
            Instruction::Addu {
                rd: Register::S2,
                rs: Register::A0,
                rt: Register::ZERO,
            },
        ),
        (
            0x20844,
            Instruction::Lui {
                rt: Register::S3,
                immediate: 0x801f,
            },
        ),
        (
            0x20848,
            Instruction::Lw {
                rt: Register::S3,
                base: Register::S3,
                offset: 0x63f0,
            },
        ),
        (
            0x2084c,
            Instruction::Addiu {
                rt: Register::V1,
                rs: Register::ZERO,
                immediate: 1,
            },
        ),
        (
            0x20864,
            Instruction::Addiu {
                rt: Register::S1,
                rs: Register::S3,
                immediate: 0x200,
            },
        ),
        (
            0x20868,
            Instruction::Addu {
                rd: Register::V0,
                rs: Register::S1,
                rt: Register::S2,
            },
        ),
        (
            0x2086c,
            Instruction::Sb {
                rt: Register::V1,
                base: Register::S3,
                offset: 0x201,
            },
        ),
        (
            0x20870,
            Instruction::Sb {
                rt: Register::V1,
                base: Register::V0,
                offset: 3,
            },
        ),
        (
            0x208b8,
            Instruction::Addu {
                rd: Register::A3,
                rs: Register::ZERO,
                rt: Register::ZERO,
            },
        ),
        (
            0x208d4,
            Instruction::Addu {
                rd: Register::V0,
                rs: Register::S1,
                rt: Register::A3,
            },
        ),
        (
            0x209c0,
            Instruction::Addiu {
                rt: Register::A3,
                rs: Register::A3,
                immediate: 1,
            },
        ),
        (
            0x209c4,
            Instruction::Slti {
                rt: Register::V0,
                rs: Register::A3,
                immediate: 5,
            },
        ),
        (
            0x209c8,
            Instruction::Bne {
                rs: Register::V0,
                rt: Register::ZERO,
                target: 0x800c_28d4,
            },
        ),
        (
            0x209d0,
            Instruction::Addiu {
                rt: Register::S3,
                rs: Register::S3,
                immediate: 0x1180,
            },
        ),
        (
            0x209d4,
            Instruction::Addiu {
                rt: Register::S1,
                rs: Register::S3,
                immediate: 0x200,
            },
        ),
        (
            0x209e0,
            Instruction::Addiu {
                rt: Register::V1,
                rs: Register::ZERO,
                immediate: 1,
            },
        ),
        (
            0x209e4,
            Instruction::Addu {
                rd: Register::V0,
                rs: Register::S1,
                rt: Register::S2,
            },
        ),
        (
            0x209e8,
            Instruction::Sb {
                rt: Register::V1,
                base: Register::S3,
                offset: 0x201,
            },
        ),
        (
            0x209ec,
            Instruction::Sb {
                rt: Register::V1,
                base: Register::V0,
                offset: 3,
            },
        ),
    ] {
        write_instruction(&mut data, offset, instruction);
    }
    data
}

fn write_instruction(data: &mut [u8], offset: usize, instruction: Instruction) {
    let word = encode(&instruction, BASE + offset as u32).unwrap();
    data[offset..offset + 4].copy_from_slice(&word.to_le_bytes());
}
