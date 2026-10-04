//! Source-bound practical-exam pause-menu sprite constructor.

use anyhow::{Context, Result, ensure};
use psx_r3000a::{Instruction, Register, decode};

pub(super) const CONSUMER_RECORD: &str = "DAT1/SIKENG22.BIN";
pub(super) const PRIMITIVE_LAYOUT_EVIDENCE_OFFSETS: [usize; 12] = [
    0x2fc0, 0x2fc4, 0x2fc8, 0x2fcc, 0x304c, 0x3084, 0x309c, 0x30a0, 0x30b4, 0x30b8, 0x30c0, 0x30d8,
];

const OVERLAY_RUNTIME_BASE: u32 = 0x8017_a000;

pub(super) fn validate_pause_menu_consumer(overlay: &[u8]) -> Result<()> {
    for (offset, expected) in pause_menu_layout_instructions() {
        let actual = decode_instruction(overlay, offset)?;
        ensure!(
            actual == expected,
            "SIKENG22 pause-menu consumer changed at +0x{offset:04x}: expected {expected:?}, found {actual:?}"
        );
    }
    Ok(())
}

fn pause_menu_layout_instructions() -> Vec<(usize, Instruction)> {
    const A1: Register = Register::A1;
    const A2: Register = Register::A2;
    const A3: Register = Register::A3;
    const FP: Register = Register::FP;
    const S0: Register = Register::S0;
    const S2: Register = Register::S2;
    const S5: Register = Register::S5;
    const V0: Register = Register::V0;
    const V1: Register = Register::V1;
    const ZERO: Register = Register::ZERO;

    vec![
        addiu(0x2fc0, A2, ZERO, 960),
        addiu(0x2fc4, A3, ZERO, 256),
        lui(0x2fc8, S0, 0x8018),
        addiu(0x2fcc, S0, S0, -5984),
        addiu(0x304c, A1, ZERO, 511),
        sb(0x3078, V0, S0, 14),
        sb(0x307c, V0, S0, 13),
        sb(0x3080, V0, S0, 12),
        lbu(0x3084, V1, S2, 0),
        addiu(0x3088, S2, S2, 1),
        sll(0x308c, V0, V1, 1),
        addu(0x3090, V0, V0, V1),
        sll(0x3094, V0, V0, 3),
        addiu(0x3098, V0, V0, -64),
        sb(0x309c, V0, S0, 20),
        lbu(0x30a0, V1, S2, 0),
        addiu(0x30a4, S2, S2, 1),
        sll(0x30a8, V0, V1, 1),
        addu(0x30ac, V0, V0, V1),
        sll(0x30b0, V0, V0, 2),
        sb(0x30b4, V0, S0, 21),
        lbu(0x30b8, V1, S2, 0),
        addiu(0x30bc, V0, ZERO, 12),
        sh(0x30c0, V0, S0, 26),
        sh(0x30c4, S5, S0, 16),
        sh(0x30c8, FP, S0, 18),
        sll(0x30cc, V0, V1, 1),
        addu(0x30d0, V0, V0, V1),
        sll(0x30d4, V0, V0, 3),
        sh(0x30d8, V0, S0, 24),
    ]
}

fn decode_instruction(source: &[u8], offset: usize) -> Result<Instruction> {
    let word = u32::from_le_bytes(
        source
            .get(offset..offset + 4)
            .with_context(|| format!("truncated SIKENG22 instruction at +0x{offset:04x}"))?
            .try_into()?,
    );
    decode(word, OVERLAY_RUNTIME_BASE + offset as u32)
        .with_context(|| format!("failed to decode SIKENG22 instruction at +0x{offset:04x}"))
}

fn addiu(offset: usize, rt: Register, rs: Register, immediate: i16) -> (usize, Instruction) {
    (offset, Instruction::Addiu { rt, rs, immediate })
}

fn addu(offset: usize, rd: Register, rs: Register, rt: Register) -> (usize, Instruction) {
    (offset, Instruction::Addu { rd, rs, rt })
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

fn lui(offset: usize, rt: Register, immediate: u16) -> (usize, Instruction) {
    (offset, Instruction::Lui { rt, immediate })
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

#[cfg(test)]
mod tests {
    use psx_r3000a::{Instruction, Register, encode};

    use super::{
        OVERLAY_RUNTIME_BASE, pause_menu_layout_instructions, validate_pause_menu_consumer,
    };

    #[test]
    fn pause_menu_consumer_keeps_the_over_page_and_twelve_pixel_sprite_layout() {
        let source = valid_consumer_fixture();
        validate_pause_menu_consumer(&source).unwrap();

        let mut drifted = source;
        write_instruction(
            &mut drifted,
            0x30bc,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::ZERO,
                immediate: 16,
            },
        );
        let error = validate_pause_menu_consumer(&drifted).unwrap_err();
        assert!(error.to_string().contains("pause-menu consumer changed"));
    }

    fn valid_consumer_fixture() -> Vec<u8> {
        let mut source = vec![0_u8; 0x30dc];
        for (offset, instruction) in pause_menu_layout_instructions() {
            write_instruction(&mut source, offset, instruction);
        }
        source
    }

    fn write_instruction(target: &mut [u8], offset: usize, instruction: Instruction) {
        target[offset..offset + 4].copy_from_slice(
            &encode(&instruction, OVERLAY_RUNTIME_BASE + offset as u32)
                .unwrap()
                .to_le_bytes(),
        );
    }
}
