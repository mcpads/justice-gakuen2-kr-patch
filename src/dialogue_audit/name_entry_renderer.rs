use anyhow::{Result, ensure};
use psx_r3000a::{Instruction, Register};

use super::name_entry::{OVERLAY_RUNTIME_BASE, decode_instruction};
use super::name_entry_model::{
    DialogueNameEntryRendererAudit, DialogueNameEntryRendererFieldAudit,
};

pub(super) const REDISPLAY_ROUTINE_OFFSET: usize = 0x7454;
pub(super) const CODE_LOOKUP_TABLE_OFFSET: usize = 0x0d14;
pub(super) const LAYOUT_TRIPLET_TABLE_OFFSET: usize = 0x0ee4;
pub(super) const ATLAS_INITIALIZER_OFFSET: usize = 0x8b14;

const NAME_FONT_SOURCE_BUFFER: u32 = 0x800e_9000;
const SPRITE_CELL_WIDTH: usize = 20;
const SPRITE_CELL_HEIGHT: usize = 20;

const FIELD_SPECS: [(&str, usize, usize, u32, usize, usize); 3] = [
    ("family_name", 0x12, 6, 0x801c_8118, 160, 100),
    ("given_name", 0x22, 6, 0x801c_82d8, 320, 100),
    ("nickname", 0x32, 4, 0x801c_8498, 160, 140),
];

pub(super) fn audit_name_renderer(overlay: &[u8]) -> Result<DialogueNameEntryRendererAudit> {
    require_instructions(
        overlay,
        &redisplay_instructions(),
        "name-entry redisplay consumer",
    )?;
    require_instructions(
        overlay,
        &atlas_initializer_instructions(),
        "name-entry atlas initializer",
    )?;

    let fields = FIELD_SPECS
        .into_iter()
        .map(
            |(name, record_offset, visible_slot_count, sprite_buffer, x, y)| {
                DialogueNameEntryRendererFieldAudit {
                    name: name.to_string(),
                    record_offset,
                    visible_slot_count,
                    sprite_buffer_runtime_address: hex_address(sprite_buffer),
                    origin_x: x,
                    origin_y: y,
                }
            },
        )
        .collect();

    Ok(DialogueNameEntryRendererAudit {
        redisplay_routine_file_offset: hex_offset(REDISPLAY_ROUTINE_OFFSET),
        redisplay_routine_runtime_address: runtime_address(REDISPLAY_ROUTINE_OFFSET),
        code_lookup_table_file_offset: hex_offset(CODE_LOOKUP_TABLE_OFFSET),
        code_lookup_table_runtime_address: runtime_address(CODE_LOOKUP_TABLE_OFFSET),
        layout_triplet_table_file_offset: hex_offset(LAYOUT_TRIPLET_TABLE_OFFSET),
        layout_triplet_table_runtime_address: runtime_address(LAYOUT_TRIPLET_TABLE_OFFSET),
        layout_triplet_size_bytes: 3,
        sprite_cell_width: SPRITE_CELL_WIDTH,
        sprite_cell_height: SPRITE_CELL_HEIGHT,
        fields,
        atlas_initializer_file_offset: hex_offset(ATLAS_INITIALIZER_OFFSET),
        atlas_initializer_runtime_address: runtime_address(ATLAS_INITIALIZER_OFFSET),
        uploaded_source_buffers: [0x800d_0000, NAME_FONT_SOURCE_BUFFER, 0x8010_1800]
            .into_iter()
            .map(hex_address)
            .collect(),
        name_font_source_buffer_runtime_address: hex_address(NAME_FONT_SOURCE_BUFFER),
        tagged_hangul_consumer_installed: false,
    })
}

fn redisplay_instructions() -> Vec<(usize, Instruction)> {
    vec![
        (
            REDISPLAY_ROUTINE_OFFSET,
            Instruction::Addiu {
                rt: Register::SP,
                rs: Register::SP,
                immediate: -64,
            },
        ),
        (
            0x74b8,
            Instruction::Lui {
                rt: Register::S5,
                immediate: 0x801c,
            },
        ),
        (
            0x74bc,
            Instruction::Ori {
                rt: Register::S5,
                rs: Register::S5,
                immediate: 0x8118,
            },
        ),
        (
            0x74c0,
            Instruction::Addiu {
                rt: Register::S4,
                rs: Register::A0,
                immediate: 0x12,
            },
        ),
        (
            0x74c4,
            Instruction::Addiu {
                rt: Register::S7,
                rs: Register::ZERO,
                immediate: 6,
            },
        ),
        (
            0x74d4,
            Instruction::Lui {
                rt: Register::S5,
                immediate: 0x801c,
            },
        ),
        (
            0x74d8,
            Instruction::Ori {
                rt: Register::S5,
                rs: Register::S5,
                immediate: 0x82d8,
            },
        ),
        (
            0x74dc,
            Instruction::Addiu {
                rt: Register::S4,
                rs: Register::A0,
                immediate: 0x22,
            },
        ),
        (
            0x74e0,
            Instruction::Addiu {
                rt: Register::S7,
                rs: Register::ZERO,
                immediate: 6,
            },
        ),
        (
            0x74f0,
            Instruction::Lui {
                rt: Register::S5,
                immediate: 0x801c,
            },
        ),
        (
            0x74f4,
            Instruction::Ori {
                rt: Register::S5,
                rs: Register::S5,
                immediate: 0x8498,
            },
        ),
        (
            0x74f8,
            Instruction::Addiu {
                rt: Register::S4,
                rs: Register::A0,
                immediate: 0x32,
            },
        ),
        (
            0x74fc,
            Instruction::Addiu {
                rt: Register::S7,
                rs: Register::ZERO,
                immediate: 4,
            },
        ),
        (
            0x7514,
            Instruction::Lhu {
                rt: Register::A3,
                base: Register::S4,
                offset: 0,
            },
        ),
        (
            0x7518,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::ZERO,
                immediate: 0x3001,
            },
        ),
        (
            0x7524,
            Instruction::Lui {
                rt: Register::A1,
                immediate: 0x8018,
            },
        ),
        (
            0x7528,
            Instruction::Addiu {
                rt: Register::A1,
                rs: Register::A1,
                immediate: -21228,
            },
        ),
        (
            0x7534,
            Instruction::Lhu {
                rt: Register::A0,
                base: Register::A1,
                offset: 0,
            },
        ),
        (
            0x7568,
            Instruction::Addiu {
                rt: Register::A1,
                rs: Register::A1,
                immediate: 2,
            },
        ),
        (
            0x7588,
            Instruction::Lui {
                rt: Register::V0,
                immediate: 0x8018,
            },
        ),
        (
            0x758c,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::V0,
                immediate: -20764,
            },
        ),
        (
            0x7634,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::ZERO,
                immediate: SPRITE_CELL_WIDTH as i16,
            },
        ),
        (
            0x7638,
            Instruction::Sh {
                rt: Register::V0,
                base: Register::S1,
                offset: 16,
            },
        ),
        (
            0x763c,
            Instruction::Sh {
                rt: Register::V0,
                base: Register::S1,
                offset: 18,
            },
        ),
        (
            0x76bc,
            Instruction::Addiu {
                rt: Register::S4,
                rs: Register::S4,
                immediate: 2,
            },
        ),
    ]
}

fn atlas_initializer_instructions() -> Vec<(usize, Instruction)> {
    vec![
        (
            ATLAS_INITIALIZER_OFFSET,
            Instruction::Lui {
                rt: Register::V0,
                immediate: 0x801f,
            },
        ),
        (
            0x8b90,
            Instruction::Lui {
                rt: Register::A0,
                immediate: 0x800d,
            },
        ),
        (
            0x8b9c,
            Instruction::Jalr {
                rd: Register::RA,
                rs: Register::V0,
            },
        ),
        (
            0x8ba0,
            Instruction::Addiu {
                rt: Register::A1,
                rs: Register::ZERO,
                immediate: 0x02ce,
            },
        ),
        (
            0x8bb8,
            Instruction::Jalr {
                rd: Register::RA,
                rs: Register::V0,
            },
        ),
        (
            0x8bbc,
            Instruction::Lui {
                rt: Register::A0,
                immediate: 0x800d,
            },
        ),
        (
            0x8bc8,
            Instruction::Lui {
                rt: Register::A0,
                immediate: 0x800e,
            },
        ),
        (
            0x8bd4,
            Instruction::Jalr {
                rd: Register::RA,
                rs: Register::V0,
            },
        ),
        (
            0x8bd8,
            Instruction::Ori {
                rt: Register::A0,
                rs: Register::A0,
                immediate: 0x9000,
            },
        ),
        (
            0x8be4,
            Instruction::Lui {
                rt: Register::A0,
                immediate: 0x8010,
            },
        ),
        (
            0x8bf0,
            Instruction::Jalr {
                rd: Register::RA,
                rs: Register::V0,
            },
        ),
        (
            0x8bf4,
            Instruction::Ori {
                rt: Register::A0,
                rs: Register::A0,
                immediate: 0x1800,
            },
        ),
    ]
}

#[cfg(test)]
pub(super) fn install_name_renderer_fixture(overlay: &mut [u8]) {
    use psx_r3000a::encode;

    for (offset, instruction) in redisplay_instructions()
        .into_iter()
        .chain(atlas_initializer_instructions())
    {
        let pc = OVERLAY_RUNTIME_BASE + offset as u32;
        overlay[offset..offset + 4]
            .copy_from_slice(&encode(&instruction, pc).unwrap().to_le_bytes());
    }
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
    hex_address(OVERLAY_RUNTIME_BASE + offset as u32)
}

fn hex_address(address: u32) -> String {
    format!("0x{address:08x}")
}

fn hex_offset(offset: usize) -> String {
    format!("0x{offset:04x}")
}
