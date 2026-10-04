use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};
use psx_r3000a::{Instruction, Register};

use crate::source_disc::LoadedImage;

use super::profile_validation::{
    ensure_instruction, ensure_only_reachable_direct_entries, ensure_register_is_preserved_between,
    runtime_address,
};
use super::writer_evidence::{
    ValidatedSelectorWriterEvidence, kanri_custom_index_writer_evidence,
    validate_kanri_custom_index_source,
};

#[derive(Clone, Copy)]
struct LiteralWriter {
    producer_offset: usize,
    store_offset: usize,
    destination_low: u16,
    register: Register,
    value: u8,
}

const KANRI_LITERAL_WRITERS: &[LiteralWriter] = &[
    literal(0x5308, 0x5320, 0x6702, Register::V0, 30),
    literal(0x605c, 0x6064, 0x6702, Register::V0, 30),
];

const MAIN_LITERAL_WRITERS: &[LiteralWriter] = &[
    literal(0x729c, 0x72a4, 0x6702, Register::V0, 2),
    literal(0x72a8, 0x72b0, 0x6704, Register::V0, 7),
    literal(0x73a4, 0x73c0, 0x6702, Register::V1, 2),
    literal(0x73a8, 0x73c8, 0x6704, Register::V0, 7),
    literal(0x74f0, 0x7500, 0x6702, Register::V0, 2),
    literal(0x750c, 0x7520, 0x6702, Register::V0, 2),
    literal(0x7524, 0x7534, 0x6704, Register::V0, 7),
    literal(0x75e0, 0x75e8, 0x6702, Register::V0, 2),
    literal(0x75ec, 0x75f4, 0x6704, Register::V0, 7),
    literal(0x78b8, 0x78c0, 0x6702, Register::V0, 2),
    literal(0x78c4, 0x78cc, 0x6704, Register::V0, 7),
    literal(0x7984, 0x79f8, 0x5c38, Register::S0, 3),
    literal(0x7b08, 0x7b10, 0x5c38, Register::V0, 4),
];

const PLSEL3_LITERAL_WRITERS: &[LiteralWriter] = &[
    literal(0x700, 0x708, 0x5c38, Register::V0, 3),
    literal(0x3bf0, 0x3bf8, 0x5c38, Register::V0, 3),
];

const PLSEL4_LITERAL_WRITERS: &[LiteralWriter] = &[
    literal(0x9b0, 0x9b8, 0x5c38, Register::V1, 8),
    literal(0x3ccc, 0x3cd4, 0x5c38, Register::V0, 8),
];

const fn literal(
    producer_offset: usize,
    store_offset: usize,
    destination_low: u16,
    register: Register,
    value: u8,
) -> LiteralWriter {
    LiteralWriter {
        producer_offset,
        store_offset,
        destination_low,
        register,
        value,
    }
}

pub(super) fn validated_upstream_writer_evidence(
    image: &LoadedImage,
    reachable_instruction_offsets: &BTreeSet<usize>,
) -> Result<BTreeMap<usize, ValidatedSelectorWriterEvidence>> {
    let mut evidence = BTreeMap::new();
    match image.path.as_str() {
        "DAT1/KANRI.BIN" => {
            validate_kanri_writers(image, reachable_instruction_offsets, &mut evidence)?
        }
        "DAT1/PLSEL2.BIN" => validate_plsel2_writers(image, &mut evidence)?,
        "DAT1/PLSEL3.BIN" => validate_plsel3_writers(image, &mut evidence)?,
        "DAT1/PLSEL4.BIN" => validate_plsel4_writers(image, &mut evidence)?,
        "DAT1/PLSEL5.BIN" => validate_plsel5_writers(image, &mut evidence)?,
        "DAT1/SIKEN.BIN" => {
            validate_decremented_runtime_byte_writer(image, 0x0fc4, 0x0fd4, 0x0fdc, &mut evidence)?
        }
        "DAT1/SIKEN2.BIN" => {
            validate_decremented_runtime_byte_writer(image, 0x0c74, 0x0c84, 0x0c8c, &mut evidence)?
        }
        "SLPS_021.20" => {
            validate_main_writers(image, reachable_instruction_offsets, &mut evidence)?
        }
        _ => {}
    }
    Ok(evidence)
}

fn validate_decremented_runtime_byte_writer(
    image: &LoadedImage,
    load_address_offset: usize,
    arithmetic_offset: usize,
    store_offset: usize,
    result: &mut BTreeMap<usize, ValidatedSelectorWriterEvidence>,
) -> Result<()> {
    ensure_instruction(
        image,
        load_address_offset,
        Instruction::Lui {
            rt: Register::V0,
            immediate: 0x801f,
        },
        "decremented runtime-byte address",
    )?;
    ensure_instruction(
        image,
        load_address_offset + 4,
        Instruction::Lbu {
            rt: Register::V0,
            base: Register::V0,
            offset: 0x6301,
        },
        "decremented runtime-byte load",
    )?;
    ensure_register_is_preserved_between(
        image,
        load_address_offset + 8,
        arithmetic_offset,
        Register::V0,
        "decremented runtime byte",
    )?;
    ensure_instruction(
        image,
        arithmetic_offset,
        Instruction::Addiu {
            rt: Register::V0,
            rs: Register::V0,
            immediate: -1,
        },
        "decremented runtime-byte arithmetic",
    )?;
    validate_direct_byte_store(image, store_offset, 0x1a0e, Register::V0)?;
    insert(
        result,
        store_offset,
        runtime_source(
            "decremented_runtime_byte",
            vec![[0x801f_6301, 0x801f_6302]],
            "validated runtime-byte load, decrement, and direct store",
        ),
        &image.path,
    )
}

fn validate_plsel2_writers(
    image: &LoadedImage,
    result: &mut BTreeMap<usize, ValidatedSelectorWriterEvidence>,
) -> Result<()> {
    validate_bounded_decrement_writer(image, 0xc68, 0xc74, 0xc78, 0xc88, 0xc90)?;
    insert(
        result,
        0xc90,
        bounded_value(
            "guarded_byte_decrement",
            [2, 254],
            "an unsigned byte at least three is decremented once before write-back",
        ),
        &image.path,
    )
}

fn validate_kanri_writers(
    image: &LoadedImage,
    reachable_instruction_offsets: &BTreeSet<usize>,
    result: &mut BTreeMap<usize, ValidatedSelectorWriterEvidence>,
) -> Result<()> {
    insert_literal_writers(image, KANRI_LITERAL_WRITERS, result)?;
    validate_kanri_custom_index_source(image, reachable_instruction_offsets)?;
    validate_direct_byte_store(image, 0x5328, 0x6717, Register::S2)?;
    insert(
        result,
        0x5328,
        kanri_custom_index_writer_evidence(),
        &image.path,
    )?;
    validate_direct_byte_store(image, 0x606c, 0x6703, Register::ZERO)?;
    insert(
        result,
        0x606c,
        exact_value(
            "fixed_zero",
            0,
            "typed store from the architectural zero register",
        ),
        &image.path,
    )?;
    ensure_instruction(
        image,
        0x5fcc,
        Instruction::Lui {
            rt: Register::V1,
            immediate: 0x801f,
        },
        "KANRI source-byte address",
    )?;
    ensure_instruction(
        image,
        0x5fd0,
        Instruction::Lbu {
            rt: Register::V1,
            base: Register::V1,
            offset: 0x18e0,
        },
        "KANRI source-byte load",
    )?;
    ensure_register_is_preserved_between(image, 0x5fd4, 0x6074, Register::V1, "KANRI source byte")?;
    validate_direct_byte_store(image, 0x6074, 0x6706, Register::V1)?;
    insert(
        result,
        0x6074,
        runtime_source(
            "global_runtime_byte",
            vec![[0x801f_18e0, 0x801f_18e1]],
            "validated direct global-byte load preserved through the configuration store",
        ),
        &image.path,
    )
}

fn validate_plsel3_writers(
    image: &LoadedImage,
    result: &mut BTreeMap<usize, ValidatedSelectorWriterEvidence>,
) -> Result<()> {
    insert_literal_writers(image, PLSEL3_LITERAL_WRITERS, result)?;
    validate_bounded_decrement_writer(image, 0xb64, 0xb70, 0xb74, 0xb84, 0xb8c)?;
    insert(
        result,
        0xb8c,
        bounded_value(
            "guarded_byte_decrement",
            [2, 254],
            "an unsigned byte at least three is decremented once before write-back",
        ),
        &image.path,
    )?;
    validate_incremented_byte_writer(image, 0x35e8, 0x35f4, 0x3604, Register::V1, 0x5c39)?;
    insert(
        result,
        0x3604,
        runtime_source(
            "incremented_runtime_state",
            vec![[0x801f_5c39, 0x801f_5c3a]],
            "validated byte load, increment, and write-back",
        ),
        &image.path,
    )?;
    validate_direct_byte_store(image, 0x3b68, 0x5c39, Register::ZERO)?;
    insert(
        result,
        0x3b68,
        exact_value(
            "fixed_zero",
            0,
            "typed store from the architectural zero register",
        ),
        &image.path,
    )
}

fn validate_plsel4_writers(
    image: &LoadedImage,
    result: &mut BTreeMap<usize, ValidatedSelectorWriterEvidence>,
) -> Result<()> {
    insert_literal_writers(image, PLSEL4_LITERAL_WRITERS, result)?;
    validate_direct_byte_store(image, 0x3c50, 0x5c39, Register::ZERO)?;
    insert(
        result,
        0x3c50,
        exact_value(
            "fixed_zero",
            0,
            "typed store from the architectural zero register",
        ),
        &image.path,
    )
}

fn validate_bounded_decrement_writer(
    image: &LoadedImage,
    load_address_offset: usize,
    comparison_offset: usize,
    branch_offset: usize,
    arithmetic_offset: usize,
    store_offset: usize,
) -> Result<()> {
    ensure_instruction(
        image,
        load_address_offset,
        Instruction::Lui {
            rt: Register::V1,
            immediate: 0x801f,
        },
        "guarded decrement address",
    )?;
    ensure_instruction(
        image,
        load_address_offset + 4,
        Instruction::Lbu {
            rt: Register::V1,
            base: Register::V1,
            offset: 0x5c38,
        },
        "guarded decrement byte load",
    )?;
    ensure_instruction(
        image,
        comparison_offset,
        Instruction::Sltiu {
            rt: Register::V0,
            rs: Register::V1,
            immediate: 3,
        },
        "guarded decrement lower-bound check",
    )?;
    let branch_target = runtime_address(image, store_offset + 28)?;
    ensure_instruction(
        image,
        branch_offset,
        Instruction::Bne {
            rs: Register::V0,
            rt: Register::ZERO,
            target: branch_target,
        },
        "guarded decrement bypass",
    )?;
    ensure_register_is_preserved_between(
        image,
        load_address_offset + 8,
        arithmetic_offset,
        Register::V1,
        "guarded decrement source",
    )?;
    ensure_instruction(
        image,
        arithmetic_offset,
        Instruction::Addiu {
            rt: Register::V1,
            rs: Register::V1,
            immediate: -1,
        },
        "guarded decrement arithmetic",
    )?;
    validate_direct_byte_store(image, store_offset, 0x5c38, Register::V1)
}

fn validate_plsel5_writers(
    image: &LoadedImage,
    result: &mut BTreeMap<usize, ValidatedSelectorWriterEvidence>,
) -> Result<()> {
    validate_incremented_byte_writer(image, 0x3944, 0x3954, 0x395c, Register::V1, 0x5c39)?;
    insert(
        result,
        0x395c,
        runtime_source(
            "incremented_runtime_state",
            vec![[0x801f_5c39, 0x801f_5c3a]],
            "validated byte load, increment, and write-back before an unsigned limit check",
        ),
        &image.path,
    )?;

    for writer in [
        literal(0x3c18, 0x3c20, 0x5c38, Register::V1, 2),
        literal(0x9db0, 0x9dd8, 0x5c38, Register::V0, 2),
    ] {
        validate_literal_writer(image, &writer)?;
        insert(
            result,
            writer.store_offset,
            exact_value(
                "fixed_round_limit",
                writer.value,
                "validated local literal producer and direct byte store",
            ),
            &image.path,
        )?;
    }

    validate_direct_byte_store(image, 0x9dc8, 0x5c39, Register::ZERO)?;
    insert(
        result,
        0x9dc8,
        exact_value(
            "fixed_zero",
            0,
            "typed store from the architectural zero register",
        ),
        &image.path,
    )?;

    validate_plsel5_table_group_modulo(image)?;
    insert(
        result,
        0x47a0,
        bounded_value(
            "unsigned_modulo",
            [0, 6],
            "validated unsigned-16-bit modulo-seven reduction",
        ),
        &image.path,
    )?;

    validate_plsel5_table_loads(image)?;
    insert(
        result,
        0x39a8,
        table_values(
            image,
            &table_offsets(&[0, 2]),
            "validated two-byte round selector and seven-row table lookup",
        )?,
        &image.path,
    )?;
    insert(
        result,
        0x3a58,
        table_values(
            image,
            &table_offsets(&[1, 3]),
            "validated two-byte round selector and seven-row table lookup",
        )?,
        &image.path,
    )?;
    insert(
        result,
        0x47c8,
        table_values(
            image,
            &table_offsets(&[0]),
            "validated modulo-seven row selector and first table field",
        )?,
        &image.path,
    )?;
    insert(
        result,
        0x4850,
        table_values(
            image,
            &table_offsets(&[1]),
            "validated modulo-seven row selector and second table field",
        )?,
        &image.path,
    )?;

    for (store_offset, destination_low) in [
        (0x3a30, 0x6708),
        (0x3ac8, 0x6709),
        (0x482c, 0x6708),
        (0x48cc, 0x6709),
    ] {
        validate_direct_byte_store(
            image,
            store_offset,
            destination_low,
            if store_offset < 0x4000 {
                Register::A1
            } else {
                Register::A2
            },
        )?;
        insert(
            result,
            store_offset,
            bounded_value(
                "bounded_zero_slot_search",
                [0, 7],
                "validated zero-based search with an eight-entry loop bound and zero fallback",
            ),
            &image.path,
        )?;
    }
    validate_plsel5_search_bounds(image)
}

fn validate_main_writers(
    image: &LoadedImage,
    reachable_instruction_offsets: &BTreeSet<usize>,
    result: &mut BTreeMap<usize, ValidatedSelectorWriterEvidence>,
) -> Result<()> {
    insert_literal_writers(image, MAIN_LITERAL_WRITERS, result)?;
    validate_main_argument_writer(image, reachable_instruction_offsets)?;
    insert(
        result,
        0xeffc,
        runtime_source(
            "forwarded_global_runtime_byte",
            vec![[0x801f_5c7f, 0x801f_5c80]],
            "the only reachable direct caller loads a global byte into a0; the callee preserves it in s1 through the store",
        ),
        &image.path,
    )?;
    validate_direct_byte_store(image, 0xf004, 0x6703, Register::ZERO)?;
    insert(
        result,
        0xf004,
        exact_value(
            "fixed_zero",
            0,
            "typed store from the architectural zero register",
        ),
        &image.path,
    )?;
    validate_zero_fill(image, 0x500e0)?;
    insert(
        result,
        0x500e0,
        exact_value("zero_fill", 0, "validated byte-wise zero-fill loop"),
        &image.path,
    )
}

fn insert_literal_writers(
    image: &LoadedImage,
    writers: &[LiteralWriter],
    result: &mut BTreeMap<usize, ValidatedSelectorWriterEvidence>,
) -> Result<()> {
    for writer in writers {
        validate_literal_writer(image, writer)?;
        insert(
            result,
            writer.store_offset,
            exact_value(
                "fixed_literal",
                writer.value,
                "validated local literal producer and direct byte store",
            ),
            &image.path,
        )?;
    }
    Ok(())
}

fn validate_literal_writer(image: &LoadedImage, writer: &LiteralWriter) -> Result<()> {
    ensure_instruction(
        image,
        writer.producer_offset,
        Instruction::Addiu {
            rt: writer.register,
            rs: Register::ZERO,
            immediate: i16::from(writer.value),
        },
        "upstream literal producer",
    )?;
    ensure_register_is_preserved_between(
        image,
        writer.producer_offset + 4,
        writer.store_offset,
        writer.register,
        "upstream literal",
    )?;
    validate_direct_byte_store(
        image,
        writer.store_offset,
        writer.destination_low,
        writer.register,
    )
}

fn validate_incremented_byte_writer(
    image: &LoadedImage,
    load_address_offset: usize,
    arithmetic_offset: usize,
    store_offset: usize,
    register: Register,
    address_low: u16,
) -> Result<()> {
    let expected = [
        Instruction::Lui {
            rt: register,
            immediate: 0x801f,
        },
        Instruction::Lbu {
            rt: register,
            base: register,
            offset: address_low as i16,
        },
    ];
    for (index, instruction) in expected.into_iter().enumerate() {
        ensure_instruction(
            image,
            load_address_offset + index * 4,
            instruction,
            "incremented-byte source",
        )?;
    }
    ensure_instruction(
        image,
        arithmetic_offset,
        Instruction::Addiu {
            rt: register,
            rs: register,
            immediate: 1,
        },
        "incremented-byte arithmetic",
    )?;
    validate_direct_byte_store(image, store_offset, address_low, register)
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
        "upstream direct-store address",
    )?;
    ensure_instruction(
        image,
        store_offset,
        Instruction::Sb {
            rt: source,
            base: Register::AT,
            offset: destination_low as i16,
        },
        "upstream direct byte store",
    )
}

fn validate_main_argument_writer(
    image: &LoadedImage,
    reachable_instruction_offsets: &BTreeSet<usize>,
) -> Result<()> {
    let function_offset = 0xef74;
    let call_offset = 0xede0;
    ensure_instruction(
        image,
        0xedc4,
        Instruction::Lui {
            rt: Register::A0,
            immediate: 0x801f,
        },
        "main forwarded-byte address",
    )?;
    ensure_instruction(
        image,
        0xedc8,
        Instruction::Lbu {
            rt: Register::A0,
            base: Register::A0,
            offset: 0x5c7f,
        },
        "main forwarded-byte load",
    )?;
    ensure_register_is_preserved_between(
        image,
        0xedcc,
        call_offset,
        Register::A0,
        "main forwarded-byte argument",
    )?;
    ensure_instruction(
        image,
        call_offset,
        Instruction::Jal {
            target: runtime_address(image, function_offset)?,
        },
        "main forwarded-byte call",
    )?;
    ensure_only_reachable_direct_entries(
        image,
        reachable_instruction_offsets,
        &[call_offset],
        runtime_address(image, function_offset)?,
        "main forwarded-byte function",
    )?;
    ensure_instruction(
        image,
        0xef7c,
        Instruction::Addu {
            rd: Register::S1,
            rs: Register::A0,
            rt: Register::ZERO,
        },
        "main forwarded-byte preservation",
    )?;
    ensure_register_is_preserved_between(
        image,
        0xef80,
        0xeffc,
        Register::S1,
        "main forwarded byte",
    )?;
    validate_direct_byte_store(image, 0xeffc, 0x6702, Register::S1)
}

fn validate_zero_fill(image: &LoadedImage, store_offset: usize) -> Result<()> {
    let expected = [
        Instruction::Sb {
            rt: Register::ZERO,
            base: Register::A0,
            offset: 0,
        },
        Instruction::Addiu {
            rt: Register::A1,
            rs: Register::A1,
            immediate: -1,
        },
        Instruction::Bgtz {
            rs: Register::A1,
            target: runtime_address(image, store_offset)?,
        },
        Instruction::Addiu {
            rt: Register::A0,
            rs: Register::A0,
            immediate: 1,
        },
    ];
    for (index, instruction) in expected.into_iter().enumerate() {
        ensure_instruction(
            image,
            store_offset + index * 4,
            instruction,
            "upstream zero fill",
        )?;
    }
    Ok(())
}

fn validate_plsel5_table_group_modulo(image: &LoadedImage) -> Result<()> {
    let expected = [
        (
            0x4760,
            Instruction::Lui {
                rt: Register::A0,
                immediate: 0x2492,
            },
        ),
        (
            0x4764,
            Instruction::Ori {
                rt: Register::A0,
                rs: Register::A0,
                immediate: 0x4925,
            },
        ),
        (
            0x4768,
            Instruction::Andi {
                rt: Register::V0,
                rs: Register::V0,
                immediate: u16::MAX,
            },
        ),
        (
            0x476c,
            Instruction::Multu {
                rs: Register::V0,
                rt: Register::A0,
            },
        ),
        (0x477c, Instruction::Mfhi { rd: Register::A0 }),
        (
            0x4780,
            Instruction::Subu {
                rd: Register::V1,
                rs: Register::V0,
                rt: Register::A0,
            },
        ),
        (
            0x4784,
            Instruction::Srl {
                rd: Register::V1,
                rt: Register::V1,
                shift: 1,
            },
        ),
        (
            0x4788,
            Instruction::Addu {
                rd: Register::A0,
                rs: Register::A0,
                rt: Register::V1,
            },
        ),
        (
            0x478c,
            Instruction::Srl {
                rd: Register::A0,
                rt: Register::A0,
                shift: 2,
            },
        ),
        (
            0x4790,
            Instruction::Sll {
                rd: Register::V1,
                rt: Register::A0,
                shift: 3,
            },
        ),
        (
            0x4794,
            Instruction::Subu {
                rd: Register::V1,
                rs: Register::V1,
                rt: Register::A0,
            },
        ),
        (
            0x4798,
            Instruction::Subu {
                rd: Register::V0,
                rs: Register::V0,
                rt: Register::V1,
            },
        ),
    ];
    for (offset, instruction) in expected {
        ensure_instruction(
            image,
            offset,
            instruction,
            "PLSEL5 modulo-seven table group",
        )?;
    }
    validate_direct_byte_store(image, 0x47a0, 0x5c90, Register::V0)
}

fn validate_plsel5_table_loads(image: &LoadedImage) -> Result<()> {
    let expected = [
        (
            0x394c,
            Instruction::Lui {
                rt: Register::V0,
                immediate: 0x801f,
            },
        ),
        (
            0x3950,
            Instruction::Lbu {
                rt: Register::V0,
                base: Register::V0,
                offset: 0x5c38,
            },
        ),
        (
            0x3968,
            Instruction::Andi {
                rt: Register::V1,
                rs: Register::V1,
                immediate: 255,
            },
        ),
        (
            0x396c,
            Instruction::Sltu {
                rd: Register::V0,
                rs: Register::V1,
                rt: Register::V0,
            },
        ),
        (
            0x3970,
            Instruction::Beq {
                rs: Register::V0,
                rt: Register::ZERO,
                target: runtime_address(image, 0x3acc)?,
            },
        ),
        (
            0x3974,
            Instruction::Sll {
                rd: Register::V1,
                rt: Register::V1,
                shift: 1,
            },
        ),
        (
            0x397c,
            Instruction::Lbu {
                rt: Register::V0,
                base: Register::V0,
                offset: 0x5c90,
            },
        ),
        (
            0x3984,
            Instruction::Sll {
                rd: Register::V0,
                rt: Register::V0,
                shift: 3,
            },
        ),
        (
            0x3988,
            Instruction::Addu {
                rd: Register::V1,
                rs: Register::V1,
                rt: Register::V0,
            },
        ),
        (
            0x398c,
            Instruction::Lui {
                rt: Register::AT,
                immediate: 0x800a,
            },
        ),
        (
            0x3990,
            Instruction::Addu {
                rd: Register::AT,
                rs: Register::AT,
                rt: Register::V1,
            },
        ),
        (
            0x3994,
            Instruction::Lbu {
                rt: Register::A0,
                base: Register::AT,
                offset: 0x5590,
            },
        ),
        (
            0x3a14,
            Instruction::Lui {
                rt: Register::V0,
                immediate: 0x801f,
            },
        ),
        (
            0x3a18,
            Instruction::Lbu {
                rt: Register::V0,
                base: Register::V0,
                offset: 0x5c39,
            },
        ),
        (
            0x3a1c,
            Instruction::Lui {
                rt: Register::V1,
                immediate: 0x801f,
            },
        ),
        (
            0x3a20,
            Instruction::Lbu {
                rt: Register::V1,
                base: Register::V1,
                offset: 0x5c90,
            },
        ),
        (
            0x3a24,
            Instruction::Lui {
                rt: Register::A0,
                immediate: 0x800a,
            },
        ),
        (
            0x3a28,
            Instruction::Addiu {
                rt: Register::A0,
                rs: Register::A0,
                immediate: 0x5591,
            },
        ),
        (
            0x3a34,
            Instruction::Sll {
                rd: Register::V0,
                rt: Register::V0,
                shift: 1,
            },
        ),
        (
            0x3a38,
            Instruction::Addu {
                rd: Register::V0,
                rs: Register::V0,
                rt: Register::A0,
            },
        ),
        (
            0x3a3c,
            Instruction::Sll {
                rd: Register::V1,
                rt: Register::V1,
                shift: 3,
            },
        ),
        (
            0x3a40,
            Instruction::Addu {
                rd: Register::V0,
                rs: Register::V0,
                rt: Register::V1,
            },
        ),
        (
            0x3a44,
            Instruction::Lbu {
                rt: Register::A0,
                base: Register::V0,
                offset: 0,
            },
        ),
        (
            0x47a4,
            Instruction::Andi {
                rt: Register::V0,
                rs: Register::V0,
                immediate: 255,
            },
        ),
        (
            0x47a8,
            Instruction::Sll {
                rd: Register::V0,
                rt: Register::V0,
                shift: 3,
            },
        ),
        (
            0x47ac,
            Instruction::Lui {
                rt: Register::AT,
                immediate: 0x800a,
            },
        ),
        (
            0x47b0,
            Instruction::Addu {
                rd: Register::AT,
                rs: Register::AT,
                rt: Register::V0,
            },
        ),
        (
            0x47b4,
            Instruction::Lbu {
                rt: Register::A0,
                base: Register::AT,
                offset: 0x5590,
            },
        ),
        (
            0x4820,
            Instruction::Lui {
                rt: Register::V0,
                immediate: 0x801f,
            },
        ),
        (
            0x4824,
            Instruction::Lbu {
                rt: Register::V0,
                base: Register::V0,
                offset: 0x5c90,
            },
        ),
        (
            0x4830,
            Instruction::Sll {
                rd: Register::V0,
                rt: Register::V0,
                shift: 3,
            },
        ),
        (
            0x4834,
            Instruction::Lui {
                rt: Register::AT,
                immediate: 0x800a,
            },
        ),
        (
            0x4838,
            Instruction::Addu {
                rd: Register::AT,
                rs: Register::AT,
                rt: Register::V0,
            },
        ),
        (
            0x483c,
            Instruction::Lbu {
                rt: Register::A0,
                base: Register::AT,
                offset: 0x5591,
            },
        ),
    ];
    for (offset, instruction) in expected {
        ensure_instruction(image, offset, instruction, "PLSEL5 bounded table load")?;
    }
    Ok(())
}

fn validate_plsel5_search_bounds(image: &LoadedImage) -> Result<()> {
    let expected = [
        (
            0x39c0,
            Instruction::Addu {
                rd: Register::A1,
                rs: Register::ZERO,
                rt: Register::ZERO,
            },
        ),
        (
            0x39c4,
            Instruction::Addu {
                rd: Register::V1,
                rs: Register::ZERO,
                rt: Register::ZERO,
            },
        ),
        (
            0x39e4,
            Instruction::Addiu {
                rt: Register::V1,
                rs: Register::V1,
                immediate: 1,
            },
        ),
        (
            0x39ec,
            Instruction::Sltiu {
                rt: Register::V0,
                rs: Register::V0,
                immediate: 8,
            },
        ),
        (
            0x39f0,
            Instruction::Bne {
                rs: Register::V0,
                rt: Register::ZERO,
                target: runtime_address(image, 0x39d0)?,
            },
        ),
        (
            0x39dc,
            Instruction::Beq {
                rs: Register::V0,
                rt: Register::ZERO,
                target: runtime_address(image, 0x3934)?,
            },
        ),
        (
            0x3934,
            Instruction::J {
                target: runtime_address(image, 0x39f8)?,
            },
        ),
        (
            0x3938,
            Instruction::Addu {
                rd: Register::A1,
                rs: Register::V1,
                rt: Register::ZERO,
            },
        ),
        (
            0x3a70,
            Instruction::Addu {
                rd: Register::A1,
                rs: Register::ZERO,
                rt: Register::ZERO,
            },
        ),
        (
            0x3a74,
            Instruction::Addu {
                rd: Register::V1,
                rs: Register::ZERO,
                rt: Register::ZERO,
            },
        ),
        (
            0x3a94,
            Instruction::Addiu {
                rt: Register::V1,
                rs: Register::V1,
                immediate: 1,
            },
        ),
        (
            0x3a9c,
            Instruction::Sltiu {
                rt: Register::V0,
                rs: Register::V0,
                immediate: 8,
            },
        ),
        (
            0x3aa0,
            Instruction::Bne {
                rs: Register::V0,
                rt: Register::ZERO,
                target: runtime_address(image, 0x3a80)?,
            },
        ),
        (
            0x3a8c,
            Instruction::Beq {
                rs: Register::V0,
                rt: Register::ZERO,
                target: runtime_address(image, 0x393c)?,
            },
        ),
        (
            0x393c,
            Instruction::J {
                target: runtime_address(image, 0x3aa8)?,
            },
        ),
        (
            0x3940,
            Instruction::Addu {
                rd: Register::A1,
                rs: Register::V1,
                rt: Register::ZERO,
            },
        ),
        (
            0x47d8,
            Instruction::Addu {
                rd: Register::A2,
                rs: Register::ZERO,
                rt: Register::ZERO,
            },
        ),
        (
            0x47dc,
            Instruction::Addu {
                rd: Register::S1,
                rs: Register::ZERO,
                rt: Register::ZERO,
            },
        ),
        (
            0x47f4,
            Instruction::Addiu {
                rt: Register::S1,
                rs: Register::S1,
                immediate: 1,
            },
        ),
        (
            0x47f8,
            Instruction::Slti {
                rt: Register::V0,
                rs: Register::S1,
                immediate: 8,
            },
        ),
        (
            0x47fc,
            Instruction::Bne {
                rs: Register::V0,
                rt: Register::ZERO,
                target: runtime_address(image, 0x47e4)?,
            },
        ),
        (
            0x47ec,
            Instruction::Beq {
                rs: Register::V0,
                rt: Register::ZERO,
                target: runtime_address(image, 0x471c)?,
            },
        ),
        (
            0x471c,
            Instruction::J {
                target: runtime_address(image, 0x4804)?,
            },
        ),
        (
            0x4720,
            Instruction::Addu {
                rd: Register::A2,
                rs: Register::S1,
                rt: Register::ZERO,
            },
        ),
        (
            0x4860,
            Instruction::Addu {
                rd: Register::A2,
                rs: Register::ZERO,
                rt: Register::ZERO,
            },
        ),
        (
            0x4864,
            Instruction::Addu {
                rd: Register::S1,
                rs: Register::ZERO,
                rt: Register::ZERO,
            },
        ),
        (
            0x487c,
            Instruction::Addiu {
                rt: Register::S1,
                rs: Register::S1,
                immediate: 1,
            },
        ),
        (
            0x4880,
            Instruction::Slti {
                rt: Register::V0,
                rs: Register::S1,
                immediate: 8,
            },
        ),
        (
            0x4884,
            Instruction::Bne {
                rs: Register::V0,
                rt: Register::ZERO,
                target: runtime_address(image, 0x486c)?,
            },
        ),
        (
            0x4874,
            Instruction::Beq {
                rs: Register::V0,
                rt: Register::ZERO,
                target: runtime_address(image, 0x4724)?,
            },
        ),
        (
            0x4724,
            Instruction::J {
                target: runtime_address(image, 0x488c)?,
            },
        ),
        (
            0x4728,
            Instruction::Addu {
                rd: Register::A2,
                rs: Register::S1,
                rt: Register::ZERO,
            },
        ),
    ];
    for (offset, instruction) in expected {
        ensure_instruction(
            image,
            offset,
            instruction,
            "PLSEL5 bounded zero-slot search",
        )?;
    }
    Ok(())
}

fn table_offsets(fields: &[usize]) -> Vec<usize> {
    (0..7)
        .flat_map(|row| fields.iter().map(move |field| 0x3590 + row * 8 + field))
        .collect()
}

fn table_values(
    image: &LoadedImage,
    offsets: &[usize],
    evidence: &'static str,
) -> Result<ValidatedSelectorWriterEvidence> {
    let runtime_base = image
        .runtime_base
        .context("PLSEL5 table source lacks a runtime base")?;
    let mut values = BTreeSet::new();
    let mut source_runtime_byte_ranges = Vec::with_capacity(offsets.len());
    for &offset in offsets {
        values.insert(
            *image
                .data
                .get(offset)
                .with_context(|| format!("{} table byte +0x{offset:x} is absent", image.path))?,
        );
        let address = runtime_base
            .checked_add(u32::try_from(offset)?)
            .context("PLSEL5 table address overflow")?;
        source_runtime_byte_ranges.push([address, address + 1]);
    }
    Ok(ValidatedSelectorWriterEvidence {
        classification: "embedded_table_lookup",
        value_resolution: "exact_candidates",
        exact_value_candidates: values.into_iter().collect(),
        bounded_value_range: None,
        source_runtime_byte_ranges,
        evidence,
    })
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
        "{image_path} repeats upstream selector writer profile +0x{store_offset:x}"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_candidates_follow_table_bytes_instead_of_locking_asset_values() {
        let mut image = LoadedImage {
            path: "DAT1/PLSEL5.BIN".to_string(),
            data: vec![0; 0x35c4],
            runtime_base: Some(0x800a_2000),
            entrypoints: Vec::new(),
        };
        image.data[0x3590] = 2;
        image.data[0x3598] = 4;

        let original = table_values(&image, &[0x3590, 0x3598], "fixture").unwrap();
        image.data[0x3598] = 6;
        let changed = table_values(&image, &[0x3590, 0x3598], "fixture").unwrap();

        assert_eq!(original.exact_value_candidates, [2, 4]);
        assert_eq!(changed.exact_value_candidates, [2, 6]);
    }
}
