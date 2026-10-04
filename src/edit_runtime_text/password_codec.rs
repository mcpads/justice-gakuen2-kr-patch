//! Preserve four raw name words in a 24-symbol character password.
//! The 20-symbol importer and native stat validation remain compatible.
use crate::decoded_record_write_plan::{
    CandidateRecordWrite, CandidateWriteClaim, DecodedRecordWritePlan,
};
use crate::pipeline::sha256_bytes;
use crate::psx_machine_code_sources::PsxMachineCodeSources;
use anyhow::{Result, ensure};
use expected_write::WriteIntent;
use psx_r3000a::{Assembler, Instruction as I, Register as R, load_address};

const BASE: u32 = 0x8017_a000;
const START: usize = 0x7378;
const END: usize = 0x7848;
const STATS: [usize; 9] = [1, 3, 4, 5, 7, 8, 10, 11, 12];
const BUFFER: i16 = 0x80;
const _: () = assert!(BUFFER as usize + 24 <= 0x100);
// The native password packet arena 0x6640..0x6b80 fits 24 double-buffered
// 28-byte packets; the following keyboard arena stays at 0x6b80.

fn imm(a: &mut Assembler, r: R, v: i16) {
    add(a, r, R::ZERO, v);
}
fn add(a: &mut Assembler, r: R, s: R, v: i16) {
    a.emit(I::Addiu {
        rt: r,
        rs: s,
        immediate: v,
    });
}
fn mov(a: &mut Assembler, r: R, s: R) {
    a.emit(I::Addu {
        rd: r,
        rs: s,
        rt: R::ZERO,
    });
}
fn plus(a: &mut Assembler, r: R, s: R, t: R) {
    a.emit(I::Addu {
        rd: r,
        rs: s,
        rt: t,
    });
}
fn and(a: &mut Assembler, r: R, s: R, v: u16) {
    a.emit(I::Andi {
        rt: r,
        rs: s,
        immediate: v,
    });
}
fn shl(a: &mut Assembler, r: R, s: R, v: u8) {
    a.emit(I::Sll {
        rd: r,
        rt: s,
        shift: v,
    });
}
fn shr(a: &mut Assembler, r: R, s: R, v: u8) {
    a.emit(I::Srl {
        rd: r,
        rt: s,
        shift: v,
    });
}
fn byte(a: &mut Assembler, r: R, b: R, o: i16) {
    a.emit(I::Lbu {
        rt: r,
        base: b,
        offset: o,
    })
    .emit(I::nop());
}
fn half(a: &mut Assembler, r: R, b: R, o: i16) {
    a.emit(I::Lhu {
        rt: r,
        base: b,
        offset: o,
    })
    .emit(I::nop());
}
fn sb(a: &mut Assembler, r: R, b: R, o: i16) {
    a.emit(I::Sb {
        rt: r,
        base: b,
        offset: o,
    });
}
fn sh(a: &mut Assembler, r: R, b: R, o: i16) {
    a.emit(I::Sh {
        rt: r,
        base: b,
        offset: o,
    });
}
fn jump(a: &mut Assembler, l: &str) {
    a.beq(R::ZERO, R::ZERO, l).emit(I::nop());
}
fn eq(a: &mut Assembler, r: R, s: R, l: &str) {
    a.beq(r, s, l).emit(I::nop());
}
fn ne(a: &mut Assembler, r: R, s: R, l: &str) {
    a.bne(r, s, l).emit(I::nop());
}
fn less(a: &mut Assembler, r: R, s: R, n: u16) {
    a.emit(I::Sltiu {
        rt: r,
        rs: s,
        immediate: n as i16,
    });
}
fn ret(a: &mut Assembler, result: i16) {
    imm(a, R::V0, result);
    add(a, R::SP, R::SP, 0x60);
    a.emit(I::Jr { rs: R::RA }).emit(I::nop());
}

pub(super) fn program(membership: u32, ascii_table: u32) -> Result<(Vec<I>, u32)> {
    let mut a = Assembler::new();
    // Native encoder prefix has packed the nine stat bytes into R::SP halfwords;
    // R::A1 is the original registration and R::T1 the destination password buffer.
    for i in 0..8 {
        byte(&mut a, R::V0, R::A1, 0x10 + i);
        sb(&mut a, R::V0, R::SP, 0x28 + i);
    }
    for (i, slot) in STATS.into_iter().enumerate() {
        byte(&mut a, R::V0, R::SP, (slot * 2) as i16);
        if i == 5 {
            a.emit(I::Ori {
                rt: R::V0,
                rs: R::V0,
                immediate: 0xa0,
            });
        }
        sb(&mut a, R::V0, R::SP, 0x30 + i as i16);
    }
    add(&mut a, R::T2, R::SP, 0x28);
    imm(&mut a, R::T3, 17);
    imm(&mut a, R::T4, 0);
    a.label("encode_sum");
    byte(&mut a, R::V0, R::T2, 0);
    plus(&mut a, R::T4, R::T4, R::V0);
    add(&mut a, R::T2, R::T2, 1);
    add(&mut a, R::T3, R::T3, -1);
    ne(&mut a, R::T3, R::ZERO, "encode_sum");
    sb(&mut a, R::T4, R::SP, 0x39);
    add(&mut a, R::T2, R::SP, 0x28);
    imm(&mut a, R::T3, 6);
    a.label("encode_group");
    byte(&mut a, R::V0, R::T2, 0);
    byte(&mut a, R::V1, R::T2, 1);
    byte(&mut a, R::A0, R::T2, 2);
    shr(&mut a, R::A1, R::V0, 2);
    sb(&mut a, R::A1, R::T1, 0);
    shl(&mut a, R::V0, R::V0, 4);
    shr(&mut a, R::A1, R::V1, 4);
    a.emit(I::Or {
        rd: R::V0,
        rs: R::V0,
        rt: R::A1,
    });
    and(&mut a, R::V0, R::V0, 63);
    sb(&mut a, R::V0, R::T1, 1);
    shl(&mut a, R::V1, R::V1, 2);
    shr(&mut a, R::A1, R::A0, 6);
    a.emit(I::Or {
        rd: R::V1,
        rs: R::V1,
        rt: R::A1,
    });
    and(&mut a, R::V1, R::V1, 63);
    sb(&mut a, R::V1, R::T1, 2);
    and(&mut a, R::A0, R::A0, 63);
    sb(&mut a, R::A0, R::T1, 3);
    add(&mut a, R::T1, R::T1, 4);
    add(&mut a, R::T2, R::T2, 3);
    add(&mut a, R::T3, R::T3, -1);
    ne(&mut a, R::T3, R::ZERO, "encode_group");
    ret(&mut a, 0);
    let decoder = BASE + START as u32 + a.assemble(BASE + START as u32)?.bytes().len() as u32;
    // Match the native decoder ABI: R::A0 is a registration index, R::A1 input.
    add(&mut a, R::SP, R::SP, -0x60);
    shl(&mut a, R::T0, R::A0, 2);
    plus(&mut a, R::T0, R::T0, R::A0);
    shl(&mut a, R::T0, R::T0, 3);
    a.emit_all(load_address(R::V0, 0x801f_5804));
    plus(&mut a, R::T0, R::T0, R::V0);
    mov(&mut a, R::T2, R::A1);
    imm(&mut a, R::T3, 24);
    a.emit(I::Lw {
        rt: R::V0,
        base: R::T2,
        offset: 20,
    });
    imm(&mut a, R::V1, -1);
    ne(&mut a, R::V0, R::V1, "decode_start");
    imm(&mut a, R::T3, 20);
    a.label("decode_start");
    imm(&mut a, R::T4, 0);
    imm(&mut a, R::T5, 0);
    imm(&mut a, R::T6, 0);
    add(&mut a, R::T7, R::SP, 0x28);
    a.label("decode_symbol");
    plus(&mut a, R::V0, R::T2, R::T4);
    byte(&mut a, R::V0, R::V0, 0);
    imm(&mut a, R::V1, 24);
    eq(&mut a, R::T3, R::V1, "direct_symbol");
    // Invert the unchanged retail substitution for legacy 20-character input.
    a.emit_all(load_address(R::A0, 0x8017_a954));
    plus(&mut a, R::A0, R::A0, R::T4);
    byte(&mut a, R::A0, R::A0, 0);
    shl(&mut a, R::A0, R::A0, 6);
    a.emit_all(load_address(R::A1, 0x8017_a854));
    plus(&mut a, R::A0, R::A0, R::A1);
    imm(&mut a, R::V1, 0);
    a.label("legacy_search");
    plus(&mut a, R::A1, R::A0, R::V1);
    byte(&mut a, R::A1, R::A1, 0);
    eq(&mut a, R::A1, R::V0, "legacy_found");
    add(&mut a, R::V1, R::V1, 1);
    less(&mut a, R::A1, R::V1, 64);
    ne(&mut a, R::A1, R::ZERO, "legacy_search");
    jump(&mut a, "invalid");
    a.label("legacy_found");
    mov(&mut a, R::V0, R::V1);
    a.label("direct_symbol");
    less(&mut a, R::V1, R::V0, 64);
    eq(&mut a, R::V1, R::ZERO, "invalid");
    shl(&mut a, R::T5, R::T5, 6);
    a.emit(I::Or {
        rd: R::T5,
        rs: R::T5,
        rt: R::V0,
    });
    add(&mut a, R::T6, R::T6, 6);
    less(&mut a, R::V1, R::T6, 8);
    ne(&mut a, R::V1, R::ZERO, "next_symbol");
    add(&mut a, R::T6, R::T6, -8);
    a.emit(I::Srlv {
        rd: R::V0,
        rt: R::T5,
        rs: R::T6,
    });
    sb(&mut a, R::V0, R::T7, 0);
    add(&mut a, R::T7, R::T7, 1);
    a.label("next_symbol");
    add(&mut a, R::T4, R::T4, 1);
    ne(&mut a, R::T4, R::T3, "decode_symbol");
    add(&mut a, R::T7, R::T7, -1);
    add(&mut a, R::T4, R::SP, 0x28);
    imm(&mut a, R::T5, 0);
    a.label("decode_sum");
    byte(&mut a, R::V0, R::T4, 0);
    plus(&mut a, R::T5, R::T5, R::V0);
    add(&mut a, R::T4, R::T4, 1);
    ne(&mut a, R::T4, R::T7, "decode_sum");
    byte(&mut a, R::V0, R::T7, 0);
    and(&mut a, R::T5, R::T5, 255);
    ne(&mut a, R::V0, R::T5, "invalid");
    imm(&mut a, R::V1, 20);
    eq(&mut a, R::T3, R::V1, "legacy_payload");
    byte(&mut a, R::V0, R::SP, 0x35);
    and(&mut a, R::V0, R::V0, 0xa0);
    imm(&mut a, R::V1, 0xa0);
    ne(&mut a, R::V0, R::V1, "invalid");
    // Validate all four words before touching the decode record. Resident name
    // pack membership and ASCII tables are the same owners used by redisplay.
    add(&mut a, R::T4, R::SP, 0x28);
    imm(&mut a, R::T5, 4);
    a.label("validate_name");
    half(&mut a, R::V0, R::T4, 0);
    less(&mut a, R::V1, R::V0, 0x1000);
    ne(&mut a, R::V1, R::ZERO, "next_name");
    imm(&mut a, R::V1, 0x3001);
    eq(&mut a, R::V0, R::V1, "next_name");
    a.emit(I::Xori {
        rt: R::V1,
        rs: R::V0,
        immediate: 0x8000,
    });
    less(&mut a, R::A0, R::V1, 11172);
    eq(&mut a, R::A0, R::ZERO, "ascii_name");
    shr(&mut a, R::A0, R::V1, 3);
    a.emit_all(load_address(R::A1, membership));
    plus(&mut a, R::A0, R::A0, R::A1);
    byte(&mut a, R::A0, R::A0, 0);
    and(&mut a, R::V1, R::V1, 7);
    a.emit(I::Srlv {
        rd: R::A0,
        rt: R::A0,
        rs: R::V1,
    });
    and(&mut a, R::A0, R::A0, 1);
    eq(&mut a, R::A0, R::ZERO, "invalid");
    jump(&mut a, "next_name");
    a.label("ascii_name");
    a.emit(I::Xori {
        rt: R::V1,
        rs: R::V0,
        immediate: 0x4000,
    });
    less(&mut a, R::A0, R::V1, 128);
    eq(&mut a, R::A0, R::ZERO, "invalid");
    shl(&mut a, R::V1, R::V1, 1);
    a.emit_all(load_address(R::A0, ascii_table));
    plus(&mut a, R::A0, R::A0, R::V1);
    half(&mut a, R::A0, R::A0, 0);
    a.emit(I::Ori {
        rt: R::A1,
        rs: R::ZERO,
        immediate: 0xffff,
    });
    eq(&mut a, R::A0, R::A1, "invalid");
    a.label("next_name");
    add(&mut a, R::T4, R::T4, 2);
    add(&mut a, R::T5, R::T5, -1);
    ne(&mut a, R::T5, R::ZERO, "validate_name");
    for i in 0..4 {
        half(&mut a, R::V0, R::SP, 0x28 + 2 * i);
        sh(&mut a, R::V0, R::T0, 0x10 + 2 * i);
    }
    imm(&mut a, R::V0, 0x3001);
    sh(&mut a, R::V0, R::T0, 0x18);
    for (i, slot) in STATS.into_iter().enumerate() {
        byte(&mut a, R::V0, R::SP, 0x30 + i as i16);
        sh(&mut a, R::V0, R::SP, (slot * 2) as i16);
    }
    jump(&mut a, "validate_stats");
    a.label("legacy_payload");
    // Feed the original name-alias decoder and stat validator.
    add(&mut a, R::T4, R::SP, 0x28);
    mov(&mut a, R::T5, R::SP);
    imm(&mut a, R::T6, 15);
    a.label("copy_legacy_bytes");
    byte(&mut a, R::V0, R::T4, 0);
    sh(&mut a, R::V0, R::T5, 0);
    add(&mut a, R::T4, R::T4, 1);
    add(&mut a, R::T5, R::T5, 2);
    add(&mut a, R::T6, R::T6, -1);
    ne(&mut a, R::T6, R::ZERO, "copy_legacy_bytes");
    imm(&mut a, R::T1, 0);
    imm(&mut a, R::A3, 0);
    a.emit(I::J {
        target: BASE + END as u32,
    })
    .emit(I::nop());
    a.label("validate_stats");
    imm(&mut a, R::T1, 0);
    a.emit(I::J {
        target: 0x8018_18e8,
    })
    .emit(I::nop());
    a.label("invalid");
    ret(&mut a, 1);
    let placed = a.assemble(BASE + START as u32)?;
    ensure!(
        placed.bytes().len() <= END - START,
        "password codec exceeds source-owned code: {} > {}",
        placed.bytes().len(),
        END - START
    );
    let mut instructions = placed.instructions().to_vec();
    instructions.resize((END - START) / 4, I::nop());
    Ok((instructions, decoder))
}

pub(super) fn register(
    source: &[u8],
    kanri: &[u8],
    plan: &mut DecodedRecordWritePlan<'_>,
    machines: &mut PsxMachineCodeSources,
    membership: u32,
    ascii_table: u32,
) -> Result<()> {
    ensure!(
        sha256_bytes(source) == "f160d227922e5928eda71a1d332fa6c463a4ba57826d35ca72d3c6e0581b90d4",
        "password codec source changed"
    );
    // KANRI initializes this mode's entire 0x100-byte state, including 0x80..98.
    ensure!(
        kanri[0x17ac..0x17bc]
            == [
                0x1f, 0x80, 0x04, 0x3c, 0x00, 0x1a, 0x84, 0x34, 0x88, 0xa8, 0x02, 0x0c, 0x00, 0x01,
                0x05, 0x24
            ],
        "KANRI state ownership changed"
    );
    let (instructions, decoder) = program(membership, ascii_table)?;
    let mut patches = vec![(START, instructions)];
    let offsets = [
        0x1688, 0x1914, 0x1af0, 0x1b4c, 0x2178, 0x23e8, 0x2df0, 0x3404, 0x3434, 0x34b0, 0x34d4,
        0x358c, 0x3658, 0x36a8, 0x36e0, 0x4218, 0x5c84, 0x6b88,
    ];
    for offset in offsets {
        let instruction = psx_r3000a::decode(
            u32::from_le_bytes(source[offset..offset + 4].try_into()?),
            BASE + offset as u32,
        )?;
        let replacement = match instruction {
            I::Lbu {
                rt,
                base,
                offset: 0x2e,
            } => I::Lbu {
                rt,
                base,
                offset: BUFFER,
            },
            I::Sb {
                rt,
                base,
                offset: 0x2e,
            } => I::Sb {
                rt,
                base,
                offset: BUFFER,
            },
            I::Addiu {
                rt,
                rs,
                immediate: 0x2e,
            } => I::Addiu {
                rt,
                rs,
                immediate: BUFFER,
            },
            _ => anyhow::bail!("password buffer consumer changed at {offset:x}"),
        };
        patches.push((offset, vec![replacement]));
    }
    for (offset, old, new) in [
        (0x1838, 20, 24),
        (0x235c, 20, 24),
        (0x2de8, 19, 23),
        (0x2dec, 19, 23),
        (0x33f4, 19, 23),
        (0x3594, 20, 24),
        (0x35b0, 19, 23),
        (0x364c, 19, 23),
        (0x41fc, 19, 23),
        (0x4200, 19, 23),
        (0x6b70, 19, 23),
        (0x6b74, 19, 23),
    ] {
        let i = psx_r3000a::decode(
            u32::from_le_bytes(source[offset..offset + 4].try_into()?),
            BASE + offset as u32,
        )?;
        let r = match i {
            I::Addiu { rt, rs, immediate } if immediate == old => I::Addiu {
                rt,
                rs,
                immediate: new,
            },
            I::Slti { rt, rs, immediate } if immediate == old => I::Slti {
                rt,
                rs,
                immediate: new,
            },
            _ => anyhow::bail!("password capacity consumer changed at {offset:x}"),
        };
        patches.push((offset, vec![r]));
    }
    for offset in [0x34b8, 0x36b0] {
        ensure!(
            u32::from_le_bytes(source[offset..offset + 4].try_into()?) == 0x0c06_055d,
            "legacy decoder call changed"
        );
        patches.push((offset, vec![I::Jal { target: decoder }]));
    }
    let mut candidate = source.to_vec();
    let mut claims = Vec::new();
    for (offset, instructions) in patches {
        let origin = BASE + offset as u32;
        let bytes = psx_r3000a::encode_le_bytes(&instructions, origin)?;
        candidate[offset..offset + bytes.len()].copy_from_slice(&bytes);
        let id = format!("password-codec-{offset:x}");
        let provenance = machines.register(&id, origin, instructions)?;
        claims.push(CandidateWriteClaim {
            id,
            purpose: "preserve raw names, import legacy passwords and bound shared input storage"
                .into(),
            range: offset..offset + bytes.len(),
            intent: WriteIntent::MachineCode(provenance),
        });
    }
    let hash = sha256_bytes(source);
    plan.register_candidate(CandidateRecordWrite {
        owner: "character password codec",
        source_sha256: &hash,
        candidate: &candidate,
        claims,
    })
}

#[cfg(test)]
#[path = "password_codec_tests.rs"]
mod tests;
