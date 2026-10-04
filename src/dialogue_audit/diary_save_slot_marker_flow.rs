use anyhow::{Context, Result, ensure};
use psx_r3000a::{Instruction, Register, decode};

use super::format::hex_address;
use super::selector_address_flow_model::DiarySaveSlotMarkerFlow;

const MGAME_RUNTIME_BASE: u32 = 0x800a_2000;
const SELECTOR_INDEX: usize = 5;
const CLEAR_PROMPT_ENTRY_INDEX: usize = 11;
const CLEAR_PROMPT_ACTION_ROW_INDEX: usize = 25;
const ACTION_ROW_SIZE: usize = 8;
const ACTION_TABLE_OFFSET: usize = 0x24fc;
const ACTION_TABLE_RUNTIME_LOW: i16 = 0x44fc;
const CLEAR_PROMPT_ACTION_ROW_OFFSET: usize =
    ACTION_TABLE_OFFSET + CLEAR_PROMPT_ACTION_ROW_INDEX * ACTION_ROW_SIZE;
const CLEAR_PROMPT_ACTION_ROW: [u8; ACTION_ROW_SIZE] = [11, 0, 2, 2, 0, 0, 0, 0];

const CLEAR_PROMPT_FLAG_RUNTIME_ADDRESS: u32 = 0x801f_1952;
const ALTERNATE_SAVE_PATH_FLAG_RUNTIME_ADDRESS: u32 = 0x801f_1954;
const SENTINEL_STATE_RUNTIME_ADDRESS: u32 = 0x801f_18ec;
const SELECTED_SLOT_RUNTIME_ADDRESS: u32 = 0x801f_1950;
const SAVE_BUFFER_POINTER_STORAGE_RUNTIME_ADDRESS: u32 = 0x801f_63f0;
const SAVE_BUFFER_HEADER_OFFSET: u16 = 0x200;
const SAVE_BUFFER_STATE_MARKER_OFFSET: u16 = 0x201;
const SAVE_BUFFER_SLOT_MARKER_START_OFFSET: u16 = 0x203;
const SLOT_MARKER_COUNT: usize = 5;
const MIRROR_BUFFER_OFFSET: u16 = 0x1180;
const MARKER_VALUE: u8 = 1;
const SENTINEL_STATE_VALUE: u8 = 0xff;
const REQUIRED_ALTERNATE_STATE_FLAG_VALUE: u8 = 0;

const MARKER_PRODUCER_RUNTIME_ADDRESS: u32 = 0x800c_282c;
const MARKER_PRODUCER_CALL_OFFSET: usize = 0x1f9dc;

pub(super) fn validate_diary_save_slot_marker_flow(
    mgame: &[u8],
) -> Result<DiarySaveSlotMarkerFlow> {
    ensure!(
        mgame.get(
            CLEAR_PROMPT_ACTION_ROW_OFFSET
                ..CLEAR_PROMPT_ACTION_ROW_OFFSET + CLEAR_PROMPT_ACTION_ROW.len()
        ) == Some(CLEAR_PROMPT_ACTION_ROW.as_slice()),
        "Diary clear-save prompt action row changed"
    );

    validate_clear_prompt_selector_path(mgame)?;
    validate_clear_prompt_flag_path(mgame)?;
    validate_marker_producer(mgame)?;

    let marker_producer_call_offsets = direct_call_offsets(mgame, MARKER_PRODUCER_RUNTIME_ADDRESS)?;
    ensure!(
        marker_producer_call_offsets == [MARKER_PRODUCER_CALL_OFFSET],
        "Diary save-slot marker producer caller population changed"
    );
    validate_instructions(
        mgame,
        &[
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
                    offset: SELECTED_SLOT_RUNTIME_ADDRESS as i16,
                },
            ),
            (
                MARKER_PRODUCER_CALL_OFFSET,
                Instruction::Jal {
                    target: MARKER_PRODUCER_RUNTIME_ADDRESS,
                },
            ),
        ],
        "Diary save-slot marker producer call",
    )?;

    Ok(DiarySaveSlotMarkerFlow {
        kind: "Justice Gakuen 2 Diary save-slot marker flow".to_string(),
        evidence_scope: "static source-bound MGAME control and data flow; natural gameplay reachability remains a runtime gate".to_string(),
        selector_index: SELECTOR_INDEX,
        clear_prompt_entry_index: CLEAR_PROMPT_ENTRY_INDEX,
        clear_prompt_action_row_index: CLEAR_PROMPT_ACTION_ROW_INDEX,
        clear_prompt_action_row_runtime_address: hex_address(runtime_address(
            CLEAR_PROMPT_ACTION_ROW_OFFSET,
        )?),
        clear_prompt_flag_runtime_address: hex_address(CLEAR_PROMPT_FLAG_RUNTIME_ADDRESS),
        alternate_save_path_flag_runtime_address: hex_address(
            ALTERNATE_SAVE_PATH_FLAG_RUNTIME_ADDRESS,
        ),
        sentinel_state_runtime_address: hex_address(SENTINEL_STATE_RUNTIME_ADDRESS),
        sentinel_state_value: SENTINEL_STATE_VALUE,
        required_alternate_state_flag_value: REQUIRED_ALTERNATE_STATE_FLAG_VALUE,
        selected_slot_runtime_address: hex_address(SELECTED_SLOT_RUNTIME_ADDRESS),
        save_buffer_pointer_storage_runtime_address: hex_address(
            SAVE_BUFFER_POINTER_STORAGE_RUNTIME_ADDRESS,
        ),
        save_buffer_header_offset: hex_offset(SAVE_BUFFER_HEADER_OFFSET),
        save_buffer_state_marker_offset: hex_offset(SAVE_BUFFER_STATE_MARKER_OFFSET),
        save_buffer_slot_marker_start_offset: hex_offset(SAVE_BUFFER_SLOT_MARKER_START_OFFSET),
        slot_marker_count: SLOT_MARKER_COUNT,
        mirror_buffer_offset: hex_offset(MIRROR_BUFFER_OFFSET),
        marker_value: MARKER_VALUE,
        marker_producer_runtime_address: hex_address(MARKER_PRODUCER_RUNTIME_ADDRESS),
        marker_producer_call_runtime_addresses: marker_producer_call_offsets
            .into_iter()
            .map(runtime_address)
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .map(hex_address)
            .collect(),
    })
}

fn validate_clear_prompt_selector_path(mgame: &[u8]) -> Result<()> {
    validate_instructions(
        mgame,
        &[
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
                    offset: ACTION_TABLE_RUNTIME_LOW,
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
        ],
        "Diary clear-save selector path",
    )
}

fn validate_clear_prompt_flag_path(mgame: &[u8]) -> Result<()> {
    validate_instructions(
        mgame,
        &[
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
                    offset: SENTINEL_STATE_RUNTIME_ADDRESS as i16,
                },
            ),
            (
                0x05578,
                Instruction::Addiu {
                    rt: Register::V0,
                    rs: Register::ZERO,
                    immediate: SENTINEL_STATE_VALUE as i16,
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
                    immediate: MARKER_VALUE as i16,
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
                    offset: CLEAR_PROMPT_FLAG_RUNTIME_ADDRESS as i16,
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
                    offset: ALTERNATE_SAVE_PATH_FLAG_RUNTIME_ADDRESS as i16,
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
                    offset: CLEAR_PROMPT_FLAG_RUNTIME_ADDRESS as i16,
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
                    immediate: CLEAR_PROMPT_ACTION_ROW_INDEX as i16,
                },
            ),
        ],
        "Diary clear-save prompt flag path",
    )
}

fn validate_marker_producer(mgame: &[u8]) -> Result<()> {
    validate_instructions(
        mgame,
        &[
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
                    offset: SAVE_BUFFER_POINTER_STORAGE_RUNTIME_ADDRESS as i16,
                },
            ),
            (
                0x2084c,
                Instruction::Addiu {
                    rt: Register::V1,
                    rs: Register::ZERO,
                    immediate: MARKER_VALUE as i16,
                },
            ),
            (
                0x20864,
                Instruction::Addiu {
                    rt: Register::S1,
                    rs: Register::S3,
                    immediate: SAVE_BUFFER_HEADER_OFFSET as i16,
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
                    offset: SAVE_BUFFER_STATE_MARKER_OFFSET as i16,
                },
            ),
            (
                0x20870,
                Instruction::Sb {
                    rt: Register::V1,
                    base: Register::V0,
                    offset: (SAVE_BUFFER_SLOT_MARKER_START_OFFSET - SAVE_BUFFER_HEADER_OFFSET)
                        as i16,
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
                    immediate: SLOT_MARKER_COUNT as i16,
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
                    immediate: MIRROR_BUFFER_OFFSET as i16,
                },
            ),
            (
                0x209d4,
                Instruction::Addiu {
                    rt: Register::S1,
                    rs: Register::S3,
                    immediate: SAVE_BUFFER_HEADER_OFFSET as i16,
                },
            ),
            (
                0x209e0,
                Instruction::Addiu {
                    rt: Register::V1,
                    rs: Register::ZERO,
                    immediate: MARKER_VALUE as i16,
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
                    offset: SAVE_BUFFER_STATE_MARKER_OFFSET as i16,
                },
            ),
            (
                0x209ec,
                Instruction::Sb {
                    rt: Register::V1,
                    base: Register::V0,
                    offset: (SAVE_BUFFER_SLOT_MARKER_START_OFFSET - SAVE_BUFFER_HEADER_OFFSET)
                        as i16,
                },
            ),
        ],
        "Diary save-slot marker producer",
    )
}

fn validate_instructions(
    mgame: &[u8],
    expected: &[(usize, Instruction)],
    role: &str,
) -> Result<()> {
    for (offset, expected_instruction) in expected {
        let pc = runtime_address(*offset)?;
        let instruction = decode(read_word(mgame, *offset)?, pc)
            .with_context(|| format!("failed to decode {role} at {pc:#010x}"))?;
        ensure!(
            instruction == *expected_instruction,
            "{role} grammar changed at {pc:#010x}"
        );
    }
    Ok(())
}

fn direct_call_offsets(mgame: &[u8], target: u32) -> Result<Vec<usize>> {
    let mut calls = Vec::new();
    for offset in (0..mgame.len().saturating_sub(3)).step_by(4) {
        let pc = runtime_address(offset)?;
        if matches!(decode(read_word(mgame, offset)?, pc), Ok(Instruction::Jal { target: decoded }) if decoded == target)
        {
            calls.push(offset);
        }
    }
    Ok(calls)
}

fn read_word(data: &[u8], offset: usize) -> Result<u32> {
    let bytes = data
        .get(offset..offset + 4)
        .with_context(|| format!("truncated MGAME instruction at +0x{offset:05x}"))?;
    Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
}

fn runtime_address(offset: usize) -> Result<u32> {
    MGAME_RUNTIME_BASE
        .checked_add(u32::try_from(offset)?)
        .context("Diary save-slot marker runtime address overflow")
}

fn hex_offset(offset: u16) -> String {
    format!("0x{offset:x}")
}
