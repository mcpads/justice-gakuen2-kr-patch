use anyhow::{Result, ensure};
use expected_write::WriteIntent;
use psx_r3000a::{Instruction, Register, encode_le_bytes};

use super::model::{DiaryHeaderEntry, DiaryHeaderTextLayout};
use crate::decoded_record_write_plan::{
    CandidateRecordWrite, CandidateWriteClaim, DecodedRecordWritePlan,
};
use crate::psx_machine_code_sources::PsxMachineCodeSources;

const OFFSET: usize = 0x19454;
const ORIGIN: u32 = 0x800bb454;
const ACTION_ROWS: [[u8; 8]; 4] = [
    [3, 9, 4, 168, 72, 60, 12, 0],
    [2, 9, 4, 144, 96, 60, 12, 0],
    [5, 9, 4, 0, 144, 60, 12, 0],
    [2, 9, 4, 120, 144, 60, 12, 0],
];

pub(super) fn validate_entries(entries: &[DiaryHeaderEntry]) -> Result<()> {
    for (id, row) in [
        ("action-progress", 0),
        ("movement-transition-title", 1),
        ("action-status", 2),
        ("action-diary", 3),
    ] {
        let entry = entries
            .iter()
            .find(|e| e.id == id)
            .ok_or_else(|| anyhow::anyhow!("missing action label {id}"))?;
        let descriptor = ACTION_ROWS[row];
        ensure!(
            entry.cell.x == 256 + usize::from(descriptor[3])
                && entry.cell.y == usize::from(descriptor[4])
                && entry.cell.width == usize::from(descriptor[0]) * 24
                && entry.cell.height == 24
                && matches!(entry.layout, DiaryHeaderTextLayout::Continuous),
            "action label {id} must use continuous text in its native cell"
        );
    }
    Ok(())
}

fn program() -> Vec<Instruction> {
    use Instruction::*;
    // Only the text loop changes. The descriptor extent and background drawing
    // remain native. Register::S4 offsets the first slice; Register::S7 advances adjacent slices.
    // The supported descriptor table contains only positive counts. Its 255
    // sentinel takes the separate single-sprite path before this division.
    vec![
        // All sliced labels are authored, including movement. Preserve the
        // original block size without retaining its obsolete row exception.
        Sll {
            rd: Register::ZERO,
            rt: Register::ZERO,
            shift: 0,
        },
        Sll {
            rd: Register::ZERO,
            rt: Register::ZERO,
            shift: 0,
        },
        Sll {
            rd: Register::ZERO,
            rt: Register::ZERO,
            shift: 0,
        },
        Sll {
            rd: Register::ZERO,
            rt: Register::ZERO,
            shift: 0,
        },
        Sll {
            rd: Register::ZERO,
            rt: Register::ZERO,
            shift: 0,
        },
        Sll {
            rd: Register::ZERO,
            rt: Register::ZERO,
            shift: 0,
        },
        Sll {
            rd: Register::ZERO,
            rt: Register::ZERO,
            shift: 0,
        },
        Sll {
            rd: Register::ZERO,
            rt: Register::ZERO,
            shift: 0,
        },
        Sll {
            rd: Register::T0,
            rt: Register::V1,
            shift: 1,
        },
        Addu {
            rd: Register::T0,
            rs: Register::T0,
            rt: Register::V1,
        },
        Sll {
            rd: Register::T0,
            rt: Register::T0,
            shift: 2,
        }, // count * 12
        Subu {
            rd: Register::S4,
            rs: Register::A0,
            rt: Register::T0,
        },
        Addiu {
            rt: Register::S7,
            rs: Register::ZERO,
            immediate: 24,
        },
        Addu {
            rd: Register::S5,
            rs: Register::ZERO,
            rt: Register::ZERO,
        },
        Lui {
            rt: Register::S6,
            immediate: 0x801f,
        },
        Ori {
            rt: Register::S6,
            rs: Register::S6,
            immediate: 0x608c,
        },
        Srl {
            rd: Register::FP,
            rt: Register::S7,
            shift: 1,
        },
        Addiu {
            rt: Register::S3,
            rs: Register::ZERO,
            immediate: 0xc0,
        },
    ]
}

pub(crate) fn register(
    source: &[u8],
    source_sha256: &str,
    plan: &mut DecodedRecordWritePlan,
    machine: &mut PsxMachineCodeSources,
) -> Result<()> {
    super::menu_consumer::validate_diary_menu_consumer(source)?;
    ensure!(
        source.get(0x1f84..0x1fa4) == Some(ACTION_ROWS.concat().as_slice()),
        "action-menu source descriptors changed"
    );
    // Full source identity is checked by the write plan; bind the loop as well.
    let original: [u32; 18] = [
        0x0043001a, 0x14600002, 0, 0x0007000d, 0x2401ffff, 0x14610004, 0x3c018000, 0x14410002, 0,
        0x0006000d, 0x0000b812, 0x10600032, 0x0000a821, 0x3c16801f, 0x36d6608c, 0x0017f042,
        0x0000a021, 0x241300c0,
    ];
    let expected: Vec<u8> = original.into_iter().flat_map(u32::to_le_bytes).collect();
    ensure!(
        source.get(OFFSET..OFFSET + expected.len()) == Some(expected.as_slice()),
        "action-menu source spacing loop changed"
    );
    let instructions = program();
    let bytes = encode_le_bytes(&instructions, ORIGIN)?;
    ensure!(
        bytes.len() == expected.len(),
        "action spacing exceeds the native loop"
    );
    let mut candidate = source.to_vec();
    candidate[OFFSET..OFFSET + bytes.len()].copy_from_slice(&bytes);
    let provenance = machine.register("mgame:action-label-spacing", ORIGIN, instructions)?;
    plan.register_candidate(CandidateRecordWrite {
        owner: "action label spacing",
        source_sha256,
        candidate: &candidate,
        claims: vec![CandidateWriteClaim {
            id: "mgame:action-label-spacing".into(),
            purpose: "center continuous action words without changing button backgrounds".into(),
            range: OFFSET..OFFSET + bytes.len(),
            intent: WriteIntent::MachineCode(provenance),
        }],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Execute the emitted block, including branch and load delays, for every
    // supported descriptor. This checks slice placement rather than code shape.
    fn run(row: &[u8]) -> [u32; 32] {
        let bytes = encode_le_bytes(&program(), ORIGIN).unwrap();
        let mut r = [0u32; 32];
        let idx = |v: Register| usize::from(v.index());
        r[idx(Register::V0)] = u32::from(row[5]) * 2;
        r[idx(Register::V1)] = u32::from(row[0]);
        r[idx(Register::A0)] = u32::from(row[5]);
        let mut pc = ORIGIN;
        let mut jump = None;
        let mut load = None;
        let mut lo = 0;
        while pc < ORIGIN + bytes.len() as u32 {
            let offset = (pc - ORIGIN) as usize;
            let ins = psx_r3000a::decode(
                u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()),
                pc,
            )
            .unwrap();
            let previous_jump = jump.take();
            let previous_load = load.take();
            let mut write = None;
            use Instruction::*;
            match ins {
                Div { rs, rt } => lo = r[idx(rs)] / r[idx(rt)],
                Mflo { rd } => write = Some((rd, lo)),
                Addu { rd, rs, rt } => write = Some((rd, r[idx(rs)].wrapping_add(r[idx(rt)]))),
                Subu { rd, rs, rt } => write = Some((rd, r[idx(rs)].wrapping_sub(r[idx(rt)]))),
                Addiu { rt, rs, immediate } => {
                    write = Some((rt, r[idx(rs)].wrapping_add(immediate as i32 as u32)))
                }
                Xori { rt, rs, immediate } => write = Some((rt, r[idx(rs)] ^ u32::from(immediate))),
                Ori { rt, rs, immediate } => write = Some((rt, r[idx(rs)] | u32::from(immediate))),
                Lui { rt, immediate } => write = Some((rt, u32::from(immediate) << 16)),
                Sll { rd, rt, shift } => write = Some((rd, r[idx(rt)] << shift)),
                Srl { rd, rt, shift } => write = Some((rd, r[idx(rt)] >> shift)),
                Lbu { rt, base, offset } => {
                    assert_eq!((base, offset), (Register::S2, 0xd4));
                    load = Some((rt, u32::from(row[3])));
                }
                Bne { rs, rt, target } => {
                    jump = Some(if r[idx(rs)] != r[idx(rt)] {
                        target
                    } else {
                        pc + 8
                    })
                }
                Beq { rs, rt, target } => {
                    jump = Some(if r[idx(rs)] == r[idx(rt)] {
                        target
                    } else {
                        pc + 8
                    })
                }
                other => panic!("unexpected spacing instruction {other:?}"),
            }
            for (register, value) in previous_load.into_iter().chain(write) {
                if register != Register::ZERO {
                    r[idx(register)] = value;
                }
            }
            pc = previous_jump.unwrap_or(pc + 4);
        }
        r
    }

    #[test]
    #[ignore = "requires the original disc in roms/"]
    fn all_authored_diary_slices_including_movement_are_adjacent() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let source = crate::source_disc::SupportedSourceDisc::open(
            &root.join("roms/Shiritsu Justice Gakuen - Nekketsu Seishun Nikki 2 (Japan).cue"),
        )
        .unwrap();
        let (_, source) = source.read_record("DAT1/MGAME.BIN").unwrap();
        let sha = crate::pipeline::sha256_bytes(&source);
        let mut plan = DecodedRecordWritePlan::new("DAT1/MGAME.BIN", &source, &sha).unwrap();
        let mut machine = PsxMachineCodeSources::default();
        register(&source, &sha, &mut plan, &mut machine).unwrap();
        let patched = plan.apply(Some(&machine)).unwrap();
        assert_eq!(&source[..OFFSET], &patched[..OFFSET]);
        assert_eq!(&source[OFFSET + 72..], &patched[OFFSET + 72..]);
        for (i, row) in source[0x1f84..0x1f84 + 19 * 8]
            .as_chunks::<8>()
            .0
            .iter()
            .enumerate()
        {
            assert!(row[0] > 0);
            if row[0] == 255 {
                continue;
            }
            let r = run(row);
            let step = r[usize::from(Register::S7.index())];
            let shift = r[usize::from(Register::S4.index())] as i32;
            let half_step = r[usize::from(Register::FP.index())] as i32;
            for slice in 0..i32::from(row[0]) {
                let actual = -i32::from(row[5]) + shift + half_step - 12 + slice * step as i32;
                let expected = -i32::from(row[0]) * 12 + slice * 24;
                assert_eq!(actual, expected, "descriptor {i}, slice {slice}");
            }
        }
    }
}
