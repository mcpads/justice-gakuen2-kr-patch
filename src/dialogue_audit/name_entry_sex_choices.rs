use anyhow::{Result, ensure};
use psx_r3000a::{Instruction, Register};

use crate::tim::Cell;

use super::name_entry::{OVERLAY_RUNTIME_BASE, decode_instruction};
use super::name_entry_fixed_graphics_model::NameEntryFixedGraphicSurfaceSpec;
use super::name_entry_model::DialogueNameEntrySexChoiceSurfaceAudit;

const PRODUCER_OFFSET: usize = 0x2edc;
const TEXTURE_PAGE: u8 = 0x0c;
const TEXTURE_PAGE_WIDTH: usize = 256;
const CHOICE_COUNT: usize = 2;
const FIRST_U: usize = 180;
const V: usize = 140;
const CELL_SIZE: usize = 20;

const PRODUCER_INSTRUCTIONS: [(usize, Instruction); 13] = [
    (
        0x2f30,
        Instruction::Addiu {
            rt: Register::S6,
            rs: Register::ZERO,
            immediate: 2,
        },
    ),
    (
        0x2f44,
        Instruction::Addiu {
            rt: Register::FP,
            rs: Register::ZERO,
            immediate: CELL_SIZE as i16,
        },
    ),
    (
        0x2f4c,
        Instruction::Addiu {
            rt: Register::S3,
            rs: Register::ZERO,
            immediate: FIRST_U as i16,
        },
    ),
    (
        0x2ff8,
        Instruction::Addiu {
            rt: Register::T0,
            rs: Register::ZERO,
            immediate: V as i16,
        },
    ),
    (
        0x2ffc,
        Instruction::Sh {
            rt: Register::V0,
            base: Register::S0,
            offset: 22,
        },
    ),
    (
        0x300c,
        Instruction::Sb {
            rt: Register::S3,
            base: Register::S0,
            offset: 20,
        },
    ),
    (
        0x3010,
        Instruction::Sb {
            rt: Register::T0,
            base: Register::S0,
            offset: 21,
        },
    ),
    (
        0x3014,
        Instruction::Sh {
            rt: Register::FP,
            base: Register::S0,
            offset: 24,
        },
    ),
    (
        0x3018,
        Instruction::Sh {
            rt: Register::FP,
            base: Register::S0,
            offset: 26,
        },
    ),
    (
        0x3030,
        Instruction::Addiu {
            rt: Register::S4,
            rs: Register::S4,
            immediate: 50,
        },
    ),
    (
        0x3040,
        Instruction::Addiu {
            rt: Register::S3,
            rs: Register::S3,
            immediate: CELL_SIZE as i16,
        },
    ),
    (
        0x3068,
        Instruction::Slt {
            rd: Register::V0,
            rs: Register::S2,
            rt: Register::S6,
        },
    ),
    (
        0x306c,
        Instruction::Bne {
            rs: Register::V0,
            rt: Register::ZERO,
            target: OVERLAY_RUNTIME_BASE + 0x2f54,
        },
    ),
];

pub(super) fn audit_sex_choice_surface(
    overlay: &[u8],
) -> Result<DialogueNameEntrySexChoiceSurfaceAudit> {
    for (offset, expected) in PRODUCER_INSTRUCTIONS {
        ensure!(
            decode_instruction(overlay, offset)? == expected,
            "sex-choice producer changed at {offset:#x}"
        );
    }

    Ok(DialogueNameEntrySexChoiceSurfaceAudit {
        id: "sex-choices".to_string(),
        producer_file_offset: format!("0x{PRODUCER_OFFSET:04x}"),
        producer_runtime_address: format!(
            "0x{:08x}",
            OVERLAY_RUNTIME_BASE + PRODUCER_OFFSET as u32
        ),
        logical_choice_count: CHOICE_COUNT,
        texture_page: TEXTURE_PAGE,
        source_cells: source_cells(),
        producer_instruction_file_offsets: PRODUCER_INSTRUCTIONS
            .iter()
            .map(|(offset, _)| format!("0x{offset:04x}"))
            .collect(),
        all_producer_instructions_match: true,
    })
}

pub(super) fn sex_choices_spec() -> NameEntryFixedGraphicSurfaceSpec {
    NameEntryFixedGraphicSurfaceSpec {
        id: "sex-choices",
        translation_file: "sex-choices.json",
        translation_kind: "Justice Gakuen 2 source-bound enrollment sex-choice graphic translation",
        build_kind: "Justice Gakuen 2 enrollment sex-choice fixed-graphic build",
        target: "enrollment sex-choice Korean text",
        tim_offset: 0x19_000,
        image_x: 768,
        image_y: 0,
        image_width: 768,
        image_height: 256,
        clut_width: 352,
        clut_height: 1,
    }
}

fn source_cells() -> Vec<Cell> {
    (0..CHOICE_COUNT)
        .map(|index| Cell {
            x: usize::from(TEXTURE_PAGE - 0x0c) * TEXTURE_PAGE_WIDTH + FIRST_U + index * CELL_SIZE,
            y: V,
            width: CELL_SIZE,
            height: CELL_SIZE,
        })
        .collect()
}
