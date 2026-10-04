use anyhow::{Context, Result, ensure};
use psx_r3000a::{Instruction, Register, decode};

pub(super) const OVERLAY_RUNTIME_BASE: u32 = 0x800a_2000;
pub(super) const STOCK_LABEL_RENDERER_RUNTIME_ADDRESS: u32 = 0x800a_83d0;
pub(super) const CATEGORY_HANDLER_RUNTIME_ADDRESS: u32 = 0x800a_938c;
pub(super) const CATEGORY_JUMP_TABLE_OFFSET: usize = 0x151d4;
pub(super) const CATEGORY_STATE_TARGETS: [u32; 7] = [
    0x800a_9554,
    0x800a_9634,
    0x800a_9724,
    0x800a_97b0,
    0x800a_9820,
    0x800a_98a0,
    0x800a_98bc,
];
pub(super) const RENDERER_CALL_OFFSETS: [usize; 5] = [0x75c8, 0x7690, 0x7744, 0x7808, 0x78b4];
pub(super) const RENDERER_CALLING_STATES: [u8; 6] = [0, 1, 2, 3, 4, 5];
pub(super) const NONCALLING_EXIT_STATE: u8 = 6;
pub(super) const SOURCE_GLYPH_CODES: [u16; 2] = [0x0271, 0x026a];
pub(super) const OUTPUT_GLYPH_CODES: [u16; 2] = [0x0341, 0x034a];
pub(super) const SCREEN_POSITIONS: [[u16; 2]; 2] = [[282, 107], [302, 107]];
pub(super) const SPRITE_SIZE: [u16; 2] = [20, 20];
pub(super) const SELECTOR_INSTRUCTION_OFFSETS: [usize; 4] = [0x66d8, 0x66e0, 0x66e4, 0x66e8];
pub(super) const TEXTURE_PAGE_INSTRUCTION_OFFSET: usize = 0x673c;
pub(super) const CHANGED_INSTRUCTION_OFFSETS: [usize; 3] = [0x66e0, 0x66e8, 0x673c];

pub(super) fn validate_stock_label_consumer(source: &[u8]) -> Result<()> {
    for (offset, expected) in renderer_source_instructions() {
        let actual = decode_instruction(source, offset)?;
        ensure!(
            actual == expected,
            "KOUBAI2 stock-label renderer changed at +0x{offset:04x}: expected {expected:?}, found {actual:?}"
        );
    }
    for (offset, expected) in category_dispatch_instructions() {
        let actual = decode_instruction(source, offset)?;
        ensure!(
            actual == expected,
            "KOUBAI2 bonus-category dispatch changed at +0x{offset:04x}: expected {expected:?}, found {actual:?}"
        );
    }
    ensure!(
        read_category_state_targets(source)? == CATEGORY_STATE_TARGETS,
        "KOUBAI2 bonus-category state table changed"
    );
    ensure!(
        renderer_call_offsets(source) == RENDERER_CALL_OFFSETS,
        "KOUBAI2 stock-label renderer caller set changed"
    );
    ensure!(
        CATEGORY_STATE_TARGETS[usize::from(NONCALLING_EXIT_STATE)] == OVERLAY_RUNTIME_BASE + 0x78bc,
        "KOUBAI2 exit state no longer bypasses the stock-label renderer"
    );
    Ok(())
}

pub(super) fn validate_output_stock_label_consumer(output: &[u8]) -> Result<()> {
    for (offset, expected) in renderer_output_instructions() {
        let actual = decode_instruction(output, offset)?;
        ensure!(
            actual == expected,
            "KOUBAI2 Korean stock-label renderer differs at +0x{offset:04x}: expected {expected:?}, found {actual:?}"
        );
    }
    ensure!(
        renderer_call_offsets(output) == RENDERER_CALL_OFFSETS,
        "KOUBAI2 Korean stock-label renderer caller set changed"
    );
    ensure!(
        read_category_state_targets(output)? == CATEGORY_STATE_TARGETS,
        "KOUBAI2 Korean stock-label state table changed"
    );
    Ok(())
}

pub(super) fn renderer_source_instructions() -> Vec<(usize, Instruction)> {
    renderer_instructions(7, 6, 0x0280)
}

pub(super) fn renderer_output_instructions() -> Vec<(usize, Instruction)> {
    renderer_instructions(4, 4, 0x02c0)
}

fn renderer_instructions(
    first_row: i16,
    second_row: i16,
    texture_page_x: i16,
) -> Vec<(usize, Instruction)> {
    const A0: Register = Register::A0;
    const A2: Register = Register::A2;
    const S0: Register = Register::S0;
    const S1: Register = Register::S1;
    const S3: Register = Register::S3;
    const S4: Register = Register::S4;
    const S5: Register = Register::S5;
    const S6: Register = Register::S6;
    const S7: Register = Register::S7;
    const T0: Register = Register::T0;
    const V0: Register = Register::V0;
    const ZERO: Register = Register::ZERO;

    vec![
        addiu(0x66c8, S5, ZERO, 282),
        addu(0x66cc, S3, ZERO, ZERO),
        addiu(0x66d4, S6, ZERO, 0x06b4),
        // S1=1 is both the first U column and the second-iteration discriminator.
        addiu(0x66d8, S1, ZERO, 1),
        bne(0x66dc, S3, S1, 0x66ec),
        addiu(0x66e0, S4, ZERO, first_row),
        addiu(0x66e4, S1, ZERO, 10),
        addiu(0x66e8, S4, ZERO, second_row),
        addu(0x66ec, A0, S7, S6),
        addiu(0x66f0, S6, S6, 0x38),
        addiu(0x66fc, S3, S3, 1),
        addiu(0x673c, A2, ZERO, texture_page_x),
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
        slti(0x681c, V0, S3, 2),
        bne(0x6820, V0, ZERO, 0x66dc),
        addiu(0x6824, S1, ZERO, 1),
    ]
}

pub(super) fn category_dispatch_instructions() -> Vec<(usize, Instruction)> {
    const AT: Register = Register::AT;
    const S3: Register = Register::S3;
    const V0: Register = Register::V0;
    const V1: Register = Register::V1;
    const ZERO: Register = Register::ZERO;

    let mut instructions = vec![
        lbu(0x7528, V1, S3, 0x10),
        sltiu(0x7530, V0, V1, 7),
        beq(0x7534, V0, ZERO, 0x78bc),
        sll(0x7538, V0, V1, 2),
        lui(0x753c, AT, 0x800b),
        addu(0x7540, AT, AT, V0),
        lw(0x7544, V0, AT, 0x71d4),
        (0x754c, Instruction::Jr { rs: V0 }),
    ];
    instructions.extend(RENDERER_CALL_OFFSETS.map(|offset| {
        (
            offset,
            Instruction::Jal {
                target: STOCK_LABEL_RENDERER_RUNTIME_ADDRESS,
            },
        )
    }));
    instructions
}

pub(super) fn read_category_state_targets(source: &[u8]) -> Result<[u32; 7]> {
    let mut targets = [0; 7];
    for (index, target) in targets.iter_mut().enumerate() {
        *target = read_u32(source, CATEGORY_JUMP_TABLE_OFFSET + index * 4)?;
    }
    Ok(targets)
}

pub(super) fn selector_instruction_sha256_input(source: &[u8]) -> Result<Vec<u8>> {
    let mut bytes = Vec::with_capacity(20);
    for offset in SELECTOR_INSTRUCTION_OFFSETS
        .into_iter()
        .chain([TEXTURE_PAGE_INSTRUCTION_OFFSET])
    {
        bytes.extend_from_slice(
            source
                .get(offset..offset + 4)
                .with_context(|| format!("truncated selector instruction at +0x{offset:04x}"))?,
        );
    }
    Ok(bytes)
}

pub(super) fn decode_instruction(source: &[u8], offset: usize) -> Result<Instruction> {
    let bytes = source
        .get(offset..offset + 4)
        .with_context(|| format!("truncated KOUBAI2 instruction at +0x{offset:04x}"))?;
    let word = u32::from_le_bytes(bytes.try_into().expect("four-byte instruction"));
    decode(word, OVERLAY_RUNTIME_BASE + offset as u32)
        .with_context(|| format!("failed to decode KOUBAI2 instruction at +0x{offset:04x}"))
}

fn renderer_call_offsets(source: &[u8]) -> Vec<usize> {
    (0..source.len().saturating_sub(3))
        .step_by(4)
        .filter(|offset| {
            matches!(
                decode_instruction(source, *offset),
                Ok(Instruction::Jal {
                    target: STOCK_LABEL_RENDERER_RUNTIME_ADDRESS,
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

fn sll(offset: usize, rd: Register, rt: Register, shift: u8) -> (usize, Instruction) {
    (offset, Instruction::Sll { rd, rt, shift })
}

fn addu(offset: usize, rd: Register, rs: Register, rt: Register) -> (usize, Instruction) {
    (offset, Instruction::Addu { rd, rs, rt })
}

fn slti(offset: usize, rt: Register, rs: Register, immediate: i16) -> (usize, Instruction) {
    (offset, Instruction::Slti { rt, rs, immediate })
}

fn sltiu(offset: usize, rt: Register, rs: Register, immediate: i16) -> (usize, Instruction) {
    (offset, Instruction::Sltiu { rt, rs, immediate })
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
