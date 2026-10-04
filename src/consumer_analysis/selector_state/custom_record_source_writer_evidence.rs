use std::collections::BTreeMap;

use anyhow::{Result, ensure};
use psx_r3000a::{Instruction, Register};

use crate::source_disc::LoadedImage;

use super::profile_validation::{
    ensure_direct_call_offsets, ensure_instruction, ensure_register_is_preserved_between,
    runtime_address,
};
use super::writer_evidence::ValidatedSelectorWriterEvidence;

#[derive(Clone, Copy)]
struct GlobalByteCopyProfile {
    source_lui_offset: usize,
    source_load_offset: usize,
    source_low: u16,
    store_lui_offset: usize,
    store_offset: usize,
    destination_low: u16,
    register: Register,
}

pub(super) fn validated_custom_record_source_writer_evidence(
    image: &LoadedImage,
) -> Result<BTreeMap<usize, ValidatedSelectorWriterEvidence>> {
    let mut result = BTreeMap::new();
    match image.path.as_str() {
        "DAT1/MGAME.BIN" => validate_mgame_writers(image, &mut result)?,
        "DAT1/MGENT.BIN" => validate_mgent_writers(image, &mut result)?,
        "DAT1/MGTIT.BIN" => validate_mgtit_writer(image, &mut result)?,
        _ => {}
    }
    Ok(result)
}

fn validate_mgame_writers(
    image: &LoadedImage,
    result: &mut BTreeMap<usize, ValidatedSelectorWriterEvidence>,
) -> Result<()> {
    validate_mgame_primary_fallback_predecessor_writer(image, result)?;
    validate_mgame_primary_fallback_global_writers(image, result)?;
    for profile in [
        GlobalByteCopyProfile {
            source_lui_offset: 0x90f8,
            source_load_offset: 0x90fc,
            source_low: 0x1bc6,
            store_lui_offset: 0x912c,
            store_offset: 0x9130,
            destination_low: 0x185a,
            register: Register::A1,
        },
        GlobalByteCopyProfile {
            source_lui_offset: 0x9100,
            source_load_offset: 0x9104,
            source_low: 0x1bc7,
            store_lui_offset: 0x9134,
            store_offset: 0x9138,
            destination_low: 0x1854,
            register: Register::A2,
        },
    ] {
        validate_global_byte_copy(image, &profile)?;
        insert(
            result,
            profile.store_offset,
            runtime_source(
                "global_runtime_byte",
                vec![[
                    0x801f_0000 + u32::from(profile.source_low),
                    0x801f_0001 + u32::from(profile.source_low),
                ]],
                "validated direct global-byte load preserved through the custom-record source store",
            ),
            &image.path,
        )?;
    }

    ensure_sequence(
        image,
        &[
            (
                0xcd64,
                Instruction::Lui {
                    rt: Register::AT,
                    immediate: 0x801f,
                },
            ),
            (
                0xcd68,
                Instruction::Sb {
                    rt: Register::ZERO,
                    base: Register::AT,
                    offset: 0x18dd,
                },
            ),
        ],
        "MGAME fixed secondary source",
    )?;
    insert(
        result,
        0xcd68,
        exact_value(
            "fixed_zero",
            0,
            "validated direct byte store from the architectural zero register",
        ),
        &image.path,
    )?;

    validate_mgame_primary_lookup(image, 0xcdf0, 0xce1c)?;
    insert(
        result,
        0xce1c,
        runtime_source(
            "runtime_indexed_embedded_word_byte",
            vec![[0x801f_1854, 0x801f_1855], [0x801f_185a, 0x801f_185b]],
            "validated embedded-word lookup indexed by two runtime global bytes and low-byte store",
        ),
        &image.path,
    )?;
    validate_mgame_primary_lookup_with_side_effects(image)?;
    insert(
        result,
        0xce68,
        runtime_source(
            "runtime_indexed_embedded_word_byte",
            vec![[0x801f_1854, 0x801f_1855], [0x801f_185a, 0x801f_185b]],
            "validated second embedded-word lookup indexed by the same two runtime global bytes and low-byte store",
        ),
        &image.path,
    )?;

    validate_mgame_secondary_lookup(image)?;
    insert(
        result,
        0xf150,
        runtime_source(
            "runtime_indexed_embedded_byte",
            vec![
                [0x801f_1834, 0x801f_1835],
                [0x801f_183c, 0x801f_183d],
                [0x801f_1844, 0x801f_1845],
                [0x801f_18dd, 0x801f_18de],
            ],
            "validated embedded-byte lookup whose row and flags come from four runtime global bytes",
        ),
        &image.path,
    )
}

fn validate_mgame_primary_fallback_predecessor_writer(
    image: &LoadedImage,
    result: &mut BTreeMap<usize, ValidatedSelectorWriterEvidence>,
) -> Result<()> {
    validate_mgame_primary_fallback_predecessor_producer_chain(image)?;
    ensure_sequence(
        image,
        &[
            (
                0x16c38,
                Instruction::Lw {
                    rt: Register::S1,
                    base: Register::S0,
                    offset: 0x2c,
                },
            ),
            (
                0x16d04,
                Instruction::Lbu {
                    rt: Register::V0,
                    base: Register::S1,
                    offset: 0x3b,
                },
            ),
            (
                0x16d0c,
                Instruction::Bne {
                    rs: Register::V0,
                    rt: Register::ZERO,
                    target: runtime_address(image, 0x16d28)?,
                },
            ),
            (0x16d10, Instruction::nop()),
            (
                0x16d28,
                Instruction::Lbu {
                    rt: Register::V0,
                    base: Register::S1,
                    offset: 0x34,
                },
            ),
            (
                0x16d2c,
                Instruction::Lui {
                    rt: Register::AT,
                    immediate: 0x801f,
                },
            ),
            (
                0x16d30,
                Instruction::Sb {
                    rt: Register::V0,
                    base: Register::AT,
                    offset: 0x1bc3,
                },
            ),
        ],
        "MGAME primary-fallback predecessor source",
    )?;
    ensure_register_is_preserved_between(
        image,
        0x16c3c,
        0x16d28,
        Register::S1,
        "MGAME primary-fallback predecessor structure pointer",
    )?;
    insert(
        result,
        0x16d30,
        ValidatedSelectorWriterEvidence {
            classification: "runtime_structure_byte",
            value_resolution: "runtime_source",
            exact_value_candidates: vec![0],
            bounded_value_range: None,
            source_runtime_byte_ranges: Vec::new(),
            evidence: "validated task lifecycle copies constructor field +0x0a into runtime-structure byte +0x34; six direct constructor callers force zero while the remaining caller forwards byte +0x26 from its runtime object",
        },
        &image.path,
    )
}

fn validate_mgame_primary_fallback_predecessor_producer_chain(image: &LoadedImage) -> Result<()> {
    let primary_constructor = runtime_address(image, 0x17644)?;
    let secondary_constructor = runtime_address(image, 0x176d4)?;
    let structure_initializer = runtime_address(image, 0x16af0)?;
    ensure_direct_call_offsets(
        image,
        primary_constructor,
        &[0x156bc, 0x156e0, 0x15724],
        "MGAME selector-15 primary task constructor",
    )?;
    ensure_direct_call_offsets(
        image,
        secondary_constructor,
        &[0x63e4, 0x6690, 0x67b4, 0xbf3c],
        "MGAME selector-15 secondary task constructor",
    )?;
    ensure_direct_call_offsets(
        image,
        structure_initializer,
        &[0x16ab8],
        "MGAME selector-15 structure initializer",
    )?;

    ensure_sequence(
        image,
        &[
            (
                0x17650,
                Instruction::Addu {
                    rd: Register::S0,
                    rs: Register::A1,
                    rt: Register::ZERO,
                },
            ),
            (
                0x17660,
                Instruction::Addu {
                    rd: Register::S2,
                    rs: Register::A3,
                    rt: Register::ZERO,
                },
            ),
            (
                0x17688,
                Instruction::Andi {
                    rt: Register::V0,
                    rs: Register::S0,
                    immediate: 0xff,
                },
            ),
            (
                0x1768c,
                Instruction::Beq {
                    rs: Register::V0,
                    rt: Register::ZERO,
                    target: runtime_address(image, 0x176a0)?,
                },
            ),
            (
                0x1769c,
                Instruction::Sh {
                    rt: Register::ZERO,
                    base: Register::V1,
                    offset: 0x0a,
                },
            ),
            (
                0x176a0,
                Instruction::Andi {
                    rt: Register::V0,
                    rs: Register::S2,
                    immediate: 0xff,
                },
            ),
            (
                0x176a8,
                Instruction::Sh {
                    rt: Register::V0,
                    base: Register::V1,
                    offset: 0x0a,
                },
            ),
        ],
        "MGAME selector-15 primary task field producer",
    )?;
    ensure_sequence(
        image,
        &[
            (
                0x176dc,
                Instruction::Addu {
                    rd: Register::S0,
                    rs: Register::A0,
                    rt: Register::ZERO,
                },
            ),
            (
                0x176ec,
                Instruction::Addu {
                    rd: Register::S2,
                    rs: Register::A2,
                    rt: Register::ZERO,
                },
            ),
            (
                0x17710,
                Instruction::Andi {
                    rt: Register::V0,
                    rs: Register::S0,
                    immediate: 0xff,
                },
            ),
            (
                0x17714,
                Instruction::Beq {
                    rs: Register::V0,
                    rt: Register::ZERO,
                    target: runtime_address(image, 0x17728)?,
                },
            ),
            (
                0x17724,
                Instruction::Sh {
                    rt: Register::ZERO,
                    base: Register::V1,
                    offset: 0x0a,
                },
            ),
            (
                0x17728,
                Instruction::Andi {
                    rt: Register::V0,
                    rs: Register::S2,
                    immediate: 0xff,
                },
            ),
            (
                0x17730,
                Instruction::Sh {
                    rt: Register::V0,
                    base: Register::V1,
                    offset: 0x0a,
                },
            ),
        ],
        "MGAME selector-15 secondary task field producer",
    )?;

    ensure_sequence(
        image,
        &[
            (
                0x156b4,
                Instruction::Addu {
                    rd: Register::A1,
                    rs: Register::ZERO,
                    rt: Register::ZERO,
                },
            ),
            (
                0x156c0,
                Instruction::Addu {
                    rd: Register::A3,
                    rs: Register::ZERO,
                    rt: Register::ZERO,
                },
            ),
            (
                0x156d8,
                Instruction::Addiu {
                    rt: Register::A1,
                    rs: Register::ZERO,
                    immediate: 1,
                },
            ),
            (
                0x156e4,
                Instruction::Addu {
                    rd: Register::A3,
                    rs: Register::ZERO,
                    rt: Register::ZERO,
                },
            ),
            (
                0x15720,
                Instruction::Lbu {
                    rt: Register::A3,
                    base: Register::S1,
                    offset: 0x26,
                },
            ),
            (
                0x15728,
                Instruction::Addu {
                    rd: Register::A1,
                    rs: Register::ZERO,
                    rt: Register::ZERO,
                },
            ),
        ],
        "MGAME selector-15 primary constructor callers",
    )?;
    ensure_sequence(
        image,
        &[
            (
                0x63e8,
                Instruction::Addu {
                    rd: Register::A2,
                    rs: Register::ZERO,
                    rt: Register::ZERO,
                },
            ),
            (
                0x6680,
                Instruction::Addu {
                    rd: Register::A2,
                    rs: Register::ZERO,
                    rt: Register::ZERO,
                },
            ),
            (
                0x67b8,
                Instruction::Addu {
                    rd: Register::A2,
                    rs: Register::ZERO,
                    rt: Register::ZERO,
                },
            ),
            (
                0xbf40,
                Instruction::Addu {
                    rd: Register::A2,
                    rs: Register::ZERO,
                    rt: Register::ZERO,
                },
            ),
        ],
        "MGAME selector-15 secondary constructor callers",
    )?;
    ensure_register_is_preserved_between(
        image,
        0x6684,
        0x6690,
        Register::A2,
        "MGAME selector-15 secondary constructor zero argument",
    )?;

    ensure_sequence(
        image,
        &[
            (
                0x16ab8,
                Instruction::Jal {
                    target: structure_initializer,
                },
            ),
            (
                0x16b00,
                Instruction::Lw {
                    rt: Register::S0,
                    base: Register::A0,
                    offset: 0x2c,
                },
            ),
            (
                0x16b24,
                Instruction::Lbu {
                    rt: Register::V1,
                    base: Register::A0,
                    offset: 0x0a,
                },
            ),
            (
                0x16b40,
                Instruction::Sb {
                    rt: Register::V1,
                    base: Register::S0,
                    offset: 0x35,
                },
            ),
            (
                0x16b44,
                Instruction::Sb {
                    rt: Register::V1,
                    base: Register::S0,
                    offset: 0x34,
                },
            ),
        ],
        "MGAME selector-15 structure-byte initialization",
    )?;
    ensure_register_is_preserved_between(
        image,
        0x16b28,
        0x16b44,
        Register::V1,
        "MGAME selector-15 structure-byte value",
    )
}

fn validate_mgame_primary_fallback_global_writers(
    image: &LoadedImage,
    result: &mut BTreeMap<usize, ValidatedSelectorWriterEvidence>,
) -> Result<()> {
    ensure_sequence(
        image,
        &[
            (
                0x8eb0,
                Instruction::Lui {
                    rt: Register::AT,
                    immediate: 0x801f,
                },
            ),
            (
                0x8eb4,
                Instruction::Sb {
                    rt: Register::ZERO,
                    base: Register::AT,
                    offset: 0x1bc7,
                },
            ),
        ],
        "MGAME fixed primary-fallback global source",
    )?;
    insert(
        result,
        0x8eb4,
        exact_value(
            "fixed_zero",
            0,
            "validated direct zero store to the primary-fallback global source",
        ),
        &image.path,
    )?;

    ensure_sequence(
        image,
        &[
            (
                0x8f0c,
                Instruction::Lui {
                    rt: Register::V1,
                    immediate: 0x801f,
                },
            ),
            (
                0x8f10,
                Instruction::Lbu {
                    rt: Register::V1,
                    base: Register::V1,
                    offset: 0x1bc3,
                },
            ),
            (
                0x8f1c,
                Instruction::Lui {
                    rt: Register::AT,
                    immediate: 0x801f,
                },
            ),
            (
                0x8f20,
                Instruction::Sb {
                    rt: Register::V1,
                    base: Register::AT,
                    offset: 0x1bc7,
                },
            ),
        ],
        "MGAME forwarded primary-fallback global source",
    )?;
    ensure_register_is_preserved_between(
        image,
        0x8f14,
        0x8f20,
        Register::V1,
        "MGAME primary-fallback predecessor byte",
    )?;
    insert(
        result,
        0x8f20,
        runtime_source(
            "forwarded_global_runtime_byte",
            vec![[0x801f_1bc3, 0x801f_1bc4]],
            "validated identity-preserving global-byte load and direct primary-fallback source store",
        ),
        &image.path,
    )
}

fn validate_mgent_writers(
    image: &LoadedImage,
    result: &mut BTreeMap<usize, ValidatedSelectorWriterEvidence>,
) -> Result<()> {
    for (load_offset, store_lui_offset, store_offset, destination_low) in [
        (0x2154, 0x2160, 0x2164, 0x185a),
        (0x2cf4, 0x2d00, 0x2d04, 0x1854),
    ] {
        ensure_sequence(
            image,
            &[
                (
                    load_offset,
                    Instruction::Lbu {
                        rt: Register::V0,
                        base: Register::S0,
                        offset: 12,
                    },
                ),
                (
                    store_lui_offset,
                    Instruction::Lui {
                        rt: Register::AT,
                        immediate: 0x801f,
                    },
                ),
                (
                    store_offset,
                    Instruction::Sb {
                        rt: Register::V0,
                        base: Register::AT,
                        offset: destination_low as i16,
                    },
                ),
            ],
            "MGENT runtime-record source",
        )?;
        ensure_register_is_preserved_between(
            image,
            load_offset + 4,
            store_offset,
            Register::V0,
            "MGENT runtime-record source",
        )?;
        insert(
            result,
            store_offset,
            runtime_source(
                "runtime_structure_byte",
                Vec::new(),
                "validated byte load from the active runtime record and direct custom-record source store",
            ),
            &image.path,
        )?;
    }

    ensure_sequence(
        image,
        &[
            (
                0x218c,
                Instruction::Lui {
                    rt: Register::AT,
                    immediate: 0x801f,
                },
            ),
            (
                0x2190,
                Instruction::Sb {
                    rt: Register::ZERO,
                    base: Register::AT,
                    offset: 0x1854,
                },
            ),
        ],
        "MGENT fixed primary source",
    )?;
    insert(
        result,
        0x2190,
        exact_value(
            "fixed_zero",
            0,
            "validated direct byte store from the architectural zero register",
        ),
        &image.path,
    )
}

fn validate_mgtit_writer(
    image: &LoadedImage,
    result: &mut BTreeMap<usize, ValidatedSelectorWriterEvidence>,
) -> Result<()> {
    let copy_runtime_address = runtime_address(image, 0x5430)?;
    ensure_sequence(
        image,
        &[
            (
                0x5430,
                Instruction::Beq {
                    rs: Register::A0,
                    rt: Register::ZERO,
                    target: 0x800a_745c,
                },
            ),
            (
                0x5438,
                Instruction::Blez {
                    rs: Register::A2,
                    target: 0x800a_7458,
                },
            ),
            (
                0x543c,
                Instruction::Addu {
                    rd: Register::V1,
                    rs: Register::A0,
                    rt: Register::ZERO,
                },
            ),
            (
                0x5440,
                Instruction::Lbu {
                    rt: Register::V0,
                    base: Register::A0,
                    offset: 0,
                },
            ),
            (
                0x5444,
                Instruction::Addiu {
                    rt: Register::A0,
                    rs: Register::A0,
                    immediate: 1,
                },
            ),
            (
                0x5448,
                Instruction::Addiu {
                    rt: Register::A2,
                    rs: Register::A2,
                    immediate: -1,
                },
            ),
            (
                0x544c,
                Instruction::Sb {
                    rt: Register::V0,
                    base: Register::A1,
                    offset: 0,
                },
            ),
            (
                0x5450,
                Instruction::Bgtz {
                    rs: Register::A2,
                    target: 0x800a_7440,
                },
            ),
            (
                0x5454,
                Instruction::Addiu {
                    rt: Register::A1,
                    rs: Register::A1,
                    immediate: 1,
                },
            ),
        ],
        "MGTIT runtime buffer copy",
    )?;
    validate_mgtit_runtime_buffer_callers(image, copy_runtime_address)?;
    insert(
        result,
        0x544c,
        runtime_source(
            "runtime_buffer_byte_copy",
            vec![[0x801f_1a04, 0x801f_1a05]],
            "validated complete call chain from the fixed 0x801f1a00 state block; call +0x28e8 copies the guarded 0x801f1a04-selected 512-byte page at *(0x801f63f0)+6400+index*512 into 0x801f1800..0x801f19ff",
        ),
        &image.path,
    )
}

fn validate_mgtit_runtime_buffer_callers(
    image: &LoadedImage,
    copy_runtime_address: u32,
) -> Result<()> {
    ensure_direct_call_offsets(
        image,
        copy_runtime_address,
        &[0x2628, 0x28e8],
        "MGTIT runtime-buffer byte copy",
    )?;
    validate_mgtit_source_page_index_chain(image)?;
    ensure_sequence(
        image,
        &[
            (
                0x2604,
                Instruction::Lui {
                    rt: Register::S0,
                    immediate: 0x801f,
                },
            ),
            (
                0x2608,
                Instruction::Lw {
                    rt: Register::S0,
                    base: Register::S0,
                    offset: 0x63f0,
                },
            ),
            (
                0x260c,
                Instruction::Addiu {
                    rt: Register::A2,
                    rs: Register::ZERO,
                    immediate: 4480,
                },
            ),
            (
                0x2620,
                Instruction::Addu {
                    rd: Register::A0,
                    rs: Register::S0,
                    rt: Register::ZERO,
                },
            ),
            (
                0x2624,
                Instruction::Addiu {
                    rt: Register::S0,
                    rs: Register::S0,
                    immediate: 4480,
                },
            ),
            (
                0x2628,
                Instruction::Jal {
                    target: copy_runtime_address,
                },
            ),
            (
                0x262c,
                Instruction::Addu {
                    rd: Register::A1,
                    rs: Register::S0,
                    rt: Register::ZERO,
                },
            ),
        ],
        "MGTIT initial runtime-buffer relocation caller",
    )?;
    ensure_register_is_preserved_between(
        image,
        0x2610,
        0x2620,
        Register::S0,
        "MGTIT initial runtime-buffer source",
    )?;
    ensure_register_is_preserved_between(
        image,
        0x2610,
        0x2628,
        Register::A2,
        "MGTIT initial runtime-buffer length",
    )?;
    ensure_register_is_preserved_between(
        image,
        0x2624,
        0x2628,
        Register::A0,
        "MGTIT initial runtime-buffer source argument",
    )?;

    ensure_sequence(
        image,
        &[
            (
                0x287c,
                Instruction::Lui {
                    rt: Register::V0,
                    immediate: 0x801f,
                },
            ),
            (
                0x2880,
                Instruction::Lw {
                    rt: Register::V0,
                    base: Register::V0,
                    offset: 0x63f0,
                },
            ),
            (
                0x288c,
                Instruction::Addu {
                    rd: Register::S0,
                    rs: Register::A0,
                    rt: Register::ZERO,
                },
            ),
            (
                0x2898,
                Instruction::Addiu {
                    rt: Register::S1,
                    rs: Register::V0,
                    immediate: 4992,
                },
            ),
            (
                0x28d4,
                Instruction::Lui {
                    rt: Register::A1,
                    immediate: 0x801f,
                },
            ),
            (
                0x28d8,
                Instruction::Ori {
                    rt: Register::A1,
                    rs: Register::A1,
                    immediate: 0x1800,
                },
            ),
            (
                0x28dc,
                Instruction::Sll {
                    rd: Register::A0,
                    rt: Register::S0,
                    shift: 9,
                },
            ),
            (
                0x28e0,
                Instruction::Addiu {
                    rt: Register::A0,
                    rs: Register::A0,
                    immediate: 1408,
                },
            ),
            (
                0x28e4,
                Instruction::Addu {
                    rd: Register::A0,
                    rs: Register::S1,
                    rt: Register::A0,
                },
            ),
            (
                0x28e8,
                Instruction::Jal {
                    target: copy_runtime_address,
                },
            ),
            (
                0x28ec,
                Instruction::Addiu {
                    rt: Register::A2,
                    rs: Register::ZERO,
                    immediate: 512,
                },
            ),
        ],
        "MGTIT custom-record source-page caller",
    )?;
    ensure_register_is_preserved_between(
        image,
        0x2884,
        0x2898,
        Register::V0,
        "MGTIT runtime allocation pointer",
    )?;
    ensure_register_is_preserved_between(
        image,
        0x2890,
        0x28dc,
        Register::S0,
        "MGTIT custom-record source-page index",
    )?;
    ensure_register_is_preserved_between(
        image,
        0x289c,
        0x28e4,
        Register::S1,
        "MGTIT custom-record source-page base",
    )
}

fn validate_mgtit_source_page_index_chain(image: &LoadedImage) -> Result<()> {
    let dispatch_runtime_address = runtime_address(image, 0x2214)?;
    let page_loader_runtime_address = runtime_address(image, 0x287c)?;
    ensure_direct_call_offsets(
        image,
        dispatch_runtime_address,
        &[0x1ecc],
        "MGTIT source-page dispatch",
    )?;
    ensure_direct_call_offsets(
        image,
        page_loader_runtime_address,
        &[0x24d4],
        "MGTIT source-page loader",
    )?;
    ensure_sequence(
        image,
        &[
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
                0x1ecc,
                Instruction::Jal {
                    target: dispatch_runtime_address,
                },
            ),
            (
                0x1ed0,
                Instruction::Addu {
                    rd: Register::A0,
                    rs: Register::S0,
                    rt: Register::ZERO,
                },
            ),
            (
                0x221c,
                Instruction::Addu {
                    rd: Register::S1,
                    rs: Register::A0,
                    rt: Register::ZERO,
                },
            ),
            (
                0x24c8,
                Instruction::Lb {
                    rt: Register::A0,
                    base: Register::S1,
                    offset: 4,
                },
            ),
            (
                0x24d4,
                Instruction::Jal {
                    target: page_loader_runtime_address,
                },
            ),
            (0x24d8, Instruction::nop()),
            (
                0x289c,
                Instruction::Slti {
                    rt: Register::V0,
                    rs: Register::S0,
                    immediate: 5,
                },
            ),
            (
                0x28a0,
                Instruction::Beq {
                    rs: Register::V0,
                    rt: Register::ZERO,
                    target: runtime_address(image, 0x28f8)?,
                },
            ),
            (
                0x28a8,
                Instruction::Lbu {
                    rt: Register::V1,
                    base: Register::V0,
                    offset: 3,
                },
            ),
            (
                0x28ac,
                Instruction::Addiu {
                    rt: Register::V0,
                    rs: Register::ZERO,
                    immediate: 1,
                },
            ),
            (
                0x28b0,
                Instruction::Bne {
                    rs: Register::V1,
                    rt: Register::V0,
                    target: runtime_address(image, 0x28fc)?,
                },
            ),
            (0x28b4, Instruction::nop()),
        ],
        "MGTIT source-page index chain",
    )?;
    ensure_register_is_preserved_between(
        image,
        0x1b6c,
        0x1b98,
        Register::S0,
        "MGTIT source-page state-block high half",
    )?;
    ensure_register_is_preserved_between(
        image,
        0x1b9c,
        0x1ed0,
        Register::S0,
        "MGTIT source-page state-block pointer",
    )?;
    ensure_register_is_preserved_between(
        image,
        0x2220,
        0x24c8,
        Register::S1,
        "MGTIT source-page state-block argument",
    )?;
    ensure_register_is_preserved_between(
        image,
        0x24cc,
        0x24d4,
        Register::A0,
        "MGTIT source-page index argument",
    )
}

fn validate_mgame_primary_lookup(
    image: &LoadedImage,
    first_offset: usize,
    store_offset: usize,
) -> Result<()> {
    ensure_sequence(
        image,
        &[
            (
                first_offset,
                Instruction::Lui {
                    rt: Register::V0,
                    immediate: 0x801f,
                },
            ),
            (
                first_offset + 4,
                Instruction::Lbu {
                    rt: Register::V0,
                    base: Register::V0,
                    offset: 0x185a,
                },
            ),
            (
                first_offset + 8,
                Instruction::Lui {
                    rt: Register::V1,
                    immediate: 0x801f,
                },
            ),
            (
                first_offset + 12,
                Instruction::Lbu {
                    rt: Register::V1,
                    base: Register::V1,
                    offset: 0x1854,
                },
            ),
            (
                first_offset + 16,
                Instruction::Sll {
                    rd: Register::V0,
                    rt: Register::V0,
                    shift: 1,
                },
            ),
            (
                first_offset + 20,
                Instruction::Addu {
                    rd: Register::V0,
                    rs: Register::V0,
                    rt: Register::V1,
                },
            ),
            (
                first_offset + 24,
                Instruction::Sll {
                    rd: Register::V0,
                    rt: Register::V0,
                    shift: 2,
                },
            ),
            (
                first_offset + 28,
                Instruction::Lui {
                    rt: Register::AT,
                    immediate: 0x800a,
                },
            ),
            (
                first_offset + 32,
                Instruction::Addu {
                    rd: Register::AT,
                    rs: Register::AT,
                    rt: Register::V0,
                },
            ),
            (
                first_offset + 36,
                Instruction::Lw {
                    rt: Register::V0,
                    base: Register::AT,
                    offset: 11620,
                },
            ),
            (
                store_offset - 4,
                Instruction::Lui {
                    rt: Register::AT,
                    immediate: 0x801f,
                },
            ),
            (
                store_offset,
                Instruction::Sb {
                    rt: Register::V0,
                    base: Register::AT,
                    offset: 0x185f,
                },
            ),
        ],
        "MGAME primary source lookup",
    )
}

fn validate_mgame_primary_lookup_with_side_effects(image: &LoadedImage) -> Result<()> {
    ensure_sequence(
        image,
        &[
            (
                0xce2c,
                Instruction::Lui {
                    rt: Register::A1,
                    immediate: 0x801f,
                },
            ),
            (
                0xce30,
                Instruction::Lbu {
                    rt: Register::A1,
                    base: Register::A1,
                    offset: 0x185a,
                },
            ),
            (
                0xce34,
                Instruction::Lui {
                    rt: Register::V1,
                    immediate: 0x801f,
                },
            ),
            (
                0xce38,
                Instruction::Lbu {
                    rt: Register::V1,
                    base: Register::V1,
                    offset: 0x1854,
                },
            ),
            (
                0xce4c,
                Instruction::Sll {
                    rd: Register::V0,
                    rt: Register::A1,
                    shift: 1,
                },
            ),
            (
                0xce50,
                Instruction::Addu {
                    rd: Register::V0,
                    rs: Register::V0,
                    rt: Register::V1,
                },
            ),
            (
                0xce54,
                Instruction::Sll {
                    rd: Register::V0,
                    rt: Register::V0,
                    shift: 2,
                },
            ),
            (
                0xce58,
                Instruction::Lui {
                    rt: Register::AT,
                    immediate: 0x800a,
                },
            ),
            (
                0xce5c,
                Instruction::Addu {
                    rd: Register::AT,
                    rs: Register::AT,
                    rt: Register::V0,
                },
            ),
            (
                0xce60,
                Instruction::Lw {
                    rt: Register::V0,
                    base: Register::AT,
                    offset: 11620,
                },
            ),
            (
                0xce64,
                Instruction::Lui {
                    rt: Register::AT,
                    immediate: 0x801f,
                },
            ),
            (
                0xce68,
                Instruction::Sb {
                    rt: Register::V0,
                    base: Register::AT,
                    offset: 0x185f,
                },
            ),
        ],
        "MGAME primary source lookup with side effects",
    )
}

fn validate_mgame_secondary_lookup(image: &LoadedImage) -> Result<()> {
    ensure_sequence(
        image,
        &[
            (
                0xf0e0,
                Instruction::Lui {
                    rt: Register::V0,
                    immediate: 0x801f,
                },
            ),
            (
                0xf0e4,
                Instruction::Lbu {
                    rt: Register::V0,
                    base: Register::V0,
                    offset: 0x1834,
                },
            ),
            (
                0xf0e8,
                Instruction::Lui {
                    rt: Register::A2,
                    immediate: 0x801f,
                },
            ),
            (
                0xf0ec,
                Instruction::Lbu {
                    rt: Register::A2,
                    base: Register::A2,
                    offset: 0x18dd,
                },
            ),
            (
                0xf0f0,
                Instruction::Lui {
                    rt: Register::V1,
                    immediate: 0x801f,
                },
            ),
            (
                0xf0f4,
                Instruction::Lbu {
                    rt: Register::V1,
                    base: Register::V1,
                    offset: 0x1844,
                },
            ),
            (
                0xf104,
                Instruction::Lui {
                    rt: Register::V0,
                    immediate: 0x801f,
                },
            ),
            (
                0xf108,
                Instruction::Lbu {
                    rt: Register::V0,
                    base: Register::V0,
                    offset: 0x183c,
                },
            ),
            (
                0xf130,
                Instruction::Lui {
                    rt: Register::V1,
                    immediate: 0x800a,
                },
            ),
            (
                0xf134,
                Instruction::Addiu {
                    rt: Register::V1,
                    rs: Register::V1,
                    immediate: 11800,
                },
            ),
            (
                0xf138,
                Instruction::Sll {
                    rd: Register::V0,
                    rt: Register::A2,
                    shift: 3,
                },
            ),
            (
                0xf13c,
                Instruction::Addu {
                    rd: Register::V0,
                    rs: Register::V0,
                    rt: Register::V1,
                },
            ),
            (
                0xf140,
                Instruction::Andi {
                    rt: Register::V1,
                    rs: Register::A0,
                    immediate: 0xff,
                },
            ),
            (
                0xf144,
                Instruction::Addu {
                    rd: Register::V0,
                    rs: Register::V0,
                    rt: Register::V1,
                },
            ),
            (
                0xf148,
                Instruction::Lbu {
                    rt: Register::V0,
                    base: Register::V0,
                    offset: 0,
                },
            ),
            (
                0xf14c,
                Instruction::Lui {
                    rt: Register::AT,
                    immediate: 0x801f,
                },
            ),
            (
                0xf150,
                Instruction::Sb {
                    rt: Register::V0,
                    base: Register::AT,
                    offset: 0x18dd,
                },
            ),
        ],
        "MGAME secondary source lookup",
    )
}

fn validate_global_byte_copy(image: &LoadedImage, profile: &GlobalByteCopyProfile) -> Result<()> {
    ensure_sequence(
        image,
        &[
            (
                profile.source_lui_offset,
                Instruction::Lui {
                    rt: profile.register,
                    immediate: 0x801f,
                },
            ),
            (
                profile.source_load_offset,
                Instruction::Lbu {
                    rt: profile.register,
                    base: profile.register,
                    offset: profile.source_low as i16,
                },
            ),
            (
                profile.store_lui_offset,
                Instruction::Lui {
                    rt: Register::AT,
                    immediate: 0x801f,
                },
            ),
            (
                profile.store_offset,
                Instruction::Sb {
                    rt: profile.register,
                    base: Register::AT,
                    offset: profile.destination_low as i16,
                },
            ),
        ],
        "custom-record global source copy",
    )?;
    ensure_register_is_preserved_between(
        image,
        profile.source_load_offset + 4,
        profile.store_offset,
        profile.register,
        "custom-record global source copy",
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
        "{image_path} repeats custom-record source writer profile +0x{store_offset:x}"
    );
    Ok(())
}
