//! Source-bound adapters for the eleven native roster-name loops in PLSEL2..5.
use super::selector_loader::{overlaps, ram_range};
use super::{RosterAdapterLayout, RosterValue, build_roster_adapter};
use crate::{
    decoded_record_write_plan::{
        CandidateRecordWrite, CandidateWriteClaim, DecodedRecordWritePlan,
    },
    psx_machine_code_sources::PsxMachineCodeSources,
};
use anyhow::{Result, bail, ensure};
use expected_write::WriteIntent;
use psx_r3000a::{Instruction::*, Register as R, encode_le_bytes};
const ORIGIN: u32 = 0x800a2000;

#[derive(Clone, Debug)]
pub struct RosterSourceHooks {
    pub overlay: Vec<u8>,
    pub adapters: Vec<u8>,
    pub code_ranges: Vec<[usize; 2]>,
}

struct Site {
    guard: usize,
    center: usize,
    next: usize,
    context: RosterAdapterLayout,
}
fn sites(number: u8) -> Result<(&'static str, usize, Vec<Site>)> {
    let context = || RosterAdapterLayout {
        origin: 0,
        capacity: 384,
        renderer: 0,
        legacy: 0,
        next: 0,
        packet_base: 0,
        packet_extra: None,
        packet_row: None,
        length: R::S3,
        x: RosterValue::Constant(0),
        y: RosterValue::Constant(0),
        y_bias: 0,
        ot_offset: 0x1068,
    };
    Ok(match number {
        2 => (
            "d1a202ec600aa5f916083a2e1189b31c1f6f52969be7ef42de2ce56c2bb7cb9c",
            20454,
            vec![
                Site {
                    guard: 0x2218,
                    center: 0x2314,
                    next: 0x238c,
                    context: RosterAdapterLayout {
                        packet_base: 0x70,
                        packet_extra: None,
                        packet_row: None,
                        length: R::S3,
                        x: RosterValue::StackWord(0x28),
                        y: RosterValue::Constant(193),
                        y_bias: 0,
                        ot_offset: 0x1068,
                        ..context()
                    },
                },
                Site {
                    guard: 0x3364,
                    center: 0x345c,
                    next: 0x34d4,
                    context: RosterAdapterLayout {
                        packet_base: 0x78,
                        packet_extra: None,
                        packet_row: None,
                        length: R::S3,
                        x: RosterValue::StackWord(0x28),
                        y: RosterValue::Constant(193),
                        y_bias: 0,
                        ot_offset: 0x1068,
                        ..context()
                    },
                },
                Site {
                    guard: 0x3b84,
                    center: 0x3c80,
                    next: 0x3cf4,
                    context: RosterAdapterLayout {
                        packet_base: 0x68,
                        packet_extra: None,
                        packet_row: None,
                        length: R::S3,
                        x: RosterValue::Register(R::S7),
                        y: RosterValue::Constant(393),
                        y_bias: 0,
                        ot_offset: 0x1068,
                        ..context()
                    },
                },
            ],
        ),
        3 => (
            "bb8bc2987572d3c11eee86d60473bde270979f0c6b37bab6a01e9f8bad68bff5",
            36296,
            vec![
                Site {
                    guard: 0x2dbc,
                    center: 0x2eb8,
                    next: 0x2f30,
                    context: RosterAdapterLayout {
                        packet_base: 0x48,
                        packet_extra: None,
                        packet_row: None,
                        length: R::S2,
                        x: RosterValue::StackWord(0x20),
                        y: RosterValue::StackWord(0x60),
                        y_bias: 73,
                        ot_offset: 0x1068,
                        ..context()
                    },
                },
                Site {
                    guard: 0x58a8,
                    center: 0x59b8,
                    next: 0x5a2c,
                    context: RosterAdapterLayout {
                        packet_base: 0x28,
                        packet_extra: None,
                        packet_row: Some([0x48, 0x18]),
                        length: R::S3,
                        x: RosterValue::StackWord(0x20),
                        y: RosterValue::StackHalf(0x50),
                        y_bias: 0,
                        ot_offset: 0x1060,
                        ..context()
                    },
                },
                Site {
                    guard: 0x7a98,
                    center: 0x7ba0,
                    next: 0x7c10,
                    context: RosterAdapterLayout {
                        packet_base: 0x38,
                        packet_extra: Some(0x50),
                        packet_row: None,
                        length: R::S3,
                        x: RosterValue::StackWord(0x28),
                        y: RosterValue::Register(R::FP),
                        y_bias: 0,
                        ot_offset: 0x1068,
                        ..context()
                    },
                },
            ],
        ),
        4 => (
            "ef6563a0a6bdfab248a8f8ff950c6ca72259132026ea63d6939735ebc2d64a3c",
            41676,
            vec![
                Site {
                    guard: 0x2db8,
                    center: 0x2eb4,
                    next: 0x2f2c,
                    context: RosterAdapterLayout {
                        packet_base: 0x48,
                        packet_extra: None,
                        packet_row: None,
                        length: R::S2,
                        x: RosterValue::StackWord(0x20),
                        y: RosterValue::StackWord(0x60),
                        y_bias: 73,
                        ot_offset: 0x1068,
                        ..context()
                    },
                },
                Site {
                    guard: 0x72a8,
                    center: 0x73b0,
                    next: 0x7420,
                    context: RosterAdapterLayout {
                        packet_base: 0x30,
                        packet_extra: Some(0x48),
                        packet_row: None,
                        length: R::S3,
                        x: RosterValue::Register(R::S7),
                        y: RosterValue::StackHalf(0x58),
                        y_bias: 0,
                        ot_offset: 0x1068,
                        ..context()
                    },
                },
                Site {
                    guard: 0x8e34,
                    center: 0x8f3c,
                    next: 0x8fac,
                    context: RosterAdapterLayout {
                        packet_base: 0x38,
                        packet_extra: Some(0x50),
                        packet_row: None,
                        length: R::S3,
                        x: RosterValue::StackWord(0x28),
                        y: RosterValue::Register(R::FP),
                        y_bias: 0,
                        ot_offset: 0x1068,
                        ..context()
                    },
                },
            ],
        ),
        5 => (
            "3461b0ed24bb41acbbf44546d2c65ded2ac1d4d295423ed355d3f6b8352eca11",
            50940,
            vec![
                Site {
                    guard: 0x8c68,
                    center: 0x8d68,
                    next: 0x8dd8,
                    context: RosterAdapterLayout {
                        packet_base: 0x78,
                        packet_extra: None,
                        packet_row: None,
                        length: R::S3,
                        x: RosterValue::Register(R::FP),
                        y: RosterValue::Constant(169),
                        y_bias: 0,
                        ot_offset: 0x1068,
                        ..context()
                    },
                },
                Site {
                    guard: 0xa5dc,
                    center: 0xa6e0,
                    next: 0xa754,
                    context: RosterAdapterLayout {
                        packet_base: 0x38,
                        packet_extra: Some(0x50),
                        packet_row: None,
                        length: R::S3,
                        x: RosterValue::StackWord(0x28),
                        y: RosterValue::StackHalf(0x58),
                        y_bias: 0,
                        ot_offset: 0x1068,
                        ..context()
                    },
                },
            ],
        ),
        _ => bail!("roster hooks only bind PLSEL2..5"),
    })
}

/// Emits source hooks and out-of-line adapters only. Loading the shared renderer,
/// reserving RAM/VRAM and storing the programs belong to the mode installer.
pub fn build_roster_source_hooks(
    number: u8,
    source: &[u8],
    adapter_origin: u32,
    adapter_capacity: usize,
    renderer: u32,
    bootstrap: u32,
) -> Result<RosterSourceHooks> {
    let (hash, length, sites) = sites(number)?;
    ensure!(source.len() == length, "roster overlay extent changed");
    let record = format!("DAT1/PLSEL{number}.BIN");
    let mut plan = DecodedRecordWritePlan::new(&record, source, hash)?;
    let reservation = ram_range(adapter_origin, adapter_capacity)?;
    ensure!(
        adapter_origin.is_multiple_of(4)
            && !overlaps(&reservation, &ram_range(ORIGIN, source.len())?)
            && !overlaps(&reservation, &ram_range(renderer, 4)?),
        "roster adapters overlap overlay or renderer"
    );
    let mut overlay = source.to_vec();
    let mut adapters = Vec::new();
    let mut claims = Vec::new();
    let mut code_ranges = Vec::new();
    let mut verifier = PsxMachineCodeSources::default();
    if number == 5 {
        // Cooperation retains the selected EDIT slot under character ID 30.
        // The native ready renderer only admits the battle IDs 31/32, so it
        // otherwise draws the fixed EDIT tile despite a valid nickname slot.
        let offset = 0xa3b8;
        let address = ORIGIN + offset as u32;
        let expected = [0x24a2ffe1u32, 0x2c420002]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>();
        ensure!(
            source[offset..offset + 8] == expected,
            "cooperation ready character-ID guard changed"
        );
        let instructions = vec![
            Addiu {
                rt: R::V0,
                rs: R::A1,
                immediate: -30,
            },
            Sltiu {
                rt: R::V0,
                rs: R::V0,
                immediate: 3,
            },
        ];
        let bytes = encode_le_bytes(&instructions, address)?;
        let id = "cooperation-ready-edit-name";
        let provenance = verifier.register(id, address, instructions)?;
        overlay[offset..offset + 8].copy_from_slice(&bytes);
        claims.push(CandidateWriteClaim {
            id: id.into(),
            purpose: "use the registered nickname for selected EDIT ID 30 and battle IDs 31/32"
                .into(),
            range: offset..offset + 8,
            intent: WriteIntent::MachineCode(provenance),
        });
        code_ranges.push([offset, offset + 8]);
    }
    // Reload sites include mode entry, standings and result/reentry paths.
    let load_sites: &[usize] = match number {
        2 => &[0x082c, 0x2884, 0x3ea4],
        3 => &[0x0754, 0x3350],
        4 => &[0x09f0, 0x3344],
        5 => &[0x4218, 0x457c],
        _ => unreachable!(),
    };
    for &offset in load_sites {
        let end = offset + 16;
        let entry = super::build_selector_bootstrap_entry(&source[offset..end], bootstrap)?;
        let address = ORIGIN + offset as u32;
        let instructions = psx_r3000a::verify_placed_program(&entry, address)?;
        let id = format!("roster-load-{offset:x}");
        let provenance = verifier.register(&id, address, instructions)?;
        overlay[offset..end].copy_from_slice(&entry);
        claims.push(CandidateWriteClaim {
            id,
            purpose: "reconstruct name suppliers after every selector resource load".into(),
            range: offset..end,
            intent: WriteIntent::MachineCode(provenance),
        });
        code_ranges.push([offset, end]);
    }
    for mut site in sites {
        let target = adapter_origin + u32::try_from(adapters.len())?;
        site.context.origin = target;
        site.context.capacity = adapter_capacity
            .checked_sub(adapters.len())
            .ok_or_else(|| anyhow::anyhow!("roster adapter reservation exhausted"))?;
        site.context.renderer = renderer;
        site.context.legacy = ORIGIN + site.guard as u32 + 12;
        site.context.next = ORIGIN + site.next as u32;
        adapters.extend(build_roster_adapter(&site.context)?);
        let length = site.context.length;
        let native_center = encode_le_bytes(
            &[
                Sll {
                    rd: R::V0,
                    rt: length,
                    shift: 1,
                },
                Addu {
                    rd: R::V0,
                    rs: R::V0,
                    rt: length,
                },
                Sll {
                    rd: R::V0,
                    rt: R::V0,
                    shift: 2,
                },
            ],
            ORIGIN + site.center as u32,
        )?;
        let guard: Vec<u8> = [
            0x2e820100u32,
            0x10400000 | ((site.next - site.guard - 8) / 4) as u32,
        ]
        .iter()
        .flat_map(|w| w.to_le_bytes())
        .collect();
        let advance = encode_le_bytes(
            &[Addiu {
                rt: R::S6,
                rs: R::S6,
                immediate: 12,
            }],
            ORIGIN + site.next as u32,
        )?;
        for (offset, expected, instructions) in [
            (
                site.guard,
                guard,
                vec![J { target }, psx_r3000a::Instruction::nop()],
            ),
            (
                site.center,
                native_center,
                super::roster_adapter::roster_text_width(length, R::V0).to_vec(),
            ),
            (
                site.next,
                advance,
                vec![Addiu {
                    rt: R::S6,
                    rs: R::S6,
                    immediate: super::roster_adapter::ROSTER_ADVANCE,
                }],
            ),
        ] {
            let end = offset + expected.len();
            ensure!(
                source.get(offset..end) == Some(expected.as_slice()),
                "roster source instructions changed at {offset:x}"
            );
            let address = ORIGIN + offset as u32;
            let bytes = encode_le_bytes(&instructions, address)?;
            ensure!(bytes.len() == expected.len(), "roster hook changed extent");
            let id = format!("roster-name-{offset:x}");
            let provenance = verifier.register(&id, address, instructions)?;
            overlay[offset..end].copy_from_slice(&bytes);
            claims.push(CandidateWriteClaim {
                id,
                purpose: "display canonical roster names with consistent advance".into(),
                range: offset..end,
                intent: WriteIntent::MachineCode(provenance),
            });
            code_ranges.push([offset, end]);
        }
    }
    plan.register_candidate(CandidateRecordWrite {
        owner: "roster-name-runtime",
        source_sha256: hash,
        candidate: &overlay,
        claims,
    })?;
    Ok(RosterSourceHooks {
        overlay: plan.apply(Some(&verifier))?,
        adapters,
        code_ranges,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires original source disc"]
    fn roster_hooks_bind_all_eleven_original_consumers() {
        for number in 2..=5 {
            let source = crate::source_disc::test_source_disc()
                .read_record(&format!("DAT1/PLSEL{number}.BIN"))
                .unwrap()
                .1;
            let result = build_roster_source_hooks(
                number, &source, 0x800b7000, 1024, 0x800af000, 0xa00dc340,
            )
            .unwrap();
            assert_eq!(
                result.code_ranges.len(),
                match number {
                    2 => 12,
                    5 => 9,
                    _ => 11,
                }
            );
            if number == 5 {
                let mut guard = result.overlay[0xa3b8..0xa3c0].to_vec();
                guard.extend(
                    encode_le_bytes(
                        &[Jr { rs: R::RA }, psx_r3000a::Instruction::nop()],
                        ORIGIN + 0xa3c0,
                    )
                    .unwrap(),
                );
                let mut memory = vec![0; 0x200000];
                for id in 0..=255 {
                    let mut registers = [0; 32];
                    registers[5] = id;
                    registers[31] = 0x800f0000;
                    crate::name_input::runtime_test_machine::execute(
                        &guard,
                        ORIGIN + 0xa3b8,
                        &mut registers,
                        &mut memory,
                    );
                    assert_eq!(registers[2], u32::from((30..=32).contains(&id)));
                    assert_eq!(registers[5], id);
                }
            }
            for (i, &byte) in source.iter().enumerate() {
                if !result.code_ranges.iter().any(|&[a, z]| (a..z).contains(&i)) {
                    assert_eq!(result.overlay[i], byte);
                }
            }
            for [a, z] in result.code_ranges {
                for i in a..z {
                    let mut bad = source.clone();
                    bad[i] ^= 1;
                    assert!(
                        build_roster_source_hooks(
                            number, &bad, 0x800b7000, 1024, 0x800af000, 0xa00dc340
                        )
                        .is_err()
                    );
                }
            }
            assert!(
                build_roster_source_hooks(
                    number, &source, 0x800a3000, 1024, 0x800af000, 0xa00dc340
                )
                .is_err()
            );
            assert!(
                build_roster_source_hooks(number, &source, 0x800b7000, 32, 0x800af000, 0xa00dc340)
                    .is_err()
            );
        }
    }
}
