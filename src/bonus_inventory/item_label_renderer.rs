//! CD lists share the compact inventory renderer through a neutral-brightness
//! tail call. Card actions and dialogs keep the secondary parser.

use anyhow::{Result, ensure};
use expected_write::WriteIntent;
use psx_r3000a::{Instruction as I, Register as R, encode_le_bytes, verify_placed_program};

use crate::decoded_record_write_plan::{
    CandidateRecordWrite, CandidateWriteClaim, DecodedDataClaim, DecodedRecordWritePlan,
};
use crate::pipeline::sha256_bytes;
use crate::psx_machine_code_sources::PsxMachineCodeSources;

use super::{ADVANCE, BASE, expect_instruction};

// Repacking all 87 labels leaves this aligned tail in their original region.
// The final four bytes before the card-action string are left untouched.
pub(super) const CD_ENTRY: usize = 0x7d8;
const RENDERER: u32 = 0x800a_b86c;
const SECONDARY: u32 = 0x800a_b374;

fn cd_entry() -> Vec<I> {
    vec![
        I::Addiu {
            rt: R::T0,
            rs: R::ZERO,
            immediate: 0x80,
        },
        I::J { target: RENDERER },
        // This is the tail jump's delay slot. The callee reads the argument
        // after allocating its own frame; the caller's return address survives.
        I::Sw {
            rt: R::T0,
            base: R::SP,
            offset: 0x14,
        },
    ]
}

pub(super) fn install(
    source: &[u8],
    text_candidate: &[u8],
    data_ranges: Vec<[usize; 2]>,
) -> Result<(Vec<u8>, Vec<[usize; 2]>)> {
    validate_consumers(source)?;
    let hash = sha256_bytes(source);
    let mut plan = DecodedRecordWritePlan::new("DAT1/KOUBAI2.BIN", source, &hash)?;
    plan.register_data_candidate(
        "bonus item strings and pointers",
        &hash,
        text_candidate,
        &DecodedDataClaim::from_ranges(
            "bonus-item-text",
            "repack the owned item-label records",
            data_ranges.clone(),
        ),
    )?;
    let mut machine = PsxMachineCodeSources::default();
    let mut ranges = data_ranges;
    for (name, offset, instructions) in programs() {
        let address = BASE + offset as u32;
        let bytes = encode_le_bytes(&instructions, address)?;
        let mut candidate = source.to_vec();
        candidate[offset..offset + bytes.len()].copy_from_slice(&bytes);
        let provenance = machine.register(format!("bonus-items:{name}"), address, instructions)?;
        plan.register_candidate(CandidateRecordWrite {
            owner: name,
            source_sha256: &hash,
            candidate: &candidate,
            claims: vec![CandidateWriteClaim {
                id: format!("bonus-items:{name}"),
                purpose: "route item lists through the shared compact renderer".into(),
                range: offset..offset + bytes.len(),
                intent: WriteIntent::MachineCode(provenance),
            }],
        })?;
        ranges.push([offset, offset + bytes.len()]);
    }
    let output = plan.apply(Some(&machine))?;
    verify_output(&output)?;
    Ok((output, ranges))
}

fn programs() -> Vec<(&'static str, usize, Vec<I>)> {
    vec![
        ("cd-brightness-entry", CD_ENTRY, cd_entry()),
        (
            "cd-list-entry",
            0x6e78,
            vec![
                I::Jal {
                    target: BASE + CD_ENTRY as u32,
                },
                I::Addu {
                    rd: R::A3,
                    rs: R::S6,
                    rt: R::ZERO,
                },
            ],
        ),
        (
            "cd-list-refresh",
            0x777c,
            vec![
                I::Jal {
                    target: BASE + CD_ENTRY as u32,
                },
                I::Addiu {
                    rt: R::S1,
                    rs: R::S1,
                    immediate: 1,
                },
            ],
        ),
        (
            "blank-advance",
            0x9908,
            vec![I::Addiu {
                rt: R::S3,
                rs: R::S3,
                immediate: ADVANCE as i16,
            }],
        ),
        (
            "glyph-advance",
            0x9a24,
            vec![I::Addiu {
                rt: R::S3,
                rs: R::S3,
                immediate: ADVANCE as i16,
            }],
        ),
    ]
}

fn verify_output(output: &[u8]) -> Result<()> {
    for (name, offset, instructions) in programs() {
        let bytes = &output[offset..offset + instructions.len() * 4];
        ensure!(
            verify_placed_program(bytes, BASE + offset as u32)? == instructions,
            "placed bonus item renderer differs: {name}"
        );
    }
    Ok(())
}

fn validate_consumers(source: &[u8]) -> Result<()> {
    for (offset, instruction) in [
        (
            0x6e14,
            I::Addiu {
                rt: R::SP,
                rs: R::SP,
                immediate: -56,
            },
        ),
        (
            0x738c,
            I::Addiu {
                rt: R::SP,
                rs: R::SP,
                immediate: -56,
            },
        ),
        (
            0x6e44,
            I::Sw {
                rt: R::S0,
                base: R::SP,
                offset: 0x18,
            },
        ),
        (
            0x73b4,
            I::Sw {
                rt: R::S0,
                base: R::SP,
                offset: 0x18,
            },
        ),
        (
            0x6e50,
            I::Addiu {
                rt: R::S5,
                rs: R::S5,
                immediate: 0x3308,
            },
        ),
        (
            0x7754,
            I::Addiu {
                rt: R::S5,
                rs: R::S5,
                immediate: 0x3308,
            },
        ),
        (0x6e78, I::Jal { target: SECONDARY }),
        (0x777c, I::Jal { target: SECONDARY }),
        (
            0x6e7c,
            I::Addu {
                rd: R::A3,
                rs: R::S6,
                rt: R::ZERO,
            },
        ),
        (
            0x7780,
            I::Addiu {
                rt: R::S1,
                rs: R::S1,
                immediate: 1,
            },
        ),
        (
            0x6e80,
            I::Slti {
                rt: R::V0,
                rs: R::S1,
                immediate: 13,
            },
        ),
        (
            0x7784,
            I::Slti {
                rt: R::V0,
                rs: R::S1,
                immediate: 13,
            },
        ),
        (
            0x98ac,
            I::Lhu {
                rt: R::S5,
                base: R::SP,
                offset: 0x4c,
            },
        ),
        (
            0x98bc,
            I::Sll {
                rd: R::V0,
                rt: R::A1,
                shift: 1,
            },
        ),
        (
            0x98c0,
            I::Addu {
                rd: R::V0,
                rs: R::V0,
                rt: R::A1,
            },
        ),
        (
            0x98c4,
            I::Sll {
                rd: R::V0,
                rt: R::V0,
                shift: 3,
            },
        ),
        (
            0x98c8,
            I::Subu {
                rd: R::V0,
                rs: R::V0,
                rt: R::A1,
            },
        ),
        (
            0x98cc,
            I::Sll {
                rd: R::V0,
                rt: R::V0,
                shift: 2,
            },
        ),
        (
            0x98d0,
            I::Subu {
                rd: R::V0,
                rs: R::V0,
                rt: R::A1,
            },
        ),
        (
            0x98d4,
            I::Sll {
                rd: R::V0,
                rt: R::V0,
                shift: 3,
            },
        ),
        (
            0x98d8,
            I::Addiu {
                rt: R::V0,
                rs: R::V0,
                immediate: 0x874,
            },
        ),
        (
            0x9954,
            I::Addiu {
                rt: R::S4,
                rs: R::S4,
                immediate: 0x38,
            },
        ),
        (
            0x99b8,
            I::Sb {
                rt: R::S5,
                base: R::S0,
                offset: 0xc,
            },
        ),
        (
            0x99bc,
            I::Sb {
                rt: R::S5,
                base: R::S0,
                offset: 0xd,
            },
        ),
        (
            0x99c0,
            I::Sb {
                rt: R::S5,
                base: R::S0,
                offset: 0xe,
            },
        ),
        (
            0x93c8,
            I::Addiu {
                rt: R::S6,
                rs: R::ZERO,
                immediate: 0x80,
            },
        ),
        (
            0x94d0,
            I::Sb {
                rt: R::S6,
                base: R::S0,
                offset: 0xc,
            },
        ),
        (
            0x94d4,
            I::Sb {
                rt: R::S6,
                base: R::S0,
                offset: 0xd,
            },
        ),
        (
            0x94d8,
            I::Sb {
                rt: R::S6,
                base: R::S0,
                offset: 0xe,
            },
        ),
        (
            0x9908,
            I::Addiu {
                rt: R::S3,
                rs: R::S3,
                immediate: 20,
            },
        ),
        (
            0x9a24,
            I::Addiu {
                rt: R::S3,
                rs: R::S3,
                immediate: 20,
            },
        ),
    ] {
        expect_instruction(source, offset, instruction)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cd_tail_call_sets_brightness_and_preserves_arguments_and_return() {
        let mut registers = std::array::from_fn::<_, 32, _>(|i| 0x1000 + i as u32 * 4);
        registers[0] = 0;
        let before = registers;
        let mut stack = [0xface_u32; 14];
        let mut destination = None;
        let bytes = encode_le_bytes(&cd_entry(), BASE + CD_ENTRY as u32).unwrap();
        for i in verify_placed_program(&bytes, BASE + CD_ENTRY as u32).unwrap() {
            match i {
                I::Addiu { rt, rs, immediate } => {
                    registers[rt.index() as usize] =
                        registers[rs.index() as usize].wrapping_add_signed(i32::from(immediate))
                }
                I::J { target } => destination = Some(target),
                I::Sw { rt, base, offset } => {
                    assert_eq!(base, R::SP);
                    stack[offset as usize / 4] = registers[rt.index() as usize];
                }
                other => panic!("unexpected wrapper effect: {other:?}"),
            }
        }
        assert_eq!(destination, Some(RENDERER));
        assert_eq!(stack[5], 0x80);
        assert!(
            stack
                .iter()
                .enumerate()
                .all(|(i, v)| i == 5 || *v == 0xface)
        );
        for (i, value) in before.iter().enumerate() {
            if i != R::T0.index() as usize {
                assert_eq!(registers[i], *value);
            }
        }
    }
}
