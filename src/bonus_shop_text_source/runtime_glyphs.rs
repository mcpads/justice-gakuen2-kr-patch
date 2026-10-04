use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};
use psx_r3000a::{Instruction, Register, decode};

use crate::bonus_shop_source::OVERLAY_RUNTIME_BASE;

const DECIMAL_DIGIT_LOOKUP_OFFSET: usize = 0x3a14;
const DECIMAL_DIGIT_COLUMNS: [u8; 10] = [9, 0, 1, 2, 3, 4, 5, 6, 7, 8];
const PAGE_COUNTER_SEPARATOR_CODE: u16 = 0x0059;
const PRODUCT_HEADING_QUOTE_CODES: [u16; 2] = [0x0197, 0x0198];

pub(super) fn protected_runtime_glyph_codes(source_overlay: &[u8]) -> Result<BTreeSet<u16>> {
    validate_runtime_glyph_consumers(source_overlay)?;

    Ok(DECIMAL_DIGIT_COLUMNS
        .into_iter()
        .map(u16::from)
        .chain([PAGE_COUNTER_SEPARATOR_CODE])
        .chain(PRODUCT_HEADING_QUOTE_CODES)
        .collect())
}

fn validate_runtime_glyph_consumers(source: &[u8]) -> Result<()> {
    ensure!(
        source.get(
            DECIMAL_DIGIT_LOOKUP_OFFSET..DECIMAL_DIGIT_LOOKUP_OFFSET + DECIMAL_DIGIT_COLUMNS.len()
        ) == Some(DECIMAL_DIGIT_COLUMNS.as_slice()),
        "KOUBAI decimal digit-to-atlas lookup changed"
    );
    for (offset, expected) in source_consumer_instructions() {
        let actual = decode_instruction(source, offset)?;
        ensure!(
            actual == expected,
            "KOUBAI runtime glyph consumer changed at +0x{offset:04x}: expected {expected:?}, found {actual:?}"
        );
    }
    Ok(())
}

pub(super) fn source_consumer_instructions() -> Vec<(usize, Instruction)> {
    const A0: Register = Register::A0;
    const A1: Register = Register::A1;
    const AT: Register = Register::AT;
    const S0: Register = Register::S0;
    const S2: Register = Register::S2;
    const SP: Register = Register::SP;
    const T0: Register = Register::T0;
    const T1: Register = Register::T1;
    const V0: Register = Register::V0;
    const V1: Register = Register::V1;
    const ZERO: Register = Register::ZERO;

    vec![
        // The heading renderer draws quotes before/after the label without
        // reading a three-byte selector. These cells remain live even when
        // every pointer-table record containing quotes has been translated.
        addiu(0x7aa8, Register::A2, ZERO, 0x0280),
        addiu(0x7b70, V0, ZERO, 0x008c),
        sb(0x7b74, V0, S0, 0x14),
        addiu(0x7b78, V0, ZERO, 0x00b4),
        sb(0x7b7c, V0, S0, 0x15),
        addiu(0x7d80, Register::A2, ZERO, 0x0280),
        addiu(0x7e3c, V0, ZERO, 0x00a0),
        sb(0x7e40, V0, S0, 0x14),
        addiu(0x7e44, V0, ZERO, 0x00b4),
        sb(0x7e48, V0, S0, 0x15),
        // Shop prices divide a packed decimal value by ten, then use the
        // source table at 0x3a14 to select row-zero digit cells.
        lbu(0x8128, V0, V1, 0x3c2f),
        lui(0x812c, T0, 0xcccc),
        ori(0x8130, T0, T0, 0xcccd),
        multu(0x8134, V0, T0),
        mfhi(0x8138, T0),
        srl(0x813c, V0, T0, 3),
        sw(0x8144, V0, SP, 0x10),
        lbu(0x8148, A0, V1, 0x3c2f),
        multu(0x8154, A0, T0),
        mfhi(0x815c, T0),
        srl(0x8160, V1, T0, 3),
        sll(0x8164, V0, V1, 2),
        addu(0x8168, V0, V0, V1),
        sll(0x816c, V0, V0, 1),
        subu(0x8170, A0, A0, V0),
        addiu(0x8268, A1, ZERO, 0x01e3),
        lui(0x82ac, AT, 0x800a),
        addu(0x82b0, AT, AT, V0),
        lbu(0x82b4, V1, AT, 0x5a14),
        sb(0x82bc, ZERO, S0, 0x15),
        sll(0x82d0, V0, V1, 2),
        addu(0x82d4, V0, V0, V1),
        sll(0x82d8, V0, V0, 2),
        sb(0x82e0, V0, S0, 0x14),
        // The upper-right item/page counter performs the same decimal lookup
        // for both numbers surrounding its fixed slash glyph.
        addiu(0x686c, S2, S2, 1),
        lui(0x6870, V0, 0x6666),
        ori(0x6874, V0, V0, 0x6667),
        mult(0x6878, S2, V0),
        sra(0x6888, V0, S2, 31),
        mfhi(0x688c, T1),
        sra(0x6890, V1, T1, 2),
        subu(0x6894, V1, V1, V0),
        sll(0x6898, V0, V1, 2),
        addu(0x689c, V0, V0, V1),
        sll(0x68a0, V0, V0, 1),
        subu(0x68a4, V0, S2, V0),
        addiu(0x6954, A1, ZERO, 0x01e3),
        lui(0x6980, AT, 0x800a),
        addu(0x6984, AT, AT, V0),
        lbu(0x6988, V1, AT, 0x5a14),
        sb(0x6990, ZERO, S0, 0x15),
        sll(0x69a4, V0, V1, 2),
        addu(0x69a8, V0, V0, V1),
        sll(0x69ac, V0, V0, 2),
        sb(0x69b0, V0, S0, 0x14),
        addiu(0x6ab0, A1, ZERO, 0x01e3),
        addiu(0x6acc, V0, ZERO, 0x00b4),
        sb(0x6ad0, V0, S0, 0x14),
        addiu(0x6ad4, V0, ZERO, 0x0064),
        sb(0x6adc, V0, S0, 0x15),
        addiu(0x6bec, A1, ZERO, 0x01e3),
        lui(0x6c14, AT, 0x800a),
        addu(0x6c18, AT, AT, V0),
        lbu(0x6c1c, V1, AT, 0x5a14),
        sb(0x6c28, ZERO, S0, 0x15),
        sll(0x6c3c, V0, V1, 2),
        addu(0x6c40, V0, V0, V1),
        sll(0x6c44, V0, V0, 2),
        sb(0x6c48, V0, S0, 0x14),
    ]
}

fn decode_instruction(source: &[u8], offset: usize) -> Result<Instruction> {
    let bytes = source
        .get(offset..offset + 4)
        .with_context(|| format!("truncated KOUBAI instruction at +0x{offset:04x}"))?;
    decode(
        u32::from_le_bytes(bytes.try_into().expect("four-byte instruction")),
        OVERLAY_RUNTIME_BASE + offset as u32,
    )
    .with_context(|| format!("failed to decode KOUBAI instruction at +0x{offset:04x}"))
}

fn addiu(offset: usize, rt: Register, rs: Register, immediate: i16) -> (usize, Instruction) {
    (offset, Instruction::Addiu { rt, rs, immediate })
}

fn addu(offset: usize, rd: Register, rs: Register, rt: Register) -> (usize, Instruction) {
    (offset, Instruction::Addu { rd, rs, rt })
}

fn subu(offset: usize, rd: Register, rs: Register, rt: Register) -> (usize, Instruction) {
    (offset, Instruction::Subu { rd, rs, rt })
}

fn sll(offset: usize, rd: Register, rt: Register, shift: u8) -> (usize, Instruction) {
    (offset, Instruction::Sll { rd, rt, shift })
}

fn srl(offset: usize, rd: Register, rt: Register, shift: u8) -> (usize, Instruction) {
    (offset, Instruction::Srl { rd, rt, shift })
}

fn sra(offset: usize, rd: Register, rt: Register, shift: u8) -> (usize, Instruction) {
    (offset, Instruction::Sra { rd, rt, shift })
}

fn lui(offset: usize, rt: Register, immediate: u16) -> (usize, Instruction) {
    (offset, Instruction::Lui { rt, immediate })
}

fn ori(offset: usize, rt: Register, rs: Register, immediate: u16) -> (usize, Instruction) {
    (offset, Instruction::Ori { rt, rs, immediate })
}

fn mult(offset: usize, rs: Register, rt: Register) -> (usize, Instruction) {
    (offset, Instruction::Mult { rs, rt })
}

fn multu(offset: usize, rs: Register, rt: Register) -> (usize, Instruction) {
    (offset, Instruction::Multu { rs, rt })
}

fn mfhi(offset: usize, rd: Register) -> (usize, Instruction) {
    (offset, Instruction::Mfhi { rd })
}

fn lbu(offset: usize, rt: Register, base: Register, displacement: i16) -> (usize, Instruction) {
    (
        offset,
        Instruction::Lbu {
            rt,
            base,
            offset: displacement,
        },
    )
}

fn sw(offset: usize, rt: Register, base: Register, displacement: i16) -> (usize, Instruction) {
    (
        offset,
        Instruction::Sw {
            rt,
            base,
            offset: displacement,
        },
    )
}

fn sb(offset: usize, rt: Register, base: Register, displacement: i16) -> (usize, Instruction) {
    (
        offset,
        Instruction::Sb {
            rt,
            base,
            offset: displacement,
        },
    )
}
