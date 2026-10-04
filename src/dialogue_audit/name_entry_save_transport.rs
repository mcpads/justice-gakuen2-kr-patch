use anyhow::{Context, Result, ensure};
use psx_r3000a::{Instruction, Register, decode};

use super::name_entry_model::{DialogueNameSaveCopyEdgeAudit, DialogueNameSaveTransportAudit};

const MAIN_TEXT_RUNTIME_BASE: u32 = 0x8001_0000;
const SAVE_STAGING_RUNTIME_ADDRESS: u32 = 0x801f_5800;
const SAVE_BLOCK_BYTE_COUNT: usize = 0x400;
const BYTE_COPY_ROUTINE_RUNTIME_ADDRESS: u32 = 0x8006_0088;
const BYTE_LOAD_RUNTIME_ADDRESS: u32 = 0x8006_0098;
const BYTE_STORE_RUNTIME_ADDRESS: u32 = 0x8006_00a4;
const LOAD_COPY_CALL_RUNTIME_ADDRESS: u32 = 0x8001_d530;
const SAVE_COPY_CALL_RUNTIME_ADDRESS: u32 = 0x8001_d9fc;

pub(super) fn audit_name_save_transport(
    source_path: &str,
    main_text: &[u8],
    runtime_base: u32,
) -> Result<DialogueNameSaveTransportAudit> {
    ensure!(
        runtime_base == MAIN_TEXT_RUNTIME_BASE,
        "unexpected main executable text runtime base"
    );

    validate_byte_copy(main_text, runtime_base)?;
    validate_load_edge(main_text, runtime_base)?;
    validate_save_edge(main_text, runtime_base)?;

    Ok(DialogueNameSaveTransportAudit {
        source_path: source_path.to_string(),
        staging_runtime_address: hex_address(SAVE_STAGING_RUNTIME_ADDRESS),
        copied_byte_count: SAVE_BLOCK_BYTE_COUNT,
        byte_copy_routine_runtime_address: hex_address(BYTE_COPY_ROUTINE_RUNTIME_ADDRESS),
        byte_load_runtime_address: hex_address(BYTE_LOAD_RUNTIME_ADDRESS),
        byte_store_runtime_address: hex_address(BYTE_STORE_RUNTIME_ADDRESS),
        save: DialogueNameSaveCopyEdgeAudit {
            direction: "staging_to_card_buffer".to_string(),
            call_runtime_address: hex_address(SAVE_COPY_CALL_RUNTIME_ADDRESS),
            staging_argument_register: "a0".to_string(),
            card_buffer_argument_register: "a1".to_string(),
            card_buffer_offset: "0x0380".to_string(),
        },
        load: DialogueNameSaveCopyEdgeAudit {
            direction: "card_buffer_to_staging".to_string(),
            call_runtime_address: hex_address(LOAD_COPY_CALL_RUNTIME_ADDRESS),
            staging_argument_register: "a1".to_string(),
            card_buffer_argument_register: "a0".to_string(),
            card_buffer_offset: "0x0180".to_string(),
        },
        preserves_all_slot_bits: true,
        name_record_staging_offset: None,
    })
}

fn validate_byte_copy(main_text: &[u8], runtime_base: u32) -> Result<()> {
    for (address, expected) in [
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
    ] {
        ensure_instruction(main_text, runtime_base, address, expected)?;
    }
    Ok(())
}

fn validate_load_edge(main_text: &[u8], runtime_base: u32) -> Result<()> {
    for (address, expected) in [
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
            LOAD_COPY_CALL_RUNTIME_ADDRESS,
            Instruction::Jal {
                target: BYTE_COPY_ROUTINE_RUNTIME_ADDRESS,
            },
        ),
        (
            0x8001_d534,
            Instruction::Addiu {
                rt: Register::A2,
                rs: Register::ZERO,
                immediate: SAVE_BLOCK_BYTE_COUNT as i16,
            },
        ),
    ] {
        ensure_instruction(main_text, runtime_base, address, expected)?;
    }
    Ok(())
}

fn validate_save_edge(main_text: &[u8], runtime_base: u32) -> Result<()> {
    for (address, expected) in [
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
            SAVE_COPY_CALL_RUNTIME_ADDRESS,
            Instruction::Jal {
                target: BYTE_COPY_ROUTINE_RUNTIME_ADDRESS,
            },
        ),
        (
            0x8001_da00,
            Instruction::Addiu {
                rt: Register::A2,
                rs: Register::ZERO,
                immediate: SAVE_BLOCK_BYTE_COUNT as i16,
            },
        ),
    ] {
        ensure_instruction(main_text, runtime_base, address, expected)?;
    }
    Ok(())
}

fn ensure_instruction(
    main_text: &[u8],
    runtime_base: u32,
    runtime_address: u32,
    expected: Instruction,
) -> Result<()> {
    let offset = runtime_address
        .checked_sub(runtime_base)
        .context("main executable instruction precedes loaded text")?;
    let offset = usize::try_from(offset)?;
    let bytes = main_text
        .get(offset..offset + 4)
        .context("main executable instruction is truncated")?;
    let word = u32::from_le_bytes(bytes.try_into()?);
    let actual = decode(word, runtime_address)?;
    ensure!(
        actual == expected,
        "name save transport instruction changed at {runtime_address:#010x}"
    );
    Ok(())
}

fn hex_address(address: u32) -> String {
    format!("0x{address:08x}")
}
