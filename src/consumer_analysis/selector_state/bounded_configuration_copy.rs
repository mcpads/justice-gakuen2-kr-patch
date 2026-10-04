use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Result, ensure};
use psx_r3000a::{Instruction, Register};

use crate::source_disc::LoadedImage;

use super::profile_validation::{
    ensure_instruction, ensure_no_other_reachable_direct_entry,
    ensure_register_is_preserved_between, runtime_address,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct BoundedConfigurationCopyEvidence {
    pub(super) destination_addresses: BTreeSet<u32>,
    pub(super) source_instruction_offset: usize,
    pub(super) source_addresses: BTreeSet<u32>,
    pub(super) source_runtime_byte_range: [u32; 2],
}

#[derive(Clone, Copy)]
struct ThreeStreamCopyProfile {
    image_path: &'static str,
    outer_counter_setup_offset: usize,
    source_lui_offset: usize,
    source_low_offset: usize,
    destination_lui_offset: usize,
    destination_low_offset: usize,
    stream_setup_offset: usize,
}

const THREE_STREAM_COPY_PROFILES: &[ThreeStreamCopyProfile] = &[
    ThreeStreamCopyProfile {
        image_path: "DAT1/SIKEN.BIN",
        outer_counter_setup_offset: 0x29e4,
        source_lui_offset: 0x29e8,
        source_low_offset: 0x29ec,
        destination_lui_offset: 0x29f0,
        destination_low_offset: 0x29f4,
        stream_setup_offset: 0x29f8,
    },
    ThreeStreamCopyProfile {
        image_path: "DAT1/SIKEN2.BIN",
        outer_counter_setup_offset: 0x1fdc,
        source_lui_offset: 0x1f8c,
        source_low_offset: 0x1fac,
        destination_lui_offset: 0x1fb4,
        destination_low_offset: 0x1fd4,
        stream_setup_offset: 0x1ff4,
    },
];

pub(super) fn validated_bounded_configuration_copies(
    image: &LoadedImage,
    reachable_instruction_offsets: &BTreeSet<usize>,
) -> Result<BTreeMap<usize, BoundedConfigurationCopyEvidence>> {
    let mut result = BTreeMap::new();
    for profile in THREE_STREAM_COPY_PROFILES
        .iter()
        .filter(|profile| profile.image_path == image.path)
    {
        merge_evidence(
            &mut result,
            validate_three_stream_copy(image, reachable_instruction_offsets, profile)?,
            &image.path,
        )?;
    }
    if image.path == "DAT1/SKHAY.BIN" {
        merge_evidence(
            &mut result,
            validate_skhay_two_stream_copy(image, reachable_instruction_offsets)?,
            &image.path,
        )?;
    }
    Ok(result)
}

fn merge_evidence(
    result: &mut BTreeMap<usize, BoundedConfigurationCopyEvidence>,
    additions: BTreeMap<usize, BoundedConfigurationCopyEvidence>,
    image_path: &str,
) -> Result<()> {
    for (store_offset, evidence) in additions {
        ensure!(
            result.insert(store_offset, evidence).is_none(),
            "{image_path} repeats bounded configuration-copy store +0x{store_offset:x}"
        );
    }
    Ok(())
}

fn validate_three_stream_copy(
    image: &LoadedImage,
    reachable_instruction_offsets: &BTreeSet<usize>,
    profile: &ThreeStreamCopyProfile,
) -> Result<BTreeMap<usize, BoundedConfigurationCopyEvidence>> {
    let setup = profile.stream_setup_offset;
    ensure_instruction(
        image,
        profile.outer_counter_setup_offset,
        Instruction::Addu {
            rd: Register::A0,
            rs: Register::ZERO,
            rt: Register::ZERO,
        },
        "configuration-copy outer-counter setup",
    )?;
    ensure_literal_pointer(
        image,
        profile.source_lui_offset,
        profile.source_low_offset,
        Register::T3,
        0x6700,
        "configuration-copy source",
    )?;
    ensure_literal_pointer(
        image,
        profile.destination_lui_offset,
        profile.destination_low_offset,
        Register::T2,
        0x6480,
        "configuration-copy destination",
    )?;

    let expected = [
        Instruction::Addiu {
            rt: Register::A1,
            rs: Register::T2,
            immediate: 124,
        },
        Instruction::Addiu {
            rt: Register::T1,
            rs: Register::T3,
            immediate: 23,
        },
        Instruction::Addiu {
            rt: Register::T0,
            rs: Register::T2,
            immediate: 24,
        },
        Instruction::Addiu {
            rt: Register::A3,
            rs: Register::T3,
            immediate: 6,
        },
        Instruction::Addiu {
            rt: Register::A2,
            rs: Register::T2,
            immediate: 20,
        },
        Instruction::Addiu {
            rt: Register::V1,
            rs: Register::T3,
            immediate: 2,
        },
        Instruction::Addiu {
            rt: Register::T4,
            rs: Register::T2,
            immediate: 126,
        },
        Instruction::Lbu {
            rt: Register::V0,
            base: Register::V1,
            offset: 0,
        },
        Instruction::Addiu {
            rt: Register::V1,
            rs: Register::V1,
            immediate: 1,
        },
        Instruction::Sb {
            rt: Register::V0,
            base: Register::A2,
            offset: 0,
        },
        Instruction::Lbu {
            rt: Register::V0,
            base: Register::A3,
            offset: 0,
        },
        Instruction::Addiu {
            rt: Register::A3,
            rs: Register::A3,
            immediate: 1,
        },
        Instruction::Addiu {
            rt: Register::A2,
            rs: Register::A2,
            immediate: 1,
        },
        Instruction::Sb {
            rt: Register::V0,
            base: Register::T0,
            offset: 0,
        },
        Instruction::Lbu {
            rt: Register::V0,
            base: Register::T1,
            offset: 0,
        },
        Instruction::Addiu {
            rt: Register::T1,
            rs: Register::T1,
            immediate: 1,
        },
        Instruction::Sb {
            rt: Register::V0,
            base: Register::A1,
            offset: 0,
        },
        Instruction::Addiu {
            rt: Register::A1,
            rs: Register::A1,
            immediate: 1,
        },
        Instruction::Slt {
            rd: Register::V0,
            rs: Register::A1,
            rt: Register::T4,
        },
        Instruction::Bne {
            rs: Register::V0,
            rt: Register::ZERO,
            target: runtime_address(image, setup + 28)?,
        },
        Instruction::Addiu {
            rt: Register::T0,
            rs: Register::T0,
            immediate: 1,
        },
        Instruction::Addiu {
            rt: Register::T3,
            rs: Register::T3,
            immediate: 2,
        },
        Instruction::Addiu {
            rt: Register::A0,
            rs: Register::A0,
            immediate: 1,
        },
        Instruction::Slti {
            rt: Register::V0,
            rs: Register::A0,
            immediate: 2,
        },
        Instruction::Bne {
            rs: Register::V0,
            rt: Register::ZERO,
            target: runtime_address(image, setup)?,
        },
        Instruction::Addiu {
            rt: Register::T2,
            rs: Register::T2,
            immediate: 2,
        },
    ];
    ensure_sequence(image, setup, &expected, "three-stream configuration copy")?;
    ensure_copy_loop_entries(
        image,
        reachable_instruction_offsets,
        setup,
        setup + 28,
        setup + 76,
        setup + 96,
    )?;

    Ok(BTreeMap::from([
        (
            setup + 36,
            copy_evidence(
                0x801f_6494..=0x801f_6497,
                setup + 28,
                [0x801f_6702, 0x801f_6706],
            ),
        ),
        (
            setup + 52,
            copy_evidence(
                0x801f_6498..=0x801f_649b,
                setup + 40,
                [0x801f_6706, 0x801f_670a],
            ),
        ),
        (
            setup + 64,
            copy_evidence(
                0x801f_64fc..=0x801f_64ff,
                setup + 56,
                [0x801f_6717, 0x801f_671b],
            ),
        ),
    ]))
}

fn validate_skhay_two_stream_copy(
    image: &LoadedImage,
    reachable_instruction_offsets: &BTreeSet<usize>,
) -> Result<BTreeMap<usize, BoundedConfigurationCopyEvidence>> {
    const SETUP: usize = 0x574;
    ensure_instruction(
        image,
        0x4ec,
        Instruction::Addu {
            rd: Register::T8,
            rs: Register::ZERO,
            rt: Register::ZERO,
        },
        "SKHAY configuration-copy outer-counter setup",
    )?;
    ensure_literal_pointer(
        image,
        0x50c,
        0x514,
        Register::T1,
        0x6700,
        "SKHAY configuration-copy source",
    )?;
    ensure_literal_pointer(
        image,
        0x534,
        0x53c,
        Register::A3,
        0x6480,
        "SKHAY configuration-copy destination",
    )?;

    let expected = [
        Instruction::Addiu {
            rt: Register::V1,
            rs: Register::A3,
            immediate: 24,
        },
        Instruction::Addiu {
            rt: Register::A2,
            rs: Register::T1,
            immediate: 6,
        },
        Instruction::Addiu {
            rt: Register::A1,
            rs: Register::A3,
            immediate: 20,
        },
        Instruction::Addiu {
            rt: Register::A0,
            rs: Register::T1,
            immediate: 2,
        },
        Instruction::Addiu {
            rt: Register::T0,
            rs: Register::A3,
            immediate: 26,
        },
        Instruction::Lbu {
            rt: Register::V0,
            base: Register::A0,
            offset: 0,
        },
        Instruction::Addiu {
            rt: Register::A0,
            rs: Register::A0,
            immediate: 1,
        },
        Instruction::Sb {
            rt: Register::V0,
            base: Register::A1,
            offset: 0,
        },
        Instruction::Lbu {
            rt: Register::V0,
            base: Register::A2,
            offset: 0,
        },
        Instruction::Addiu {
            rt: Register::A2,
            rs: Register::A2,
            immediate: 1,
        },
        Instruction::Sb {
            rt: Register::V0,
            base: Register::V1,
            offset: 0,
        },
        Instruction::Addiu {
            rt: Register::V1,
            rs: Register::V1,
            immediate: 1,
        },
        Instruction::Slt {
            rd: Register::V0,
            rs: Register::V1,
            rt: Register::T0,
        },
        Instruction::Bne {
            rs: Register::V0,
            rt: Register::ZERO,
            target: runtime_address(image, SETUP + 20)?,
        },
        Instruction::Addiu {
            rt: Register::A1,
            rs: Register::A1,
            immediate: 1,
        },
        Instruction::Addiu {
            rt: Register::T1,
            rs: Register::T1,
            immediate: 2,
        },
        Instruction::Addiu {
            rt: Register::T8,
            rs: Register::T8,
            immediate: 1,
        },
        Instruction::Slti {
            rt: Register::V0,
            rs: Register::T8,
            immediate: 2,
        },
        Instruction::Bne {
            rs: Register::V0,
            rt: Register::ZERO,
            target: runtime_address(image, SETUP)?,
        },
        Instruction::Addiu {
            rt: Register::A3,
            rs: Register::A3,
            immediate: 2,
        },
    ];
    ensure_sequence(
        image,
        SETUP,
        &expected,
        "SKHAY two-stream configuration copy",
    )?;
    ensure_copy_loop_entries(
        image,
        reachable_instruction_offsets,
        SETUP,
        SETUP + 20,
        SETUP + 52,
        SETUP + 72,
    )?;

    Ok(BTreeMap::from([
        (
            SETUP + 28,
            copy_evidence(
                0x801f_6494..=0x801f_6497,
                SETUP + 20,
                [0x801f_6702, 0x801f_6706],
            ),
        ),
        (
            SETUP + 40,
            copy_evidence(
                0x801f_6498..=0x801f_649b,
                SETUP + 32,
                [0x801f_6706, 0x801f_670a],
            ),
        ),
    ]))
}

fn ensure_literal_pointer(
    image: &LoadedImage,
    lui_offset: usize,
    low_offset: usize,
    register: Register,
    low: u16,
    role: &str,
) -> Result<()> {
    ensure_instruction(
        image,
        lui_offset,
        Instruction::Lui {
            rt: register,
            immediate: 0x801f,
        },
        role,
    )?;
    ensure_register_is_preserved_between(image, lui_offset + 4, low_offset, register, role)?;
    ensure_instruction(
        image,
        low_offset,
        Instruction::Ori {
            rt: register,
            rs: register,
            immediate: low,
        },
        role,
    )
}

fn ensure_sequence(
    image: &LoadedImage,
    start_offset: usize,
    expected: &[Instruction],
    role: &str,
) -> Result<()> {
    for (index, instruction) in expected.iter().enumerate() {
        ensure_instruction(image, start_offset + index * 4, instruction.clone(), role)?;
    }
    Ok(())
}

fn ensure_copy_loop_entries(
    image: &LoadedImage,
    reachable_instruction_offsets: &BTreeSet<usize>,
    outer_setup_offset: usize,
    inner_head_offset: usize,
    inner_branch_offset: usize,
    outer_branch_offset: usize,
) -> Result<()> {
    ensure!(
        reachable_instruction_offsets.contains(&outer_setup_offset)
            && reachable_instruction_offsets.contains(&inner_head_offset),
        "{} bounded configuration-copy loop is outside the reachable closure",
        image.path
    );
    ensure_no_other_reachable_direct_entry(
        image,
        reachable_instruction_offsets,
        inner_branch_offset,
        runtime_address(image, inner_head_offset)?,
        "bounded configuration-copy inner loop",
    )?;
    ensure_no_other_reachable_direct_entry(
        image,
        reachable_instruction_offsets,
        outer_branch_offset,
        runtime_address(image, outer_setup_offset)?,
        "bounded configuration-copy outer loop",
    )
}

fn copy_evidence(
    destination_addresses: std::ops::RangeInclusive<u32>,
    source_instruction_offset: usize,
    source_runtime_byte_range: [u32; 2],
) -> BoundedConfigurationCopyEvidence {
    BoundedConfigurationCopyEvidence {
        destination_addresses: destination_addresses.collect(),
        source_instruction_offset,
        source_addresses: (source_runtime_byte_range[0]..source_runtime_byte_range[1]).collect(),
        source_runtime_byte_range,
    }
}

#[cfg(test)]
mod tests {
    use psx_r3000a::Assembler;

    use super::*;
    use crate::source_disc::LoadedImageEntrypoint;

    const RUNTIME_BASE: u32 = 0x800a_2000;
    const TEST_PROFILE: ThreeStreamCopyProfile = ThreeStreamCopyProfile {
        image_path: "DAT1/SYNTH.BIN",
        outer_counter_setup_offset: 0,
        source_lui_offset: 4,
        source_low_offset: 8,
        destination_lui_offset: 12,
        destination_low_offset: 16,
        stream_setup_offset: 20,
    };

    #[test]
    fn three_stream_copy_retains_only_its_finite_destination_and_source_ranges() {
        let image = test_image(2);
        let reachable = (0..image.data.len()).step_by(4).collect();

        let evidence = validate_three_stream_copy(&image, &reachable, &TEST_PROFILE).unwrap();

        assert_eq!(
            evidence[&56].destination_addresses,
            (0x801f_6494..=0x801f_6497).collect()
        );
        assert_eq!(
            evidence[&56].source_runtime_byte_range,
            [0x801f_6702, 0x801f_6706]
        );
        assert_eq!(evidence[&56].source_instruction_offset, 48);
        assert_eq!(
            evidence[&56].source_addresses,
            (0x801f_6702..0x801f_6706).collect()
        );
        assert_eq!(
            evidence[&72].destination_addresses,
            (0x801f_6498..=0x801f_649b).collect()
        );
        assert_eq!(
            evidence[&84].destination_addresses,
            (0x801f_64fc..=0x801f_64ff).collect()
        );
    }

    #[test]
    fn three_stream_copy_rejects_a_changed_outer_bound() {
        let image = test_image(3);
        let reachable = (0..image.data.len()).step_by(4).collect();

        let error = validate_three_stream_copy(&image, &reachable, &TEST_PROFILE).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("three-stream configuration copy changed")
        );
    }

    fn test_image(outer_bound: i16) -> LoadedImage {
        let setup_runtime_address = RUNTIME_BASE + TEST_PROFILE.stream_setup_offset as u32;
        let inner_runtime_address = setup_runtime_address + 28;
        let instructions = [
            Instruction::Addu {
                rd: Register::A0,
                rs: Register::ZERO,
                rt: Register::ZERO,
            },
            Instruction::Lui {
                rt: Register::T3,
                immediate: 0x801f,
            },
            Instruction::Ori {
                rt: Register::T3,
                rs: Register::T3,
                immediate: 0x6700,
            },
            Instruction::Lui {
                rt: Register::T2,
                immediate: 0x801f,
            },
            Instruction::Ori {
                rt: Register::T2,
                rs: Register::T2,
                immediate: 0x6480,
            },
            Instruction::Addiu {
                rt: Register::A1,
                rs: Register::T2,
                immediate: 124,
            },
            Instruction::Addiu {
                rt: Register::T1,
                rs: Register::T3,
                immediate: 23,
            },
            Instruction::Addiu {
                rt: Register::T0,
                rs: Register::T2,
                immediate: 24,
            },
            Instruction::Addiu {
                rt: Register::A3,
                rs: Register::T3,
                immediate: 6,
            },
            Instruction::Addiu {
                rt: Register::A2,
                rs: Register::T2,
                immediate: 20,
            },
            Instruction::Addiu {
                rt: Register::V1,
                rs: Register::T3,
                immediate: 2,
            },
            Instruction::Addiu {
                rt: Register::T4,
                rs: Register::T2,
                immediate: 126,
            },
            Instruction::Lbu {
                rt: Register::V0,
                base: Register::V1,
                offset: 0,
            },
            Instruction::Addiu {
                rt: Register::V1,
                rs: Register::V1,
                immediate: 1,
            },
            Instruction::Sb {
                rt: Register::V0,
                base: Register::A2,
                offset: 0,
            },
            Instruction::Lbu {
                rt: Register::V0,
                base: Register::A3,
                offset: 0,
            },
            Instruction::Addiu {
                rt: Register::A3,
                rs: Register::A3,
                immediate: 1,
            },
            Instruction::Addiu {
                rt: Register::A2,
                rs: Register::A2,
                immediate: 1,
            },
            Instruction::Sb {
                rt: Register::V0,
                base: Register::T0,
                offset: 0,
            },
            Instruction::Lbu {
                rt: Register::V0,
                base: Register::T1,
                offset: 0,
            },
            Instruction::Addiu {
                rt: Register::T1,
                rs: Register::T1,
                immediate: 1,
            },
            Instruction::Sb {
                rt: Register::V0,
                base: Register::A1,
                offset: 0,
            },
            Instruction::Addiu {
                rt: Register::A1,
                rs: Register::A1,
                immediate: 1,
            },
            Instruction::Slt {
                rd: Register::V0,
                rs: Register::A1,
                rt: Register::T4,
            },
            Instruction::Bne {
                rs: Register::V0,
                rt: Register::ZERO,
                target: inner_runtime_address,
            },
            Instruction::Addiu {
                rt: Register::T0,
                rs: Register::T0,
                immediate: 1,
            },
            Instruction::Addiu {
                rt: Register::T3,
                rs: Register::T3,
                immediate: 2,
            },
            Instruction::Addiu {
                rt: Register::A0,
                rs: Register::A0,
                immediate: 1,
            },
            Instruction::Slti {
                rt: Register::V0,
                rs: Register::A0,
                immediate: outer_bound,
            },
            Instruction::Bne {
                rs: Register::V0,
                rt: Register::ZERO,
                target: setup_runtime_address,
            },
            Instruction::Addiu {
                rt: Register::T2,
                rs: Register::T2,
                immediate: 2,
            },
            Instruction::Jr { rs: Register::RA },
            Instruction::nop(),
        ];
        let mut assembler = Assembler::new();
        for instruction in instructions {
            assembler.emit(instruction);
        }
        let program = assembler.assemble(RUNTIME_BASE).unwrap();
        LoadedImage {
            path: TEST_PROFILE.image_path.to_string(),
            data: program.bytes().to_vec(),
            runtime_base: Some(RUNTIME_BASE),
            entrypoints: vec![LoadedImageEntrypoint {
                role: "primary",
                source_reference_kind: "fixture",
                source_reference_offset: 0,
                runtime_address: RUNTIME_BASE,
            }],
        }
    }
}
