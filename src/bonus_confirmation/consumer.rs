use anyhow::{Context, Result, ensure};
use psx_r3000a::{Instruction, Register, decode};

use super::glyph_ownership::validate_allocated_glyph_ownership;

pub(super) const OVERLAY_RUNTIME_BASE: u32 = 0x800a_2000;
pub(super) const COMPOSER_RUNTIME_ADDRESS: u32 = 0x800a_ab54;
pub(super) const COMMAND_RENDERER_RUNTIME_ADDRESS: u32 = 0x800a_b5ac;
pub(super) const EXIT_CALLER_RUNTIME_ADDRESS: u32 = 0x800a_b0cc;
pub(super) const DIRECT_CALLER_RUNTIME_ADDRESSES: [u32; 3] =
    [0x800a_b0cc, 0x800a_b348, 0x800a_fac4];
pub(super) const DIRECT_CALLER_SELECTOR_VALUES: [u8; 3] = [0, 1, 2];

pub(super) const MEMORY_CARD_DESTINATION_POINTER_OFFSET: usize = 0x13ec;
pub(super) const MEMORY_CARD_COPY_PROMPT_POINTER_OFFSET: usize = 0x13f0;

pub(super) const CHOICE_X_ORIGIN_INSTRUCTION_OFFSET: usize = 0x8d70;
pub(super) const CHOICE_SEPARATOR_INDEX_INSTRUCTION_OFFSET: usize = 0x8ed4;
pub(super) const CHOICE_LOOP_BOUND_INSTRUCTION_OFFSET: usize = 0x8ee8;

pub(super) fn validate_confirmation_source_consumer(source: &[u8]) -> Result<()> {
    validate_confirmation_composed_consumer(source)?;
    for (offset, expected) in source_stock_renderer_evidence() {
        let actual = decode_instruction(source, offset)?;
        ensure!(
            actual == expected,
            "KOUBAI2 source stock-label evidence changed at +0x{offset:04x}: expected {expected:?}, found {actual:?}"
        );
    }
    Ok(())
}

pub(super) fn validate_confirmation_composed_consumer(source: &[u8]) -> Result<()> {
    for (offset, expected) in source_consumer_instructions() {
        let actual = decode_instruction(source, offset)?;
        ensure!(
            actual == expected,
            "KOUBAI2 confirmation consumer changed at +0x{offset:04x}: expected {expected:?}, found {actual:?}"
        );
    }
    ensure!(
        direct_composer_call_offsets(source) == [0x90cc, 0x9348, 0xdac4],
        "KOUBAI2 confirmation composer direct caller set changed"
    );
    ensure_command_pointers(source)?;
    validate_allocated_glyph_ownership(source)
}

pub(super) fn source_consumer_instructions() -> Vec<(usize, Instruction)> {
    const A0: Register = Register::A0;
    const A1: Register = Register::A1;
    const A2: Register = Register::A2;
    const A3: Register = Register::A3;
    const S1: Register = Register::S1;
    const S3: Register = Register::S3;
    const S4: Register = Register::S4;
    const S6: Register = Register::S6;
    const SP: Register = Register::SP;
    const V0: Register = Register::V0;
    const V1: Register = Register::V1;
    const ZERO: Register = Register::ZERO;

    vec![
        addiu(0x8b54, SP, SP, -96),
        andi(0x8b5c, A0, A2, 0xff),
        beq(0x8b8c, A0, ZERO, 0x8ba8),
        beq(0x8b98, A0, V0, 0x8bb8),
        lui(0x8ba8, S1, 0x800a),
        addiu(0x8bac, S1, S1, 0x367c),
        lui(0x8bb8, S1, 0x800a),
        addiu(0x8bbc, S1, S1, 0x3694),
        addiu(0x8bc0, V0, ZERO, 2),
        beq(0x8bc4, V1, V0, 0x8d68),
        lbu(0x8bcc, S6, S1, 0),
        addiu(0x8bd0, S1, S1, 1),
        sll(0x8be8, V1, S6, 2),
        addu(0x8bec, V1, V1, S6),
        sll(0x8bf0, V1, V1, 2),
        addiu(0x8bf4, V0, ZERO, 512),
        subu(0x8bf8, V0, V0, V1),
        lbu(0x8c10, A2, S1, 0),
        lbu(0x8cd4, V1, S1, 0),
        lbu(0x8cf4, V1, S1, 0),
        slt(0x8d5c, V0, S4, S6),
        bne(0x8d60, V0, ZERO, 0x8c10),
        lui(0x8d68, S1, 0x800a),
        addiu(0x8d6c, S1, S1, 0x36c0),
        addiu(CHOICE_X_ORIGIN_INSTRUCTION_OFFSET, S3, ZERO, 186),
        lbu(0x8d90, A2, S1, 0),
        lbu(0x8e4c, V1, S1, 0),
        lbu(0x8e6c, V1, S1, 0),
        addiu(0x8eac, S3, S3, 20),
        addiu(CHOICE_SEPARATOR_INDEX_INSTRUCTION_OFFSET, V0, ZERO, 1),
        bne(0x8ed8, S4, V0, 0x8ee4),
        addiu(0x8ee0, S3, S3, 40),
        addiu(0x8ee4, S4, S4, 1),
        slti(CHOICE_LOOP_BOUND_INSTRUCTION_OFFSET, V0, S4, 5),
        bne(0x8eec, V0, ZERO, 0x8d90),
        lhu(0x90b0, V1, S4, 2),
        addiu(0x90b4, V0, ZERO, 10),
        addu(0x90c8, A2, ZERO, ZERO),
        composer_call(0x90cc),
        addu(0x90d0, A3, ZERO, ZERO),
        lhu(0x9330, V1, S4, 2),
        addiu(0x9334, V0, ZERO, 2),
        bne(0x9338, V1, V0, 0x9350),
        addu(0x933c, A0, S4, ZERO),
        lbu(0x9340, A1, A0, 0x29),
        addiu(0x9344, A2, ZERO, 1),
        composer_call(0x9348),
        addiu(0x934c, A3, ZERO, 120),
        addu(0xda84, A2, ZERO, ZERO),
        lui(0xda88, A1, 0x800a),
        lw(0xda8c, A1, A1, 0x33ec),
        addiu(0xda90, A3, ZERO, 0x0136),
        command_renderer_call(0xda94),
        sw(0xda98, ZERO, SP, 0x10),
        addu(0xda9c, A0, S1, ZERO),
        addiu(0xdaa0, A2, ZERO, 2),
        lui(0xdaa4, A1, 0x800a),
        lw(0xdaa8, A1, A1, 0x33f0),
        addiu(0xdaac, A3, ZERO, 0x014e),
        command_renderer_call(0xdab0),
        sw(0xdab4, ZERO, SP, 0x10),
        addu(0xdab8, A0, S1, ZERO),
        addiu(0xdabc, A2, ZERO, 2),
        lbu(0xdac0, A1, S1, 0x2f),
        composer_call(0xdac4),
        addiu(0xdac8, A3, ZERO, 120),
    ]
}

pub(super) fn source_stock_renderer_evidence() -> Vec<(usize, Instruction)> {
    const A2: Register = Register::A2;
    const S0: Register = Register::S0;
    const S1: Register = Register::S1;
    const S3: Register = Register::S3;
    const S4: Register = Register::S4;
    const S5: Register = Register::S5;
    const T0: Register = Register::T0;
    const V0: Register = Register::V0;
    const ZERO: Register = Register::ZERO;

    vec![
        addiu(0x66c8, S5, ZERO, 282),
        addu(0x66cc, S3, ZERO, ZERO),
        addiu(0x66d8, S1, ZERO, 1),
        bne(0x66dc, S3, S1, 0x66ec),
        addiu(0x66e0, S4, ZERO, 7),
        addiu(0x66e4, S1, ZERO, 10),
        addiu(0x66e8, S4, ZERO, 6),
        addiu(0x673c, A2, ZERO, 0x0280),
        addiu(0x67a0, T0, ZERO, 107),
        sll(0x67a8, V0, S1, 2),
        addu(0x67ac, V0, V0, S1),
        sll(0x67b0, V0, V0, 2),
        sb(0x67b4, V0, S0, 0x14),
        sll(0x67b8, V0, S4, 2),
        addu(0x67bc, V0, V0, S4),
        sll(0x67c0, V0, V0, 2),
        sb(0x67c4, V0, S0, 0x15),
        addiu(0x67c8, V0, ZERO, 20),
        sh(0x67cc, S5, S0, 0x10),
        sh(0x67d0, T0, S0, 0x12),
        sh(0x67d4, V0, S0, 0x18),
        sh(0x67d8, V0, S0, 0x1a),
        addiu(0x67f4, S5, S5, 20),
        addiu(0x66fc, S3, S3, 1),
        slti(0x681c, V0, S3, 2),
        bne(0x6820, V0, ZERO, 0x66dc),
        addiu(0x6824, S1, ZERO, 1),
    ]
}

pub(super) fn decode_instruction(source: &[u8], offset: usize) -> Result<Instruction> {
    let bytes = source
        .get(offset..offset + 4)
        .with_context(|| format!("truncated KOUBAI2 instruction at +0x{offset:04x}"))?;
    let word = u32::from_le_bytes(bytes.try_into().expect("four-byte instruction"));
    decode(word, OVERLAY_RUNTIME_BASE + offset as u32)
        .with_context(|| format!("failed to decode KOUBAI2 instruction at +0x{offset:04x}"))
}

fn ensure_command_pointers(source: &[u8]) -> Result<()> {
    for (offset, expected) in [
        (MEMORY_CARD_DESTINATION_POINTER_OFFSET, 0x800a_2fe4),
        (MEMORY_CARD_COPY_PROMPT_POINTER_OFFSET, 0x800a_3004),
    ] {
        ensure!(
            read_u32(source, offset)? == expected,
            "KOUBAI2 memory-card command pointer changed at +0x{offset:04x}"
        );
    }
    Ok(())
}

fn direct_composer_call_offsets(source: &[u8]) -> Vec<usize> {
    (0..source.len().saturating_sub(3))
        .step_by(4)
        .filter(|offset| {
            matches!(
                decode_instruction(source, *offset),
                Ok(Instruction::Jal {
                    target: COMPOSER_RUNTIME_ADDRESS,
                })
            )
        })
        .collect()
}

fn read_u32(source: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        source
            .get(offset..offset + 4)
            .with_context(|| format!("truncated KOUBAI2 word at +0x{offset:04x}"))?
            .try_into()?,
    ))
}

fn addiu(offset: usize, rt: Register, rs: Register, immediate: i16) -> (usize, Instruction) {
    (offset, Instruction::Addiu { rt, rs, immediate })
}

fn andi(offset: usize, rt: Register, rs: Register, immediate: u16) -> (usize, Instruction) {
    (offset, Instruction::Andi { rt, rs, immediate })
}

fn lui(offset: usize, rt: Register, immediate: u16) -> (usize, Instruction) {
    (offset, Instruction::Lui { rt, immediate })
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

fn lhu(offset: usize, rt: Register, base: Register, displacement: i16) -> (usize, Instruction) {
    (
        offset,
        Instruction::Lhu {
            rt,
            base,
            offset: displacement,
        },
    )
}

fn lw(offset: usize, rt: Register, base: Register, displacement: i16) -> (usize, Instruction) {
    (
        offset,
        Instruction::Lw {
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

fn sh(offset: usize, rt: Register, base: Register, displacement: i16) -> (usize, Instruction) {
    (
        offset,
        Instruction::Sh {
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

fn sll(offset: usize, rd: Register, rt: Register, shift: u8) -> (usize, Instruction) {
    (offset, Instruction::Sll { rd, rt, shift })
}

fn addu(offset: usize, rd: Register, rs: Register, rt: Register) -> (usize, Instruction) {
    (offset, Instruction::Addu { rd, rs, rt })
}

fn subu(offset: usize, rd: Register, rs: Register, rt: Register) -> (usize, Instruction) {
    (offset, Instruction::Subu { rd, rs, rt })
}

fn slt(offset: usize, rd: Register, rs: Register, rt: Register) -> (usize, Instruction) {
    (offset, Instruction::Slt { rd, rs, rt })
}

fn slti(offset: usize, rt: Register, rs: Register, immediate: i16) -> (usize, Instruction) {
    (offset, Instruction::Slti { rt, rs, immediate })
}

fn beq(offset: usize, rs: Register, rt: Register, target: usize) -> (usize, Instruction) {
    (
        offset,
        Instruction::Beq {
            rs,
            rt,
            target: OVERLAY_RUNTIME_BASE + target as u32,
        },
    )
}

fn bne(offset: usize, rs: Register, rt: Register, target: usize) -> (usize, Instruction) {
    (
        offset,
        Instruction::Bne {
            rs,
            rt,
            target: OVERLAY_RUNTIME_BASE + target as u32,
        },
    )
}

fn composer_call(offset: usize) -> (usize, Instruction) {
    (
        offset,
        Instruction::Jal {
            target: COMPOSER_RUNTIME_ADDRESS,
        },
    )
}

fn command_renderer_call(offset: usize) -> (usize, Instruction) {
    (
        offset,
        Instruction::Jal {
            target: COMMAND_RENDERER_RUNTIME_ADDRESS,
        },
    )
}
