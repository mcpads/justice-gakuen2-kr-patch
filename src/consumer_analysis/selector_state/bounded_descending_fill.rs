use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};
use psx_r3000a::{Instruction, Register};

use crate::source_disc::LoadedImage;

use super::profile_validation::{
    ensure_instruction, ensure_no_other_reachable_direct_entry,
    ensure_register_is_preserved_between,
};

#[derive(Clone, Copy)]
struct BoundedDescendingFillProfile {
    image_path: &'static str,
    counter_setup_offset: usize,
    pointer_lui_offset: usize,
    pointer_low_offset: usize,
    store_offset: usize,
    counter_register: Register,
    pointer_register: Register,
    value_register: Register,
    iteration_maximum: i16,
    pointer_low: u16,
    store_displacement: i16,
    store_width_bytes: u8,
    pointer_step: i16,
}

const BOUNDED_DESCENDING_FILL_PROFILES: &[BoundedDescendingFillProfile] = &[
    profile(
        "DAT1/MGAME.BIN",
        0x3e78,
        0x3e7c,
        0x3e80,
        0x3ea4,
        Register::V1,
        Register::A0,
    ),
    profile(
        "DAT1/MGAME.BIN",
        0x3f14,
        0x3f18,
        0x3f1c,
        0x3f34,
        Register::V1,
        Register::A0,
    ),
    profile(
        "DAT1/MGAME.BIN",
        0x3fd4,
        0x3fd8,
        0x3fdc,
        0x4000,
        Register::V1,
        Register::A0,
    ),
    profile(
        "DAT1/MGAME.BIN",
        0x4058,
        0x405c,
        0x4060,
        0x4084,
        Register::V1,
        Register::A0,
    ),
    profile(
        "DAT1/MGAME01.BIN",
        0x08b0,
        0x08b4,
        0x08b8,
        0x08bc,
        Register::V0,
        Register::V1,
    ),
    profile(
        "DAT1/MGAME06.BIN",
        0x08e0,
        0x08e4,
        0x08e8,
        0x08ec,
        Register::V0,
        Register::V1,
    ),
    profile(
        "DAT1/MGAME07.BIN",
        0x07f8,
        0x07fc,
        0x0800,
        0x0804,
        Register::V0,
        Register::V1,
    ),
    profile(
        "DAT1/SIKEN.BIN",
        0x2864,
        0x2870,
        0x287c,
        0x289c,
        Register::A0,
        Register::A1,
    ),
    profile(
        "DAT1/SIKEN2.BIN",
        0x1e20,
        0x1e2c,
        0x1e38,
        0x1e60,
        Register::A0,
        Register::A1,
    ),
    BoundedDescendingFillProfile {
        image_path: "DAT1/SIKEN2.BIN",
        counter_setup_offset: 0x0e4c,
        pointer_lui_offset: 0x0e88,
        pointer_low_offset: 0x0f3c,
        store_offset: 0x0fd8,
        counter_register: Register::T9,
        pointer_register: Register::T8,
        value_register: Register::S3,
        iteration_maximum: 9,
        pointer_low: 0x5a8d,
        store_displacement: 26,
        store_width_bytes: 1,
        pointer_step: -1,
    },
    BoundedDescendingFillProfile {
        image_path: "DAT1/MGAME.BIN",
        counter_setup_offset: 0x42d8,
        pointer_lui_offset: 0x42e4,
        pointer_low_offset: 0x42e8,
        store_offset: 0x42ec,
        counter_register: Register::V1,
        pointer_register: Register::V0,
        value_register: Register::ZERO,
        iteration_maximum: 6,
        pointer_low: 0x1806,
        store_displacement: 376,
        store_width_bytes: 1,
        pointer_step: -1,
    },
    BoundedDescendingFillProfile {
        image_path: "DAT1/MGAME.BIN",
        counter_setup_offset: 0x2762c,
        pointer_lui_offset: 0x27630,
        pointer_low_offset: 0x27634,
        store_offset: 0x27638,
        counter_register: Register::V1,
        pointer_register: Register::V0,
        value_register: Register::S0,
        iteration_maximum: 19,
        pointer_low: 0x1c26,
        store_displacement: 14,
        store_width_bytes: 2,
        pointer_step: -2,
    },
    profile(
        "DAT1/SKHAY.BIN",
        0x0228,
        0x022c,
        0x0238,
        0x0258,
        Register::T8,
        Register::A0,
    ),
    profile(
        "SLPS_021.20",
        0x7290,
        0x7294,
        0x7298,
        0x72d8,
        Register::V1,
        Register::A0,
    ),
    profile(
        "SLPS_021.20",
        0x7398,
        0x739c,
        0x73a0,
        0x73e4,
        Register::A0,
        Register::A1,
    ),
    profile(
        "SLPS_021.20",
        0x74c0,
        0x74c4,
        0x74c8,
        0x74cc,
        Register::V0,
        Register::V1,
    ),
    profile(
        "SLPS_021.20",
        0x7600,
        0x7604,
        0x7608,
        0x761c,
        Register::V0,
        Register::V1,
    ),
    profile(
        "SLPS_021.20",
        0x77b4,
        0x77bc,
        0x77c8,
        0x77e8,
        Register::V1,
        Register::A0,
    ),
    profile(
        "SLPS_021.20",
        0x7a0c,
        0x7a10,
        0x7a14,
        0x7a30,
        Register::V0,
        Register::V1,
    ),
    profile(
        "SLPS_021.20",
        0x7d68,
        0x7d6c,
        0x7d70,
        0x7d74,
        Register::A0,
        Register::V0,
    ),
];

const fn profile(
    image_path: &'static str,
    counter_setup_offset: usize,
    pointer_lui_offset: usize,
    pointer_low_offset: usize,
    store_offset: usize,
    counter_register: Register,
    pointer_register: Register,
) -> BoundedDescendingFillProfile {
    BoundedDescendingFillProfile {
        image_path,
        counter_setup_offset,
        pointer_lui_offset,
        pointer_low_offset,
        store_offset,
        counter_register,
        pointer_register,
        value_register: counter_register,
        iteration_maximum: 3,
        pointer_low: 0x6483,
        store_displacement: 0x80,
        store_width_bytes: 1,
        pointer_step: -1,
    }
}

pub(super) fn validated_bounded_descending_fill_addresses(
    image: &LoadedImage,
    reachable_instruction_offsets: &BTreeSet<usize>,
) -> Result<BTreeMap<usize, BTreeSet<u32>>> {
    let mut result = BTreeMap::new();
    for profile in BOUNDED_DESCENDING_FILL_PROFILES
        .iter()
        .filter(|profile| profile.image_path == image.path)
    {
        let addresses = validate_profile(image, reachable_instruction_offsets, profile)?;
        ensure!(
            result.insert(profile.store_offset, addresses).is_none(),
            "{} has repeated bounded descending-fill store +0x{:x}",
            image.path,
            profile.store_offset
        );
    }
    Ok(result)
}

fn validate_profile(
    image: &LoadedImage,
    reachable_instruction_offsets: &BTreeSet<usize>,
    profile: &BoundedDescendingFillProfile,
) -> Result<BTreeSet<u32>> {
    let runtime_base = image
        .runtime_base
        .context("bounded descending-fill image lacks a runtime base")?;
    let store_runtime_address = runtime_base
        .checked_add(u32::try_from(profile.store_offset)?)
        .context("bounded descending-fill store address overflow")?;

    ensure_instruction(
        image,
        profile.counter_setup_offset,
        Instruction::Addiu {
            rt: profile.counter_register,
            rs: Register::ZERO,
            immediate: profile.iteration_maximum,
        },
        "counter setup",
    )?;
    ensure_instruction(
        image,
        profile.pointer_lui_offset,
        Instruction::Lui {
            rt: profile.pointer_register,
            immediate: 0x801f,
        },
        "pointer high half",
    )?;
    ensure_instruction(
        image,
        profile.pointer_low_offset,
        Instruction::Ori {
            rt: profile.pointer_register,
            rs: profile.pointer_register,
            immediate: profile.pointer_low,
        },
        "pointer low half",
    )?;
    let expected_store = match profile.store_width_bytes {
        1 => Instruction::Sb {
            rt: profile.value_register,
            base: profile.pointer_register,
            offset: profile.store_displacement,
        },
        2 => Instruction::Sh {
            rt: profile.value_register,
            base: profile.pointer_register,
            offset: profile.store_displacement,
        },
        other => anyhow::bail!(
            "{} bounded descending-fill has unsupported {other}-byte store",
            image.path
        ),
    };
    ensure_instruction(image, profile.store_offset, expected_store, "store")?;
    ensure_instruction(
        image,
        profile.store_offset + 4,
        Instruction::Addiu {
            rt: profile.counter_register,
            rs: profile.counter_register,
            immediate: -1,
        },
        "counter decrement",
    )?;
    ensure_instruction(
        image,
        profile.store_offset + 8,
        Instruction::Bgez {
            rs: profile.counter_register,
            target: store_runtime_address,
        },
        "loop branch",
    )?;
    ensure_instruction(
        image,
        profile.store_offset + 12,
        Instruction::Addiu {
            rt: profile.pointer_register,
            rs: profile.pointer_register,
            immediate: profile.pointer_step,
        },
        "pointer decrement",
    )?;

    ensure!(
        reachable_instruction_offsets.contains(&profile.counter_setup_offset)
            && reachable_instruction_offsets.contains(&profile.store_offset),
        "{} bounded descending-fill setup or store is outside the reachable closure",
        image.path
    );
    ensure_register_is_preserved_between(
        image,
        profile.counter_setup_offset + 4,
        profile.store_offset,
        profile.counter_register,
        "counter",
    )?;
    ensure_register_is_preserved_between(
        image,
        profile.pointer_low_offset + 4,
        profile.store_offset,
        profile.pointer_register,
        "pointer",
    )?;
    ensure_no_other_reachable_direct_entry(
        image,
        reachable_instruction_offsets,
        profile.store_offset + 8,
        store_runtime_address,
        "bounded descending-fill store",
    )?;

    ensure!(
        profile.iteration_maximum >= 0,
        "{} bounded descending-fill iteration maximum is negative",
        image.path
    );
    ensure!(
        profile.pointer_step < 0,
        "{} bounded descending-fill pointer step is not negative",
        image.path
    );
    let initial_address =
        0x801f_0000_i64 + i64::from(profile.pointer_low) + i64::from(profile.store_displacement);
    let mut addresses = BTreeSet::new();
    for iteration in 0..=profile.iteration_maximum {
        let store_address =
            initial_address + i64::from(iteration) * i64::from(profile.pointer_step);
        for byte_offset in 0..profile.store_width_bytes {
            addresses.insert(
                u32::try_from(store_address + i64::from(byte_offset))
                    .context("bounded descending-fill store address is outside u32")?,
            );
        }
    }
    Ok(addresses)
}

#[cfg(test)]
mod tests {
    use psx_r3000a::{Assembler, Instruction};

    use super::*;
    use crate::source_disc::LoadedImageEntrypoint;

    const RUNTIME_BASE: u32 = 0x800a_2000;
    const TEST_PROFILE: BoundedDescendingFillProfile = BoundedDescendingFillProfile {
        image_path: "DAT1/SYNTH.BIN",
        counter_setup_offset: 0,
        pointer_lui_offset: 4,
        pointer_low_offset: 8,
        store_offset: 12,
        counter_register: Register::V0,
        pointer_register: Register::V1,
        value_register: Register::V0,
        iteration_maximum: 3,
        pointer_low: 0x6483,
        store_displacement: 0x80,
        store_width_bytes: 1,
        pointer_step: -1,
    };

    const WIDE_TEST_PROFILE: BoundedDescendingFillProfile = BoundedDescendingFillProfile {
        image_path: "DAT1/SYNTH.BIN",
        counter_setup_offset: 0,
        pointer_lui_offset: 4,
        pointer_low_offset: 8,
        store_offset: 12,
        counter_register: Register::T9,
        pointer_register: Register::T8,
        value_register: Register::S3,
        iteration_maximum: 9,
        pointer_low: 0x5a8d,
        store_displacement: 26,
        store_width_bytes: 1,
        pointer_step: -1,
    };

    const HALFWORD_TEST_PROFILE: BoundedDescendingFillProfile = BoundedDescendingFillProfile {
        image_path: "DAT1/SYNTH.BIN",
        counter_setup_offset: 0,
        pointer_lui_offset: 4,
        pointer_low_offset: 8,
        store_offset: 12,
        counter_register: Register::V1,
        pointer_register: Register::V0,
        value_register: Register::S0,
        iteration_maximum: 2,
        pointer_low: 0x1c26,
        store_displacement: 14,
        store_width_bytes: 2,
        pointer_step: -2,
    };

    #[test]
    fn bounded_loop_retains_only_its_four_actual_store_addresses() {
        let image = test_image(&TEST_PROFILE, 3);
        let reachable = (0..image.data.len()).step_by(4).collect();

        let addresses = validate_profile(&image, &reachable, &TEST_PROFILE).unwrap();

        assert_eq!(
            addresses,
            BTreeSet::from([0x801f_6500, 0x801f_6501, 0x801f_6502, 0x801f_6503])
        );
    }

    #[test]
    fn bounded_loop_uses_its_declared_count_pointer_and_store_displacement() {
        let image = test_image(&WIDE_TEST_PROFILE, 9);
        let reachable = (0..image.data.len()).step_by(4).collect();

        let addresses = validate_profile(&image, &reachable, &WIDE_TEST_PROFILE).unwrap();

        assert_eq!(
            addresses,
            (0x801f_5a9e..=0x801f_5aa7).collect::<BTreeSet<_>>()
        );
    }

    #[test]
    fn bounded_loop_retains_every_byte_of_strided_halfword_stores() {
        let image = test_image(&HALFWORD_TEST_PROFILE, 2);
        let reachable = (0..image.data.len()).step_by(4).collect();

        let addresses = validate_profile(&image, &reachable, &HALFWORD_TEST_PROFILE).unwrap();

        assert_eq!(
            addresses,
            (0x801f_1c30..=0x801f_1c35).collect::<BTreeSet<_>>()
        );
    }

    #[test]
    fn bounded_loop_rejects_a_changed_iteration_count() {
        let image = test_image(&TEST_PROFILE, 4);
        let reachable = (0..image.data.len()).step_by(4).collect();

        let error = validate_profile(&image, &reachable, &TEST_PROFILE).unwrap_err();

        assert!(error.to_string().contains("counter setup changed"));
    }

    fn test_image(profile: &BoundedDescendingFillProfile, iteration_maximum: i16) -> LoadedImage {
        let instructions = [
            Instruction::Addiu {
                rt: profile.counter_register,
                rs: Register::ZERO,
                immediate: iteration_maximum,
            },
            Instruction::Lui {
                rt: profile.pointer_register,
                immediate: 0x801f,
            },
            Instruction::Ori {
                rt: profile.pointer_register,
                rs: profile.pointer_register,
                immediate: profile.pointer_low,
            },
            match profile.store_width_bytes {
                1 => Instruction::Sb {
                    rt: profile.value_register,
                    base: profile.pointer_register,
                    offset: profile.store_displacement,
                },
                2 => Instruction::Sh {
                    rt: profile.value_register,
                    base: profile.pointer_register,
                    offset: profile.store_displacement,
                },
                _ => unreachable!("test profile uses a supported store width"),
            },
            Instruction::Addiu {
                rt: profile.counter_register,
                rs: profile.counter_register,
                immediate: -1,
            },
            Instruction::Bgez {
                rs: profile.counter_register,
                target: RUNTIME_BASE + 12,
            },
            Instruction::Addiu {
                rt: profile.pointer_register,
                rs: profile.pointer_register,
                immediate: profile.pointer_step,
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
