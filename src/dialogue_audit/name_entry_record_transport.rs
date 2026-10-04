use anyhow::{Context, Result, ensure};
use psx_r3000a::{Instruction, Register, decode};

use crate::name_input::NICKNAME_COMPANION_TO_NICKNAME_BYTE_DISPLACEMENT;

use super::name_entry_model::{DialogueNameRecordFieldAudit, DialogueNameRecordTransportAudit};
use super::script_source::{MGAME_PATH, MGAME_SHA256, MGAME_SIZE};

const RUNTIME_BASE: u32 = 0x800a_2000;
const WORKSPACE_POINTER_RUNTIME_ADDRESS: u32 = 0x801f_63f0;
const LIVE_RECORD_RUNTIME_ADDRESS: u32 = 0x801f_1800;
const RECORD_BYTE_COUNT: usize = 0x0200;
const SLOT_STRIDE: usize = 0x0200;
const SAVE_RECORD_WORKSPACE_OFFSET: usize = 0x0780;
const CARD_RECORD_WORKSPACE_OFFSET: usize = 0x1900;
const WORKSPACE_TO_CARD_DELTA: usize = 0x1180;
const WORKSPACE_CLONE_BYTE_COUNT: usize = 0x1180;

const BYTE_COPY_ROUTINE_RUNTIME_ADDRESS: u32 = 0x800c_9f98;
const BYTE_LOAD_RUNTIME_ADDRESS: u32 = 0x800c_9fa8;
const BYTE_STORE_RUNTIME_ADDRESS: u32 = 0x800c_9fb4;
const SAVE_CALL_RUNTIME_ADDRESS: u32 = 0x800c_28b0;
const WORKSPACE_CLONE_CALL_RUNTIME_ADDRESS: u32 = 0x800c_25dc;
const LOAD_CALL_RUNTIME_ADDRESS: u32 = 0x800c_2ab8;

const FIELD_SPECS: [(&str, usize, usize, usize); 4] = [
    ("family_name", 0x0066, 6, 0x0072),
    ("given_name", 0x0076, 6, 0x0082),
    ("nickname_companion", 0x0086, 4, 0x008e),
    (
        "nickname",
        0x0086 + NICKNAME_COMPANION_TO_NICKNAME_BYTE_DISPLACEMENT as usize,
        4,
        0x009e,
    ),
];

pub(super) fn audit_name_record_transport(
    mgame: &[u8],
) -> Result<DialogueNameRecordTransportAudit> {
    ensure!(mgame.len() == MGAME_SIZE, "unexpected {MGAME_PATH} size");
    validate_byte_copy(mgame)?;
    validate_save_edge(mgame)?;
    validate_workspace_clone(mgame)?;
    validate_load_edge(mgame)?;

    Ok(DialogueNameRecordTransportAudit {
        source_path: MGAME_PATH.to_string(),
        source_sha256: MGAME_SHA256.to_string(),
        runtime_base: hex_address(RUNTIME_BASE),
        workspace_pointer_runtime_address: hex_address(WORKSPACE_POINTER_RUNTIME_ADDRESS),
        live_record_runtime_address: hex_address(LIVE_RECORD_RUNTIME_ADDRESS),
        record_byte_count: RECORD_BYTE_COUNT,
        slot_stride: SLOT_STRIDE,
        save_record_workspace_offset: hex_offset(SAVE_RECORD_WORKSPACE_OFFSET),
        card_record_workspace_offset: hex_offset(CARD_RECORD_WORKSPACE_OFFSET),
        workspace_to_card_delta: hex_offset(WORKSPACE_TO_CARD_DELTA),
        workspace_clone_byte_count: WORKSPACE_CLONE_BYTE_COUNT,
        byte_copy_routine_runtime_address: hex_address(BYTE_COPY_ROUTINE_RUNTIME_ADDRESS),
        byte_load_runtime_address: hex_address(BYTE_LOAD_RUNTIME_ADDRESS),
        byte_store_runtime_address: hex_address(BYTE_STORE_RUNTIME_ADDRESS),
        save_call_runtime_address: hex_address(SAVE_CALL_RUNTIME_ADDRESS),
        workspace_clone_call_runtime_address: hex_address(WORKSPACE_CLONE_CALL_RUNTIME_ADDRESS),
        load_call_runtime_address: hex_address(LOAD_CALL_RUNTIME_ADDRESS),
        fields: FIELD_SPECS
            .into_iter()
            .map(
                |(name, record_offset, visible_slot_count, terminator_offset)| {
                    DialogueNameRecordFieldAudit {
                        name: name.to_string(),
                        record_offset: hex_offset(record_offset),
                        visible_slot_count,
                        terminator_offset: hex_offset(terminator_offset),
                    }
                },
            )
            .collect(),
        preserves_all_slot_bits: true,
        memory_card_file_offset: None,
    })
}

fn validate_byte_copy(mgame: &[u8]) -> Result<()> {
    for (address, expected) in [
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
    ] {
        ensure_instruction(mgame, address, expected)?;
    }
    Ok(())
}

fn validate_save_edge(mgame: &[u8]) -> Result<()> {
    for (address, expected) in [
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
            SAVE_CALL_RUNTIME_ADDRESS,
            Instruction::Jal {
                target: BYTE_COPY_ROUTINE_RUNTIME_ADDRESS,
            },
        ),
        (
            0x800c_28b4,
            Instruction::Addiu {
                rt: Register::A2,
                rs: Register::ZERO,
                immediate: RECORD_BYTE_COUNT as i16,
            },
        ),
    ] {
        ensure_instruction(mgame, address, expected)?;
    }
    Ok(())
}

fn validate_workspace_clone(mgame: &[u8]) -> Result<()> {
    for (address, expected) in [
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
                immediate: WORKSPACE_TO_CARD_DELTA as i16,
            },
        ),
        (
            WORKSPACE_CLONE_CALL_RUNTIME_ADDRESS,
            Instruction::Jal {
                target: BYTE_COPY_ROUTINE_RUNTIME_ADDRESS,
            },
        ),
        (
            0x800c_25e0,
            Instruction::Addiu {
                rt: Register::A2,
                rs: Register::ZERO,
                immediate: WORKSPACE_CLONE_BYTE_COUNT as i16,
            },
        ),
    ] {
        ensure_instruction(mgame, address, expected)?;
    }
    Ok(())
}

fn validate_load_edge(mgame: &[u8]) -> Result<()> {
    for (address, expected) in [
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
            LOAD_CALL_RUNTIME_ADDRESS,
            Instruction::Jal {
                target: BYTE_COPY_ROUTINE_RUNTIME_ADDRESS,
            },
        ),
        (
            0x800c_2abc,
            Instruction::Addiu {
                rt: Register::A2,
                rs: Register::ZERO,
                immediate: RECORD_BYTE_COUNT as i16,
            },
        ),
    ] {
        ensure_instruction(mgame, address, expected)?;
    }
    Ok(())
}

fn ensure_instruction(mgame: &[u8], runtime_address: u32, expected: Instruction) -> Result<()> {
    let offset = runtime_address
        .checked_sub(RUNTIME_BASE)
        .context("MGAME instruction precedes the overlay runtime base")?;
    let offset = usize::try_from(offset)?;
    let bytes = mgame
        .get(offset..offset + 4)
        .context("MGAME instruction is truncated")?;
    let word = u32::from_le_bytes(bytes.try_into()?);
    let actual = decode(word, runtime_address)?;
    ensure!(
        actual == expected,
        "in-game name-record transport instruction changed at {runtime_address:#010x}"
    );
    Ok(())
}

fn hex_address(address: u32) -> String {
    format!("0x{address:08x}")
}

fn hex_offset(offset: usize) -> String {
    format!("0x{offset:04x}")
}
