use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Result, ensure};
use psx_r3000a::{Instruction, Register};

use crate::source_disc::LoadedImage;

use super::profile_validation::{
    ensure_instruction, ensure_only_reachable_direct_entries, ensure_register_is_preserved_between,
    runtime_address,
};
use super::writer_evidence::ValidatedSelectorWriterEvidence;

#[derive(Clone, Copy)]
struct GlobalByteCopyProfile {
    source_lui_offset: usize,
    source_load_offset: usize,
    source_low: u16,
    store_address_lui_offset: Option<usize>,
    store_offset: usize,
    source_register: Register,
    destination_base_register: Register,
    destination_displacement: i16,
}

pub(super) fn validated_custom_record_writer_evidence(
    image: &LoadedImage,
    reachable_instruction_offsets: &BTreeSet<usize>,
) -> Result<BTreeMap<usize, ValidatedSelectorWriterEvidence>> {
    let mut result = BTreeMap::new();
    match image.path.as_str() {
        "DAT1/KANRI.BIN" => validate_kanri_writers(image, &mut result)?,
        "DAT1/MGAME06.BIN" => validate_mgame06_writers(image, &mut result)?,
        "DAT1/SIKEN.BIN" => {
            validate_siken_writers(image, reachable_instruction_offsets, &mut result)?
        }
        "DAT1/SIKEN2.BIN" => validate_siken2_writers(image, &mut result)?,
        "DAT1/SKHAY.BIN" => validate_skhay_writers(image, &mut result)?,
        _ => {}
    }
    Ok(result)
}

fn validate_kanri_writers(
    image: &LoadedImage,
    result: &mut BTreeMap<usize, ValidatedSelectorWriterEvidence>,
) -> Result<()> {
    for profile in [
        global_copy(0x5fd4, 0x5fd8, 0x185f, 0x6078, 0x607c, Register::A0, 0x5a84),
        global_copy(0x5fdc, 0x5fe0, 0x18dd, 0x6080, 0x6084, Register::A1, 0x5a85),
        global_copy(0x5fe4, 0x5fe8, 0x1854, 0x6090, 0x6094, Register::A2, 0x5a87),
    ] {
        insert_global_byte_copy(image, result, &profile)?;
    }
    Ok(())
}

fn validate_mgame06_writers(
    image: &LoadedImage,
    result: &mut BTreeMap<usize, ValidatedSelectorWriterEvidence>,
) -> Result<()> {
    let fixed_secondary = [
        (
            0x0d04,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::ZERO,
                immediate: 33,
            },
        ),
        (
            0x0d2c,
            Instruction::Lui {
                rt: Register::AT,
                immediate: 0x801f,
            },
        ),
        (
            0x0d30,
            Instruction::Sb {
                rt: Register::V0,
                base: Register::AT,
                offset: 0x5a85,
            },
        ),
    ];
    ensure_sequence(image, &fixed_secondary, "MGAME06 fixed record field")?;
    ensure_register_is_preserved_between(
        image,
        0x0d08,
        0x0d30,
        Register::V0,
        "MGAME06 fixed record field",
    )?;
    insert(
        result,
        0x0d30,
        exact_candidates(
            "fixed_literal",
            vec![33],
            "validated literal producer preserved through the direct custom-record byte store",
        ),
        &image.path,
    )?;

    let runtime_structure = [
        (
            0x0cac,
            Instruction::Lui {
                rt: Register::V0,
                immediate: 0x800b,
            },
        ),
        (
            0x0cb0,
            Instruction::Lw {
                rt: Register::V0,
                base: Register::V0,
                offset: -24916,
            },
        ),
        (
            0x0cb8,
            Instruction::Lbu {
                rt: Register::A0,
                base: Register::V0,
                offset: 16,
            },
        ),
        (
            0x0cc0,
            Instruction::Addu {
                rd: Register::V0,
                rs: Register::V0,
                rt: Register::A0,
            },
        ),
        (
            0x0cc8,
            Instruction::Lb {
                rt: Register::A1,
                base: Register::V0,
                offset: 92,
            },
        ),
        (
            0x0d3c,
            Instruction::Lui {
                rt: Register::AT,
                immediate: 0x801f,
            },
        ),
        (
            0x0d40,
            Instruction::Sb {
                rt: Register::A1,
                base: Register::AT,
                offset: 0x5a84,
            },
        ),
    ];
    ensure_sequence(
        image,
        &runtime_structure,
        "MGAME06 runtime-structure record field",
    )?;
    ensure_register_is_preserved_between(
        image,
        0x0ccc,
        0x0d40,
        Register::A1,
        "MGAME06 runtime-structure record field",
    )?;
    insert(
        result,
        0x0d40,
        runtime_source(
            "runtime_structure_byte",
            Vec::new(),
            "validated signed byte load through a runtime-selected structure and direct custom-record byte store",
        ),
        &image.path,
    )?;

    let indexed_table = [
        (
            0x0d44,
            Instruction::Lui {
                rt: Register::AT,
                immediate: 0x800a,
            },
        ),
        (
            0x0d48,
            Instruction::Addu {
                rd: Register::AT,
                rs: Register::AT,
                rt: Register::A1,
            },
        ),
        (
            0x0d4c,
            Instruction::Lbu {
                rt: Register::V1,
                base: Register::AT,
                offset: 0x203c,
            },
        ),
        (
            0x0dc4,
            Instruction::Lui {
                rt: Register::AT,
                immediate: 0x801f,
            },
        ),
        (
            0x0dc8,
            Instruction::Sb {
                rt: Register::V1,
                base: Register::AT,
                offset: 0x5a87,
            },
        ),
    ];
    ensure_sequence(
        image,
        &indexed_table,
        "MGAME06 runtime-indexed record field",
    )?;
    ensure_register_is_preserved_between(
        image,
        0x0d50,
        0x0dc8,
        Register::V1,
        "MGAME06 runtime-indexed record field",
    )?;
    insert(
        result,
        0x0dc8,
        runtime_source(
            "runtime_indexed_embedded_byte",
            Vec::new(),
            "validated embedded-table byte load indexed by the runtime structure field and direct custom-record byte store",
        ),
        &image.path,
    )
}

fn validate_siken_writers(
    image: &LoadedImage,
    reachable_instruction_offsets: &BTreeSet<usize>,
    result: &mut BTreeMap<usize, ValidatedSelectorWriterEvidence>,
) -> Result<()> {
    validate_record_pointer(image, 0x10f4, 0x114c, Register::T8)?;
    for profile in [
        pointer_global_copy(
            0x11d8,
            0x11dc,
            0x185f,
            0x124c,
            Register::V0,
            Register::T8,
            0,
        ),
        pointer_global_copy(
            0x11e0,
            0x11e4,
            0x18dd,
            0x1250,
            Register::V1,
            Register::T8,
            1,
        ),
        pointer_global_copy(
            0x11f0,
            0x11f4,
            0x1854,
            0x1258,
            Register::A1,
            Register::T8,
            3,
        ),
    ] {
        insert_global_byte_copy(image, result, &profile)?;
    }
    ensure_register_is_preserved_between(
        image,
        0x1150,
        0x1358,
        Register::T8,
        "SIKEN custom-record pointer",
    )?;

    let rewrite = [
        (
            0x133c,
            Instruction::Beq {
                rs: Register::V1,
                rt: Register::V0,
                target: runtime_address(image, 0x1354)?,
            },
        ),
        (
            0x1340,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::ZERO,
                immediate: 12,
            },
        ),
        (
            0x1344,
            Instruction::Beq {
                rs: Register::V1,
                rt: Register::V0,
                target: runtime_address(image, 0x1358)?,
            },
        ),
        (
            0x1348,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::ZERO,
                immediate: 10,
            },
        ),
        (
            0x134c,
            Instruction::J {
                target: runtime_address(image, 0x135c)?,
            },
        ),
        (0x1350, Instruction::nop()),
        (
            0x1354,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::ZERO,
                immediate: 8,
            },
        ),
        (
            0x1358,
            Instruction::Sb {
                rt: Register::V0,
                base: Register::T8,
                offset: 1,
            },
        ),
    ];
    ensure_sequence(image, &rewrite, "SIKEN conditional record-field rewrite")?;
    ensure_only_reachable_direct_entries(
        image,
        reachable_instruction_offsets,
        &[0x133c],
        runtime_address(image, 0x1354)?,
        "SIKEN literal-eight record-field block",
    )?;
    ensure_only_reachable_direct_entries(
        image,
        reachable_instruction_offsets,
        &[0x1344],
        runtime_address(image, 0x1358)?,
        "SIKEN conditional record-field store",
    )?;
    insert(
        result,
        0x1358,
        exact_candidates(
            "conditional_literal_rewrite",
            vec![8, 10],
            "validated complete local branch grammar: source byte seven becomes eight and source byte twelve becomes ten; all other values bypass the store",
        ),
        &image.path,
    )
}

fn validate_siken2_writers(
    image: &LoadedImage,
    result: &mut BTreeMap<usize, ValidatedSelectorWriterEvidence>,
) -> Result<()> {
    for profile in [
        global_copy(0x0e60, 0x0e64, 0x185f, 0x0e8c, 0x0e90, Register::V0, 0x5a84),
        global_copy(0x0e68, 0x0e6c, 0x18dd, 0x0e94, 0x0e98, Register::V1, 0x5a85),
        global_copy(0x0e78, 0x0e7c, 0x1854, 0x0ea4, 0x0ea8, Register::A1, 0x5a87),
    ] {
        insert_global_byte_copy(image, result, &profile)?;
    }
    Ok(())
}

fn validate_skhay_writers(
    image: &LoadedImage,
    result: &mut BTreeMap<usize, ValidatedSelectorWriterEvidence>,
) -> Result<()> {
    validate_record_pointer(image, 0x0220, 0x0224, Register::T7)?;
    for profile in [
        pointer_global_copy(
            0x03dc,
            0x03e0,
            0x18dd,
            0x0418,
            Register::V0,
            Register::T7,
            1,
        ),
        pointer_global_copy(
            0x03ec,
            0x03f0,
            0x1854,
            0x0420,
            Register::A0,
            Register::T7,
            3,
        ),
    ] {
        insert_global_byte_copy(image, result, &profile)?;
    }
    ensure_register_is_preserved_between(
        image,
        0x0228,
        0x0424,
        Register::T7,
        "SKHAY custom-record pointer",
    )?;

    let indexed_table = [
        (
            0x035c,
            Instruction::Lui {
                rt: Register::A0,
                immediate: 0x800a,
            },
        ),
        (
            0x0360,
            Instruction::Addiu {
                rt: Register::A0,
                rs: Register::A0,
                immediate: 0x2010,
            },
        ),
        (
            0x0374,
            Instruction::Lui {
                rt: Register::V0,
                immediate: 0x801f,
            },
        ),
        (
            0x0378,
            Instruction::Lbu {
                rt: Register::V0,
                base: Register::V0,
                offset: 0x185a,
            },
        ),
        (
            0x037c,
            Instruction::Lui {
                rt: Register::V1,
                immediate: 0x801f,
            },
        ),
        (
            0x0380,
            Instruction::Lbu {
                rt: Register::V1,
                base: Register::V1,
                offset: 0x1854,
            },
        ),
        (
            0x03cc,
            Instruction::Sll {
                rd: Register::V0,
                rt: Register::V0,
                shift: 1,
            },
        ),
        (
            0x03d0,
            Instruction::Addu {
                rd: Register::V0,
                rs: Register::V0,
                rt: Register::A0,
            },
        ),
        (
            0x03d4,
            Instruction::Addu {
                rd: Register::V0,
                rs: Register::V0,
                rt: Register::V1,
            },
        ),
        (
            0x03d8,
            Instruction::Lbu {
                rt: Register::T6,
                base: Register::V0,
                offset: 0,
            },
        ),
        (
            0x0424,
            Instruction::Sb {
                rt: Register::T6,
                base: Register::T7,
                offset: 0,
            },
        ),
    ];
    ensure_sequence(image, &indexed_table, "SKHAY runtime-indexed record field")?;
    ensure_register_is_preserved_between(
        image,
        0x03dc,
        0x0424,
        Register::T6,
        "SKHAY runtime-indexed record field",
    )?;
    insert(
        result,
        0x0424,
        runtime_source(
            "runtime_indexed_embedded_byte",
            vec![[0x801f_1854, 0x801f_1855], [0x801f_185a, 0x801f_185b]],
            "validated embedded-table byte load indexed by two runtime global bytes and direct custom-record byte store",
        ),
        &image.path,
    )
}

fn global_copy(
    source_lui_offset: usize,
    source_load_offset: usize,
    source_low: u16,
    store_address_lui_offset: usize,
    store_offset: usize,
    source_register: Register,
    destination_low: u16,
) -> GlobalByteCopyProfile {
    GlobalByteCopyProfile {
        source_lui_offset,
        source_load_offset,
        source_low,
        store_address_lui_offset: Some(store_address_lui_offset),
        store_offset,
        source_register,
        destination_base_register: Register::AT,
        destination_displacement: destination_low as i16,
    }
}

fn pointer_global_copy(
    source_lui_offset: usize,
    source_load_offset: usize,
    source_low: u16,
    store_offset: usize,
    source_register: Register,
    destination_base_register: Register,
    destination_displacement: i16,
) -> GlobalByteCopyProfile {
    GlobalByteCopyProfile {
        source_lui_offset,
        source_load_offset,
        source_low,
        store_address_lui_offset: None,
        store_offset,
        source_register,
        destination_base_register,
        destination_displacement,
    }
}

fn insert_global_byte_copy(
    image: &LoadedImage,
    result: &mut BTreeMap<usize, ValidatedSelectorWriterEvidence>,
    profile: &GlobalByteCopyProfile,
) -> Result<()> {
    ensure_instruction(
        image,
        profile.source_lui_offset,
        Instruction::Lui {
            rt: profile.source_register,
            immediate: 0x801f,
        },
        "custom-record source address",
    )?;
    ensure_instruction(
        image,
        profile.source_load_offset,
        Instruction::Lbu {
            rt: profile.source_register,
            base: profile.source_register,
            offset: profile.source_low as i16,
        },
        "custom-record source byte",
    )?;
    ensure_register_is_preserved_between(
        image,
        profile.source_load_offset + 4,
        profile.store_offset,
        profile.source_register,
        "custom-record source byte",
    )?;
    if let Some(lui_offset) = profile.store_address_lui_offset {
        ensure_instruction(
            image,
            lui_offset,
            Instruction::Lui {
                rt: profile.destination_base_register,
                immediate: 0x801f,
            },
            "custom-record destination address",
        )?;
    }
    ensure_instruction(
        image,
        profile.store_offset,
        Instruction::Sb {
            rt: profile.source_register,
            base: profile.destination_base_register,
            offset: profile.destination_displacement,
        },
        "custom-record destination byte",
    )?;
    insert(
        result,
        profile.store_offset,
        runtime_source(
            "global_runtime_byte",
            vec![[
                0x801f_0000 + u32::from(profile.source_low),
                0x801f_0001 + u32::from(profile.source_low),
            ]],
            "validated direct global-byte load preserved through the custom-record byte store",
        ),
        &image.path,
    )
}

fn validate_record_pointer(
    image: &LoadedImage,
    lui_offset: usize,
    low_offset: usize,
    register: Register,
) -> Result<()> {
    ensure_instruction(
        image,
        lui_offset,
        Instruction::Lui {
            rt: register,
            immediate: 0x801f,
        },
        "custom-record pointer high half",
    )?;
    ensure_instruction(
        image,
        low_offset,
        Instruction::Ori {
            rt: register,
            rs: register,
            immediate: 0x5a84,
        },
        "custom-record pointer low half",
    )
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

fn exact_candidates(
    classification: &'static str,
    values: Vec<u8>,
    evidence: &'static str,
) -> ValidatedSelectorWriterEvidence {
    ValidatedSelectorWriterEvidence {
        classification,
        value_resolution: "exact_candidates",
        exact_value_candidates: values,
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
        "{image_path} repeats custom-record writer profile +0x{store_offset:x}"
    );
    Ok(())
}
