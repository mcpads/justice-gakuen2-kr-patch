use anyhow::{Context, Result, ensure};
use psx_r3000a::{Instruction, Register, decode};

use super::command_sequence::RETURN_LABEL_SEQUENCE;
use super::glyph_ownership::{GlyphOwnershipValidation, validate_allocated_glyph_ownership};

pub(super) const OVERLAY_RUNTIME_BASE: u32 = 0x800a_2000;
pub(super) const POINTER_TABLE_OFFSET: usize = 0x11e0;
pub(super) const POINTER_TABLE_END: usize = 0x1418;
pub(super) const DIRECT_SELECTOR_REGION: [usize; 2] = [POINTER_TABLE_END, 0x3afc];
pub(super) const COMMAND_RENDERER_RUNTIME_ADDRESS: u32 = 0x800a_b5ac;
pub(super) const CALLER_RUNTIME_ADDRESSES: [u32; 2] = [0x800a_ea98, 0x800b_04b8];

const EXECUTABLE_REGION_START: usize = 0x3afc;

pub(super) fn validate_return_label_consumer(source: &[u8]) -> Result<GlyphOwnershipValidation> {
    validate_return_label_composed_consumer(source)?;
    validate_allocated_glyph_ownership(source)
}

pub(super) fn validate_return_label_composed_consumer(source: &[u8]) -> Result<()> {
    for (offset, expected) in source_consumer_instructions() {
        let actual = decode_instruction(source, offset)?;
        ensure!(
            actual == expected,
            "KOUBAI2 J-BANK return-label consumer changed at +0x{offset:04x}: expected {expected:?}, found {actual:?}"
        );
    }
    ensure!(
        read_u32(source, RETURN_LABEL_SEQUENCE.pointer_storage_offset)?
            == OVERLAY_RUNTIME_BASE + RETURN_LABEL_SEQUENCE.sequence_offset as u32,
        "KOUBAI2 J-BANK return-label pointer changed"
    );
    ensure!(
        pointer_load_offsets(source, 0x33b4) == [0xca90, 0xe4b0],
        "KOUBAI2 J-BANK return-label pointer load-site set changed"
    );
    Ok(())
}

pub(super) fn source_consumer_instructions() -> Vec<(usize, Instruction)> {
    const A0: Register = Register::A0;
    const A1: Register = Register::A1;
    const A2: Register = Register::A2;
    const A3: Register = Register::A3;
    const S0: Register = Register::S0;
    const S2: Register = Register::S2;
    const S3: Register = Register::S3;
    const S6: Register = Register::S6;
    const S7: Register = Register::S7;
    const SP: Register = Register::SP;
    const V0: Register = Register::V0;
    const V1: Register = Register::V1;
    const ZERO: Register = Register::ZERO;

    vec![
        // Renderer grammar and fixed 20-pixel glyph advance.
        addu(0x95bc, S2, A1, ZERO),
        addu(0x95cc, S7, A3, ZERO),
        lbu(0x9618, A2, S2, 0),
        addiu(0x9634, V0, ZERO, 0x81),
        andi(0x9638, V1, A2, 0xff),
        beq(0x963c, V1, V0, 0x9838),
        addiu(0x9640, S6, ZERO, 0x80),
        andi(0x9644, V1, A2, 0xff),
        bne(0x9648, V1, S6, 0x96a0),
        addiu(0x964c, V0, ZERO, 0x63),
        addiu(0x9654, S2, S2, 1),
        bne(0x96a0, V1, V0, 0x96b4),
        addu(0x96a4, A0, ZERO, ZERO),
        addiu(0x96a8, S3, S3, 20),
        jump(0x96ac, 0x800a_b824),
        addiu(0x96b0, S2, S2, 3),
        // Ordinary glyph commands consume page, column, and row bytes, then advance 20 px.
        addiu(0x96b4, S2, S2, 1),
        lbu(0x979c, V1, S2, 0),
        addiu(0x97a0, S2, S2, 1),
        sb(0x97b8, V0, S0, 0x14),
        lbu(0x97bc, V1, S2, 0),
        sb(0x97e0, V0, S0, 0x15),
        addiu(0x97ec, S2, S2, 1),
        addiu(0x97fc, S3, S3, 20),
        lbu(0x9824, A2, S2, 0),
        addiu(0x9828, V0, ZERO, 0x81),
        andi(0x982c, V1, A2, 0xff),
        bne(0x9830, V1, V0, 0x9644),
        // First J-BANK return-label caller.
        lui(0xca8c, A1, 0x800a),
        lw(0xca90, A1, A1, 0x33b4),
        addiu(0xca94, A3, ZERO, 0x0110),
        renderer_call(0xca98),
        sw(0xca9c, ZERO, SP, 0x10),
        // Second J-BANK return-label caller.
        lui(0xe4ac, A1, 0x800a),
        lw(0xe4b0, A1, A1, 0x33b4),
        addiu(0xe4b4, A3, ZERO, 0x0110),
        renderer_call(0xe4b8),
        sw(0xe4bc, ZERO, SP, 0x10),
    ]
}

pub(super) fn decode_instruction(source: &[u8], offset: usize) -> Result<Instruction> {
    let bytes = source
        .get(offset..offset + 4)
        .with_context(|| format!("truncated KOUBAI2 instruction at +0x{offset:04x}"))?;
    decode(
        u32::from_le_bytes(bytes.try_into().expect("four-byte instruction")),
        OVERLAY_RUNTIME_BASE + offset as u32,
    )
    .with_context(|| format!("failed to decode KOUBAI2 instruction at +0x{offset:04x}"))
}

fn pointer_load_offsets(source: &[u8], displacement: i16) -> Vec<usize> {
    (EXECUTABLE_REGION_START..source.len().saturating_sub(3))
        .step_by(4)
        .filter(|offset| {
            matches!(
                decode_instruction(source, *offset),
                Ok(Instruction::Lw {
                    rt: Register::A1,
                    base: Register::A1,
                    offset,
                }) if offset == displacement
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

fn addu(offset: usize, rd: Register, rs: Register, rt: Register) -> (usize, Instruction) {
    (offset, Instruction::Addu { rd, rs, rt })
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
