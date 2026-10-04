use anyhow::{Context, Result, ensure};
use psx_r3000a::{Instruction, Register, decode};

use super::command_sequences::CARD_ACQUISITION_SEQUENCES;
use super::glyph_ownership::{GlyphOwnershipValidation, validate_allocated_glyph_ownership};

pub(super) const OVERLAY_RUNTIME_BASE: u32 = 0x800a_2000;
pub(super) const POINTER_TABLE_OFFSET: usize = 0x11e0;
pub(super) const POINTER_TABLE_END: usize = 0x1418;
pub(super) const SECONDARY_PARSER_RUNTIME_ADDRESS: u32 = 0x800a_b374;
pub(super) const CARD_NUMBER_RENDERER_RUNTIME_ADDRESS: u32 = 0x800b_0520;
pub(super) const FOLLOWING_POINTER_STORAGE_OFFSET: usize = 0x1400;
pub(super) const FOLLOWING_SEQUENCE_OFFSET: usize = 0x106c;
pub(super) const CALLER_RUNTIME_ADDRESSES: [u32; 5] = [
    0x800a_f344,
    0x800a_f370,
    0x800a_f410,
    0x800a_f42c,
    0x800a_f478,
];

pub(super) fn validate_card_acquisition_consumer(
    source: &[u8],
) -> Result<GlyphOwnershipValidation> {
    validate_card_acquisition_composed_consumer(source)?;
    validate_allocated_glyph_ownership(source)
}

pub(super) fn validate_card_acquisition_composed_consumer(source: &[u8]) -> Result<()> {
    for (offset, expected) in source_consumer_instructions() {
        let actual = decode_instruction(source, offset)?;
        ensure!(
            actual == expected,
            "KOUBAI2 card-acquisition consumer changed at +0x{offset:04x}: expected {expected:?}, found {actual:?}"
        );
    }
    ensure_command_pointers(source)?;
    ensure!(
        card_number_renderer_call_offsets(source) == [0xd3a4],
        "KOUBAI2 card-number renderer caller set changed"
    );
    Ok(())
}

pub(super) fn source_consumer_instructions() -> Vec<(usize, Instruction)> {
    const A1: Register = Register::A1;
    const A2: Register = Register::A2;
    const A3: Register = Register::A3;
    const S2: Register = Register::S2;
    const S3: Register = Register::S3;
    const S4: Register = Register::S4;
    const SP: Register = Register::SP;
    const V0: Register = Register::V0;
    const V1: Register = Register::V1;
    const ZERO: Register = Register::ZERO;

    vec![
        // Full secondary command parser grammar.
        addiu(0x9374, SP, SP, -64),
        addu(0x937c, S2, A1, ZERO),
        addiu(0x9394, V0, ZERO, 0x81),
        lbu(0x93b4, A2, S2, 0),
        andi(0x93bc, V1, A2, 0xff),
        beq(0x93c0, V1, V0, 0x9578),
        addiu(0x93e8, S2, S2, 1),
        addiu(0x93fc, S2, S2, 3),
        addiu(0x9400, S2, S2, 1),
        lbu(0x94dc, V1, S2, 0),
        addiu(0x94e0, S2, S2, 1),
        lbu(0x94fc, V1, S2, 0),
        addiu(0x952c, S2, S2, 1),
        lbu(0x9564, A2, S2, 0),
        bne(0x9570, V1, V0, 0x93d0),
        // First rare label and card-number prefix.
        addu(0xd328, A2, ZERO, ZERO),
        addiu(0xd32c, A3, ZERO, 48),
        lui(0xd334, A1, 0x800a),
        lw(0xd338, A1, A1, 0x33f4),
        parser_call(0xd344),
        sw(0xd348, S3, SP, 0x10),
        addiu(0xd350, A2, ZERO, 1),
        addu(0xd354, A3, S4, ZERO),
        lui(0xd35c, V0, 0x800a),
        addiu(0xd360, V0, V0, 0x33f8),
        lw(0xd368, A1, V0, 0),
        parser_call(0xd370),
        sw(0xd374, S3, SP, 0x10),
        card_number_renderer_call(0xd3a4),
        // Second rare label and card-number prefix.
        addiu(0xd3f0, A2, ZERO, 2),
        addiu(0xd3f4, A3, ZERO, 48),
        lui(0xd400, A1, 0x800a),
        lw(0xd404, A1, A1, 0x33f4),
        parser_call(0xd410),
        sw(0xd414, S3, SP, 0x10),
        addiu(0xd41c, A2, ZERO, 3),
        lui(0xd420, A1, 0x800a),
        lw(0xd424, A1, A1, 0x33f8),
        addiu(0xd428, A3, ZERO, 88),
        parser_call(0xd42c),
        sw(0xd430, S3, SP, 0x10),
        // The game supplies the gap before the acquisition suffix.
        addiu(0xd464, S3, S3, 20),
        addiu(0xd468, A2, ZERO, 4),
        lui(0xd46c, A1, 0x800a),
        lw(0xd470, A1, A1, 0x33fc),
        addiu(0xd474, A3, ZERO, 48),
        parser_call(0xd478),
        sw(0xd47c, S3, SP, 0x10),
    ]
}

fn ensure_command_pointers(source: &[u8]) -> Result<()> {
    for spec in CARD_ACQUISITION_SEQUENCES {
        ensure!(
            read_u32(source, spec.pointer_storage_offset)?
                == OVERLAY_RUNTIME_BASE + spec.sequence_offset as u32,
            "KOUBAI2 card-acquisition pointer changed for {}",
            spec.id
        );
    }
    ensure!(
        read_u32(source, FOLLOWING_POINTER_STORAGE_OFFSET)?
            == OVERLAY_RUNTIME_BASE + FOLLOWING_SEQUENCE_OFFSET as u32,
        "KOUBAI2 card-acquisition trailing sequence boundary changed"
    );
    Ok(())
}

fn card_number_renderer_call_offsets(source: &[u8]) -> Vec<usize> {
    (0..source.len().saturating_sub(3))
        .step_by(4)
        .filter(|offset| {
            matches!(
                decode_instruction(source, *offset),
                Ok(Instruction::Jal {
                    target: CARD_NUMBER_RENDERER_RUNTIME_ADDRESS,
                })
            )
        })
        .collect()
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

fn parser_call(offset: usize) -> (usize, Instruction) {
    (
        offset,
        Instruction::Jal {
            target: SECONDARY_PARSER_RUNTIME_ADDRESS,
        },
    )
}

fn card_number_renderer_call(offset: usize) -> (usize, Instruction) {
    (
        offset,
        Instruction::Jal {
            target: CARD_NUMBER_RENDERER_RUNTIME_ADDRESS,
        },
    )
}
