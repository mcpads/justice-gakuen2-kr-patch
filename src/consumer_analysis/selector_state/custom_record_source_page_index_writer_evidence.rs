use std::collections::BTreeMap;

use anyhow::{Result, ensure};
use psx_r3000a::{Instruction, Register};

use crate::source_disc::LoadedImage;

use super::profile_validation::{ensure_instruction, ensure_register_is_preserved_between};
use super::writer_evidence::ValidatedSelectorWriterEvidence;

#[derive(Clone, Copy)]
struct PracticalResultPageIndexWriterProfile {
    image_path: &'static str,
    literal_store_offset: usize,
    increment_load_offset: usize,
    increment_store_offset: usize,
}

const PRACTICAL_RESULT_PAGE_INDEX_WRITERS: &[PracticalResultPageIndexWriterProfile] = &[
    PracticalResultPageIndexWriterProfile {
        image_path: "DAT1/SIKENG21.BIN",
        literal_store_offset: 0x3de8,
        increment_load_offset: 0x3ee4,
        increment_store_offset: 0x3efc,
    },
    PracticalResultPageIndexWriterProfile {
        image_path: "DAT1/SIKENG22.BIN",
        literal_store_offset: 0x3de8,
        increment_load_offset: 0x3ee4,
        increment_store_offset: 0x3efc,
    },
    PracticalResultPageIndexWriterProfile {
        image_path: "DAT1/SIKENGO1.BIN",
        literal_store_offset: 0x47d0,
        increment_load_offset: 0x48cc,
        increment_store_offset: 0x48e4,
    },
    PracticalResultPageIndexWriterProfile {
        image_path: "DAT1/SIKENGO2.BIN",
        literal_store_offset: 0x47d0,
        increment_load_offset: 0x48cc,
        increment_store_offset: 0x48e4,
    },
];

pub(super) fn validated_custom_record_source_page_index_writer_evidence(
    image: &LoadedImage,
) -> Result<BTreeMap<usize, ValidatedSelectorWriterEvidence>> {
    let mut result = BTreeMap::new();
    if image.path == "DAT1/MGENT.BIN" {
        validate_mgent_indexed_writer(image, &mut result)?;
    }
    if let Some(profile) = PRACTICAL_RESULT_PAGE_INDEX_WRITERS
        .iter()
        .find(|profile| profile.image_path == image.path)
    {
        validate_practical_result_writers(image, profile, &mut result)?;
    }
    Ok(result)
}

fn validate_mgent_indexed_writer(
    image: &LoadedImage,
    result: &mut BTreeMap<usize, ValidatedSelectorWriterEvidence>,
) -> Result<()> {
    let expected = [
        (
            0x17c4,
            Instruction::Lui {
                rt: Register::A0,
                immediate: 0x801f,
            },
        ),
        (
            0x17c8,
            Instruction::Lui {
                rt: Register::V0,
                immediate: 0x801f,
            },
        ),
        (
            0x17cc,
            Instruction::Lbu {
                rt: Register::V0,
                base: Register::V0,
                offset: 0x180a,
            },
        ),
        (
            0x17d0,
            Instruction::Ori {
                rt: Register::A0,
                rs: Register::A0,
                immediate: 0x5e00,
            },
        ),
        (
            0x17d8,
            Instruction::Sll {
                rd: Register::V0,
                rt: Register::V0,
                shift: 1,
            },
        ),
        (
            0x17e8,
            Instruction::Addu {
                rd: Register::V0,
                rs: Register::V0,
                rt: Register::A0,
            },
        ),
        (
            0x17f4,
            Instruction::Lhu {
                rt: Register::V0,
                base: Register::V0,
                offset: 0,
            },
        ),
        (
            0x1800,
            Instruction::Lui {
                rt: Register::AT,
                immediate: 0x801f,
            },
        ),
        (
            0x1804,
            Instruction::Sh {
                rt: Register::V0,
                base: Register::AT,
                offset: 0x1a04,
            },
        ),
    ];
    for (offset, instruction) in expected {
        ensure_instruction(image, offset, instruction, "MGENT source-page index writer")?;
    }
    ensure_register_is_preserved_between(
        image,
        0x17d4,
        0x17e8,
        Register::A0,
        "MGENT source-page table base",
    )?;
    ensure_register_is_preserved_between(
        image,
        0x17f8,
        0x1804,
        Register::V0,
        "MGENT source-page table value",
    )?;
    insert(
        result,
        0x1804,
        runtime_source(
            "exam_indexed_runtime_halfword",
            vec![[0x801f_180a, 0x801f_180b], [0x801f_5e00, 0x801f_6000]],
            "uses the unsigned exam-state byte to select one halfword from the exact 256-entry runtime table",
        ),
        &image.path,
    )
}

fn validate_practical_result_writers(
    image: &LoadedImage,
    profile: &PracticalResultPageIndexWriterProfile,
    result: &mut BTreeMap<usize, ValidatedSelectorWriterEvidence>,
) -> Result<()> {
    ensure_instruction(
        image,
        profile.literal_store_offset - 8,
        Instruction::Addiu {
            rt: Register::V0,
            rs: Register::ZERO,
            immediate: 30,
        },
        "practical-result source-page literal",
    )?;
    validate_direct_halfword_store(image, profile.literal_store_offset, Register::V0)?;
    insert(
        result,
        profile.literal_store_offset,
        exact_value(
            "literal_30",
            30,
            "loads literal 30 and stores it directly to the source-page index halfword",
        ),
        &image.path,
    )?;

    ensure_instruction(
        image,
        profile.increment_load_offset - 4,
        Instruction::Lui {
            rt: Register::V0,
            immediate: 0x801f,
        },
        "practical-result source-page increment address",
    )?;
    ensure_instruction(
        image,
        profile.increment_load_offset,
        Instruction::Lhu {
            rt: Register::V0,
            base: Register::V0,
            offset: 0x1a04,
        },
        "practical-result source-page increment load",
    )?;
    ensure_instruction(
        image,
        profile.increment_store_offset - 12,
        Instruction::Addiu {
            rt: Register::V0,
            rs: Register::V0,
            immediate: 1,
        },
        "practical-result source-page increment",
    )?;
    ensure_register_is_preserved_between(
        image,
        profile.increment_load_offset + 4,
        profile.increment_store_offset - 12,
        Register::V0,
        "practical-result source-page prior value",
    )?;
    ensure_register_is_preserved_between(
        image,
        profile.increment_store_offset - 8,
        profile.increment_store_offset,
        Register::V0,
        "practical-result source-page incremented value",
    )?;
    validate_direct_halfword_store(image, profile.increment_store_offset, Register::V0)?;
    insert(
        result,
        profile.increment_store_offset,
        runtime_source(
            "incremented_runtime_halfword",
            vec![[0x801f_1a04, 0x801f_1a06]],
            "loads the exact source-page index halfword, increments it once, and stores it back",
        ),
        &image.path,
    )
}

fn validate_direct_halfword_store(
    image: &LoadedImage,
    store_offset: usize,
    source: Register,
) -> Result<()> {
    ensure_instruction(
        image,
        store_offset - 4,
        Instruction::Lui {
            rt: Register::AT,
            immediate: 0x801f,
        },
        "source-page index destination address",
    )?;
    ensure_instruction(
        image,
        store_offset,
        Instruction::Sh {
            rt: source,
            base: Register::AT,
            offset: 0x1a04,
        },
        "source-page index halfword store",
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
        "{image_path} repeats source-page index writer evidence at +0x{store_offset:x}"
    );
    Ok(())
}
