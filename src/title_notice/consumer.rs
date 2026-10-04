use anyhow::{Context, Result, ensure};
use psx_r3000a::{Instruction, Register, decode};

pub(super) fn validate_empty_slot_record(offset: usize, size: usize) -> Result<()> {
    // +0x228 is a byte-identical copy, but the slot renderer selects +0x6d4.
    ensure!(
        offset == 0x6d4 && size == 16,
        "Continue empty-slot record is not its native consumer target"
    );
    Ok(())
}

pub(super) fn validate_continue_slot_consumers(source: &[u8]) -> Result<()> {
    use Instruction::*;
    use Register as R;
    let instructions = [
        (
            0x3b64,
            Lui {
                rt: R::A1,
                immediate: 0x800a,
            },
        ),
        (
            0x3b68,
            Addiu {
                rt: R::A1,
                rs: R::A1,
                immediate: 0x236c,
            },
        ),
        (
            0x3b7c,
            Jal {
                target: 0x800a_50b4,
            },
        ),
        (
            0x3b98,
            Jal {
                target: 0x800a_68d4,
            },
        ),
        (
            0x4900,
            Addiu {
                rt: R::FP,
                rs: R::ZERO,
                immediate: 120,
            },
        ),
        (
            0x4910,
            Addiu {
                rt: R::S2,
                rs: R::ZERO,
                immediate: 83,
            },
        ),
        (
            0x4974,
            Lbu {
                rt: R::V1,
                base: R::V0,
                offset: 3,
            },
        ),
        (
            0x4978,
            Addiu {
                rt: R::V0,
                rs: R::ZERO,
                immediate: 1,
            },
        ),
        (
            0x497c,
            Bne {
                rs: R::V1,
                rt: R::V0,
                target: 0x800a_69e8,
            },
        ),
        (
            0x4990,
            Lbu {
                rt: R::V0,
                base: R::A1,
                offset: 0x152,
            },
        ),
        (
            0x4998,
            Bne {
                rs: R::V0,
                rt: R::ZERO,
                target: 0x800a_69c0,
            },
        ),
        (
            0x49d0,
            Lui {
                rt: R::A1,
                immediate: 0x800a,
            },
        ),
        (
            0x49d4,
            Addiu {
                rt: R::A1,
                rs: R::A1,
                immediate: 0x26c4,
            },
        ),
        (
            0x49e8,
            Addiu {
                rt: R::V0,
                rs: R::ZERO,
                immediate: 255,
            },
        ),
        (
            0x49ec,
            Bne {
                rs: R::V1,
                rt: R::V0,
                target: 0x800a_6a1c,
            },
        ),
        (
            0x4a2c,
            Lui {
                rt: R::A1,
                immediate: 0x800a,
            },
        ),
        (
            0x4a30,
            Addiu {
                rt: R::A1,
                rs: R::A1,
                immediate: 0x26d4,
            },
        ),
        (
            0x4a40,
            Sw {
                rt: R::FP,
                base: R::SP,
                offset: 0x14,
            },
        ),
        (
            0x4a44,
            Jal {
                target: 0x800a_66ac,
            },
        ),
        (
            0x4a48,
            Sw {
                rt: R::S2,
                base: R::SP,
                offset: 0x18,
            },
        ),
        (
            0x4a54,
            Addiu {
                rt: R::S3,
                rs: R::S3,
                immediate: 1,
            },
        ),
        (
            0x4a58,
            Slti {
                rt: R::V0,
                rs: R::S3,
                immediate: 5,
            },
        ),
        (
            0x4a5c,
            Bne {
                rs: R::V0,
                rt: R::ZERO,
                target: 0x800a_692c,
            },
        ),
        (
            0x4a60,
            Addiu {
                rt: R::S2,
                rs: R::S2,
                immediate: 23,
            },
        ),
        (
            0x46bc,
            Addu {
                rd: R::S3,
                rs: R::A1,
                rt: R::ZERO,
            },
        ),
        (
            0x46e8,
            Lhu {
                rt: R::T1,
                base: R::S3,
                offset: 0,
            },
        ),
        (
            0x46ec,
            Addiu {
                rt: R::S3,
                rs: R::S3,
                immediate: 2,
            },
        ),
        (
            0x471c,
            Lhu {
                rt: R::T0,
                base: R::S3,
                offset: 0,
            },
        ),
        (
            0x4720,
            Addiu {
                rt: R::V0,
                rs: R::ZERO,
                immediate: 0x0fff,
            },
        ),
        (
            0x4724,
            Andi {
                rt: R::T0,
                rs: R::T0,
                immediate: 0x0fff,
            },
        ),
        (
            0x4728,
            Beq {
                rs: R::T0,
                rt: R::V0,
                target: 0x800a_6880,
            },
        ),
        (
            0x472c,
            Addiu {
                rt: R::S3,
                rs: R::S3,
                immediate: 2,
            },
        ),
        (
            0x4738,
            Sra {
                rd: R::A2,
                rt: R::T0,
                shift: 8,
            },
        ),
        (
            0x473c,
            Addiu {
                rt: R::A2,
                rs: R::A2,
                immediate: 12,
            },
        ),
        (
            0x4740,
            Sll {
                rd: R::A2,
                rt: R::A2,
                shift: 6,
            },
        ),
        (
            0x4748,
            Andi {
                rt: R::V1,
                rs: R::T0,
                immediate: 15,
            },
        ),
        (
            0x4754,
            Sll {
                rd: R::S6,
                rt: R::V0,
                shift: 2,
            },
        ),
        (
            0x4758,
            Sra {
                rd: R::V0,
                rt: R::T0,
                shift: 4,
            },
        ),
        (
            0x475c,
            Andi {
                rt: R::V0,
                rs: R::V0,
                immediate: 15,
            },
        ),
        (
            0x4768,
            Sll {
                rd: R::S5,
                rt: R::V1,
                shift: 2,
            },
        ),
        (
            0x481c,
            Addiu {
                rt: R::V0,
                rs: R::ZERO,
                immediate: 20,
            },
        ),
        (
            0x4820,
            Sb {
                rt: R::S6,
                base: R::S0,
                offset: 12,
            },
        ),
        (
            0x4824,
            Sb {
                rt: R::S5,
                base: R::S0,
                offset: 13,
            },
        ),
        (
            0x4828,
            Sh {
                rt: R::V0,
                base: R::S0,
                offset: 16,
            },
        ),
        (
            0x482c,
            Sh {
                rt: R::V0,
                base: R::S0,
                offset: 18,
            },
        ),
        (
            0x4880,
            Addiu {
                rt: R::S2,
                rs: R::S2,
                immediate: 20,
            },
        ),
    ];
    for (offset, expected) in instructions {
        let bytes = source
            .get(offset..offset + 4)
            .context("truncated Continue slot consumer")?;
        let actual = decode(
            u32::from_le_bytes(bytes.try_into()?),
            0x800a_2000 + offset as u32,
        )?;
        ensure!(
            actual == expected,
            "Continue slot consumer changed at MGTIT +{offset:#x}"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_slot_cannot_translate_the_unconsumed_duplicate() {
        validate_empty_slot_record(0x6d4, 16).unwrap();
        assert!(validate_empty_slot_record(0x228, 16).is_err());
        assert!(validate_empty_slot_record(0x6d4, 20).is_err());
    }
}
