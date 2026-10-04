use anyhow::{Context, Result, ensure};
use psx_r3000a::{Instruction, Register, decode};

use super::command_sequences::MEMORY_CARD_SWAP_SEQUENCES;
use super::glyph_ownership::{GlyphOwnershipValidation, validate_allocated_glyph_ownership};

pub(super) const OVERLAY_RUNTIME_BASE: u32 = 0x800a_2000;
pub(super) const POINTER_TABLE_OFFSET: usize = 0x11e0;
pub(super) const POINTER_TABLE_END: usize = 0x1418;
pub(super) const COMMAND_RENDERER_RUNTIME_ADDRESS: u32 = 0x800a_b5ac;
pub(super) const CALLER_RUNTIME_ADDRESSES: [u32; 2] = [0x800a_f97c, 0x800a_fdac];
pub(super) const RENDERER_LINE_WIDTH_PX: usize = 512;
pub(super) const RENDERER_COMMAND_ADVANCE_PX: usize = 20;
pub(super) const RENDERER_LINE_COMMAND_LIMIT: usize =
    RENDERER_LINE_WIDTH_PX / RENDERER_COMMAND_ADVANCE_PX;

const EXECUTABLE_REGION_START: usize = 0x3afc;

pub(super) fn validate_memory_card_swap_consumer(
    source: &[u8],
) -> Result<GlyphOwnershipValidation> {
    validate_memory_card_swap_composed_consumer(source)?;
    validate_allocated_glyph_ownership(source)
}

pub(super) fn validate_memory_card_swap_composed_consumer(source: &[u8]) -> Result<()> {
    for (offset, expected) in source_consumer_instructions() {
        let actual = decode_instruction(source, offset)?;
        ensure!(
            actual == expected,
            "KOUBAI2 memory-card-swap consumer changed at +0x{offset:04x}: expected {expected:?}, found {actual:?}"
        );
    }
    ensure_command_pointers(source)?;
    for (displacement, expected) in [(0x3410, vec![0xd974]), (0x3414, vec![0xdda4])] {
        ensure!(
            pointer_load_offsets(source, displacement) == expected,
            "KOUBAI2 memory-card-swap pointer load-site set changed for +0x{displacement:04x}"
        );
    }
    ensure!(
        !RENDERER_LINE_WIDTH_PX.is_multiple_of(RENDERER_COMMAND_ADVANCE_PX)
            && RENDERER_LINE_COMMAND_LIMIT == 25,
        "KOUBAI2 memory-card-swap renderer line geometry changed"
    );
    Ok(())
}

pub(super) fn source_consumer_instructions() -> Vec<(usize, Instruction)> {
    const A0: Register = Register::A0;
    const A1: Register = Register::A1;
    const A2: Register = Register::A2;
    const A3: Register = Register::A3;
    const S1: Register = Register::S1;
    const S2: Register = Register::S2;
    const S3: Register = Register::S3;
    const S6: Register = Register::S6;
    const S7: Register = Register::S7;
    const SP: Register = Register::SP;
    const V0: Register = Register::V0;
    const V1: Register = Register::V1;
    const ZERO: Register = Register::ZERO;

    vec![
        // The renderer measures each line as command_count * 20, centers it in 512 px,
        // treats 0x80 as a line break and advances the next baseline by 26 px.
        addu(0x95bc, S2, A1, ZERO),
        addu(0x95cc, S7, A3, ZERO),
        lbu(0x9618, A2, S2, 0),
        sll(0x961c, V1, A0, 2),
        addu(0x9620, V1, V1, A0),
        sll(0x9624, V1, V1, 2),
        addiu(0x9628, V0, ZERO, 512),
        subu(0x962c, V0, V0, V1),
        sra(0x9630, S3, V0, 1),
        addiu(0x9634, V0, ZERO, 0x81),
        andi(0x9638, V1, A2, 0xff),
        beq(0x963c, V1, V0, 0x9838),
        addiu(0x9640, S6, ZERO, 0x80),
        andi(0x9644, V1, A2, 0xff),
        bne(0x9648, V1, S6, 0x96a0),
        addiu(0x964c, V0, ZERO, 0x63),
        addiu(0x9650, S7, S7, 26),
        addiu(0x9654, S2, S2, 1),
        bne(0x96a0, V1, V0, 0x96b4),
        addu(0x96a4, A0, ZERO, ZERO),
        addiu(0x96a8, S3, S3, 20),
        jump(0x96ac, 0x800a_b824),
        addiu(0x96b0, S2, S2, 3),
        addiu(0x96b4, S2, S2, 1),
        // Replace-memory-card record caller.
        addu(0xd968, A0, S1, ZERO),
        addu(0xd96c, A2, ZERO, ZERO),
        lui(0xd970, A1, 0x800a),
        lw(0xd974, A1, A1, 0x3410),
        addiu(0xd978, A3, ZERO, 0x0136),
        renderer_call(0xd97c),
        sw(0xd980, ZERO, SP, 0x10),
        // Restore-original-memory-card record caller.
        addu(0xdd94, A0, S1, ZERO),
        addiu(0xdd98, A2, ZERO, 2),
        addiu(0xdd9c, A3, ZERO, 0x015e),
        lui(0xdda0, A1, 0x800a),
        lw(0xdda4, A1, A1, 0x3414),
        addiu(0xdda8, V0, ZERO, 6),
        renderer_call(0xddac),
        sw(0xddb0, V0, SP, 0x10),
    ]
}

fn ensure_command_pointers(source: &[u8]) -> Result<()> {
    for spec in MEMORY_CARD_SWAP_SEQUENCES {
        ensure!(
            read_u32(source, spec.pointer_storage_offset)?
                == OVERLAY_RUNTIME_BASE + spec.sequence_offset as u32,
            "KOUBAI2 memory-card-swap pointer changed for {}",
            spec.id
        );
    }
    Ok(())
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

fn sll(offset: usize, rd: Register, rt: Register, shift: u8) -> (usize, Instruction) {
    (offset, Instruction::Sll { rd, rt, shift })
}

fn sra(offset: usize, rd: Register, rt: Register, shift: u8) -> (usize, Instruction) {
    (offset, Instruction::Sra { rd, rt, shift })
}

fn subu(offset: usize, rd: Register, rs: Register, rt: Register) -> (usize, Instruction) {
    (offset, Instruction::Subu { rd, rs, rt })
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
