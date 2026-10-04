use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};
use psx_r3000a::{Instruction, Register, decode};

use super::model::StaticDeclaredIndirectJumpAudit;
use crate::source_disc::LoadedImage;

const KANRI_PATH: &str = "DAT1/KANRI.BIN";
const KANRI_RUNTIME_BASE: u32 = 0x800a_2000;
const MENU_LABEL_JUMP_OFFSET: usize = 0x23f4;
const MENU_LABEL_JUMP_TABLE_OFFSET: usize = 0x8288;
const MENU_LABEL_JUMP_TARGETS: [u32; 5] = [
    0x800a_444c,
    0x800a_444c,
    0x800a_43fc,
    0x800a_4440,
    0x800a_444c,
];

pub(super) struct DeclaredControlTransfers {
    pub(super) targets: BTreeMap<usize, BTreeSet<u32>>,
    pub(super) audits: Vec<StaticDeclaredIndirectJumpAudit>,
}

pub(super) fn declared_control_transfers(image: &LoadedImage) -> Result<DeclaredControlTransfers> {
    if image.path != KANRI_PATH {
        return Ok(DeclaredControlTransfers {
            targets: BTreeMap::new(),
            audits: Vec::new(),
        });
    }
    ensure!(
        image.runtime_base == Some(KANRI_RUNTIME_BASE),
        "KANRI declared control-transfer profile moved from its source-bound runtime base"
    );
    validate_menu_label_jump_table(image)?;

    let distinct_targets = MENU_LABEL_JUMP_TARGETS.into_iter().collect::<BTreeSet<_>>();
    Ok(DeclaredControlTransfers {
        targets: [(
            MENU_LABEL_JUMP_OFFSET,
            distinct_targets.clone(),
        )]
        .into_iter()
        .collect(),
        audits: vec![StaticDeclaredIndirectJumpAudit {
            id: "kanri_menu_label_loop_dispatch".to_string(),
            transfer_instruction_offset: hex_offset(MENU_LABEL_JUMP_OFFSET),
            transfer_runtime_address: hex_address(
                KANRI_RUNTIME_BASE + MENU_LABEL_JUMP_OFFSET as u32,
            ),
            table_offset: hex_offset(MENU_LABEL_JUMP_TABLE_OFFSET),
            table_runtime_address: hex_address(
                KANRI_RUNTIME_BASE + MENU_LABEL_JUMP_TABLE_OFFSET as u32,
            ),
            table_entry_count: MENU_LABEL_JUMP_TARGETS.len(),
            distinct_target_count: distinct_targets.len(),
            targets: MENU_LABEL_JUMP_TARGETS
                .into_iter()
                .map(hex_address)
                .collect(),
            evidence: "source-bound five-iteration loop keeps S0 as the bound and S6 as the four-byte table cursor; the exact five-entry table is validated instead of inferring a generic selector pattern".to_string(),
        }],
    })
}

fn validate_menu_label_jump_table(image: &LoadedImage) -> Result<()> {
    let instructions = [
        (
            0x23a0,
            Instruction::Addu {
                rd: Register::S0,
                rs: Register::ZERO,
                rt: Register::ZERO,
            },
        ),
        (
            0x23c8,
            Instruction::Addu {
                rd: Register::S6,
                rs: Register::ZERO,
                rt: Register::ZERO,
            },
        ),
        (
            0x23d8,
            Instruction::Sltiu {
                rt: Register::V0,
                rs: Register::S0,
                immediate: 5,
            },
        ),
        (
            0x23dc,
            Instruction::Beq {
                rs: Register::V0,
                rt: Register::ZERO,
                target: KANRI_RUNTIME_BASE + 0x24bc,
            },
        ),
        (
            0x23e4,
            Instruction::Lui {
                rt: Register::AT,
                immediate: 0x800b,
            },
        ),
        (
            0x23e8,
            Instruction::Addu {
                rd: Register::AT,
                rs: Register::AT,
                rt: Register::S6,
            },
        ),
        (
            0x23ec,
            Instruction::Lw {
                rt: Register::V0,
                base: Register::AT,
                offset: -0x5d78,
            },
        ),
        (MENU_LABEL_JUMP_OFFSET, Instruction::Jr { rs: Register::V0 }),
        (0x23f8, Instruction::nop()),
        (
            0x24c8,
            Instruction::Addiu {
                rt: Register::S6,
                rs: Register::S6,
                immediate: 4,
            },
        ),
        (
            0x24cc,
            Instruction::Addiu {
                rt: Register::S0,
                rs: Register::S0,
                immediate: 1,
            },
        ),
        (
            0x24d0,
            Instruction::Slti {
                rt: Register::V0,
                rs: Register::S0,
                immediate: 5,
            },
        ),
        (
            0x24d4,
            Instruction::Bne {
                rs: Register::V0,
                rt: Register::ZERO,
                target: KANRI_RUNTIME_BASE + 0x23d8,
            },
        ),
    ];
    for (offset, expected) in instructions {
        let actual = decode_instruction(image, offset)?;
        ensure!(
            actual == expected,
            "KANRI menu-label jump-table grammar changed at +0x{offset:04x}"
        );
    }

    let table_end = MENU_LABEL_JUMP_TABLE_OFFSET + MENU_LABEL_JUMP_TARGETS.len() * 4;
    let table = image
        .data
        .get(MENU_LABEL_JUMP_TABLE_OFFSET..table_end)
        .context("KANRI menu-label jump table is truncated")?;
    let actual_targets = table
        .as_chunks::<4>()
        .0
        .iter()
        .map(|bytes| u32::from_le_bytes(*bytes))
        .collect::<Vec<_>>();
    ensure!(
        actual_targets == MENU_LABEL_JUMP_TARGETS,
        "KANRI menu-label jump-table targets changed"
    );
    Ok(())
}

fn decode_instruction(image: &LoadedImage, offset: usize) -> Result<Instruction> {
    let bytes: [u8; 4] = image
        .data
        .get(offset..offset + 4)
        .with_context(|| format!("KANRI instruction +0x{offset:04x} is truncated"))?
        .try_into()?;
    decode(
        u32::from_le_bytes(bytes),
        KANRI_RUNTIME_BASE + offset as u32,
    )
    .with_context(|| format!("failed to decode KANRI instruction +0x{offset:04x}"))
}

fn hex_address(value: u32) -> String {
    format!("0x{value:08x}")
}

fn hex_offset(value: usize) -> String {
    format!("0x{value:08x}")
}
