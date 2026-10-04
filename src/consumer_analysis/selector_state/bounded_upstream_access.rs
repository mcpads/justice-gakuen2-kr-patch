use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};
use psx_r3000a::{Instruction, Register};

use crate::source_disc::LoadedImage;

use super::profile_validation::{
    ensure_direct_call_offsets, ensure_instruction, ensure_no_other_reachable_direct_entry,
    ensure_only_reachable_direct_entries, ensure_register_is_preserved_between, runtime_address,
};

#[derive(Clone, Copy)]
struct ModeStateClearLoopProfile {
    pointer_lui_offset: usize,
    pointer_low_offset: usize,
    counter_setup_offset: usize,
    pointer_adjust_offset: usize,
    store_offset: usize,
    pointer_base: Register,
}

const PLSEL3_MODE_STATE_CLEAR: ModeStateClearLoopProfile = ModeStateClearLoopProfile {
    pointer_lui_offset: 0x3b5c,
    pointer_low_offset: 0x3b60,
    counter_setup_offset: 0x3b6c,
    pointer_adjust_offset: 0x3b70,
    store_offset: 0x3b74,
    pointer_base: Register::A0,
};

const PLSEL4_MODE_STATE_CLEAR: ModeStateClearLoopProfile = ModeStateClearLoopProfile {
    pointer_lui_offset: 0x3c44,
    pointer_low_offset: 0x3c48,
    counter_setup_offset: 0x3c54,
    pointer_adjust_offset: 0x3c58,
    store_offset: 0x3c5c,
    pointer_base: Register::A1,
};

pub(super) fn validated_bounded_upstream_access_addresses(
    image: &LoadedImage,
    reachable_instruction_offsets: &BTreeSet<usize>,
) -> Result<BTreeMap<usize, BTreeSet<u32>>> {
    match image.path.as_str() {
        "DAT1/MGAME.BIN" => validate_mgame_record_loops(image, reachable_instruction_offsets),
        "DAT1/MGENT.BIN" => {
            validate_mgent_command_sequence_copy(image, reachable_instruction_offsets)
        }
        "DAT1/PLSEL1.BIN" => {
            validate_plsel1_configuration_loops(image, reachable_instruction_offsets)
        }
        "DAT1/PLSEL3.BIN" => validate_mode_state_clear_loop(
            image,
            reachable_instruction_offsets,
            &PLSEL3_MODE_STATE_CLEAR,
        ),
        "DAT1/PLSEL4.BIN" => validate_plsel4_descending_fill(image, reachable_instruction_offsets),
        "SLPS_021.20" => {
            validate_main_configuration_record_loop(image, reachable_instruction_offsets)
        }
        _ => Ok(BTreeMap::new()),
    }
}

fn validate_mgent_command_sequence_copy(
    image: &LoadedImage,
    reachable_instruction_offsets: &BTreeSet<usize>,
) -> Result<BTreeMap<usize, BTreeSet<u32>>> {
    let dispatcher_offset = 0x686c;
    let copy_offset = 0x6a98;
    let dispatcher_runtime_address = runtime_address(image, dispatcher_offset)?;
    let copy_runtime_address = runtime_address(image, copy_offset)?;
    let dispatcher_callers = [0x1af4, 0x1b50, 0x1fe8];
    ensure_direct_call_offsets(
        image,
        dispatcher_runtime_address,
        &dispatcher_callers,
        "MGENT command-sequence dispatcher",
    )?;
    ensure_only_reachable_direct_entries(
        image,
        reachable_instruction_offsets,
        &dispatcher_callers,
        dispatcher_runtime_address,
        "MGENT command-sequence dispatcher",
    )?;
    ensure!(
        dispatcher_callers
            .iter()
            .all(|offset| reachable_instruction_offsets.contains(offset)),
        "{} selector-state MGENT command-sequence dispatcher has an unreachable profiled caller",
        image.path
    );
    ensure_sequence(
        image,
        &[
            (
                0x1af4,
                Instruction::Jal {
                    target: dispatcher_runtime_address,
                },
            ),
            (
                0x1af8,
                Instruction::Addu {
                    rd: Register::A1,
                    rs: Register::ZERO,
                    rt: Register::ZERO,
                },
            ),
            (
                0x1b50,
                Instruction::Jal {
                    target: dispatcher_runtime_address,
                },
            ),
            (
                0x1b54,
                Instruction::Addiu {
                    rt: Register::A1,
                    rs: Register::ZERO,
                    immediate: 1,
                },
            ),
            (
                0x1fe8,
                Instruction::Jal {
                    target: dispatcher_runtime_address,
                },
            ),
            (
                0x1fec,
                Instruction::Addiu {
                    rt: Register::A1,
                    rs: Register::ZERO,
                    immediate: 2,
                },
            ),
            (
                0x6970,
                Instruction::Jal {
                    target: copy_runtime_address,
                },
            ),
            (
                0x6974,
                Instruction::Addu {
                    rd: Register::A0,
                    rs: Register::A1,
                    rt: Register::ZERO,
                },
            ),
        ],
        "MGENT command-sequence callers",
    )?;
    ensure_direct_call_offsets(
        image,
        copy_runtime_address,
        &[0x6970],
        "MGENT command-sequence copy",
    )?;
    ensure_only_reachable_direct_entries(
        image,
        reachable_instruction_offsets,
        &[0x6970],
        copy_runtime_address,
        "MGENT command-sequence copy",
    )?;
    ensure_register_is_preserved_between(
        image,
        dispatcher_offset,
        0x6974,
        Register::A1,
        "MGENT command-sequence selector",
    )?;

    let expected_copy = [
        (
            0x6a98,
            Instruction::Sll {
                rd: Register::V0,
                rt: Register::A0,
                shift: 2,
            },
        ),
        (
            0x6a9c,
            Instruction::Lui {
                rt: Register::AT,
                immediate: 0x8018,
            },
        ),
        (
            0x6aa0,
            Instruction::Addu {
                rd: Register::AT,
                rs: Register::AT,
                rt: Register::V0,
            },
        ),
        (
            0x6aa4,
            Instruction::Lw {
                rt: Register::A1,
                base: Register::AT,
                offset: -24536,
            },
        ),
        (
            0x6aa8,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::ZERO,
                immediate: 1,
            },
        ),
        (
            0x6aac,
            Instruction::Beq {
                rs: Register::A0,
                rt: Register::V0,
                target: runtime_address(image, 0x6aec)?,
            },
        ),
        (
            0x6ab0,
            Instruction::Slti {
                rt: Register::V0,
                rs: Register::A0,
                immediate: 2,
            },
        ),
        (
            0x6ab4,
            Instruction::Beq {
                rs: Register::V0,
                rt: Register::ZERO,
                target: runtime_address(image, 0x6acc)?,
            },
        ),
        (
            0x6abc,
            Instruction::Beq {
                rs: Register::A0,
                rt: Register::ZERO,
                target: runtime_address(image, 0x6ae0)?,
            },
        ),
        (
            0x6acc,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::ZERO,
                immediate: 2,
            },
        ),
        (
            0x6ad0,
            Instruction::Beq {
                rs: Register::A0,
                rt: Register::V0,
                target: runtime_address(image, 0x6af8)?,
            },
        ),
        (
            0x6ae0,
            Instruction::Lui {
                rt: Register::V1,
                immediate: 0x801f,
            },
        ),
        (
            0x6ae4,
            Instruction::J {
                target: runtime_address(image, 0x6b00)?,
            },
        ),
        (
            0x6ae8,
            Instruction::Ori {
                rt: Register::V1,
                rs: Register::V1,
                immediate: 0x1a12,
            },
        ),
        (
            0x6aec,
            Instruction::Lui {
                rt: Register::V1,
                immediate: 0x801f,
            },
        ),
        (
            0x6af0,
            Instruction::J {
                target: runtime_address(image, 0x6b00)?,
            },
        ),
        (
            0x6af4,
            Instruction::Ori {
                rt: Register::V1,
                rs: Register::V1,
                immediate: 0x1a22,
            },
        ),
        (
            0x6af8,
            Instruction::Lui {
                rt: Register::V1,
                immediate: 0x801f,
            },
        ),
        (
            0x6afc,
            Instruction::Ori {
                rt: Register::V1,
                rs: Register::V1,
                immediate: 0x1a32,
            },
        ),
        (
            0x6b00,
            Instruction::Addiu {
                rt: Register::A0,
                rs: Register::ZERO,
                immediate: 0x3001,
            },
        ),
        (
            0x6b04,
            Instruction::Lhu {
                rt: Register::V0,
                base: Register::A1,
                offset: 0,
            },
        ),
        (
            0x6b08,
            Instruction::Addiu {
                rt: Register::A1,
                rs: Register::A1,
                immediate: 2,
            },
        ),
        (
            0x6b0c,
            Instruction::Sh {
                rt: Register::V0,
                base: Register::V1,
                offset: 0,
            },
        ),
        (
            0x6b10,
            Instruction::Bne {
                rs: Register::V0,
                rt: Register::A0,
                target: runtime_address(image, 0x6b04)?,
            },
        ),
        (
            0x6b14,
            Instruction::Addiu {
                rt: Register::V1,
                rs: Register::V1,
                immediate: 2,
            },
        ),
    ];
    ensure_sequence(image, &expected_copy, "MGENT command-sequence copy")?;
    ensure_only_reachable_direct_entries(
        image,
        reachable_instruction_offsets,
        &[0x6ac4, 0x6ad8, 0x6b10],
        runtime_address(image, 0x6b04)?,
        "MGENT command-sequence copy loop",
    )?;

    for (pointer_offset, expected_pointer) in [
        (0x28, 0x8017_a010),
        (0x2c, 0x8017_a018),
        (0x30, 0x8017_a020),
    ] {
        ensure!(
            read_u32(image, pointer_offset)? == expected_pointer,
            "{} selector-state MGENT command-sequence source pointer changed at +0x{pointer_offset:x}",
            image.path
        );
        let sequence_offset = usize::try_from(expected_pointer - 0x8017_a000)?;
        for value_offset in (sequence_offset..sequence_offset + 6).step_by(2) {
            ensure!(
                read_u16(image, value_offset)? != 0x3001,
                "{} selector-state MGENT command sequence terminates early at +0x{value_offset:x}",
                image.path
            );
        }
        ensure!(
            read_u16(image, sequence_offset + 6)? == 0x3001,
            "{} selector-state MGENT command sequence lacks its fourth-halfword terminator at +0x{:x}",
            image.path,
            sequence_offset + 6
        );
    }

    Ok(BTreeMap::from([(
        0x6b0c,
        (0x801f_1a12..0x801f_1a1a)
            .chain(0x801f_1a22..0x801f_1a2a)
            .chain(0x801f_1a32..0x801f_1a3a)
            .collect(),
    )]))
}

fn read_u16(image: &LoadedImage, offset: usize) -> Result<u16> {
    let bytes = image
        .data
        .get(offset..offset.checked_add(2).context("halfword end overflow")?)
        .with_context(|| format!("{} halfword is truncated at +0x{offset:x}", image.path))?;
    Ok(u16::from_le_bytes(bytes.try_into()?))
}

fn read_u32(image: &LoadedImage, offset: usize) -> Result<u32> {
    let bytes = image
        .data
        .get(offset..offset.checked_add(4).context("word end overflow")?)
        .with_context(|| format!("{} word is truncated at +0x{offset:x}", image.path))?;
    Ok(u32::from_le_bytes(bytes.try_into()?))
}

fn validate_mgame_record_loops(
    image: &LoadedImage,
    reachable_instruction_offsets: &BTreeSet<usize>,
) -> Result<BTreeMap<usize, BTreeSet<u32>>> {
    let initialization_loop = [
        (
            0xcd84,
            Instruction::Lui {
                rt: Register::A1,
                immediate: 0x801f,
            },
        ),
        (
            0xcd88,
            Instruction::Ori {
                rt: Register::A1,
                rs: Register::A1,
                immediate: 0x180c,
            },
        ),
        (
            0xcd8c,
            Instruction::Addu {
                rd: Register::A0,
                rs: Register::ZERO,
                rt: Register::ZERO,
            },
        ),
        (
            0xcd94,
            Instruction::Lui {
                rt: Register::V1,
                immediate: 0x801f,
            },
        ),
        (
            0xcd98,
            Instruction::Ori {
                rt: Register::V1,
                rs: Register::V1,
                immediate: 0x180e,
            },
        ),
        (
            0xcd9c,
            Instruction::Slti {
                rt: Register::V0,
                rs: Register::A0,
                immediate: 8,
            },
        ),
        (
            0xcda0,
            Instruction::Beq {
                rs: Register::V0,
                rt: Register::ZERO,
                target: runtime_address(image, 0xcdc4)?,
            },
        ),
        (
            0xcda4,
            Instruction::Slti {
                rt: Register::V0,
                rs: Register::A0,
                immediate: 5,
            },
        ),
        (
            0xcda8,
            Instruction::Bne {
                rs: Register::V0,
                rt: Register::ZERO,
                target: runtime_address(image, 0xcdc4)?,
            },
        ),
        (
            0xcdb4,
            Instruction::Sh {
                rt: Register::A2,
                base: Register::V1,
                offset: 0,
            },
        ),
        (
            0xcdb8,
            Instruction::Sb {
                rt: Register::ZERO,
                base: Register::V1,
                offset: -1,
            },
        ),
        (
            0xcdc4,
            Instruction::Sb {
                rt: Register::ZERO,
                base: Register::V1,
                offset: -1,
            },
        ),
        (
            0xcdd0,
            Instruction::Sh {
                rt: Register::ZERO,
                base: Register::V1,
                offset: 0,
            },
        ),
        (
            0xcdd4,
            Instruction::Addiu {
                rt: Register::A0,
                rs: Register::A0,
                immediate: 1,
            },
        ),
        (
            0xcdd8,
            Instruction::Addiu {
                rt: Register::V1,
                rs: Register::V1,
                immediate: 8,
            },
        ),
        (
            0xcddc,
            Instruction::Slti {
                rt: Register::V0,
                rs: Register::A0,
                immediate: 9,
            },
        ),
        (
            0xcde0,
            Instruction::Bne {
                rs: Register::V0,
                rt: Register::ZERO,
                target: runtime_address(image, 0xcd9c)?,
            },
        ),
        (
            0xcde4,
            Instruction::Addiu {
                rt: Register::A1,
                rs: Register::A1,
                immediate: 8,
            },
        ),
    ];
    ensure_sequence(
        image,
        &initialization_loop,
        "MGAME strided record initialization",
    )?;
    ensure_loop_is_reachable_and_closed(
        image,
        reachable_instruction_offsets,
        0xcd9c,
        0xcde0,
        "MGAME strided record initialization",
    )?;

    let negative_value_clear_loop = [
        (
            0xd048,
            Instruction::Addu {
                rd: Register::A0,
                rs: Register::ZERO,
                rt: Register::ZERO,
            },
        ),
        (
            0xd04c,
            Instruction::Lui {
                rt: Register::V1,
                immediate: 0x801f,
            },
        ),
        (
            0xd050,
            Instruction::Ori {
                rt: Register::V1,
                rs: Register::V1,
                immediate: 0x180e,
            },
        ),
        (
            0xd054,
            Instruction::Lh {
                rt: Register::V0,
                base: Register::V1,
                offset: 0,
            },
        ),
        (
            0xd05c,
            Instruction::Bgez {
                rs: Register::V0,
                target: runtime_address(image, 0xd06c)?,
            },
        ),
        (
            0xd068,
            Instruction::Sh {
                rt: Register::ZERO,
                base: Register::V1,
                offset: 0,
            },
        ),
        (
            0xd06c,
            Instruction::Addiu {
                rt: Register::A0,
                rs: Register::A0,
                immediate: 1,
            },
        ),
        (
            0xd070,
            Instruction::Slti {
                rt: Register::V0,
                rs: Register::A0,
                immediate: 9,
            },
        ),
        (
            0xd074,
            Instruction::Bne {
                rs: Register::V0,
                rt: Register::ZERO,
                target: runtime_address(image, 0xd054)?,
            },
        ),
        (
            0xd078,
            Instruction::Addiu {
                rt: Register::V1,
                rs: Register::V1,
                immediate: 8,
            },
        ),
    ];
    ensure_sequence(
        image,
        &negative_value_clear_loop,
        "MGAME negative record-value clear",
    )?;
    ensure_loop_is_reachable_and_closed(
        image,
        reachable_instruction_offsets,
        0xd054,
        0xd074,
        "MGAME negative record-value clear",
    )?;

    let accumulation_loop = [
        (
            0xd0b0,
            Instruction::Addu {
                rd: Register::A3,
                rs: Register::ZERO,
                rt: Register::ZERO,
            },
        ),
        (
            0xd0c8,
            Instruction::Lui {
                rt: Register::V1,
                immediate: 0x801f,
            },
        ),
        (
            0xd0cc,
            Instruction::Ori {
                rt: Register::V1,
                rs: Register::V1,
                immediate: 0x180e,
            },
        ),
        (
            0xd0e4,
            Instruction::Addiu {
                rt: Register::A3,
                rs: Register::A3,
                immediate: 1,
            },
        ),
        (
            0xd0f8,
            Instruction::Sh {
                rt: Register::V0,
                base: Register::V1,
                offset: 0,
            },
        ),
        (
            0xd0fc,
            Instruction::Slti {
                rt: Register::V0,
                rs: Register::A3,
                immediate: 9,
            },
        ),
        (
            0xd100,
            Instruction::Bne {
                rs: Register::V0,
                rt: Register::ZERO,
                target: runtime_address(image, 0xd0d0)?,
            },
        ),
        (
            0xd104,
            Instruction::Addiu {
                rt: Register::V1,
                rs: Register::V1,
                immediate: 8,
            },
        ),
    ];
    ensure_sequence(
        image,
        &accumulation_loop,
        "MGAME strided record accumulation",
    )?;
    ensure_loop_is_reachable_and_closed(
        image,
        reachable_instruction_offsets,
        0xd0d0,
        0xd100,
        "MGAME strided record accumulation",
    )?;

    let conditional_middle_records = [5_u32, 6, 7];
    let conditional_outer_records = [0_u32, 1, 2, 3, 4, 8];
    let all_records = 0_u32..9;
    Ok(BTreeMap::from([
        (
            0xcdb4,
            strided_byte_addresses(0x801f_180e, 8, conditional_middle_records, 2),
        ),
        (
            0xcdb8,
            strided_byte_addresses(0x801f_180d, 8, conditional_middle_records, 1),
        ),
        (
            0xcdc4,
            strided_byte_addresses(0x801f_180d, 8, conditional_outer_records, 1),
        ),
        (
            0xcdd0,
            strided_byte_addresses(0x801f_180e, 8, conditional_outer_records, 2),
        ),
        (
            0xd068,
            strided_byte_addresses(0x801f_180e, 8, all_records.clone(), 2),
        ),
        (
            0xd0f8,
            strided_byte_addresses(0x801f_180e, 8, all_records, 2),
        ),
    ]))
}

fn strided_byte_addresses(
    first_address: u32,
    stride: u32,
    indices: impl IntoIterator<Item = u32>,
    width_bytes: u32,
) -> BTreeSet<u32> {
    indices
        .into_iter()
        .flat_map(|index| {
            let start = first_address + index * stride;
            start..start + width_bytes
        })
        .collect()
}

fn validate_mode_state_clear_loop(
    image: &LoadedImage,
    reachable_instruction_offsets: &BTreeSet<usize>,
    profile: &ModeStateClearLoopProfile,
) -> Result<BTreeMap<usize, BTreeSet<u32>>> {
    let expected = [
        (
            profile.pointer_lui_offset,
            Instruction::Lui {
                rt: profile.pointer_base,
                immediate: 0x801f,
            },
        ),
        (
            profile.pointer_low_offset,
            Instruction::Ori {
                rt: profile.pointer_base,
                rs: profile.pointer_base,
                immediate: 0x5c00,
            },
        ),
        (
            profile.counter_setup_offset,
            Instruction::Addiu {
                rt: Register::V1,
                rs: Register::ZERO,
                immediate: 7,
            },
        ),
        (
            profile.pointer_adjust_offset,
            Instruction::Addiu {
                rt: Register::V0,
                rs: profile.pointer_base,
                immediate: 67,
            },
        ),
        (
            profile.store_offset,
            Instruction::Sb {
                rt: Register::ZERO,
                base: Register::V0,
                offset: 0,
            },
        ),
        (
            profile.store_offset + 4,
            Instruction::Addiu {
                rt: Register::V1,
                rs: Register::V1,
                immediate: -1,
            },
        ),
        (
            profile.store_offset + 8,
            Instruction::Bgez {
                rs: Register::V1,
                target: runtime_address(image, profile.store_offset)?,
            },
        ),
        (
            profile.store_offset + 12,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::V0,
                immediate: -1,
            },
        ),
    ];
    ensure_sequence(image, &expected, "mode-state clear loop")?;
    ensure_loop_is_reachable_and_closed(
        image,
        reachable_instruction_offsets,
        profile.store_offset,
        profile.store_offset + 8,
        "mode-state clear loop",
    )?;
    Ok(BTreeMap::from([(
        profile.store_offset,
        (0x801f_5c3c..=0x801f_5c43).collect(),
    )]))
}

fn validate_plsel1_configuration_loops(
    image: &LoadedImage,
    reachable_instruction_offsets: &BTreeSet<usize>,
) -> Result<BTreeMap<usize, BTreeSet<u32>>> {
    let outer_setup = [
        (
            0xca8,
            Instruction::Addu {
                rd: Register::A3,
                rs: Register::S6,
                rt: Register::ZERO,
            },
        ),
        (
            0xcac,
            Instruction::Lui {
                rt: Register::T0,
                immediate: 0x801f,
            },
        ),
        (
            0xcb0,
            Instruction::Ori {
                rt: Register::T0,
                rs: Register::T0,
                immediate: 0x6700,
            },
        ),
        (
            0xcb4,
            Instruction::Lui {
                rt: Register::A1,
                immediate: 0x801f,
            },
        ),
        (
            0xcb8,
            Instruction::Ori {
                rt: Register::A1,
                rs: Register::A1,
                immediate: 0x6700,
            },
        ),
        (
            0xd20,
            Instruction::Addiu {
                rt: Register::A3,
                rs: Register::A3,
                immediate: 2,
            },
        ),
        (
            0xd24,
            Instruction::Addiu {
                rt: Register::T0,
                rs: Register::T0,
                immediate: 2,
            },
        ),
        (
            0xd28,
            Instruction::Addiu {
                rt: Register::S1,
                rs: Register::S1,
                immediate: 1,
            },
        ),
        (
            0xd2c,
            Instruction::Slti {
                rt: Register::V0,
                rs: Register::S1,
                immediate: 2,
            },
        ),
        (
            0xd30,
            Instruction::Bne {
                rs: Register::V0,
                rt: Register::ZERO,
                target: runtime_address(image, 0xcbc)?,
            },
        ),
        (
            0xd34,
            Instruction::Addiu {
                rt: Register::A1,
                rs: Register::A1,
                immediate: 1,
            },
        ),
    ];
    ensure_sequence(image, &outer_setup, "PLSEL1 configuration outer loop")?;
    ensure_loop_is_reachable_and_closed(
        image,
        reachable_instruction_offsets,
        0xcbc,
        0xd30,
        "PLSEL1 configuration outer loop",
    )?;

    let inner = [
        (
            0xcf0,
            Instruction::Addiu {
                rt: Register::V1,
                rs: Register::A3,
                immediate: 0x4a17,
            },
        ),
        (
            0xcf4,
            Instruction::Addiu {
                rt: Register::A0,
                rs: Register::T0,
                immediate: 2,
            },
        ),
        (
            0xcf8,
            Instruction::Addiu {
                rt: Register::A2,
                rs: Register::A3,
                immediate: 0x4a19,
            },
        ),
        (
            0xcfc,
            Instruction::Lbu {
                rt: Register::V0,
                base: Register::A0,
                offset: 0,
            },
        ),
        (
            0xd10,
            Instruction::Addiu {
                rt: Register::V1,
                rs: Register::V1,
                immediate: 1,
            },
        ),
        (
            0xd14,
            Instruction::Slt {
                rd: Register::V0,
                rs: Register::V1,
                rt: Register::A2,
            },
        ),
        (
            0xd18,
            Instruction::Bne {
                rs: Register::V0,
                rt: Register::ZERO,
                target: runtime_address(image, 0xcfc)?,
            },
        ),
        (
            0xd1c,
            Instruction::Addiu {
                rt: Register::A0,
                rs: Register::A0,
                immediate: 1,
            },
        ),
    ];
    ensure_sequence(image, &inner, "PLSEL1 configuration inner loop")?;
    ensure_loop_is_reachable_and_closed(
        image,
        reachable_instruction_offsets,
        0xcfc,
        0xd18,
        "PLSEL1 configuration inner loop",
    )?;

    let cleanup = [
        (
            0xe08,
            Instruction::Addu {
                rd: Register::S1,
                rs: Register::ZERO,
                rt: Register::ZERO,
            },
        ),
        (
            0xe10,
            Instruction::Lui {
                rt: Register::S0,
                immediate: 0x801f,
            },
        ),
        (
            0xe14,
            Instruction::Ori {
                rt: Register::S0,
                rs: Register::S0,
                immediate: 0x6700,
            },
        ),
        (
            0xe74,
            Instruction::Slti {
                rt: Register::V0,
                rs: Register::S1,
                immediate: 2,
            },
        ),
        (
            0xe78,
            Instruction::Bne {
                rs: Register::V0,
                rt: Register::ZERO,
                target: runtime_address(image, 0xe18)?,
            },
        ),
        (
            0xe7c,
            Instruction::Addiu {
                rt: Register::S0,
                rs: Register::S0,
                immediate: 1,
            },
        ),
    ];
    ensure_sequence(image, &cleanup, "PLSEL1 configuration cleanup loop")?;
    ensure_loop_is_reachable_and_closed(
        image,
        reachable_instruction_offsets,
        0xe18,
        0xe78,
        "PLSEL1 configuration cleanup loop",
    )?;

    let first_two = BTreeSet::from([0x801f_6700, 0x801f_6701]);
    Ok(BTreeMap::from([
        (0xccc, first_two.clone()),
        (0xcdc, first_two.clone()),
        (0xce0, first_two.clone()),
        (0xcfc, (0x801f_6702..=0x801f_6705).collect()),
        (0xe18, first_two.clone()),
        (0xe6c, first_two),
    ]))
}

fn validate_plsel4_descending_fill(
    image: &LoadedImage,
    reachable_instruction_offsets: &BTreeSet<usize>,
) -> Result<BTreeMap<usize, BTreeSet<u32>>> {
    let expected = [
        (
            0xd0c,
            Instruction::Addiu {
                rt: Register::V1,
                rs: Register::ZERO,
                immediate: 0xff,
            },
        ),
        (
            0xd10,
            Instruction::Addiu {
                rt: Register::S0,
                rs: Register::ZERO,
                immediate: 7,
            },
        ),
        (
            0xd14,
            Instruction::Lui {
                rt: Register::V0,
                immediate: 0x801f,
            },
        ),
        (
            0xd18,
            Instruction::Ori {
                rt: Register::V0,
                rs: Register::V0,
                immediate: 0x5c07,
            },
        ),
        (
            0xd1c,
            Instruction::Sb {
                rt: Register::V1,
                base: Register::V0,
                offset: 0x44,
            },
        ),
        (
            0xd20,
            Instruction::Addiu {
                rt: Register::S0,
                rs: Register::S0,
                immediate: -1,
            },
        ),
        (
            0xd24,
            Instruction::Bgez {
                rs: Register::S0,
                target: runtime_address(image, 0xd1c)?,
            },
        ),
        (
            0xd28,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::V0,
                immediate: -1,
            },
        ),
    ];
    ensure_sequence(image, &expected, "PLSEL4 descending fill")?;
    ensure_loop_is_reachable_and_closed(
        image,
        reachable_instruction_offsets,
        0xd1c,
        0xd24,
        "PLSEL4 descending fill",
    )?;
    let mut result = BTreeMap::from([(0xd1c, (0x801f_5c44..=0x801f_5c4b).collect())]);
    result.extend(validate_mode_state_clear_loop(
        image,
        reachable_instruction_offsets,
        &PLSEL4_MODE_STATE_CLEAR,
    )?);
    Ok(result)
}

fn validate_main_configuration_record_loop(
    image: &LoadedImage,
    reachable_instruction_offsets: &BTreeSet<usize>,
) -> Result<BTreeMap<usize, BTreeSet<u32>>> {
    let expected = [
        (
            0xcc04,
            Instruction::Addu {
                rd: Register::T0,
                rs: Register::ZERO,
                rt: Register::ZERO,
            },
        ),
        (
            0xcc14,
            Instruction::Lui {
                rt: Register::A1,
                immediate: 0x801f,
            },
        ),
        (
            0xcc18,
            Instruction::Ori {
                rt: Register::A1,
                rs: Register::A1,
                immediate: 0x652b,
            },
        ),
        (
            0xcc60,
            Instruction::Addiu {
                rt: Register::T0,
                rs: Register::T0,
                immediate: 1,
            },
        ),
        (
            0xcc84,
            Instruction::Addiu {
                rt: Register::A1,
                rs: Register::A1,
                immediate: 0x10,
            },
        ),
        (
            0xcc8c,
            Instruction::Slti {
                rt: Register::V0,
                rs: Register::T0,
                immediate: 5,
            },
        ),
        (
            0xcc90,
            Instruction::Bne {
                rs: Register::V0,
                rt: Register::ZERO,
                target: runtime_address(image, 0xcc54)?,
            },
        ),
    ];
    ensure_sequence(image, &expected, "main configuration-record loop")?;
    ensure_loop_is_reachable_and_closed(
        image,
        reachable_instruction_offsets,
        0xcc54,
        0xcc90,
        "main configuration-record loop",
    )?;

    let mut result = BTreeMap::new();
    for (instruction_offset, displacement) in [
        (0xcc68, -10_i32),
        (0xcc6c, -9),
        (0xcc70, -7),
        (0xcc74, -3),
        (0xcc78, -2),
        (0xcc7c, -1),
        (0xcc80, 0),
    ] {
        let mut starts = BTreeSet::new();
        for record in 0..5_u32 {
            let start = 0x801f_652b_u32
                .checked_add(record * 0x10)
                .expect("finite record address")
                .wrapping_add_signed(displacement);
            starts.insert(start);
        }
        result.insert(instruction_offset, starts);
    }
    Ok(result)
}

fn ensure_sequence(
    image: &LoadedImage,
    expected: &[(usize, Instruction)],
    role: &str,
) -> Result<()> {
    for (offset, instruction) in expected {
        ensure_instruction(image, *offset, instruction.clone(), role)?;
    }
    Ok(())
}

fn ensure_loop_is_reachable_and_closed(
    image: &LoadedImage,
    reachable_instruction_offsets: &BTreeSet<usize>,
    head_offset: usize,
    backedge_offset: usize,
    role: &str,
) -> Result<()> {
    ensure!(
        reachable_instruction_offsets.contains(&head_offset)
            && reachable_instruction_offsets.contains(&backedge_offset),
        "{} selector-state {role} is outside the reachable closure",
        image.path
    );
    ensure_no_other_reachable_direct_entry(
        image,
        reachable_instruction_offsets,
        backedge_offset,
        runtime_address(image, head_offset)?,
        role,
    )
}
