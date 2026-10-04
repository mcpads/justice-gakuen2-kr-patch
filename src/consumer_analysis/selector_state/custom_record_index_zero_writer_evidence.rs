use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Result, ensure};
use psx_r3000a::{Instruction, Register};

use crate::source_disc::LoadedImage;

use super::profile_validation::{
    ensure_instruction, ensure_register_is_preserved_between, runtime_address,
};
use super::source_bound_zero_fill::validate_profiled_zero_fill_writer;
use super::writer_evidence::{ValidatedSelectorWriterEvidence, validate_kanri_custom_index_source};

#[derive(Clone, Copy)]
struct GlobalByteCopyWriter {
    load_offset: usize,
    store_offset: usize,
}

const SIKEN_GLOBAL_BYTE_COPY_WRITERS: &[GlobalByteCopyWriter] = &[
    GlobalByteCopyWriter {
        load_offset: 0x0e4c,
        store_offset: 0x0e60,
    },
    GlobalByteCopyWriter {
        load_offset: 0x0e9c,
        store_offset: 0x0eb4,
    },
];

const SIKEN2_GLOBAL_BYTE_COPY_WRITERS: &[GlobalByteCopyWriter] = &[
    GlobalByteCopyWriter {
        load_offset: 0x0afc,
        store_offset: 0x0b10,
    },
    GlobalByteCopyWriter {
        load_offset: 0x0b4c,
        store_offset: 0x0b64,
    },
];

pub(super) fn validated_custom_record_index_zero_writer_evidence(
    image: &LoadedImage,
    reachable_instruction_offsets: &BTreeSet<usize>,
) -> Result<BTreeMap<usize, ValidatedSelectorWriterEvidence>> {
    let mut evidence = BTreeMap::new();
    match image.path.as_str() {
        "DAT1/KANRI.BIN" => {
            validate_kanri_writers(image, reachable_instruction_offsets, &mut evidence)?
        }
        "DAT1/MGAME.BIN" => validate_mgame_writers(image, &mut evidence)?,
        "DAT1/MGAME01.BIN" => validate_zero_fill_writer(image, 0x6308, &mut evidence)?,
        "DAT1/MGAME02.BIN" => validate_zero_fill_writer(image, 0x6078, &mut evidence)?,
        "DAT1/MGAME03.BIN" => validate_zero_fill_writer(image, 0x5e28, &mut evidence)?,
        "DAT1/MGAME05.BIN" => validate_zero_fill_writer(image, 0x4ee4, &mut evidence)?,
        "DAT1/MGAME06.BIN" => validate_zero_fill_writer(image, 0x756c, &mut evidence)?,
        "DAT1/MGAME07.BIN" => validate_zero_fill_writer(image, 0x68f8, &mut evidence)?,
        "DAT1/MGTIT.BIN" => validate_mgtit_exam_source_writer(image, &mut evidence)?,
        "DAT1/SIKEN.BIN" => {
            validate_global_byte_copy_writers(image, SIKEN_GLOBAL_BYTE_COPY_WRITERS, &mut evidence)?
        }
        "DAT1/SIKEN2.BIN" => validate_global_byte_copy_writers(
            image,
            SIKEN2_GLOBAL_BYTE_COPY_WRITERS,
            &mut evidence,
        )?,
        _ => {}
    }
    Ok(evidence)
}

fn validate_kanri_writers(
    image: &LoadedImage,
    reachable_instruction_offsets: &BTreeSet<usize>,
    result: &mut BTreeMap<usize, ValidatedSelectorWriterEvidence>,
) -> Result<()> {
    validate_kanri_custom_index_source(image, reachable_instruction_offsets)?;
    validate_kanri_index_limit_writer(image)?;
    insert(
        result,
        0x6848,
        runtime_source(
            "decremented_runtime_word_low_byte",
            vec![[0x801f_1a1c, 0x801f_1a20]],
            "the first bounded loop slot writes the low byte of the runtime word at 0x801f1a1c minus one",
        ),
        &image.path,
    )?;
    validate_zero_fill_writer(image, 0x8238, result)
}

fn validate_kanri_index_limit_writer(image: &LoadedImage) -> Result<()> {
    ensure_register_is_preserved_between(
        image,
        0x6190,
        0x682c,
        Register::S0,
        "KANRI custom-index state root",
    )?;
    let expected = [
        Instruction::Addu {
            rd: Register::A0,
            rs: Register::ZERO,
            rt: Register::ZERO,
        },
        Instruction::Addu {
            rd: Register::A1,
            rs: Register::S0,
            rt: Register::ZERO,
        },
        Instruction::Lh {
            rt: Register::V0,
            base: Register::A1,
            offset: 14,
        },
        Instruction::Lw {
            rt: Register::V1,
            base: Register::S0,
            offset: 28,
        },
        Instruction::nop(),
        Instruction::Slt {
            rd: Register::V0,
            rs: Register::V0,
            rt: Register::V1,
        },
        Instruction::Bne {
            rs: Register::V0,
            rt: Register::ZERO,
            target: runtime_address(image, 0x684c)?,
        },
        Instruction::Addiu {
            rt: Register::V0,
            rs: Register::V1,
            immediate: -1,
        },
        Instruction::Sh {
            rt: Register::V0,
            base: Register::A1,
            offset: 14,
        },
        Instruction::Addiu {
            rt: Register::A0,
            rs: Register::A0,
            immediate: 1,
        },
        Instruction::Slti {
            rt: Register::V0,
            rs: Register::A0,
            immediate: 2,
        },
        Instruction::Bne {
            rs: Register::V0,
            rt: Register::ZERO,
            target: runtime_address(image, 0x6830)?,
        },
        Instruction::Addiu {
            rt: Register::A1,
            rs: Register::A1,
            immediate: 2,
        },
    ];
    for (index, instruction) in expected.into_iter().enumerate() {
        ensure_instruction(
            image,
            0x6828 + index * 4,
            instruction,
            "KANRI custom-index limit writer",
        )?;
    }
    Ok(())
}

fn validate_mgame_writers(
    image: &LoadedImage,
    result: &mut BTreeMap<usize, ValidatedSelectorWriterEvidence>,
) -> Result<()> {
    validate_zero_fill_writer(image, 0x27ff0, result)?;
    let expected = [
        Instruction::Lui {
            rt: Register::V0,
            immediate: 0x801f,
        },
        Instruction::Lbu {
            rt: Register::V0,
            base: Register::V0,
            offset: 0x6302,
        },
    ];
    for (index, instruction) in expected.into_iter().enumerate() {
        ensure_instruction(
            image,
            0x3d1c + index * 4,
            instruction,
            "MGAME exam-state predecessor load",
        )?;
    }
    ensure_register_is_preserved_between(
        image,
        0x3d24,
        0x3d2c,
        Register::V0,
        "MGAME exam-state predecessor",
    )?;
    ensure_instruction(
        image,
        0x3d2c,
        Instruction::Addiu {
            rt: Register::V0,
            rs: Register::V0,
            immediate: -1,
        },
        "MGAME exam-state predecessor decrement",
    )?;
    validate_direct_byte_store(image, 0x3d34, 0x180a, Register::V0)?;
    insert(
        result,
        0x3d34,
        runtime_source(
            "decremented_runtime_byte",
            vec![[0x801f_6302, 0x801f_6303]],
            "loads the exact predecessor byte, decrements it once, and stores the result",
        ),
        &image.path,
    )
}

fn validate_mgtit_exam_source_writer(
    image: &LoadedImage,
    result: &mut BTreeMap<usize, ValidatedSelectorWriterEvidence>,
) -> Result<()> {
    ensure_instruction(
        image,
        0x24cc,
        Instruction::Lui {
            rt: Register::S0,
            immediate: 0x801f,
        },
        "MGTIT exam-state source address",
    )?;
    ensure_instruction(
        image,
        0x24d0,
        Instruction::Lbu {
            rt: Register::S0,
            base: Register::S0,
            offset: 0x180a,
        },
        "MGTIT exam-state source load",
    )?;
    ensure_instruction(
        image,
        0x24d4,
        Instruction::Jal {
            target: runtime_address(image, 0x287c)?,
        },
        "MGTIT exam-state intervening call",
    )?;
    ensure_instruction(
        image,
        0x2888,
        Instruction::Sw {
            rt: Register::S0,
            base: Register::SP,
            offset: 16,
        },
        "MGTIT callee saved-register prologue",
    )?;
    ensure_instruction(
        image,
        0x2904,
        Instruction::Lw {
            rt: Register::S0,
            base: Register::SP,
            offset: 16,
        },
        "MGTIT callee saved-register epilogue",
    )?;
    ensure_register_is_preserved_between(
        image,
        0x24d4,
        0x24e0,
        Register::S0,
        "MGTIT exam-state value",
    )?;
    validate_direct_byte_store(image, 0x24e0, 0x180a, Register::S0)?;
    insert(
        result,
        0x24e0,
        runtime_source(
            "global_runtime_byte",
            vec![[0x801f_180a, 0x801f_180b]],
            "loads and restores the same global byte around one callee-preserved call",
        ),
        &image.path,
    )
}

fn validate_zero_fill_writer(
    image: &LoadedImage,
    store_offset: usize,
    result: &mut BTreeMap<usize, ValidatedSelectorWriterEvidence>,
) -> Result<()> {
    validate_profiled_zero_fill_writer(image, store_offset)?;
    insert(
        result,
        store_offset,
        exact_value(
            "zero_fill",
            0,
            "validated byte-wise zero-fill loop, complete direct caller and byte-count set, and fixed state-block footprints",
        ),
        &image.path,
    )
}

fn validate_global_byte_copy_writers(
    image: &LoadedImage,
    writers: &[GlobalByteCopyWriter],
    result: &mut BTreeMap<usize, ValidatedSelectorWriterEvidence>,
) -> Result<()> {
    for writer in writers {
        ensure_instruction(
            image,
            writer.load_offset - 4,
            Instruction::Lui {
                rt: Register::V0,
                immediate: 0x801f,
            },
            "global-byte copy source address",
        )?;
        ensure_instruction(
            image,
            writer.load_offset,
            Instruction::Lbu {
                rt: Register::V0,
                base: Register::V0,
                offset: 0x180a,
            },
            "global-byte copy source load",
        )?;
        ensure_register_is_preserved_between(
            image,
            writer.load_offset + 4,
            writer.store_offset,
            Register::V0,
            "global-byte copy value",
        )?;
        validate_direct_byte_store(image, writer.store_offset, 0x1a0e, Register::V0)?;
        insert(
            result,
            writer.store_offset,
            runtime_source(
                "global_runtime_byte",
                vec![[0x801f_180a, 0x801f_180b]],
                "validated direct global-byte load preserved through the destination store",
            ),
            &image.path,
        )?;
    }
    Ok(())
}

fn validate_direct_byte_store(
    image: &LoadedImage,
    store_offset: usize,
    destination_low: u16,
    source: Register,
) -> Result<()> {
    ensure_instruction(
        image,
        store_offset - 4,
        Instruction::Lui {
            rt: Register::AT,
            immediate: 0x801f,
        },
        "custom-record index-zero direct-store address",
    )?;
    ensure_instruction(
        image,
        store_offset,
        Instruction::Sb {
            rt: source,
            base: Register::AT,
            offset: destination_low as i16,
        },
        "custom-record index-zero direct byte store",
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
        "{image_path} repeats custom-record index-zero writer profile +0x{store_offset:x}"
    );
    Ok(())
}
