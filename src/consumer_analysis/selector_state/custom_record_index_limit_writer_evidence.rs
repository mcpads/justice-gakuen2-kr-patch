use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Result, ensure};
use psx_r3000a::{Instruction, Register};

use crate::source_disc::LoadedImage;

use super::profile_validation::{
    ensure_direct_call_offsets, ensure_instruction, ensure_only_reachable_direct_entries,
    ensure_register_is_preserved_between, runtime_address,
};
use super::writer_evidence::{ValidatedSelectorWriterEvidence, validate_kanri_custom_index_source};

pub(super) fn validated_custom_record_index_limit_writer_evidence(
    image: &LoadedImage,
    reachable_instruction_offsets: &BTreeSet<usize>,
) -> Result<BTreeMap<usize, ValidatedSelectorWriterEvidence>> {
    let mut result = BTreeMap::new();
    match image.path.as_str() {
        "DAT1/KANRI.BIN" => {
            validate_kanri_bit_count_writers(image, reachable_instruction_offsets, &mut result)?
        }
        "DAT1/MGTIT.BIN" => {
            validate_mgtit_motion_writers(image, reachable_instruction_offsets, &mut result)?
        }
        _ => {}
    }
    Ok(result)
}

fn validate_kanri_bit_count_writers(
    image: &LoadedImage,
    reachable_instruction_offsets: &BTreeSet<usize>,
    result: &mut BTreeMap<usize, ValidatedSelectorWriterEvidence>,
) -> Result<()> {
    validate_kanri_custom_index_source(image, reachable_instruction_offsets)?;
    for offset in [0x6554, 0x6570] {
        ensure!(
            reachable_instruction_offsets.contains(&offset),
            "{} KANRI bit-count writer +0x{offset:x} is not reachable",
            image.path
        );
    }

    let expected = [
        (
            0x653c,
            Instruction::Lui {
                rt: Register::V1,
                immediate: 0x801f,
            },
        ),
        (
            0x6540,
            Instruction::Ori {
                rt: Register::V1,
                rs: Register::V1,
                immediate: 0x5800,
            },
        ),
        (
            0x6544,
            Instruction::Lhu {
                rt: Register::V0,
                base: Register::V1,
                offset: 0,
            },
        ),
        (
            0x6548,
            Instruction::Addu {
                rd: Register::A0,
                rs: Register::ZERO,
                rt: Register::ZERO,
            },
        ),
        (
            0x654c,
            Instruction::Addu {
                rd: Register::A1,
                rs: Register::V0,
                rt: Register::ZERO,
            },
        ),
        (
            0x6550,
            Instruction::Sw {
                rt: Register::A1,
                base: Register::V1,
                offset: 0,
            },
        ),
        (
            0x6554,
            Instruction::Sw {
                rt: Register::ZERO,
                base: Register::S0,
                offset: 28,
            },
        ),
        (
            0x6558,
            Instruction::Andi {
                rt: Register::V0,
                rs: Register::A1,
                immediate: 1,
            },
        ),
        (
            0x655c,
            Instruction::Beq {
                rs: Register::V0,
                rt: Register::ZERO,
                target: runtime_address(image, 0x6574)?,
            },
        ),
        (0x6560, Instruction::nop()),
        (
            0x6564,
            Instruction::Lw {
                rt: Register::V0,
                base: Register::S0,
                offset: 28,
            },
        ),
        (
            0x6568,
            Instruction::Srl {
                rd: Register::A1,
                rt: Register::A1,
                shift: 1,
            },
        ),
        (
            0x656c,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::V0,
                immediate: 1,
            },
        ),
        (
            0x6570,
            Instruction::Sw {
                rt: Register::V0,
                base: Register::S0,
                offset: 28,
            },
        ),
        (
            0x6574,
            Instruction::Addiu {
                rt: Register::A0,
                rs: Register::A0,
                immediate: 1,
            },
        ),
        (
            0x6578,
            Instruction::Slti {
                rt: Register::V0,
                rs: Register::A0,
                immediate: 16,
            },
        ),
        (
            0x657c,
            Instruction::Bne {
                rs: Register::V0,
                rt: Register::ZERO,
                target: runtime_address(image, 0x655c)?,
            },
        ),
        (
            0x6580,
            Instruction::Andi {
                rt: Register::V0,
                rs: Register::A1,
                immediate: 1,
            },
        ),
    ];
    for (offset, instruction) in expected {
        ensure_instruction(image, offset, instruction, "KANRI 16-bit mask bit count")?;
    }
    ensure_only_reachable_direct_entries(
        image,
        reachable_instruction_offsets,
        &[0x657c],
        runtime_address(image, 0x655c)?,
        "KANRI bit-count loop",
    )?;
    for target_offset in [0x6554, 0x6564, 0x6570] {
        ensure_only_reachable_direct_entries(
            image,
            reachable_instruction_offsets,
            &[],
            runtime_address(image, target_offset)?,
            "KANRI bit-count straight-line block",
        )?;
    }

    insert(
        result,
        0x6554,
        exact_value(
            "mask_bit_count_reset",
            0,
            "resets the count word before scanning the exact 16-bit mask",
        ),
        &image.path,
    )?;
    insert(
        result,
        0x6570,
        bounded_value(
            "mask_bit_count_increment",
            [1, 16],
            "increments at most once in each iteration of the validated 16-iteration mask scan",
        ),
        &image.path,
    )
}

fn validate_mgtit_motion_writers(
    image: &LoadedImage,
    reachable_instruction_offsets: &BTreeSet<usize>,
    result: &mut BTreeMap<usize, ValidatedSelectorWriterEvidence>,
) -> Result<()> {
    let function_offset = 0x1fc0;
    let caller_offset = 0x1fa4;
    let function_runtime_address = runtime_address(image, function_offset)?;
    ensure!(
        reachable_instruction_offsets.contains(&caller_offset),
        "{} MGTIT motion updater caller is not reachable",
        image.path
    );
    ensure_direct_call_offsets(
        image,
        function_runtime_address,
        &[caller_offset],
        "MGTIT motion updater",
    )?;
    ensure_only_reachable_direct_entries(
        image,
        reachable_instruction_offsets,
        &[caller_offset],
        function_runtime_address,
        "MGTIT motion updater",
    )?;

    let root_and_call = [
        (
            0x1b68,
            Instruction::Lui {
                rt: Register::S0,
                immediate: 0x801f,
            },
        ),
        (
            0x1b98,
            Instruction::Ori {
                rt: Register::S0,
                rs: Register::S0,
                immediate: 0x1a00,
            },
        ),
        (
            caller_offset,
            Instruction::Jal {
                target: function_runtime_address,
            },
        ),
        (
            0x1fa8,
            Instruction::Addu {
                rd: Register::A0,
                rs: Register::S0,
                rt: Register::ZERO,
            },
        ),
        (
            0x1fc0,
            Instruction::Addu {
                rd: Register::T4,
                rs: Register::A0,
                rt: Register::ZERO,
            },
        ),
        (
            0x1fc4,
            Instruction::Addu {
                rd: Register::T1,
                rs: Register::ZERO,
                rt: Register::ZERO,
            },
        ),
        (
            0x1fd4,
            Instruction::Addiu {
                rt: Register::T3,
                rs: Register::ZERO,
                immediate: 12,
            },
        ),
        (
            0x1fd8,
            Instruction::Addu {
                rd: Register::A2,
                rs: Register::T4,
                rt: Register::T3,
            },
        ),
    ];
    for (offset, instruction) in root_and_call {
        ensure_instruction(
            image,
            offset,
            instruction,
            "MGTIT fixed-root motion updater",
        )?;
    }
    ensure_register_is_preserved_between(
        image,
        0x1b9c,
        0x1fa8,
        Register::S0,
        "MGTIT motion-state root",
    )?;

    let first_path = [
        (
            0x1ffc,
            Instruction::Lw {
                rt: Register::V0,
                base: Register::A2,
                offset: 12,
            },
        ),
        (
            0x2000,
            Instruction::Lw {
                rt: Register::A0,
                base: Register::A2,
                offset: 4,
            },
        ),
        (
            0x2004,
            Instruction::Lw {
                rt: Register::V1,
                base: Register::A2,
                offset: 16,
            },
        ),
        (
            0x2008,
            Instruction::Lw {
                rt: Register::A1,
                base: Register::A2,
                offset: 8,
            },
        ),
        (
            0x200c,
            Instruction::Addu {
                rd: Register::V0,
                rs: Register::V0,
                rt: Register::A0,
            },
        ),
        (
            0x2010,
            Instruction::Addu {
                rd: Register::V1,
                rs: Register::V1,
                rt: Register::A1,
            },
        ),
        (
            0x2014,
            Instruction::Sw {
                rt: Register::V0,
                base: Register::A2,
                offset: 12,
            },
        ),
        (
            0x201c,
            Instruction::Sw {
                rt: Register::V1,
                base: Register::A2,
                offset: 16,
            },
        ),
    ];
    let second_path = [
        (
            0x20e8,
            Instruction::Lw {
                rt: Register::V0,
                base: Register::A2,
                offset: 12,
            },
        ),
        (
            0x20ec,
            Instruction::Lw {
                rt: Register::A0,
                base: Register::A2,
                offset: 4,
            },
        ),
        (
            0x20f0,
            Instruction::Lw {
                rt: Register::V1,
                base: Register::A2,
                offset: 16,
            },
        ),
        (
            0x20f4,
            Instruction::Lw {
                rt: Register::A1,
                base: Register::A2,
                offset: 8,
            },
        ),
        (
            0x20f8,
            Instruction::Addu {
                rd: Register::V0,
                rs: Register::V0,
                rt: Register::A0,
            },
        ),
        (
            0x20fc,
            Instruction::Addu {
                rd: Register::V1,
                rs: Register::V1,
                rt: Register::A1,
            },
        ),
        (
            0x2100,
            Instruction::Sw {
                rt: Register::V0,
                base: Register::A2,
                offset: 12,
            },
        ),
        (
            0x2108,
            Instruction::Sw {
                rt: Register::V1,
                base: Register::A2,
                offset: 16,
            },
        ),
    ];
    for (offset, instruction) in first_path.into_iter().chain(second_path) {
        ensure_instruction(image, offset, instruction, "MGTIT motion accumulation")?;
    }
    ensure_instruction(
        image,
        0x218c,
        Instruction::Sw {
            rt: Register::ZERO,
            base: Register::A2,
            offset: 16,
        },
        "MGTIT stopped motion reset",
    )?;
    let loop_tail = [
        (
            0x219c,
            Instruction::Addiu {
                rt: Register::T3,
                rs: Register::T3,
                immediate: 20,
            },
        ),
        (
            0x21a8,
            Instruction::Addiu {
                rt: Register::T1,
                rs: Register::T1,
                immediate: 1,
            },
        ),
        (
            0x21b0,
            Instruction::Slti {
                rt: Register::V0,
                rs: Register::T1,
                immediate: 3,
            },
        ),
        (
            0x21b4,
            Instruction::Bne {
                rs: Register::V0,
                rt: Register::ZERO,
                target: runtime_address(image, 0x1fd8)?,
            },
        ),
    ];
    for (offset, instruction) in loop_tail {
        ensure_instruction(image, offset, instruction, "MGTIT three-slot motion loop")?;
    }

    for offset in [0x201c, 0x2108] {
        insert(
            result,
            offset,
            runtime_source(
                "motion_position_plus_velocity",
                vec![[0x801f_1a14, 0x801f_1a18], [0x801f_1a1c, 0x801f_1a20]],
                "for the profiled first slot, adds the existing position word to its paired velocity word under the sole fixed-root caller",
            ),
            &image.path,
        )?;
    }
    insert(
        result,
        0x218c,
        exact_value(
            "stopped_motion_reset",
            0,
            "resets the profiled first-slot position word when its boundary transition stops motion",
        ),
        &image.path,
    )
}

fn exact_value(
    classification: &'static str,
    value: u8,
    evidence: &'static str,
) -> ValidatedSelectorWriterEvidence {
    ValidatedSelectorWriterEvidence {
        classification,
        value_resolution: "exact_candidates",
        exact_value_candidates: vec![value],
        bounded_value_range: None,
        source_runtime_byte_ranges: Vec::new(),
        evidence,
    }
}

fn bounded_value(
    classification: &'static str,
    range: [u8; 2],
    evidence: &'static str,
) -> ValidatedSelectorWriterEvidence {
    ValidatedSelectorWriterEvidence {
        classification,
        value_resolution: "bounded_range",
        exact_value_candidates: Vec::new(),
        bounded_value_range: Some(range),
        source_runtime_byte_ranges: Vec::new(),
        evidence,
    }
}

fn runtime_source(
    classification: &'static str,
    source_runtime_byte_ranges: Vec<[u32; 2]>,
    evidence: &'static str,
) -> ValidatedSelectorWriterEvidence {
    ValidatedSelectorWriterEvidence {
        classification,
        value_resolution: "runtime_source",
        exact_value_candidates: Vec::new(),
        bounded_value_range: None,
        source_runtime_byte_ranges,
        evidence,
    }
}

fn insert(
    result: &mut BTreeMap<usize, ValidatedSelectorWriterEvidence>,
    store_offset: usize,
    evidence: ValidatedSelectorWriterEvidence,
    image_path: &str,
) -> Result<()> {
    ensure!(
        result.insert(store_offset, evidence).is_none(),
        "{image_path} repeats custom-record index-limit writer evidence at +0x{store_offset:x}"
    );
    Ok(())
}
