use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Result, ensure};
use psx_r3000a::{Instruction, Register};

use crate::source_disc::LoadedImage;

use super::profile_validation::{
    ensure_direct_call_offsets, ensure_instruction, ensure_only_reachable_direct_entries,
    ensure_register_is_preserved_between, runtime_address,
};
use super::writer_evidence::ValidatedSelectorWriterEvidence;

#[derive(Clone, Copy)]
struct DirectZeroWriter {
    image_path: &'static str,
    store_offset: usize,
}

#[derive(Clone, Copy)]
struct DirectLiteralWriter {
    producer_offset: usize,
    store_offset: usize,
    register: Register,
    value: u8,
}

const DIRECT_ZERO_WRITERS: &[DirectZeroWriter] = &[
    zero("DAT1/KOUBAI.BIN", 0x4214),
    zero("DAT1/KOUBAI.BIN", 0x42c4),
    zero("DAT1/KOUBAI2.BIN", 0x410c),
    zero("DAT1/PLSEL2.BIN", 0x43b8),
    zero("DAT1/PLSEL3.BIN", 0x39f0),
    zero("DAT1/PLSEL4.BIN", 0x3ad4),
    zero("DAT1/PLSEL5.BIN", 0x40c4),
    zero("DAT1/PLSEL5.BIN", 0x5f38),
    zero("SLPS_021.20", 0x7760),
    zero("SLPS_021.20", 0x8030),
];

const MAIN_LITERAL_WRITERS: &[DirectLiteralWriter] = &[
    literal(0x7350, 0x7360, Register::S0, 3),
    literal(0x7438, 0x7464, Register::S0, 1),
    literal(0x7858, 0x7868, Register::S0, 3),
    literal(0x7984, 0x7994, Register::S0, 3),
    literal(0x7ab4, 0x7ac4, Register::V0, 3),
    literal(0x7b9c, 0x7bac, Register::V0, 3),
    literal(0x8110, 0x8120, Register::V0, 3),
];

const fn zero(image_path: &'static str, store_offset: usize) -> DirectZeroWriter {
    DirectZeroWriter {
        image_path,
        store_offset,
    }
}

const fn literal(
    producer_offset: usize,
    store_offset: usize,
    register: Register,
    value: u8,
) -> DirectLiteralWriter {
    DirectLiteralWriter {
        producer_offset,
        store_offset,
        register,
        value,
    }
}

pub(super) fn validated_custom_record_exam_predecessor_writer_evidence(
    image: &LoadedImage,
    reachable_instruction_offsets: &BTreeSet<usize>,
) -> Result<BTreeMap<usize, ValidatedSelectorWriterEvidence>> {
    let mut result = BTreeMap::new();
    for profile in DIRECT_ZERO_WRITERS
        .iter()
        .filter(|profile| profile.image_path == image.path)
    {
        validate_direct_store(image, profile.store_offset, Register::ZERO)?;
        insert(
            &mut result,
            profile.store_offset,
            exact_values(
                "exam_predecessor_reset",
                &[0],
                "typed direct byte store from the architectural zero register",
            ),
            &image.path,
        )?;
    }
    match image.path.as_str() {
        "DAT1/MINISEL.BIN" => {
            validate_minisel_writers(image, reachable_instruction_offsets, &mut result)?
        }
        "SLPS_021.20" => validate_main_writers(image, reachable_instruction_offsets, &mut result)?,
        _ => {}
    }
    Ok(result)
}

fn validate_minisel_writers(
    image: &LoadedImage,
    reachable_instruction_offsets: &BTreeSet<usize>,
    result: &mut BTreeMap<usize, ValidatedSelectorWriterEvidence>,
) -> Result<()> {
    let first_choice = [
        (
            0x0ae8,
            Instruction::Andi {
                rt: Register::V0,
                rs: Register::A1,
                immediate: 0x0100,
            },
        ),
        (
            0x0aec,
            Instruction::Beq {
                rs: Register::V0,
                rt: Register::ZERO,
                target: runtime_address(image, 0x0afc)?,
            },
        ),
        (0x0af0, Instruction::nop()),
        (
            0x0af4,
            Instruction::J {
                target: runtime_address(image, 0x0b00)?,
            },
        ),
        (
            0x0af8,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::ZERO,
                immediate: 1,
            },
        ),
        (
            0x0afc,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::ZERO,
                immediate: 2,
            },
        ),
    ];
    for (offset, instruction) in first_choice {
        ensure_instruction(image, offset, instruction, "MINISEL first exam-mode choice")?;
    }
    validate_direct_store(image, 0x0b0c, Register::V0)?;
    ensure_only_reachable_direct_entries(
        image,
        reachable_instruction_offsets,
        &[0x0aec],
        runtime_address(image, 0x0afc)?,
        "MINISEL first exam-mode alternative",
    )?;
    ensure_only_reachable_direct_entries(
        image,
        reachable_instruction_offsets,
        &[0x0af4],
        runtime_address(image, 0x0b00)?,
        "MINISEL first exam-mode convergence",
    )?;

    let second_choice = [
        (
            0x0b7c,
            Instruction::Andi {
                rt: Register::V0,
                rs: Register::A1,
                immediate: 0x08b0,
            },
        ),
        (
            0x0b80,
            Instruction::Bne {
                rs: Register::V0,
                rt: Register::ZERO,
                target: runtime_address(image, 0x0b8c)?,
            },
        ),
        (
            0x0b84,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::ZERO,
                immediate: 1,
            },
        ),
        (
            0x0b88,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::ZERO,
                immediate: 2,
            },
        ),
    ];
    for (offset, instruction) in second_choice {
        ensure_instruction(
            image,
            offset,
            instruction,
            "MINISEL second exam-mode choice",
        )?;
    }
    validate_direct_store(image, 0x0b98, Register::V0)?;
    ensure_only_reachable_direct_entries(
        image,
        reachable_instruction_offsets,
        &[0x0b80],
        runtime_address(image, 0x0b8c)?,
        "MINISEL second exam-mode convergence",
    )?;

    for store_offset in [0x0b0c, 0x0b98] {
        insert(
            result,
            store_offset,
            exact_values(
                "two_choice_exam_mode",
                &[1, 2],
                "validated branch alternatives select exactly one or two before the direct byte store",
            ),
            &image.path,
        )?;
    }
    Ok(())
}

fn validate_main_writers(
    image: &LoadedImage,
    reachable_instruction_offsets: &BTreeSet<usize>,
    result: &mut BTreeMap<usize, ValidatedSelectorWriterEvidence>,
) -> Result<()> {
    for profile in MAIN_LITERAL_WRITERS {
        ensure_instruction(
            image,
            profile.producer_offset,
            Instruction::Addiu {
                rt: profile.register,
                rs: Register::ZERO,
                immediate: i16::from(profile.value),
            },
            "main exam-predecessor literal",
        )?;
        ensure_register_is_preserved_between(
            image,
            profile.producer_offset + 4,
            profile.store_offset,
            profile.register,
            "main exam-predecessor literal value",
        )?;
        validate_direct_store(image, profile.store_offset, profile.register)?;
        insert(
            result,
            profile.store_offset,
            exact_values(
                "fixed_exam_mode",
                &[profile.value],
                "validated local literal producer preserved through the direct byte store",
            ),
            &image.path,
        )?;
    }

    validate_main_nonzero_pad_choice(image, reachable_instruction_offsets)?;
    insert(
        result,
        0x71e4,
        bounded_value(
            "nonzero_pad_choice",
            [1, 3],
            "the sole input sampler returns two one-bit flags and the caller excludes zero before forwarding the value",
        ),
        &image.path,
    )?;

    validate_main_one_or_three_writer(
        image,
        reachable_instruction_offsets,
        0x7564,
        0x70a0,
        &[
            (
                0x756c,
                Instruction::Addu {
                    rd: Register::S0,
                    rs: Register::A0,
                    rt: Register::ZERO,
                },
            ),
            (
                0x7570,
                Instruction::Addiu {
                    rt: Register::V0,
                    rs: Register::ZERO,
                    immediate: 3,
                },
            ),
            (
                0x7574,
                Instruction::Bne {
                    rs: Register::S0,
                    rt: Register::V0,
                    target: runtime_address(image, 0x7580)?,
                },
            ),
            (
                0x757c,
                Instruction::Addiu {
                    rt: Register::S0,
                    rs: Register::ZERO,
                    immediate: 1,
                },
            ),
        ],
        0x7594,
        Register::S0,
    )?;
    validate_main_one_or_three_writer(
        image,
        reachable_instruction_offsets,
        0x7798,
        0x70e0,
        &[
            (
                0x77a8,
                Instruction::Addu {
                    rd: Register::S0,
                    rs: Register::A0,
                    rt: Register::ZERO,
                },
            ),
            (
                0x77ac,
                Instruction::Addiu {
                    rt: Register::V0,
                    rs: Register::ZERO,
                    immediate: 3,
                },
            ),
            (
                0x77b0,
                Instruction::Bne {
                    rs: Register::S0,
                    rt: Register::V0,
                    target: runtime_address(image, 0x77bc)?,
                },
            ),
            (
                0x77b8,
                Instruction::Addiu {
                    rt: Register::S0,
                    rs: Register::ZERO,
                    immediate: 1,
                },
            ),
        ],
        0x77d8,
        Register::S0,
    )?;
    validate_main_one_or_three_writer(
        image,
        reachable_instruction_offsets,
        0x7c4c,
        0x7130,
        &[
            (
                0x7c54,
                Instruction::Addu {
                    rd: Register::S2,
                    rs: Register::A0,
                    rt: Register::ZERO,
                },
            ),
            (
                0x7c58,
                Instruction::Addiu {
                    rt: Register::V0,
                    rs: Register::ZERO,
                    immediate: 3,
                },
            ),
            (
                0x7c64,
                Instruction::Bne {
                    rs: Register::S2,
                    rt: Register::V0,
                    target: runtime_address(image, 0x7c70)?,
                },
            ),
            (
                0x7c6c,
                Instruction::Addiu {
                    rt: Register::S2,
                    rs: Register::ZERO,
                    immediate: 1,
                },
            ),
        ],
        0x7c94,
        Register::S2,
    )?;
    for store_offset in [0x7594, 0x77d8, 0x7c94] {
        insert(
            result,
            store_offset,
            exact_values(
                "normalized_exam_mode",
                &[1, 3],
                "validated branch keeps literal three and normalizes every other input to one",
            ),
            &image.path,
        )?;
    }
    Ok(())
}

fn validate_main_nonzero_pad_choice(
    image: &LoadedImage,
    reachable_instruction_offsets: &BTreeSet<usize>,
) -> Result<()> {
    let sampler_offset = 0x8554;
    let sampler_caller_offset = 0x6f60;
    ensure_direct_call_offsets(
        image,
        runtime_address(image, sampler_offset)?,
        &[sampler_caller_offset],
        "main two-pad mode sampler",
    )?;
    ensure_only_reachable_direct_entries(
        image,
        reachable_instruction_offsets,
        &[sampler_caller_offset],
        runtime_address(image, sampler_offset)?,
        "main two-pad mode sampler",
    )?;
    let sampler = [
        (
            0x8554,
            Instruction::Lui {
                rt: Register::V0,
                immediate: 0x801f,
            },
        ),
        (
            0x8558,
            Instruction::Lhu {
                rt: Register::V0,
                base: Register::V0,
                offset: 0x5e80,
            },
        ),
        (
            0x855c,
            Instruction::Lui {
                rt: Register::V1,
                immediate: 0x801f,
            },
        ),
        (
            0x8560,
            Instruction::Lhu {
                rt: Register::V1,
                base: Register::V1,
                offset: 0x5e82,
            },
        ),
        (
            0x8564,
            Instruction::Andi {
                rt: Register::V0,
                rs: Register::V0,
                immediate: 0x08b0,
            },
        ),
        (
            0x8568,
            Instruction::Andi {
                rt: Register::V1,
                rs: Register::V1,
                immediate: 0x08b0,
            },
        ),
        (
            0x856c,
            Instruction::Beq {
                rs: Register::V1,
                rt: Register::ZERO,
                target: runtime_address(image, 0x8578)?,
            },
        ),
        (
            0x8570,
            Instruction::Sltu {
                rd: Register::V0,
                rs: Register::ZERO,
                rt: Register::V0,
            },
        ),
        (
            0x8574,
            Instruction::Ori {
                rt: Register::V0,
                rs: Register::V0,
                immediate: 2,
            },
        ),
        (0x8578, Instruction::Jr { rs: Register::RA }),
        (0x857c, Instruction::nop()),
    ];
    for (offset, instruction) in sampler {
        ensure_instruction(image, offset, instruction, "main two-pad mode sampler")?;
    }

    let forwarding = [
        (
            0x6f60,
            Instruction::Jal {
                target: runtime_address(image, sampler_offset)?,
            },
        ),
        (0x6f64, Instruction::nop()),
        (
            0x6f68,
            Instruction::Addu {
                rd: Register::S0,
                rs: Register::V0,
                rt: Register::ZERO,
            },
        ),
        (
            0x6f6c,
            Instruction::Beq {
                rs: Register::S0,
                rt: Register::ZERO,
                target: runtime_address(image, 0x6e80)?,
            },
        ),
        (
            0x7080,
            Instruction::Jal {
                target: runtime_address(image, 0x71ac)?,
            },
        ),
        (
            0x7084,
            Instruction::Addu {
                rd: Register::A0,
                rs: Register::S0,
                rt: Register::ZERO,
            },
        ),
        (
            0x71b4,
            Instruction::Addu {
                rd: Register::S0,
                rs: Register::A0,
                rt: Register::ZERO,
            },
        ),
    ];
    for (offset, instruction) in forwarding {
        ensure_instruction(image, offset, instruction, "main nonzero mode forwarding")?;
    }
    ensure_direct_call_offsets(
        image,
        runtime_address(image, 0x71ac)?,
        &[0x7080],
        "main solo-mode setup",
    )?;
    ensure_only_reachable_direct_entries(
        image,
        reachable_instruction_offsets,
        &[0x7080],
        runtime_address(image, 0x71ac)?,
        "main solo-mode setup",
    )?;
    ensure_register_is_preserved_between(
        image,
        0x6f70,
        0x7084,
        Register::S0,
        "main nonzero mode value",
    )?;
    ensure_register_is_preserved_between(
        image,
        0x71b8,
        0x71e4,
        Register::S0,
        "main solo-mode value",
    )?;
    validate_direct_store(image, 0x71e4, Register::S0)
}

fn validate_main_one_or_three_writer(
    image: &LoadedImage,
    reachable_instruction_offsets: &BTreeSet<usize>,
    function_offset: usize,
    caller_offset: usize,
    expected: &[(usize, Instruction)],
    store_offset: usize,
    register: Register,
) -> Result<()> {
    let function_runtime_address = runtime_address(image, function_offset)?;
    ensure_direct_call_offsets(
        image,
        function_runtime_address,
        &[caller_offset],
        "main normalized exam-mode setup",
    )?;
    ensure_only_reachable_direct_entries(
        image,
        reachable_instruction_offsets,
        &[caller_offset],
        function_runtime_address,
        "main normalized exam-mode setup",
    )?;
    for (offset, instruction) in expected {
        ensure_instruction(
            image,
            *offset,
            instruction.clone(),
            "main normalized exam mode",
        )?;
    }
    let last_producer_offset = expected
        .last()
        .map(|(offset, _)| offset + 4)
        .expect("normalized exam-mode grammar is nonempty");
    ensure_register_is_preserved_between(
        image,
        last_producer_offset,
        store_offset,
        register,
        "main normalized exam-mode value",
    )?;
    validate_direct_store(image, store_offset, register)
}

fn validate_direct_store(image: &LoadedImage, store_offset: usize, source: Register) -> Result<()> {
    ensure_instruction(
        image,
        store_offset - 4,
        Instruction::Lui {
            rt: Register::AT,
            immediate: 0x801f,
        },
        "exam-predecessor destination address",
    )?;
    ensure_instruction(
        image,
        store_offset,
        Instruction::Sb {
            rt: source,
            base: Register::AT,
            offset: 0x6302,
        },
        "exam-predecessor byte store",
    )
}

fn exact_values(
    classification: &'static str,
    values: &[u8],
    evidence: &'static str,
) -> ValidatedSelectorWriterEvidence {
    ValidatedSelectorWriterEvidence {
        classification,
        value_resolution: "exact_candidates",
        exact_value_candidates: values.to_vec(),
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

fn insert(
    result: &mut BTreeMap<usize, ValidatedSelectorWriterEvidence>,
    store_offset: usize,
    evidence: ValidatedSelectorWriterEvidence,
    image_path: &str,
) -> Result<()> {
    ensure!(
        result.insert(store_offset, evidence).is_none(),
        "{image_path} repeats custom-record exam-predecessor writer evidence at +0x{store_offset:x}"
    );
    Ok(())
}
