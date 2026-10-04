use crate::decoded_record_write_plan::{
    CandidateRecordWrite, CandidateWriteClaim, DecodedRecordWritePlan,
};
use crate::dialogue_audit::backup_slot_text::BackupSlotTextCounts;
use crate::pipeline::sha256_bytes;
use crate::psx_machine_code_sources::PsxMachineCodeSources;
use anyhow::{Result, ensure};
use expected_write::WriteIntent;
use psx_r3000a::{Assembler, Instruction, Register, decode, encode};
use serde::Serialize;

const ORIGIN: u32 = 0x800c4474;
const END: u32 = 0x800c4590;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BackupNamesReport {
    pub loader_address: String,
    pub geometry_address: String,
    pub clear_draw_address: String,
    pub byte_count: usize,
    pub sha256: String,
    pub preserves_saved_records: bool,
    pub text_counts: BackupSlotTextCounts,
}

fn program(
    consumer: u32,
    text: BackupSlotTextCounts,
) -> Result<(Vec<u8>, Vec<Instruction>, u32, u32)> {
    let mut a = Assembler::new();
    a.emit_all([
        Instruction::Addiu {
            rt: Register::SP,
            rs: Register::SP,
            immediate: -64,
        },
        Instruction::Sw {
            rt: Register::RA,
            base: Register::SP,
            offset: 60,
        },
        Instruction::Sw {
            rt: Register::A1,
            base: Register::SP,
            offset: 48,
        },
        Instruction::Lbu {
            rt: Register::T0,
            base: Register::A0,
            offset: 3,
        },
        Instruction::Lui {
            rt: Register::T1,
            immediate: 0x800a,
        },
        Instruction::Sll {
            rd: Register::T0,
            rt: Register::T0,
            shift: 2,
        },
        Instruction::Addu {
            rd: Register::T1,
            rs: Register::T1,
            rt: Register::T0,
        },
        Instruction::Lw {
            rt: Register::T0,
            base: Register::T1,
            offset: 0x4658,
        },
        Instruction::Lui {
            rt: Register::T1,
            immediate: 20,
        },
        Instruction::Ori {
            rt: Register::T1,
            rs: Register::T1,
            immediate: 5,
        },
        Instruction::Sw {
            rt: Register::T0,
            base: Register::SP,
            offset: 24,
        },
        Instruction::Sw {
            rt: Register::T1,
            base: Register::SP,
            offset: 28,
        },
        Instruction::Sw {
            rt: Register::T0,
            base: Register::SP,
            offset: 32,
        },
        Instruction::Sw {
            rt: Register::ZERO,
            base: Register::SP,
            offset: 40,
        },
        Instruction::Addiu {
            rt: Register::T1,
            rs: Register::SP,
            immediate: 40,
        },
        Instruction::Sw {
            rt: Register::T1,
            base: Register::SP,
            offset: 16,
        },
        Instruction::Ori {
            rt: Register::T0,
            rs: Register::ZERO,
            immediate: 0x3001,
        },
        Instruction::Sh {
            rt: Register::T0,
            base: Register::SP,
            offset: 44,
        },
        Instruction::Lui {
            rt: Register::A0,
            immediate: 0x800d,
        },
        Instruction::Addiu {
            rt: Register::A1,
            rs: Register::SP,
            immediate: 44,
        },
        Instruction::Jal { target: 0x800af578 },
        Instruction::Addiu {
            rt: Register::A2,
            rs: Register::SP,
            immediate: 24,
        },
        Instruction::Lw {
            rt: Register::A1,
            base: Register::SP,
            offset: 48,
        },
        Instruction::Lui {
            rt: Register::A0,
            immediate: 0x800d,
        },
        Instruction::Ori {
            rt: Register::A0,
            rs: Register::A0,
            immediate: 0x0174,
        },
        Instruction::Addiu {
            rt: Register::A1,
            rs: Register::A1,
            immediate: 0x96,
        },
        Instruction::Addiu {
            rt: Register::A2,
            rs: Register::SP,
            immediate: 24,
        },
        Instruction::Addiu {
            rt: Register::A3,
            rs: Register::SP,
            immediate: 32,
        },
        Instruction::Jal { target: consumer },
        Instruction::Addu {
            rd: Register::T9,
            rs: Register::ZERO,
            rt: Register::ZERO,
        },
        Instruction::Lw {
            rt: Register::RA,
            base: Register::SP,
            offset: 60,
        },
        Instruction::Addiu {
            rt: Register::SP,
            rs: Register::SP,
            immediate: 64,
        },
        Instruction::Jr { rs: Register::RA },
        Instruction::nop(),
    ]);
    let geometry = ORIGIN + a.assemble(ORIGIN)?.bytes().len() as u32;
    a.emit_all([
        Instruction::Lw {
            rt: Register::T0,
            base: Register::A0,
            offset: 0x2c,
        },
        Instruction::Lbu {
            rt: Register::T1,
            base: Register::A0,
            offset: 3,
        },
        Instruction::Lui {
            rt: Register::T2,
            immediate: 0x800a,
        },
        Instruction::Sll {
            rd: Register::T1,
            rt: Register::T1,
            shift: 2,
        },
        Instruction::Addu {
            rd: Register::T2,
            rs: Register::T2,
            rt: Register::T1,
        },
        Instruction::Lhu {
            rt: Register::T1,
            base: Register::T2,
            offset: 0x465a,
        },
        Instruction::Addiu {
            rt: Register::A1,
            rs: Register::A1,
            immediate: 0x96,
        },
        Instruction::Addu {
            rd: Register::T3,
            rs: Register::ZERO,
            rt: Register::ZERO,
        },
        Instruction::Addu {
            rd: Register::T4,
            rs: Register::ZERO,
            rt: Register::ZERO,
        },
    ]);
    a.label("backup_name_cell");
    a.emit_all([
        Instruction::Lhu {
            rt: Register::T5,
            base: Register::A1,
            offset: 0,
        },
        Instruction::Ori {
            rt: Register::T6,
            rs: Register::ZERO,
            immediate: 0x3001,
        },
    ]);
    a.beq(Register::T5, Register::T6, "backup_name_geometry_done");
    a.emit_all([
        Instruction::Addiu {
            rt: Register::A1,
            rs: Register::A1,
            immediate: 2,
        },
        Instruction::Sltiu {
            rt: Register::T6,
            rs: Register::T3,
            immediate: 80,
        },
    ]);
    a.beq(Register::T6, Register::ZERO, "backup_name_geometry_done");
    a.emit_all([Instruction::Ori {
        rt: Register::T7,
        rs: Register::ZERO,
        immediate: 0x61e,
    }]);
    a.beq(Register::T5, Register::T7, "backup_name_next_cell");
    a.emit_all([
        Instruction::Addu {
            rd: Register::T8,
            rs: Register::T0,
            rt: Register::T4,
        },
        Instruction::Sb {
            rt: Register::T3,
            base: Register::T8,
            offset: 0x26c,
        },
        Instruction::Sll {
            rd: Register::T8,
            rt: Register::T4,
            shift: 1,
        },
        Instruction::Addu {
            rd: Register::T8,
            rs: Register::T8,
            rt: Register::T0,
        },
        Instruction::Sll {
            rd: Register::T6,
            rt: Register::T4,
            shift: 2,
        },
        Instruction::Addu {
            rd: Register::T6,
            rs: Register::T6,
            rt: Register::T4,
        },
        Instruction::Sll {
            rd: Register::T6,
            rt: Register::T6,
            shift: 2,
        },
        Instruction::Sb {
            rt: Register::T6,
            base: Register::T8,
            offset: 0x270,
        },
        Instruction::Sb {
            rt: Register::T1,
            base: Register::T8,
            offset: 0x271,
        },
        Instruction::Addiu {
            rt: Register::T4,
            rs: Register::T4,
            immediate: 1,
        },
    ]);
    a.label("backup_name_next_cell").jump("backup_name_cell");
    a.emit_all([Instruction::Addiu {
        rt: Register::T3,
        rs: Register::T3,
        immediate: 20,
    }]);
    a.label("backup_name_geometry_done");
    a.emit_all([
        Instruction::Jr { rs: Register::RA },
        Instruction::Sb {
            rt: Register::T4,
            base: Register::T0,
            offset: 0x278,
        },
    ]);
    let clear_draw = ORIGIN + a.assemble(ORIGIN)?.bytes().len() as u32;
    a.emit_all([Instruction::Slti {
        rt: Register::V0,
        rs: Register::S0,
        immediate: text.clear,
    }]);
    a.emit(Instruction::Beq {
        rs: Register::V0,
        rt: Register::ZERO,
        target: 0x800c414c,
    });
    a.emit_all([
        Instruction::Addiu {
            rt: Register::V0,
            rs: Register::ZERO,
            immediate: 20,
        },
        Instruction::Sb {
            rt: Register::S3,
            base: Register::A0,
            offset: 0x14,
        },
        Instruction::J { target: 0x800c40ec },
        Instruction::Sb {
            rt: Register::V0,
            base: Register::A0,
            offset: 0x15,
        },
    ]);
    let p = a.assemble(ORIGIN)?;
    ensure!(
        p.bytes().len() <= (END - ORIGIN) as usize,
        "backup name routines exceed native function pair: {}",
        p.bytes().len()
    );
    Ok((
        p.bytes().to_vec(),
        p.instructions().to_vec(),
        geometry,
        clear_draw,
    ))
}

pub(super) fn register(
    source: &[u8],
    source_sha256: &str,
    consumer: u32,
    text: BackupSlotTextCounts,
    plan: &mut DecodedRecordWritePlan<'_>,
    machine: &mut PsxMachineCodeSources,
) -> Result<BackupNamesReport> {
    let start = (ORIGIN - 0x800a2000) as usize;
    let end = (END - 0x800a2000) as usize;
    ensure!(
        sha256_bytes(&source[start..end])
            == "01d26cc750a687dc9f5181bd8f90c99f3bb083b2467eac78255a993ed40a799c",
        "backup native name function pair changed"
    );
    let (bytes, mut instructions, geometry, clear_draw) = program(consumer, text)?;
    let mut candidate = source.to_vec();
    candidate[start..end].fill(0);
    candidate[start..start + bytes.len()].copy_from_slice(&bytes);
    instructions.resize((end - start) / 4, Instruction::nop());
    let id = "mgame:backup:name-functions".to_string();
    let provenance = machine.register(id.clone(), ORIGIN, instructions)?;
    let mut claims = vec![CandidateWriteClaim {
        id,
        purpose: "render stored nickname tags without modifying save records".into(),
        range: start..end,
        intent: WriteIntent::MachineCode(provenance),
    }];
    for (address, original, replacement) in [
        (
            0x800c3a4c,
            Instruction::Jal { target: 0x800c44ac },
            Instruction::Jal { target: geometry },
        ),
        (
            0x800c3db0,
            Instruction::Slti {
                rt: Register::V0,
                rs: Register::S0,
                immediate: 6,
            },
            Instruction::Slti {
                rt: Register::V0,
                rs: Register::S0,
                immediate: text.empty,
            },
        ),
        (
            0x800c3e3c,
            Instruction::Slti {
                rt: Register::V0,
                rs: Register::S0,
                immediate: 9,
            },
            Instruction::Slti {
                rt: Register::V0,
                rs: Register::S0,
                immediate: text.error,
            },
        ),
        (
            0x800c4018,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::ZERO,
                immediate: 20,
            },
            Instruction::J { target: clear_draw },
        ),
        (
            0x800c401c,
            Instruction::Sb {
                rt: Register::S3,
                base: Register::A0,
                offset: 20,
            },
            Instruction::nop(),
        ),
    ] {
        if original == replacement {
            continue;
        }
        let offset = (address - 0x800a2000) as usize;
        ensure!(
            decode(
                u32::from_le_bytes(source[offset..offset + 4].try_into()?),
                address
            )? == original,
            "backup caller instruction changed at {address:x}"
        );
        candidate[offset..offset + 4]
            .copy_from_slice(&encode(&replacement, address)?.to_le_bytes());
        let id = format!("mgame:backup:caller:{address:08x}");
        let provenance = machine.register(id.clone(), address, vec![replacement])?;
        claims.push(CandidateWriteClaim {
            id,
            purpose: "bind stored name geometry and visible backup string lengths".into(),
            range: offset..offset + 4,
            intent: WriteIntent::MachineCode(provenance),
        });
    }
    // These tables contain the English word gap (NO|DATA / DATA|ERROR).
    // Translated strings already carry their own blank glyph cells. Keeping
    // the native gap splits Korean words, independently of the draw count.
    for (address, original, translated) in [
        (
            0x800a4638u32,
            &[0u16, 20, 60, 80, 100, 120][..],
            text.translated_empty,
        ),
        (
            0x800a4644u32,
            &[0u16, 20, 40, 60, 100, 120, 140, 160, 180][..],
            text.translated_error,
        ),
    ] {
        if !translated {
            continue;
        }
        let offset = (address - 0x800a2000) as usize;
        let end = offset + original.len() * 2;
        let expected: Vec<_> = original.iter().flat_map(|v| v.to_le_bytes()).collect();
        ensure!(
            source[offset..end] == expected,
            "backup label coordinates changed at {address:x}"
        );
        for (index, bytes) in candidate[offset..end].chunks_exact_mut(2).enumerate() {
            bytes.copy_from_slice(&(index as u16 * 20).to_le_bytes());
        }
        claims.push(CandidateWriteClaim {
            id: format!("mgame:backup:label-positions:{address:08x}"),
            purpose: "use authored blank cells instead of English word gaps".into(),
            range: offset..end,
            intent: WriteIntent::Data,
        });
    }
    plan.register_candidate(CandidateRecordWrite {
        owner: "Diary backup slot names",
        source_sha256,
        candidate: &candidate,
        claims,
    })?;
    Ok(BackupNamesReport {
        loader_address: format!("0x{ORIGIN:08x}"),
        geometry_address: format!("0x{geometry:08x}"),
        clear_draw_address: format!("0x{clear_draw:08x}"),
        byte_count: bytes.len(),
        sha256: sha256_bytes(&bytes),
        preserves_saved_records: true,
        text_counts: text,
    })
}
