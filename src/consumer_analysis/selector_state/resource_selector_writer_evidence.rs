use std::collections::BTreeMap;

use anyhow::{Result, ensure};
use psx_r3000a::{Instruction, Register};

use crate::source_disc::LoadedImage;

use super::profile_validation::{
    ensure_direct_call_offsets, ensure_instruction, ensure_register_is_preserved_between,
    runtime_address,
};
use super::writer_evidence::ValidatedSelectorWriterEvidence;

pub(super) fn validated_resource_selector_writer_evidence(
    image: &LoadedImage,
) -> Result<BTreeMap<usize, ValidatedSelectorWriterEvidence>> {
    let mut result = BTreeMap::new();
    match image.path.as_str() {
        "DAT1/KOUBAI2.BIN" => validate_koubai2_writer(image, &mut result)?,
        "SLPS_021.20" => validate_main_writers(image, &mut result)?,
        _ => {}
    }
    Ok(result)
}

fn validate_koubai2_writer(
    image: &LoadedImage,
    result: &mut BTreeMap<usize, ValidatedSelectorWriterEvidence>,
) -> Result<()> {
    let expected = [
        (
            0x4560,
            Instruction::Lui {
                rt: Register::AT,
                immediate: 0x800a,
            },
        ),
        (
            0x4564,
            Instruction::Addu {
                rd: Register::AT,
                rs: Register::AT,
                rt: Register::S0,
            },
        ),
        (
            0x4568,
            Instruction::Lbu {
                rt: Register::V0,
                base: Register::AT,
                offset: 0x3424,
            },
        ),
        (
            0x456c,
            Instruction::Lui {
                rt: Register::AT,
                immediate: 0x801f,
            },
        ),
        (
            0x4570,
            Instruction::Sb {
                rt: Register::V0,
                base: Register::AT,
                offset: 0x5c7f,
            },
        ),
    ];
    ensure_sequence(image, &expected, "KOUBAI2 resource-selector table writer")?;
    insert(
        result,
        0x4570,
        runtime_source(
            "runtime_indexed_embedded_byte",
            Vec::new(),
            "validated runtime-indexed byte load from the overlay image and direct global-byte store; the index domain remains dynamic",
        ),
        &image.path,
    )
}

fn validate_main_writers(
    image: &LoadedImage,
    result: &mut BTreeMap<usize, ValidatedSelectorWriterEvidence>,
) -> Result<()> {
    validate_main_record_seed_writer(image)?;
    insert(
        result,
        0xcc64,
        runtime_source(
            "runtime_pointer_byte",
            Vec::new(),
            "validated byte load through a runtime-selected record pointer and direct store to the first RKDEMO selector byte",
        ),
        &image.path,
    )?;
    insert(
        result,
        0xcc68,
        exact_value(
            "fixed_zero",
            0,
            "validated store from the architectural zero register to the second RKDEMO selector byte",
        ),
        &image.path,
    )?;

    validate_runtime_buffer_copy(image, 0x50088)?;
    validate_main_runtime_buffer_callers(image)?;
    insert(
        result,
        0x500a4,
        runtime_source(
            "runtime_buffer_byte_copy",
            Vec::new(),
            "validated complete direct-caller census; call +0xd530 copies exactly 1024 bytes from the runtime allocation at *(0x801f63f0)+896 into 0x801f5800..0x801f5bff",
        ),
        &image.path,
    )
}

fn validate_main_runtime_buffer_callers(image: &LoadedImage) -> Result<()> {
    let copy_runtime_address = runtime_address(image, 0x50088)?;
    ensure_direct_call_offsets(
        image,
        copy_runtime_address,
        &[
            0x6b44, 0xc010, 0xc0dc, 0xc0f8, 0xc114, 0xd4f4, 0xd508, 0xd51c, 0xd530, 0xd99c, 0xd9c0,
            0xd9d4, 0xd9e8, 0xd9fc, 0x17fa8, 0x18014, 0x18090, 0x180b4, 0x18110,
        ],
        "main runtime-buffer byte copy",
    )?;
    ensure_sequence(
        image,
        &[
            (
                0xd3bc,
                Instruction::Lui {
                    rt: Register::V0,
                    immediate: 0x801f,
                },
            ),
            (
                0xd3c0,
                Instruction::Lw {
                    rt: Register::V0,
                    base: Register::V0,
                    offset: 0x63f0,
                },
            ),
            (
                0xd3d0,
                Instruction::Addiu {
                    rt: Register::S0,
                    rs: Register::V0,
                    immediate: 512,
                },
            ),
            (
                0xd524,
                Instruction::Addiu {
                    rt: Register::A0,
                    rs: Register::S0,
                    immediate: 384,
                },
            ),
            (
                0xd528,
                Instruction::Lui {
                    rt: Register::A1,
                    immediate: 0x801f,
                },
            ),
            (
                0xd52c,
                Instruction::Ori {
                    rt: Register::A1,
                    rs: Register::A1,
                    immediate: 0x5800,
                },
            ),
            (
                0xd530,
                Instruction::Jal {
                    target: copy_runtime_address,
                },
            ),
            (
                0xd534,
                Instruction::Addiu {
                    rt: Register::A2,
                    rs: Register::ZERO,
                    immediate: 1024,
                },
            ),
        ],
        "main custom-record runtime-block caller",
    )?;
    ensure_register_is_preserved_between(
        image,
        0xd3c4,
        0xd3d0,
        Register::V0,
        "main runtime allocation pointer",
    )?;
    ensure_register_is_preserved_between(
        image,
        0xd3d4,
        0xd524,
        Register::S0,
        "main custom-record runtime-block source base",
    )
}

fn validate_main_record_seed_writer(image: &LoadedImage) -> Result<()> {
    let expected = [
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
            0xcc20,
            Instruction::Lui {
                rt: Register::A2,
                immediate: 0x801f,
            },
        ),
        (
            0xcc24,
            Instruction::Ori {
                rt: Register::A2,
                rs: Register::A2,
                immediate: 0x6520,
            },
        ),
        (
            0xcc58,
            Instruction::Lbu {
                rt: Register::V0,
                base: Register::V1,
                offset: 0,
            },
        ),
        (
            0xcc64,
            Instruction::Sb {
                rt: Register::V0,
                base: Register::A2,
                offset: 0,
            },
        ),
        (
            0xcc68,
            Instruction::Sb {
                rt: Register::ZERO,
                base: Register::A1,
                offset: -10,
            },
        ),
    ];
    ensure_sequence(image, &expected, "main RKDEMO record seeding")?;
    ensure_register_is_preserved_between(
        image,
        0xcc28,
        0xcc64,
        Register::A2,
        "main RKDEMO first selector destination",
    )?;
    ensure_register_is_preserved_between(
        image,
        0xcc1c,
        0xcc68,
        Register::A1,
        "main RKDEMO second selector destination",
    )?;
    ensure_register_is_preserved_between(
        image,
        0xcc5c,
        0xcc64,
        Register::V0,
        "main RKDEMO record selector byte",
    )
}

fn validate_runtime_buffer_copy(image: &LoadedImage, loop_offset: usize) -> Result<()> {
    let expected = [
        (
            loop_offset,
            Instruction::Beq {
                rs: Register::A0,
                rt: Register::ZERO,
                target: runtime_address(image, loop_offset + 44)?,
            },
        ),
        (
            loop_offset + 4,
            Instruction::Addu {
                rd: Register::V0,
                rs: Register::ZERO,
                rt: Register::ZERO,
            },
        ),
        (
            loop_offset + 8,
            Instruction::Blez {
                rs: Register::A2,
                target: runtime_address(image, loop_offset + 40)?,
            },
        ),
        (
            loop_offset + 12,
            Instruction::Addu {
                rd: Register::V1,
                rs: Register::A0,
                rt: Register::ZERO,
            },
        ),
        (
            loop_offset + 16,
            Instruction::Lbu {
                rt: Register::V0,
                base: Register::A0,
                offset: 0,
            },
        ),
        (
            loop_offset + 20,
            Instruction::Addiu {
                rt: Register::A0,
                rs: Register::A0,
                immediate: 1,
            },
        ),
        (
            loop_offset + 24,
            Instruction::Addiu {
                rt: Register::A2,
                rs: Register::A2,
                immediate: -1,
            },
        ),
        (
            loop_offset + 28,
            Instruction::Sb {
                rt: Register::V0,
                base: Register::A1,
                offset: 0,
            },
        ),
        (
            loop_offset + 32,
            Instruction::Bgtz {
                rs: Register::A2,
                target: runtime_address(image, loop_offset + 16)?,
            },
        ),
        (
            loop_offset + 36,
            Instruction::Addiu {
                rt: Register::A1,
                rs: Register::A1,
                immediate: 1,
            },
        ),
    ];
    ensure_sequence(image, &expected, "main runtime-buffer byte copy")
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
        "{image_path} repeats resource-selector writer profile +0x{store_offset:x}"
    );
    Ok(())
}
