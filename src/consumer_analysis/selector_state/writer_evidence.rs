use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Result, ensure};
use psx_r3000a::{Instruction, Register};

use crate::source_disc::LoadedImage;

use super::bounded_configuration_copy::BoundedConfigurationCopyEvidence;
use super::profile_validation::{
    ensure_direct_call_offsets, ensure_instruction, ensure_only_reachable_direct_entries,
    ensure_register_is_preserved_between, runtime_address,
};
use super::source_bound_zero_fill::{validate_profiled_zero_fill_writer, validate_zero_fill_loop};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ValidatedSelectorWriterEvidence {
    pub(super) classification: &'static str,
    pub(super) value_resolution: &'static str,
    pub(super) exact_value_candidates: Vec<u8>,
    pub(super) bounded_value_range: Option<[u8; 2]>,
    pub(super) source_runtime_byte_ranges: Vec<[u32; 2]>,
    pub(super) evidence: &'static str,
}

#[derive(Clone, Copy)]
struct StoreSite {
    offset: usize,
    destination_low: u16,
}

#[derive(Clone, Copy)]
struct DirectLiteralWriterProfile {
    image_path: &'static str,
    producer_offset: usize,
    store: StoreSite,
    register: Register,
    value: u8,
}

#[derive(Clone, Copy)]
struct BranchDelayLiteralWriterProfile {
    image_path: &'static str,
    branches: &'static [BranchSite],
    block_offset: usize,
    stores: &'static [StoreSite],
    value: u8,
}

#[derive(Clone, Copy)]
struct BranchSite {
    offset: usize,
    branch_when_equal: bool,
}

const DIRECT_LITERAL_WRITERS: &[DirectLiteralWriterProfile] = &[
    literal("DAT1/KANRI.BIN", 0x5308, 0x5318, 0x6494, Register::V0, 30),
    literal("DAT1/KANRI.BIN", 0x6050, 0x6058, 0x64fc, Register::V0, 16),
    literal("DAT1/MGAME01.BIN", 0x0c78, 0x0c80, 0x6496, Register::V0, 38),
    literal("DAT1/MGAME01.BIN", 0x0c98, 0x0ca0, 0x6494, Register::V0, 38),
    literal("DAT1/MGAME01.BIN", 0x0ccc, 0x0cdc, 0x649a, Register::V0, 1),
    literal("DAT1/MGAME05.BIN", 0x0c4c, 0x0c64, 0x649a, Register::V0, 1),
    literal("DAT1/SKHAY.BIN", 0x0134, 0x013c, 0x64be, Register::V0, 7),
    literal("DAT1/SKHAY.BIN", 0x0560, 0x0570, 0x64be, Register::V0, 7),
];

const BRANCH_DELAY_LITERAL_WRITERS: &[BranchDelayLiteralWriterProfile] = &[
    BranchDelayLiteralWriterProfile {
        image_path: "DAT1/MGAME01.BIN",
        branches: &[
            BranchSite {
                offset: 0x0c20,
                branch_when_equal: false,
            },
            BranchSite {
                offset: 0x0c60,
                branch_when_equal: true,
            },
        ],
        block_offset: 0x0cbc,
        stores: &[
            StoreSite {
                offset: 0x0cc0,
                destination_low: 0x6494,
            },
            StoreSite {
                offset: 0x0cc8,
                destination_low: 0x6496,
            },
        ],
        value: 38,
    },
    BranchDelayLiteralWriterProfile {
        image_path: "DAT1/MGAME05.BIN",
        branches: &[BranchSite {
            offset: 0x0c14,
            branch_when_equal: false,
        }],
        block_offset: 0x0c3c,
        stores: &[
            StoreSite {
                offset: 0x0c40,
                destination_low: 0x6494,
            },
            StoreSite {
                offset: 0x0c48,
                destination_low: 0x6496,
            },
        ],
        value: 38,
    },
];

const DIRECT_ZERO_WRITERS: &[(&str, StoreSite)] = &[
    ("DAT1/MGAME01.BIN", site(0x0c88, 0x649a)),
    ("DAT1/MGAME01.BIN", site(0x0ca8, 0x6498)),
    ("DAT1/MGAME01.BIN", site(0x0cd4, 0x6498)),
    ("DAT1/MGAME01.BIN", site(0x0cec, 0x6495)),
    ("DAT1/MGAME01.BIN", site(0x0cf4, 0x6497)),
    ("DAT1/MGAME05.BIN", site(0x0c54, 0x6498)),
    ("DAT1/MGAME05.BIN", site(0x0c5c, 0x6495)),
    ("DAT1/MGAME05.BIN", site(0x0c6c, 0x6497)),
    ("DAT1/PLSEL5.BIN", site(0x9dd0, 0x64be)),
    ("DAT1/RKDEMO.BIN", site(0x0178, 0x6495)),
    ("DAT1/RKDEMO.BIN", site(0x0180, 0x6499)),
    ("DAT1/RKDEMO.BIN", site(0x01b8, 0x649a)),
    ("DAT1/RKDEMO.BIN", site(0x01c0, 0x6497)),
    ("DAT1/RKDEMO.BIN", site(0x01c8, 0x649b)),
];

const SELECTOR_PROFILED_ZERO_FILL_WRITERS: &[(&str, usize)] = &[
    ("DAT1/MGAME01.BIN", 0x6308),
    ("DAT1/MGAME02.BIN", 0x6078),
    ("DAT1/MGAME03.BIN", 0x5e28),
    ("DAT1/MGAME05.BIN", 0x4ee4),
    ("DAT1/MGAME06.BIN", 0x756c),
    ("DAT1/MGAME07.BIN", 0x68f8),
];

const fn literal(
    image_path: &'static str,
    producer_offset: usize,
    store_offset: usize,
    destination_low: u16,
    register: Register,
    value: u8,
) -> DirectLiteralWriterProfile {
    DirectLiteralWriterProfile {
        image_path,
        producer_offset,
        store: site(store_offset, destination_low),
        register,
        value,
    }
}

const fn site(offset: usize, destination_low: u16) -> StoreSite {
    StoreSite {
        offset,
        destination_low,
    }
}

pub(super) fn validated_selector_writer_evidence(
    image: &LoadedImage,
    reachable_instruction_offsets: &BTreeSet<usize>,
    bounded_copies: &BTreeMap<usize, BoundedConfigurationCopyEvidence>,
) -> Result<BTreeMap<usize, ValidatedSelectorWriterEvidence>> {
    let mut result = BTreeMap::new();

    for profile in DIRECT_LITERAL_WRITERS
        .iter()
        .filter(|profile| profile.image_path == image.path)
    {
        validate_direct_literal_writer(image, profile)?;
        insert(
            &mut result,
            profile.store.offset,
            exact_value(
                "fixed_literal",
                profile.value,
                "validated local literal producer and direct byte store",
            ),
            &image.path,
        )?;
    }
    for profile in BRANCH_DELAY_LITERAL_WRITERS
        .iter()
        .filter(|profile| profile.image_path == image.path)
    {
        validate_branch_delay_literal_writer(image, reachable_instruction_offsets, profile)?;
        for store in profile.stores {
            insert(
                &mut result,
                store.offset,
                exact_value(
                    "fixed_literal",
                    profile.value,
                    "validated branch-delay literal producers and complete direct-entry set",
                ),
                &image.path,
            )?;
        }
    }
    for (_, store) in DIRECT_ZERO_WRITERS
        .iter()
        .filter(|(image_path, _)| *image_path == image.path)
    {
        validate_direct_store(image, *store, Register::ZERO)?;
        insert(
            &mut result,
            store.offset,
            exact_value(
                "fixed_zero",
                0,
                "typed direct byte store from the architectural zero register",
            ),
            &image.path,
        )?;
    }
    for (_, store_offset) in SELECTOR_PROFILED_ZERO_FILL_WRITERS
        .iter()
        .filter(|(image_path, _)| *image_path == image.path)
    {
        validate_profiled_zero_fill_writer(image, *store_offset)?;
        insert(
            &mut result,
            *store_offset,
            exact_value(
                "zero_fill",
                0,
                "validated byte-wise zero-fill loop, complete direct caller and byte-count set, and fixed state-block footprints",
            ),
            &image.path,
        )?;
    }
    if image.path == "SLPS_021.20" {
        validate_zero_fill_loop(image, 0x500e0)?;
        insert(
            &mut result,
            0x500e0,
            exact_value("zero_fill", 0, "validated byte-wise zero-fill loop"),
            &image.path,
        )?;
    }
    for (&store_offset, copy) in bounded_copies {
        insert(
            &mut result,
            store_offset,
            ValidatedSelectorWriterEvidence {
                classification: "configuration_byte_copy",
                value_resolution: "runtime_source",
                exact_value_candidates: Vec::new(),
                bounded_value_range: None,
                source_runtime_byte_ranges: vec![copy.source_runtime_byte_range],
                evidence: "validated finite two-by-two configuration copy",
            },
            &image.path,
        )?;
    }

    validate_image_specific_dynamic_writers(image, reachable_instruction_offsets, &mut result)?;
    Ok(result)
}

fn validate_direct_literal_writer(
    image: &LoadedImage,
    profile: &DirectLiteralWriterProfile,
) -> Result<()> {
    ensure_instruction(
        image,
        profile.producer_offset,
        Instruction::Addiu {
            rt: profile.register,
            rs: Register::ZERO,
            immediate: i16::from(profile.value),
        },
        "direct selector literal producer",
    )?;
    ensure_register_is_preserved_between(
        image,
        profile.producer_offset + 4,
        profile.store.offset,
        profile.register,
        "direct selector literal",
    )?;
    validate_direct_store(image, profile.store, profile.register)
}

fn validate_branch_delay_literal_writer(
    image: &LoadedImage,
    reachable_instruction_offsets: &BTreeSet<usize>,
    profile: &BranchDelayLiteralWriterProfile,
) -> Result<()> {
    let target = runtime_address(image, profile.block_offset)?;
    for branch_site in profile.branches {
        let branch = if branch_site.branch_when_equal {
            Instruction::Beq {
                rs: Register::V1,
                rt: Register::V0,
                target,
            }
        } else {
            Instruction::Bne {
                rs: Register::V1,
                rt: Register::V0,
                target,
            }
        };
        ensure_instruction(image, branch_site.offset, branch, "selector literal branch")?;
        ensure_instruction(
            image,
            branch_site.offset + 4,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::ZERO,
                immediate: i16::from(profile.value),
            },
            "selector branch-delay literal producer",
        )?;
    }
    let branch_offsets = profile
        .branches
        .iter()
        .map(|branch| branch.offset)
        .collect::<Vec<_>>();
    ensure_only_reachable_direct_entries(
        image,
        reachable_instruction_offsets,
        &branch_offsets,
        target,
        "selector branch literal block",
    )?;
    let last_store = profile.stores.last().expect("profile stores are non-empty");
    ensure_register_is_preserved_between(
        image,
        profile.block_offset,
        last_store.offset,
        Register::V0,
        "selector branch-delay literal",
    )?;
    for store in profile.stores {
        validate_direct_store(image, *store, Register::V0)?;
    }
    Ok(())
}

fn validate_direct_store(image: &LoadedImage, site: StoreSite, source: Register) -> Result<()> {
    ensure_instruction(
        image,
        site.offset - 4,
        Instruction::Lui {
            rt: Register::AT,
            immediate: 0x801f,
        },
        "selector direct-store address",
    )?;
    ensure_instruction(
        image,
        site.offset,
        Instruction::Sb {
            rt: source,
            base: Register::AT,
            offset: site.destination_low as i16,
        },
        "selector direct byte store",
    )
}

fn validate_image_specific_dynamic_writers(
    image: &LoadedImage,
    reachable_instruction_offsets: &BTreeSet<usize>,
    result: &mut BTreeMap<usize, ValidatedSelectorWriterEvidence>,
) -> Result<()> {
    match image.path.as_str() {
        "DAT1/KANRI.BIN" => {
            validate_kanri_custom_index_source(image, reachable_instruction_offsets)?;
            validate_direct_store(image, site(0x5330, 0x64fc), Register::S2)?;
            insert(
                result,
                0x5330,
                kanri_custom_index_writer_evidence(),
                &image.path,
            )?;
        }
        "DAT1/PLSEL5.BIN" => validate_plsel5_incremented_state(image, result)?,
        "DAT1/RKDEMO.BIN" => validate_rkdemo_writers(image, reachable_instruction_offsets, result)?,
        _ => {}
    }
    Ok(())
}

pub(super) fn validate_kanri_custom_index_source(
    image: &LoadedImage,
    reachable_instruction_offsets: &BTreeSet<usize>,
) -> Result<()> {
    let function_offset = 0x51b4;
    let caller_offsets = [0x5ea0, 0x62b0];
    ensure_direct_call_offsets(
        image,
        runtime_address(image, function_offset)?,
        &caller_offsets,
        "KANRI custom-index function",
    )?;
    for caller_offset in caller_offsets {
        ensure!(
            reachable_instruction_offsets.contains(&caller_offset),
            "{} KANRI custom-index caller +0x{caller_offset:x} is not reachable",
            image.path
        );
        ensure_instruction(
            image,
            caller_offset,
            Instruction::Jal {
                target: runtime_address(image, function_offset)?,
            },
            "KANRI custom-index caller",
        )?;
    }
    ensure_only_reachable_direct_entries(
        image,
        reachable_instruction_offsets,
        &caller_offsets,
        runtime_address(image, function_offset)?,
        "KANRI custom-index function",
    )?;
    ensure_instruction(
        image,
        0x5e9c,
        Instruction::Lh {
            rt: Register::A0,
            base: Register::S0,
            offset: 14,
        },
        "KANRI runtime custom-index argument",
    )?;
    ensure_instruction(
        image,
        0x62b4,
        Instruction::Addiu {
            rt: Register::A0,
            rs: Register::ZERO,
            immediate: 16,
        },
        "KANRI literal custom-index argument",
    )?;
    ensure_instruction(
        image,
        0x51bc,
        Instruction::Addu {
            rd: Register::S2,
            rs: Register::A0,
            rt: Register::ZERO,
        },
        "KANRI custom-index preservation",
    )?;
    ensure_register_is_preserved_between(
        image,
        0x51c0,
        0x5330,
        Register::S2,
        "KANRI custom index",
    )?;

    let runtime_source_chain = [
        (
            0x1654,
            Instruction::Lui {
                rt: Register::S1,
                immediate: 0x801f,
            },
        ),
        (
            0x1658,
            Instruction::Ori {
                rt: Register::S1,
                rs: Register::S1,
                immediate: 0x1a00,
            },
        ),
        (
            0x1854,
            Instruction::Jal {
                target: runtime_address(image, 0x6184)?,
            },
        ),
        (
            0x1858,
            Instruction::Addu {
                rd: Register::A0,
                rs: Register::S1,
                rt: Register::ZERO,
            },
        ),
        (
            0x618c,
            Instruction::Addu {
                rd: Register::S0,
                rs: Register::A0,
                rt: Register::ZERO,
            },
        ),
        (
            0x66bc,
            Instruction::Jal {
                target: runtime_address(image, 0x7a74)?,
            },
        ),
        (
            0x66c0,
            Instruction::Addu {
                rd: Register::A0,
                rs: Register::S0,
                rt: Register::ZERO,
            },
        ),
        (
            0x7a7c,
            Instruction::Addu {
                rd: Register::S0,
                rs: Register::A0,
                rt: Register::ZERO,
            },
        ),
        (
            0x7e40,
            Instruction::Jal {
                target: runtime_address(image, 0x5da4)?,
            },
        ),
        (
            0x7e44,
            Instruction::Addu {
                rd: Register::A0,
                rs: Register::S0,
                rt: Register::ZERO,
            },
        ),
        (
            0x5dbc,
            Instruction::Addu {
                rd: Register::S0,
                rs: Register::A0,
                rt: Register::ZERO,
            },
        ),
    ];
    for (offset, instruction) in runtime_source_chain {
        ensure_instruction(
            image,
            offset,
            instruction,
            "KANRI runtime custom-index source chain",
        )?;
    }
    for (function_offset, caller_offset) in [(0x6184, 0x1854), (0x7a74, 0x66bc), (0x5da4, 0x7e40)] {
        ensure_direct_call_offsets(
            image,
            runtime_address(image, function_offset)?,
            &[caller_offset],
            "KANRI runtime custom-index source chain",
        )?;
        ensure_only_reachable_direct_entries(
            image,
            reachable_instruction_offsets,
            &[caller_offset],
            runtime_address(image, function_offset)?,
            "KANRI runtime custom-index source chain",
        )?;
        ensure!(
            reachable_instruction_offsets.contains(&caller_offset),
            "{} KANRI runtime custom-index source caller +0x{caller_offset:x} is not reachable",
            image.path
        );
    }
    ensure_register_is_preserved_between(
        image,
        0x165c,
        0x1858,
        Register::S1,
        "KANRI runtime custom-index root pointer",
    )?;
    ensure_register_is_preserved_between(
        image,
        0x6190,
        0x66c0,
        Register::S0,
        "KANRI runtime custom-index first forwarding pointer",
    )?;
    ensure_register_is_preserved_between(
        image,
        0x7a80,
        0x7e44,
        Register::S0,
        "KANRI runtime custom-index second forwarding pointer",
    )?;
    ensure_register_is_preserved_between(
        image,
        0x5dc0,
        0x5e9c,
        Register::S0,
        "KANRI runtime custom-index final source pointer",
    )
}

pub(super) fn kanri_custom_index_writer_evidence() -> ValidatedSelectorWriterEvidence {
    ValidatedSelectorWriterEvidence {
        classification: "forwarded_halfword_low_byte",
        value_resolution: "runtime_source",
        exact_value_candidates: vec![16],
        bounded_value_range: None,
        source_runtime_byte_ranges: vec![[0x801f_1a0e, 0x801f_1a0f]],
        evidence: "the complete caller chain supplies either the low byte of the halfword at 0x801f1a0e or literal 16; the callee preserves the argument in s2",
    }
}

fn validate_plsel5_incremented_state(
    image: &LoadedImage,
    result: &mut BTreeMap<usize, ValidatedSelectorWriterEvidence>,
) -> Result<()> {
    let expected = [
        Instruction::Lui {
            rt: Register::V1,
            immediate: 0x801f,
        },
        Instruction::Lbu {
            rt: Register::V1,
            base: Register::V1,
            offset: 0x5c39,
        },
        Instruction::Lui {
            rt: Register::V0,
            immediate: 0x801f,
        },
        Instruction::Lbu {
            rt: Register::V0,
            base: Register::V0,
            offset: 0x5c38,
        },
        Instruction::Addiu {
            rt: Register::V1,
            rs: Register::V1,
            immediate: 1,
        },
        Instruction::Lui {
            rt: Register::AT,
            immediate: 0x801f,
        },
        Instruction::Sb {
            rt: Register::V1,
            base: Register::AT,
            offset: 0x5c39,
        },
        Instruction::Lui {
            rt: Register::AT,
            immediate: 0x801f,
        },
        Instruction::Sb {
            rt: Register::V1,
            base: Register::AT,
            offset: 0x64be,
        },
    ];
    for (index, instruction) in expected.into_iter().enumerate() {
        ensure_instruction(
            image,
            0x3944 + index * 4,
            instruction,
            "PLSEL5 incremented selector state",
        )?;
    }
    insert(
        result,
        0x3964,
        runtime_source(
            "incremented_runtime_state",
            vec![[0x801f_5c39, 0x801f_5c3a]],
            "loads, increments, mirrors, and publishes one runtime state byte",
        ),
        &image.path,
    )
}

fn validate_rkdemo_writers(
    image: &LoadedImage,
    reachable_instruction_offsets: &BTreeSet<usize>,
    result: &mut BTreeMap<usize, ValidatedSelectorWriterEvidence>,
) -> Result<()> {
    validate_rkdemo_global_selector_sources(image, reachable_instruction_offsets)?;
    validate_direct_store(image, site(0x0188, 0x6494), Register::V1)?;
    validate_direct_store(image, site(0x0190, 0x6498), Register::A0)?;
    insert(
        result,
        0x0188,
        runtime_source(
            "global_runtime_byte",
            vec![[0x801f_6520, 0x801f_6521]],
            "the mode-zero entry path loads the first resource selector from a validated global byte",
        ),
        &image.path,
    )?;
    insert(
        result,
        0x0190,
        runtime_source(
            "global_runtime_byte",
            vec![[0x801f_6521, 0x801f_6522]],
            "the mode-zero entry path loads the second resource selector from a validated global byte",
        ),
        &image.path,
    )?;
    validate_rkdemo_bounded_random(image)?;
    insert(
        result,
        0x01f4,
        ValidatedSelectorWriterEvidence {
            classification: "bounded_random",
            value_resolution: "bounded_range",
            exact_value_candidates: Vec::new(),
            bounded_value_range: Some([1, 20]),
            source_runtime_byte_ranges: Vec::new(),
            evidence: "validated unsigned-16-bit modulo-20 arithmetic followed by plus one",
        },
        &image.path,
    )
}

fn validate_rkdemo_global_selector_sources(
    image: &LoadedImage,
    reachable_instruction_offsets: &BTreeSet<usize>,
) -> Result<()> {
    let block_offset = 0x0124;
    let branch_offset = 0x00f8;
    ensure!(
        reachable_instruction_offsets.contains(&branch_offset),
        "{} RKDEMO mode-zero selector branch is not reachable",
        image.path
    );
    ensure_instruction(
        image,
        branch_offset,
        Instruction::Beq {
            rs: Register::V1,
            rt: Register::ZERO,
            target: runtime_address(image, block_offset)?,
        },
        "RKDEMO mode-zero selector branch",
    )?;
    ensure_only_reachable_direct_entries(
        image,
        reachable_instruction_offsets,
        &[branch_offset],
        runtime_address(image, block_offset)?,
        "RKDEMO global-selector block",
    )?;
    let expected = [
        (
            0x0124,
            Instruction::Lui {
                rt: Register::V1,
                immediate: 0x801f,
            },
        ),
        (
            0x0128,
            Instruction::Lbu {
                rt: Register::V1,
                base: Register::V1,
                offset: 0x6520,
            },
        ),
        (
            0x012c,
            Instruction::Lui {
                rt: Register::A0,
                immediate: 0x801f,
            },
        ),
        (
            0x0130,
            Instruction::Lbu {
                rt: Register::A0,
                base: Register::A0,
                offset: 0x6521,
            },
        ),
    ];
    for (offset, instruction) in expected {
        ensure_instruction(image, offset, instruction, "RKDEMO global selector load")?;
    }
    ensure_register_is_preserved_between(
        image,
        0x012c,
        0x0188,
        Register::V1,
        "RKDEMO first global selector",
    )?;
    ensure_register_is_preserved_between(
        image,
        0x0134,
        0x0190,
        Register::A0,
        "RKDEMO second global selector",
    )
}

fn validate_rkdemo_bounded_random(image: &LoadedImage) -> Result<()> {
    let expected = [
        Instruction::Andi {
            rt: Register::V0,
            rs: Register::V0,
            immediate: u16::MAX,
        },
        Instruction::Lui {
            rt: Register::V1,
            immediate: 0xcccc,
        },
        Instruction::Ori {
            rt: Register::V1,
            rs: Register::V1,
            immediate: 0xcccd,
        },
        Instruction::Multu {
            rs: Register::V0,
            rt: Register::V1,
        },
    ];
    for (index, instruction) in expected.into_iter().enumerate() {
        ensure_instruction(
            image,
            0x01a4 + index * 4,
            instruction,
            "RKDEMO bounded-random prefix",
        )?;
    }
    let tail = [
        (0x01cc, Instruction::Mfhi { rd: Register::A2 }),
        (
            0x01d0,
            Instruction::Srl {
                rd: Register::A0,
                rt: Register::A2,
                shift: 4,
            },
        ),
        (
            0x01d4,
            Instruction::Sll {
                rd: Register::V1,
                rt: Register::A0,
                shift: 2,
            },
        ),
        (
            0x01d8,
            Instruction::Addu {
                rd: Register::V1,
                rs: Register::V1,
                rt: Register::A0,
            },
        ),
        (
            0x01dc,
            Instruction::Sll {
                rd: Register::V1,
                rt: Register::V1,
                shift: 2,
            },
        ),
        (
            0x01e0,
            Instruction::Subu {
                rd: Register::V0,
                rs: Register::V0,
                rt: Register::V1,
            },
        ),
        (
            0x01ec,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::V0,
                immediate: 1,
            },
        ),
    ];
    for (offset, instruction) in tail {
        ensure_instruction(
            image,
            offset,
            instruction,
            "RKDEMO bounded-random reduction",
        )?;
    }
    validate_direct_store(image, site(0x01f4, 0x6496), Register::V0)
}

fn exact_value(
    classification: &'static str,
    value: u8,
    evidence: &'static str,
) -> ValidatedSelectorWriterEvidence {
    ValidatedSelectorWriterEvidence {
        classification,
        value_resolution: "exact_candidates",
        exact_value_candidates: vec![value],
        bounded_value_range: None,
        source_runtime_byte_ranges: Vec::new(),
        evidence,
    }
}

fn runtime_source(
    classification: &'static str,
    source_runtime_byte_ranges: Vec<[u32; 2]>,
    evidence: &'static str,
) -> ValidatedSelectorWriterEvidence {
    ValidatedSelectorWriterEvidence {
        classification,
        value_resolution: "runtime_source",
        exact_value_candidates: Vec::new(),
        bounded_value_range: None,
        source_runtime_byte_ranges,
        evidence,
    }
}

fn insert(
    result: &mut BTreeMap<usize, ValidatedSelectorWriterEvidence>,
    store_offset: usize,
    evidence: ValidatedSelectorWriterEvidence,
    image_path: &str,
) -> Result<()> {
    ensure!(
        result.insert(store_offset, evidence).is_none(),
        "{image_path} repeats selector writer profile +0x{store_offset:x}"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use psx_r3000a::Assembler;

    use super::*;
    use crate::source_disc::LoadedImageEntrypoint;

    const RUNTIME_BASE: u32 = 0x800a_2000;

    const TEST_PROFILE: DirectLiteralWriterProfile = DirectLiteralWriterProfile {
        image_path: "DAT1/SYNTH.BIN",
        producer_offset: 0,
        store: StoreSite {
            offset: 8,
            destination_low: 0x6494,
        },
        register: Register::V0,
        value: 30,
    };

    #[test]
    fn direct_literal_writer_requires_a_preserved_value_and_typed_store() {
        let image = literal_writer_image(30);

        validate_direct_literal_writer(&image, &TEST_PROFILE).unwrap();
    }

    #[test]
    fn direct_literal_writer_rejects_a_changed_value() {
        let image = literal_writer_image(31);

        let error = validate_direct_literal_writer(&image, &TEST_PROFILE).unwrap_err();

        assert!(error.to_string().contains("literal producer changed"));
    }

    fn literal_writer_image(value: i16) -> LoadedImage {
        let instructions = [
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::ZERO,
                immediate: value,
            },
            Instruction::Lui {
                rt: Register::AT,
                immediate: 0x801f,
            },
            Instruction::Sb {
                rt: Register::V0,
                base: Register::AT,
                offset: 0x6494,
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
