use anyhow::{Context, Result, ensure};
use psx_r3000a::{Instruction, Register, decode};

use super::command_sequence::RECORD_SPECS;
use super::glyph_ownership::{GlyphOwnershipValidation, validate_allocated_glyph_ownership};
pub(super) use crate::bonus_shop_source::{
    DIRECT_SELECTOR_REGION, OVERLAY_RUNTIME_BASE, POINTER_COMMAND_TABLE_RANGES,
};
pub(super) const COMMAND_RENDERER_RUNTIME_ADDRESS: u32 = 0x800a_90fc;
pub(super) const CALLER_RUNTIME_ADDRESS: u32 = 0x800a_7a10;
pub(super) const POINTER_VECTOR_BASE_OFFSET: usize = 0x372c;
pub(super) const POINTER_STRIDE: usize = 40;

pub(super) fn validate_exit_confirmation_consumer(
    source: &[u8],
) -> Result<GlyphOwnershipValidation> {
    validate_exit_confirmation_composed_consumer(source)?;
    validate_allocated_glyph_ownership(source)
}

pub(super) fn validate_exit_confirmation_composed_consumer(source: &[u8]) -> Result<()> {
    for (offset, expected) in source_consumer_instructions() {
        let actual = decode_instruction(source, offset)?;
        ensure!(
            actual == expected,
            "KOUBAI shop exit-confirmation consumer changed at +0x{offset:04x}: expected {expected:?}, found {actual:?}"
        );
    }
    ensure!(
        RECORD_SPECS.map(|spec| spec.clerk_index) == [0, 1],
        "KOUBAI shop exit-confirmation clerk coverage changed"
    );
    for spec in RECORD_SPECS {
        ensure!(
            POINTER_VECTOR_BASE_OFFSET + spec.clerk_index * POINTER_STRIDE
                == spec.pointer_storage_offset,
            "KOUBAI shop exit-confirmation {} clerk-to-pointer table math changed",
            spec.variant_id
        );
        ensure!(
            read_u32(source, spec.pointer_storage_offset)?
                == OVERLAY_RUNTIME_BASE + spec.record_offset as u32,
            "KOUBAI shop exit-confirmation {} pointer changed",
            spec.variant_id
        );
    }
    Ok(())
}

pub(super) fn source_consumer_instructions() -> Vec<(usize, Instruction)> {
    const A0: Register = Register::A0;
    const A1: Register = Register::A1;
    const A2: Register = Register::A2;
    const A3: Register = Register::A3;
    const AT: Register = Register::AT;
    const S0: Register = Register::S0;
    const S1: Register = Register::S1;
    const S2: Register = Register::S2;
    const S3: Register = Register::S3;
    const S4: Register = Register::S4;
    const S6: Register = Register::S6;
    const SP: Register = Register::SP;
    const V0: Register = Register::V0;
    const V1: Register = Register::V1;
    const ZERO: Register = Register::ZERO;

    vec![
        // Clerk selector -> selector * 40 -> pointer vector -> renderer A1.
        addu(0x59e8, A0, S1, ZERO),
        lbu(0x59ec, V1, A0, 0x3b98),
        addu(0x59f0, A2, ZERO, ZERO),
        sw(0x59f4, ZERO, SP, 0x10),
        sll(0x59f8, V0, V1, 2),
        addu(0x59fc, V0, V0, V1),
        sll(0x5a00, V0, V0, 3),
        lui(0x5a04, AT, 0x800a),
        addu(0x5a08, AT, AT, V0),
        lw(0x5a0c, A1, AT, 0x572c),
        renderer_call(0x5a10),
        addu(0x5a14, A3, ZERO, ZERO),
        jump(0x5a18, 0x800a_7a58),
        // Renderer entry and byte-command cursor.
        addiu(0x70fc, SP, SP, -56),
        addu(0x7110, S2, A1, ZERO),
        lbu(0x7140, A2, S2, 0),
        addiu(0x7144, V0, ZERO, 0x81),
        andi(0x7148, V1, A2, 0xff),
        beq(0x714c, V1, V0, 0x72d8),
        // 0x80 consumes one byte and resets x after advancing y by 26.
        addiu(0x7150, S6, ZERO, 0x80),
        andi(0x7158, V1, A2, 0xff),
        bne(0x715c, V1, S6, 0x7174),
        addiu(0x7160, V0, ZERO, 0x63),
        addiu(0x7164, S4, S4, 26),
        addiu(0x7168, S3, ZERO, 80),
        jump(0x716c, 0x800a_92c4),
        addiu(0x7170, S2, S2, 1),
        // 0x63 is a three-byte blank and advances x by 20.
        bne(0x7174, V1, V0, 0x7188),
        addiu(0x717c, S3, S3, 20),
        jump(0x7180, 0x800a_92c4),
        addiu(0x7184, S2, S2, 3),
        // Ordinary page/column/row commands. The renderer adds page base 9.
        addiu(0x7188, S2, S2, 1),
        addiu(0x7190, A2, V1, 9),
        sll(0x7194, A2, A2, 6),
        lbu(0x723c, V1, S2, 0),
        addiu(0x7240, S2, S2, 1),
        sll(0x724c, V0, V1, 2),
        addu(0x7250, V0, V0, V1),
        sll(0x7254, V0, V0, 2),
        sb(0x7258, V0, S0, 0x14),
        lbu(0x725c, V1, S2, 0),
        sll(0x7274, V0, V1, 2),
        addu(0x7278, V0, V0, V1),
        sll(0x727c, V0, V0, 2),
        sb(0x7280, V0, S0, 0x15),
        addiu(0x728c, S2, S2, 1),
        addiu(0x729c, S3, S3, 20),
        // Loop termination reads the next command byte.
        lbu(0x72c4, A2, S2, 0),
        addiu(0x72c8, V0, ZERO, 0x81),
        andi(0x72cc, V1, A2, 0xff),
        bne(0x72d0, V1, V0, 0x7158),
    ]
}

pub(super) fn decode_instruction(source: &[u8], offset: usize) -> Result<Instruction> {
    let bytes = source
        .get(offset..offset + 4)
        .with_context(|| format!("truncated KOUBAI instruction at +0x{offset:04x}"))?;
    decode(
        u32::from_le_bytes(bytes.try_into().expect("four-byte instruction")),
        OVERLAY_RUNTIME_BASE + offset as u32,
    )
    .with_context(|| format!("failed to decode KOUBAI instruction at +0x{offset:04x}"))
}

fn read_u32(source: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        source
            .get(offset..offset + 4)
            .with_context(|| format!("truncated KOUBAI word at +0x{offset:04x}"))?
            .try_into()?,
    ))
}

fn addiu(offset: usize, rt: Register, rs: Register, immediate: i16) -> (usize, Instruction) {
    (offset, Instruction::Addiu { rt, rs, immediate })
}

fn addu(offset: usize, rd: Register, rs: Register, rt: Register) -> (usize, Instruction) {
    (offset, Instruction::Addu { rd, rs, rt })
}

fn sll(offset: usize, rd: Register, rt: Register, shift: u8) -> (usize, Instruction) {
    (offset, Instruction::Sll { rd, rt, shift })
}

fn lui(offset: usize, rt: Register, immediate: u16) -> (usize, Instruction) {
    (offset, Instruction::Lui { rt, immediate })
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

fn andi(offset: usize, rt: Register, rs: Register, immediate: u16) -> (usize, Instruction) {
    (offset, Instruction::Andi { rt, rs, immediate })
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

fn renderer_call(offset: usize) -> (usize, Instruction) {
    (
        offset,
        Instruction::Jal {
            target: COMMAND_RENDERER_RUNTIME_ADDRESS,
        },
    )
}

fn jump(offset: usize, target: u32) -> (usize, Instruction) {
    (offset, Instruction::J { target })
}
