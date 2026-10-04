use anyhow::{Result, ensure};
use psx_r3000a::{Instruction, Register};

use crate::name_input::NAME_NAVIGATION_MAP_BYTES;
use crate::pipeline::sha256_bytes;

use super::name_entry::{OVERLAY_RUNTIME_BASE, decode_instruction};
use super::name_entry_model::DialogueNameEntryInputAudit;

pub(super) const INPUT_ROUTINE_OFFSET: usize = 0x1c44;
pub(super) const SELECTION_ROUTINE_OFFSET: usize = 0x7194;
pub(super) const SELECTED_CODE_STORE_OFFSET: usize = 0x7228;
pub(super) const NAVIGATION_PRODUCER_OFFSET: usize = 0x8828;

const PAGE_CYCLE_INPUT_MASK: u16 = 0x0010;
const PHYSICAL_PAGE_COUNT: usize = 3;
const PAGE_INDEX_OBJECT_OFFSET: i16 = 9;
const RECORD_SLOT_INDEX_OBJECT_OFFSET: i16 = 10;
const CURSOR_CELL_OBJECT_OFFSET: i16 = 12;
const FIELD_INDEX_OBJECT_OFFSET: i16 = 15;
pub(super) const NAVIGATION_DISPATCH_TABLE_OFFSET: usize = 0x8de0;
const NAVIGATION_POSITION_COUNT: usize = 97;
const NAVIGATION_CANDIDATE_POSITION_COUNT: usize = 90;
const NAVIGATION_ACTION_POSITION_COUNT: usize = 7;
const NAVIGATION_DISPATCH_TABLE_SHA256: &str =
    "123c56e12eee3774f26ef3477ed2e8085c41ce4cd8b170236cd861c1491427eb";
const NAVIGATION_HANDLER_OFFSETS: [usize; 11] = [
    0x1e80, 0x1e8c, 0x1e98, 0x1ea4, 0x1eb0, 0x1ebc, 0x1ec8, 0x1ef8, 0x1f04, 0x1f14, 0x1f34,
];
const PRIMARY_NAVIGATION_POINTER_TABLE_OFFSET: usize = 0x0670;
const NICKNAME_NAVIGATION_POINTER_TABLE_OFFSET: usize = 0x0984;
const PRIMARY_NAVIGATION_POINTERS: [usize; 3] = [0x01e4, 0x0368, 0x04ec];
const NICKNAME_NAVIGATION_POINTERS: [usize; 3] = [0x01e4, 0x067c, 0x0800];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct NameNavigationSourceSpec {
    pub id: &'static str,
    pub source_page_index: usize,
    pub file_offset: usize,
    pub source_sha256: &'static str,
}

pub(super) const NAME_NAVIGATION_SOURCE_SPECS: [NameNavigationSourceSpec; 5] = [
    NameNavigationSourceSpec {
        id: "all-fields-hiragana",
        source_page_index: 0,
        file_offset: 0x01e4,
        source_sha256: "f0bba6bb4c0a68a5003979ead08d2bbe2eef50147404d10e18e3be6e5fdafe47",
    },
    NameNavigationSourceSpec {
        id: "family-given-katakana",
        source_page_index: 1,
        file_offset: 0x0368,
        source_sha256: "f0bba6bb4c0a68a5003979ead08d2bbe2eef50147404d10e18e3be6e5fdafe47",
    },
    NameNavigationSourceSpec {
        id: "family-given-alphanumeric",
        source_page_index: 2,
        file_offset: 0x04ec,
        source_sha256: "b5b00114f8ff3c2c6604673faa58483e96e92c1fcc02a40f7eb41442935995b9",
    },
    NameNavigationSourceSpec {
        id: "nickname-katakana",
        source_page_index: 1,
        file_offset: 0x067c,
        source_sha256: "b9c271216125af7ceae6aaac165d3640a6dd2a711b1fa0118061f0eab275aa83",
    },
    NameNavigationSourceSpec {
        id: "nickname-alphanumeric",
        source_page_index: 2,
        file_offset: 0x0800,
        source_sha256: "b9ec992088d60eea581ef8bb48b9353ff52cdabd0df7da0bfc80bf87d56be563",
    },
];

pub(super) fn audit_name_input_path(overlay: &[u8]) -> Result<DialogueNameEntryInputAudit> {
    validate_page_cycle(overlay)?;
    validate_selected_code_store(overlay)?;
    let navigation_handler_count = validate_navigation_dispatch(overlay)?;
    validate_navigation_producer(overlay)?;
    let navigation_maps = load_name_navigation_source_maps(overlay)?;

    Ok(DialogueNameEntryInputAudit {
        input_routine_file_offset: hex_offset(INPUT_ROUTINE_OFFSET),
        input_routine_runtime_address: runtime_address(INPUT_ROUTINE_OFFSET),
        page_cycle_input_mask: format!("0x{PAGE_CYCLE_INPUT_MASK:04x}"),
        physical_page_count: PHYSICAL_PAGE_COUNT,
        page_index_object_offset: usize::try_from(PAGE_INDEX_OBJECT_OFFSET)?,
        record_slot_index_object_offset: usize::try_from(RECORD_SLOT_INDEX_OBJECT_OFFSET)?,
        cursor_cell_object_offset: usize::try_from(CURSOR_CELL_OBJECT_OFFSET)?,
        field_index_object_offset: usize::try_from(FIELD_INDEX_OBJECT_OFFSET)?,
        nickname_skips_first_source_page: true,
        selection_routine_file_offset: hex_offset(SELECTION_ROUTINE_OFFSET),
        selection_routine_runtime_address: runtime_address(SELECTION_ROUTINE_OFFSET),
        selected_code_store_file_offset: hex_offset(SELECTED_CODE_STORE_OFFSET),
        selected_code_store_runtime_address: runtime_address(SELECTED_CODE_STORE_OFFSET),
        selected_code_width_bytes: 2,
        family_name_record_offset: 0x12,
        given_name_record_offset: 0x22,
        nickname_record_offset: 0x32,
        navigation_dispatch_table_file_offset: hex_offset(NAVIGATION_DISPATCH_TABLE_OFFSET),
        navigation_dispatch_table_runtime_address: runtime_address(
            NAVIGATION_DISPATCH_TABLE_OFFSET,
        ),
        navigation_dispatch_table_sha256: NAVIGATION_DISPATCH_TABLE_SHA256.to_string(),
        navigation_position_count: NAVIGATION_POSITION_COUNT,
        navigation_candidate_position_count: NAVIGATION_CANDIDATE_POSITION_COUNT,
        navigation_action_position_count: NAVIGATION_ACTION_POSITION_COUNT,
        navigation_handler_count,
        navigation_uses_computed_handler_dispatch: true,
        navigation_producer_file_offset: hex_offset(NAVIGATION_PRODUCER_OFFSET),
        navigation_producer_runtime_address: runtime_address(NAVIGATION_PRODUCER_OFFSET),
        navigation_source_map_count: navigation_maps.len(),
        navigation_source_map_sha256: navigation_maps
            .iter()
            .map(|(_, bytes)| sha256_bytes(bytes))
            .collect(),
    })
}

pub(super) fn load_name_navigation_source_maps(
    overlay: &[u8],
) -> Result<Vec<(NameNavigationSourceSpec, &[u8])>> {
    for (table_offset, expected_offsets) in [
        (
            PRIMARY_NAVIGATION_POINTER_TABLE_OFFSET,
            PRIMARY_NAVIGATION_POINTERS,
        ),
        (
            NICKNAME_NAVIGATION_POINTER_TABLE_OFFSET,
            NICKNAME_NAVIGATION_POINTERS,
        ),
    ] {
        for (index, expected_offset) in expected_offsets.into_iter().enumerate() {
            let offset = table_offset + index * 4;
            let bytes = overlay
                .get(offset..offset + 4)
                .ok_or_else(|| anyhow::anyhow!("name navigation pointer table is truncated"))?;
            let pointer = u32::from_le_bytes(bytes.try_into()?);
            ensure!(
                pointer == OVERLAY_RUNTIME_BASE + u32::try_from(expected_offset)?,
                "name navigation pointer changed at {offset:#x}"
            );
        }
    }

    NAME_NAVIGATION_SOURCE_SPECS
        .into_iter()
        .map(|spec| {
            let bytes = overlay
                .get(spec.file_offset..spec.file_offset + NAME_NAVIGATION_MAP_BYTES)
                .ok_or_else(|| anyhow::anyhow!("{} navigation map is truncated", spec.id))?;
            ensure!(
                sha256_bytes(bytes) == spec.source_sha256,
                "{} navigation map changed",
                spec.id
            );
            Ok((spec, bytes))
        })
        .collect()
}

fn validate_navigation_producer(overlay: &[u8]) -> Result<()> {
    require_instructions(
        overlay,
        &navigation_producer_instructions(),
        "name-entry navigation producer",
    )
}

fn navigation_producer_instructions() -> Vec<(usize, Instruction)> {
    vec![
        (
            NAVIGATION_PRODUCER_OFFSET,
            Instruction::Lhu {
                rt: Register::A2,
                base: Register::A0,
                offset: 6,
            },
        ),
        (
            NAVIGATION_PRODUCER_OFFSET + 0x14,
            Instruction::Lb {
                rt: Register::V0,
                base: Register::A0,
                offset: CURSOR_CELL_OBJECT_OFFSET,
            },
        ),
        (
            NAVIGATION_PRODUCER_OFFSET + 0x1c,
            Instruction::Sll {
                rd: Register::V1,
                rt: Register::V0,
                shift: 2,
            },
        ),
        (
            NAVIGATION_PRODUCER_OFFSET + 0x40,
            Instruction::Lbu {
                rt: Register::V0,
                base: Register::V0,
                offset: 0,
            },
        ),
        (
            NAVIGATION_PRODUCER_OFFSET + 0x48,
            Instruction::Sb {
                rt: Register::V0,
                base: Register::A0,
                offset: CURSOR_CELL_OBJECT_OFFSET,
            },
        ),
        (
            NAVIGATION_PRODUCER_OFFSET + 0x60,
            Instruction::Lb {
                rt: Register::V0,
                base: Register::A0,
                offset: CURSOR_CELL_OBJECT_OFFSET,
            },
        ),
        (
            NAVIGATION_PRODUCER_OFFSET + 0x90,
            Instruction::Lbu {
                rt: Register::V0,
                base: Register::V0,
                offset: 0,
            },
        ),
        (
            NAVIGATION_PRODUCER_OFFSET + 0x98,
            Instruction::Sb {
                rt: Register::V0,
                base: Register::A0,
                offset: CURSOR_CELL_OBJECT_OFFSET,
            },
        ),
    ]
}

fn validate_navigation_dispatch(overlay: &[u8]) -> Result<usize> {
    require_instructions(
        overlay,
        &navigation_dispatch_instructions(),
        "name-entry navigation computed dispatch",
    )?;
    let table_bytes = overlay
        .get(
            NAVIGATION_DISPATCH_TABLE_OFFSET
                ..NAVIGATION_DISPATCH_TABLE_OFFSET + NAVIGATION_POSITION_COUNT * 4,
        )
        .ok_or_else(|| anyhow::anyhow!("name-entry navigation dispatch table is truncated"))?;
    ensure!(
        sha256_bytes(table_bytes) == NAVIGATION_DISPATCH_TABLE_SHA256,
        "name-entry navigation dispatch table changed"
    );
    let allowed_handlers = NAVIGATION_HANDLER_OFFSETS
        .into_iter()
        .map(|offset| OVERLAY_RUNTIME_BASE + u32::try_from(offset).unwrap())
        .collect::<std::collections::BTreeSet<_>>();
    let handlers = table_bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|bytes| u32::from_le_bytes(*bytes))
        .collect::<std::collections::BTreeSet<_>>();
    ensure!(
        handlers.is_subset(&allowed_handlers) && handlers.len() == allowed_handlers.len(),
        "name-entry navigation dispatch targets changed"
    );
    Ok(handlers.len())
}

fn navigation_dispatch_instructions() -> Vec<(usize, Instruction)> {
    vec![
        (
            0x1e54,
            Instruction::Lb {
                rt: Register::V1,
                base: Register::S0,
                offset: 13,
            },
        ),
        (
            0x1e5c,
            Instruction::Sltiu {
                rt: Register::V0,
                rs: Register::V1,
                immediate: NAVIGATION_POSITION_COUNT as i16,
            },
        ),
        (
            0x1e60,
            Instruction::Beq {
                rs: Register::V0,
                rt: Register::ZERO,
                target: OVERLAY_RUNTIME_BASE + 0x1f38,
            },
        ),
        (
            0x1e64,
            Instruction::Sll {
                rd: Register::V0,
                rt: Register::V1,
                shift: 2,
            },
        ),
        (
            0x1e68,
            Instruction::Lui {
                rt: Register::AT,
                immediate: 0x8018,
            },
        ),
        (
            0x1e6c,
            Instruction::Addu {
                rd: Register::AT,
                rs: Register::AT,
                rt: Register::V0,
            },
        ),
        (
            0x1e70,
            Instruction::Lw {
                rt: Register::V0,
                base: Register::AT,
                offset: 0x2de0,
            },
        ),
        (0x1e78, Instruction::Jr { rs: Register::V0 }),
    ]
}

fn validate_page_cycle(overlay: &[u8]) -> Result<()> {
    require_instructions(overlay, &page_cycle_instructions(), "name-entry page cycle")
}

fn page_cycle_instructions() -> Vec<(usize, Instruction)> {
    vec![
        (
            0x1ca0,
            Instruction::Lhu {
                rt: Register::V1,
                base: Register::S0,
                offset: 6,
            },
        ),
        (
            0x1ca8,
            Instruction::Andi {
                rt: Register::V0,
                rs: Register::V1,
                immediate: PAGE_CYCLE_INPUT_MASK,
            },
        ),
        (
            0x1cb4,
            Instruction::Lbu {
                rt: Register::V1,
                base: Register::S0,
                offset: PAGE_INDEX_OBJECT_OFFSET,
            },
        ),
        (
            0x1cb8,
            Instruction::Lui {
                rt: Register::V0,
                immediate: 0xaaaa,
            },
        ),
        (
            0x1cbc,
            Instruction::Ori {
                rt: Register::V0,
                rs: Register::V0,
                immediate: 0xaaab,
            },
        ),
        (
            0x1cc0,
            Instruction::Addiu {
                rt: Register::V1,
                rs: Register::V1,
                immediate: 1,
            },
        ),
        (
            0x1cc4,
            Instruction::Andi {
                rt: Register::V1,
                rs: Register::V1,
                immediate: 0x00ff,
            },
        ),
        (
            0x1cc8,
            Instruction::Multu {
                rs: Register::V1,
                rt: Register::V0,
            },
        ),
        (0x1ccc, Instruction::Mfhi { rd: Register::A2 }),
        (
            0x1cd0,
            Instruction::Srl {
                rd: Register::A0,
                rt: Register::A2,
                shift: 1,
            },
        ),
        (
            0x1cd4,
            Instruction::Sll {
                rd: Register::V0,
                rt: Register::A0,
                shift: 1,
            },
        ),
        (
            0x1cd8,
            Instruction::Addu {
                rd: Register::V0,
                rs: Register::V0,
                rt: Register::A0,
            },
        ),
        (
            0x1cdc,
            Instruction::Subu {
                rd: Register::A0,
                rs: Register::V1,
                rt: Register::V0,
            },
        ),
        (
            0x1ce0,
            Instruction::Lbu {
                rt: Register::V1,
                base: Register::S0,
                offset: FIELD_INDEX_OBJECT_OFFSET,
            },
        ),
        (
            0x1ce4,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::ZERO,
                immediate: 2,
            },
        ),
        (
            0x1ce8,
            Instruction::Bne {
                rs: Register::V1,
                rt: Register::V0,
                target: OVERLAY_RUNTIME_BASE + 0x1d00,
            },
        ),
        (
            0x1cec,
            Instruction::Sb {
                rt: Register::A0,
                base: Register::S0,
                offset: PAGE_INDEX_OBJECT_OFFSET,
            },
        ),
        (
            0x1cf0,
            Instruction::Andi {
                rt: Register::V0,
                rs: Register::A0,
                immediate: 0x00ff,
            },
        ),
        (
            0x1cf4,
            Instruction::Bne {
                rs: Register::V0,
                rt: Register::ZERO,
                target: OVERLAY_RUNTIME_BASE + 0x1d00,
            },
        ),
        (
            0x1cf8,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::A0,
                immediate: 1,
            },
        ),
        (
            0x1cfc,
            Instruction::Sb {
                rt: Register::V0,
                base: Register::S0,
                offset: PAGE_INDEX_OBJECT_OFFSET,
            },
        ),
    ]
}

fn validate_selected_code_store(overlay: &[u8]) -> Result<()> {
    require_instructions(
        overlay,
        &selected_code_store_instructions(),
        "name-entry selected-code store",
    )
}

fn selected_code_store_instructions() -> Vec<(usize, Instruction)> {
    vec![
        (
            0x71a8,
            Instruction::Lbu {
                rt: Register::V1,
                base: Register::S0,
                offset: FIELD_INDEX_OBJECT_OFFSET,
            },
        ),
        (
            0x71e8,
            Instruction::Addiu {
                rt: Register::A1,
                rs: Register::S0,
                immediate: 0x12,
            },
        ),
        (
            0x71f0,
            Instruction::Addiu {
                rt: Register::A1,
                rs: Register::S0,
                immediate: 0x22,
            },
        ),
        (
            0x71f4,
            Instruction::Addiu {
                rt: Register::A1,
                rs: Register::S0,
                immediate: 0x32,
            },
        ),
        (
            0x71f8,
            Instruction::Lbu {
                rt: Register::V0,
                base: Register::S0,
                offset: PAGE_INDEX_OBJECT_OFFSET,
            },
        ),
        (
            0x71fc,
            Instruction::Lb {
                rt: Register::V1,
                base: Register::S0,
                offset: CURSOR_CELL_OBJECT_OFFSET,
            },
        ),
        (
            0x7200,
            Instruction::Sll {
                rd: Register::V0,
                rt: Register::V0,
                shift: 2,
            },
        ),
        (
            0x7204,
            Instruction::Sll {
                rd: Register::V1,
                rt: Register::V1,
                shift: 1,
            },
        ),
        (
            0x7208,
            Instruction::Lui {
                rt: Register::AT,
                immediate: 0x8018,
            },
        ),
        (
            0x720c,
            Instruction::Addu {
                rd: Register::AT,
                rs: Register::AT,
                rt: Register::V0,
            },
        ),
        (
            0x7210,
            Instruction::Lw {
                rt: Register::A0,
                base: Register::AT,
                offset: -21240,
            },
        ),
        (
            0x7214,
            Instruction::Lbu {
                rt: Register::V0,
                base: Register::S0,
                offset: RECORD_SLOT_INDEX_OBJECT_OFFSET,
            },
        ),
        (
            0x7218,
            Instruction::Addu {
                rd: Register::A0,
                rs: Register::A0,
                rt: Register::V1,
            },
        ),
        (
            0x721c,
            Instruction::Sll {
                rd: Register::V0,
                rt: Register::V0,
                shift: 1,
            },
        ),
        (
            0x7220,
            Instruction::Lhu {
                rt: Register::V1,
                base: Register::A0,
                offset: 0,
            },
        ),
        (
            0x7224,
            Instruction::Addu {
                rd: Register::V0,
                rs: Register::V0,
                rt: Register::A1,
            },
        ),
        (
            SELECTED_CODE_STORE_OFFSET,
            Instruction::Sh {
                rt: Register::V1,
                base: Register::V0,
                offset: 0,
            },
        ),
        (
            0x722c,
            Instruction::Lbu {
                rt: Register::V1,
                base: Register::S0,
                offset: FIELD_INDEX_OBJECT_OFFSET,
            },
        ),
        (0x7230, Instruction::nop()),
    ]
}

#[cfg(test)]
pub(super) fn install_name_input_path_fixture(overlay: &mut [u8]) {
    use psx_r3000a::encode;

    for (offset, instruction) in page_cycle_instructions()
        .into_iter()
        .chain(selected_code_store_instructions())
        .chain(navigation_dispatch_instructions())
        .chain(navigation_producer_instructions())
    {
        let pc = OVERLAY_RUNTIME_BASE + offset as u32;
        overlay[offset..offset + 4]
            .copy_from_slice(&encode(&instruction, pc).unwrap().to_le_bytes());
    }
    for position in 0..NAVIGATION_POSITION_COUNT {
        let handler_offset = match position {
            0..=8 => 0x1ea4,
            9 => 0x1eb0,
            19 | 29 | 39 | 49 | 59 | 79 => 0x1ef8,
            60..=68 => 0x1f04,
            69 => 0x1ec8,
            80..=88 => 0x1f14,
            89 => 0x1ebc,
            90 => 0x1e80,
            91..=95 => 0x1e8c,
            96 => 0x1e98,
            _ => 0x1f34,
        };
        let offset = NAVIGATION_DISPATCH_TABLE_OFFSET + position * 4;
        overlay[offset..offset + 4]
            .copy_from_slice(&(OVERLAY_RUNTIME_BASE + handler_offset).to_le_bytes());
    }

    for (table_offset, map_offsets) in [
        (
            PRIMARY_NAVIGATION_POINTER_TABLE_OFFSET,
            PRIMARY_NAVIGATION_POINTERS,
        ),
        (
            NICKNAME_NAVIGATION_POINTER_TABLE_OFFSET,
            NICKNAME_NAVIGATION_POINTERS,
        ),
    ] {
        for (index, map_offset) in map_offsets.into_iter().enumerate() {
            let pointer = OVERLAY_RUNTIME_BASE + u32::try_from(map_offset).unwrap();
            let offset = table_offset + index * 4;
            overlay[offset..offset + 4].copy_from_slice(&pointer.to_le_bytes());
        }
    }

    for (offset, map) in [
        (0x01e4, source_navigation_fixture(9, false)),
        (0x0368, source_navigation_fixture(9, false)),
        (0x04ec, source_navigation_fixture(7, false)),
        (0x067c, source_navigation_fixture(9, true)),
        (0x0800, source_navigation_fixture(7, true)),
    ] {
        overlay[offset..offset + NAME_NAVIGATION_MAP_BYTES].copy_from_slice(&map);
    }
}

#[cfg(test)]
fn source_navigation_fixture(visible_row_count: usize, nickname: bool) -> Vec<u8> {
    const COLUMN_COUNT: usize = 10;
    const FULL_ROW_COUNT: usize = 9;
    const FIRST_ACTION: u8 = NAVIGATION_CANDIDATE_POSITION_COUNT as u8;
    const LEFT_ACTIONS: [u8; FULL_ROW_COUNT] = [90, 91, 92, 92, 93, 93, 94, 95, 96];
    const RIGHT_ACTIONS: [u8; FULL_ROW_COUNT] = [90, 91, 92, 30, 93, 93, 94, 95, 96];

    let mut map = vec![0_u8; NAME_NAVIGATION_MAP_BYTES];
    for position in 0..NAVIGATION_CANDIDATE_POSITION_COUNT {
        let row = position / COLUMN_COUNT;
        let column = position % COLUMN_COUNT;
        map[position * 4] =
            u8::try_from(((row + FULL_ROW_COUNT - 1) % FULL_ROW_COUNT) * COLUMN_COUNT + column)
                .unwrap();
        map[position * 4 + 1] =
            u8::try_from(((row + 1) % FULL_ROW_COUNT) * COLUMN_COUNT + column).unwrap();
        map[position * 4 + 2] = if column == 0 {
            LEFT_ACTIONS[row]
        } else {
            u8::try_from(position - 1).unwrap()
        };
        map[position * 4 + 3] = if column + 1 == COLUMN_COUNT {
            RIGHT_ACTIONS[row]
        } else {
            u8::try_from(position + 1).unwrap()
        };
    }

    if visible_row_count == 7 {
        for column in 0..COLUMN_COUNT {
            map[column * 4] = u8::try_from(60 + column).unwrap();
            map[(60 + column) * 4 + 1] = u8::try_from(column).unwrap();
        }
    }

    let action_up = [96, 90, 91, 92, 93, 94, 95];
    let action_down = [91, 92, 93, 94, 95, 96, 90];
    let mut action_left = [9, 19, 29, 59, 69, 79, 89];
    let mut action_right = [0, 10, 20, 50, 60, 70, 80];
    if visible_row_count == 7 {
        action_left[5] = 69;
        action_left[6] = 69;
        action_right[5] = 60;
        action_right[6] = 60;
    }
    for action_index in 0..7 {
        let position = NAVIGATION_CANDIDATE_POSITION_COUNT + action_index;
        map[position * 4] = action_up[action_index];
        map[position * 4 + 1] = action_down[action_index];
        map[position * 4 + 2] = action_left[action_index];
        map[position * 4 + 3] = action_right[action_index];
    }

    if nickname {
        map[2] = FIRST_ACTION + 1;
        map[9 * 4 + 3] = FIRST_ACTION + 1;
        map[91 * 4] = 96;
        map[96 * 4 + 1] = 91;
    }
    map
}

fn require_instructions(
    overlay: &[u8],
    expected: &[(usize, Instruction)],
    role: &str,
) -> Result<()> {
    for (offset, expected_instruction) in expected {
        let actual = decode_instruction(overlay, *offset)?;
        ensure!(
            actual == *expected_instruction,
            "{role} changed at {offset:#x}: expected {expected_instruction:?}, got {actual:?}"
        );
    }
    Ok(())
}

fn runtime_address(offset: usize) -> String {
    format!("0x{:08x}", OVERLAY_RUNTIME_BASE + offset as u32)
}

fn hex_offset(offset: usize) -> String {
    format!("0x{offset:04x}")
}
