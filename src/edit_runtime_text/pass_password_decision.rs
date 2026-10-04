//! Redirect only the two grid-action glyph reads; password values retain the
//! retail table. The native packet layout and subsequent renderer are retained.

use anyhow::{Context, Result, ensure};
use expected_write::WriteIntent;
use psx_r3000a::{Assembler, Instruction as I, Register, verify_placed_program};

use crate::decoded_record_write_plan::{
    CandidateRecordWrite, CandidateWriteClaim, DecodedRecordWritePlan,
};
use crate::pipeline::sha256_bytes;
use crate::psx_machine_code_sources::PsxMachineCodeSources;
use crate::text::read_length_prefixed_codes;

pub(super) const START: usize = 0x1cf8;
pub(super) const END: usize = 0x1d74;
const ORIGIN: u32 = 0x8017_a000 + START as u32;
const SOURCE_SHA256: &str = "b0e81381b3b66c3aef3b845bc6b9901e8c40b1b95aea4cad73243c1f9240f2e9";

pub(super) fn install(source: &[u8], kanri: &[u8], codes: &[u16]) -> Result<Vec<u8>> {
    ensure!(
        codes.len() == 2 && codes.iter().all(|code| *code < 0x0400),
        "PASS decision needs exactly two allocated MENU glyphs"
    );
    ensure!(
        read_length_prefixed_codes(kanri, 0x0698)?.get(..2) == Some(codes),
        "PASS decision alias differs from the authored KANRI decision legend"
    );
    ensure!(
        sha256_bytes(
            source
                .get(START..END)
                .context("truncated PASS grid renderer")?
        ) == SOURCE_SHA256,
        "PASS grid code and packet producer changed"
    );
    // The complete table is source-validated by preserved_pass_consumer_codes.
    // None of its grid entries may exercise the removed skip-code branch.
    ensure!(
        source[0x02cc..0x036c]
            .as_chunks::<2>()
            .0
            .iter()
            .all(|b| u16::from_le_bytes([b[0], b[1]]) < 0x0400),
        "PASS grid contains a skip or unsupported page code"
    );
    let instructions = build_program()?;
    let mut candidate = source.to_vec();
    for (index, (slot, instruction)) in candidate[START..END]
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(&instructions)
        .enumerate()
    {
        slot.copy_from_slice(
            &psx_r3000a::encode(instruction, ORIGIN + u32::try_from(index * 4)?)?.to_le_bytes(),
        );
    }
    let mut sources = PsxMachineCodeSources::default();
    let provenance = sources.register("pass-password-grid-decision", ORIGIN, instructions)?;
    let source_hash = sha256_bytes(source);
    let mut plan = DecodedRecordWritePlan::new("DAT1/PASS.BIN", source, &source_hash)?;
    plan.register_candidate(CandidateRecordWrite {
        owner: "PASS password decision renderer",
        source_sha256: &source_hash,
        candidate: &candidate,
        claims: vec![CandidateWriteClaim {
            id: "pass-password-grid-decision".into(),
            purpose: "select Korean action glyphs without changing password alphabet values".into(),
            range: START..END,
            intent: WriteIntent::MachineCode(provenance),
        }],
    })?;
    plan.apply(Some(&sources))
}

pub(super) fn build_program() -> Result<Vec<I>> {
    let mut a = Assembler::new();
    a.emit(I::Sll {
        rd: Register::V0,
        rt: Register::V1,
        shift: 1,
    })
    .emit(I::Lui {
        rt: Register::AT,
        immediate: 0x8018,
    })
    .emit(I::Sltiu {
        rt: Register::T0,
        rs: Register::V1,
        immediate: 78,
    })
    .bne(Register::T0, Register::ZERO, "read_code")
    .emit(I::Addiu {
        rt: Register::AT,
        rs: Register::AT,
        immediate: -0x5d34,
    })
    .emit(I::Lui {
        rt: Register::AT,
        immediate: 0x800a,
    })
    // Adding index*2 maps indices 78/79 to KANRI +0x069a/+0x069c.
    .emit(I::Addiu {
        rt: Register::AT,
        rs: Register::AT,
        immediate: 0x25fe,
    })
    .label("read_code")
    .emit(I::Addu {
        rd: Register::AT,
        rs: Register::AT,
        rt: Register::V0,
    })
    .emit(I::Lh {
        rt: Register::A0,
        base: Register::AT,
        offset: 0,
    })
    .emit(I::Sll {
        rd: Register::V0,
        rt: Register::V1,
        shift: 3,
    })
    .emit(I::Subu {
        rd: Register::V0,
        rs: Register::V0,
        rt: Register::V1,
    })
    .emit(I::Sll {
        rd: Register::V0,
        rt: Register::V0,
        shift: 3,
    })
    .emit(I::Lui {
        rt: Register::A1,
        immediate: 0x801d,
    })
    .emit(I::Addiu {
        rt: Register::A1,
        rs: Register::A1,
        immediate: -0x1480,
    })
    .emit(I::Addu {
        rd: Register::A1,
        rs: Register::A1,
        rt: Register::V0,
    })
    .emit(I::Sll {
        rd: Register::S2,
        rt: Register::S5,
        shift: 2,
    })
    .emit(I::Sra {
        rd: Register::A2,
        rt: Register::A0,
        shift: 8,
    })
    .emit(I::Andi {
        rt: Register::V1,
        rs: Register::A0,
        immediate: 0x0f,
    })
    .emit(I::Sll {
        rd: Register::V0,
        rt: Register::V1,
        shift: 2,
    })
    .emit(I::Addu {
        rd: Register::V0,
        rs: Register::V0,
        rt: Register::V1,
    })
    .emit(I::Sll {
        rd: Register::V0,
        rt: Register::V0,
        shift: 2,
    })
    .emit(I::Sw {
        rt: Register::V0,
        base: Register::SP,
        offset: 0x50,
    })
    // (code & 0xf0) * 5/4 is the native row*20, with fewer instructions.
    .emit(I::Andi {
        rt: Register::V1,
        rs: Register::A0,
        immediate: 0xf0,
    })
    .emit(I::Srl {
        rd: Register::V0,
        rt: Register::V1,
        shift: 2,
    })
    .emit(I::Lui {
        rt: Register::A0,
        immediate: 0x801f,
    })
    .emit(I::Lw {
        rt: Register::A0,
        base: Register::A0,
        offset: 0x608c,
    })
    .emit(I::Addu {
        rd: Register::FP,
        rs: Register::V1,
        rt: Register::V0,
    })
    .emit(I::Sll {
        rd: Register::V0,
        rt: Register::A0,
        shift: 3,
    })
    .emit(I::Subu {
        rd: Register::V0,
        rs: Register::V0,
        rt: Register::A0,
    })
    .emit(I::Sll {
        rd: Register::V0,
        rt: Register::V0,
        shift: 2,
    })
    .emit(I::Addu {
        rd: Register::S4,
        rs: Register::A1,
        rt: Register::V0,
    });
    let p = a.assemble(ORIGIN)?;
    ensure!(
        p.bytes().len() == END - START,
        "PASS decision renderer exceeds its native instruction window"
    );
    ensure!(
        verify_placed_program(p.bytes(), ORIGIN)? == p.instructions(),
        "PASS decision program placement failed"
    );
    Ok(p.instructions().to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use psx_r3000a::Register as R;
    use std::collections::BTreeMap;

    #[test]
    fn grid_renderer_preserves_alphabet_geometry_and_both_packet_buffers() {
        let program = build_program().unwrap();
        crate::psx_machine_code_sources::verify_r3000a_load_delays(
            &program,
            ORIGIN,
            "decision test",
        )
        .unwrap();
        for index in 0..80u32 {
            for buffer in 0..2u32 {
                let original_code = ((index / 60) << 8) | (((index / 12) % 12) << 4) | (index % 12);
                let replacements = [0x322, 0x170];
                let mut memory = BTreeMap::from([
                    (0x8017_a2cc + index * 2, original_code),
                    (0x800a_269a, replacements[0]),
                    (0x800a_269c, replacements[1]),
                    (0x801f_608c, buffer),
                ]);
                let before = memory.clone();
                let mut registers = [0u32; 32];
                registers[usize::from(R::V1.index())] = index;
                registers[usize::from(R::S5.index())] = 15 + (index % 10) * 10;
                registers[usize::from(R::SP.index())] = 0x1000;
                let mut pending_load: Option<(R, u32)> = None;
                let mut delayed_branch = None;
                let mut pc = ORIGIN;
                let mut reads = Vec::new();
                while pc < ORIGIN + u32::try_from(END - START).unwrap() {
                    let instruction = &program[((pc - ORIGIN) / 4) as usize];
                    let r = |reg: R| registers[usize::from(reg.index())];
                    let mut write = None;
                    let mut load = None;
                    let destination = delayed_branch.take();
                    match *instruction {
                        I::Sll { rd, rt, shift } => write = Some((rd, r(rt) << shift)),
                        I::Srl { rd, rt, shift } => write = Some((rd, r(rt) >> shift)),
                        I::Sra { rd, rt, shift } => {
                            write = Some((rd, ((r(rt) as i32) >> shift) as u32))
                        }
                        I::Lui { rt, immediate } => write = Some((rt, u32::from(immediate) << 16)),
                        I::Addu { rd, rs, rt } => write = Some((rd, r(rs).wrapping_add(r(rt)))),
                        I::Subu { rd, rs, rt } => write = Some((rd, r(rs).wrapping_sub(r(rt)))),
                        I::Addiu { rt, rs, immediate } => {
                            write = Some((rt, r(rs).wrapping_add_signed(i32::from(immediate))))
                        }
                        I::Andi { rt, rs, immediate } => {
                            write = Some((rt, r(rs) & u32::from(immediate)))
                        }
                        I::Sltiu { rt, rs, immediate } => {
                            write = Some((rt, u32::from(r(rs) < immediate as i32 as u32)))
                        }
                        I::Bne { rs, rt, target } => {
                            delayed_branch = Some(if r(rs) != r(rt) { target } else { pc + 8 })
                        }
                        I::Lh { rt, base, offset } | I::Lw { rt, base, offset } => {
                            let address = r(base).wrapping_add_signed(i32::from(offset));
                            reads.push(address);
                            load = Some((
                                rt,
                                *memory.get(&address).expect("unexpected renderer read"),
                            ));
                        }
                        I::Sw { rt, base, offset } => {
                            memory.insert(r(base).wrapping_add_signed(i32::from(offset)), r(rt));
                        }
                        _ => panic!("unsupported test instruction {instruction:?}"),
                    }
                    if let Some((reg, value)) = pending_load.take() {
                        registers[usize::from(reg.index())] = value;
                    }
                    if let Some((reg, value)) = write {
                        registers[usize::from(reg.index())] = value;
                    }
                    pending_load = load;
                    registers[0] = 0;
                    pc = destination.unwrap_or(pc + 4);
                }
                let code = if index < 78 {
                    original_code
                } else {
                    replacements[(index - 78) as usize]
                };
                assert_eq!(registers[usize::from(R::A2.index())], code >> 8);
                assert_eq!(
                    registers[usize::from(R::FP.index())],
                    ((code >> 4) & 15) * 20
                );
                assert_eq!(memory[&0x1050], (code & 15) * 20);
                assert_eq!(
                    registers[usize::from(R::S4.index())],
                    0x801c_eb80 + index * 56 + buffer * 28
                );
                assert_eq!(
                    registers[usize::from(R::S2.index())],
                    (15 + (index % 10) * 10) * 4
                );
                for (address, value) in before {
                    assert_eq!(memory[&address], value);
                }
                let expected_read = if index < 78 {
                    0x8017_a2cc + index * 2
                } else {
                    0x800a_269a + (index - 78) * 2
                };
                assert_eq!(reads, vec![expected_read, 0x801f_608c]);
            }
        }
    }
}
