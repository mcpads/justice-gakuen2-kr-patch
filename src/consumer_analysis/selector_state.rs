use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};
use psx_r3000a::decode;

use super::model::{StaticProfiledStateAccessAudit, StaticSelectorWriterEvidenceAudit};
use crate::psx_static_analysis::memory_access::decoded_memory_access;
use crate::psx_static_analysis::value_flow::{
    DerivedAddress, DerivedAddressKind, DerivedAddressScan,
};
use crate::source_disc::LoadedImage;

#[path = "selector_state/bounded_configuration_copy.rs"]
mod bounded_configuration_copy;
#[path = "selector_state/bounded_descending_fill.rs"]
mod bounded_descending_fill;
#[path = "selector_state/bounded_upstream_access.rs"]
mod bounded_upstream_access;
#[path = "selector_state/custom_record_exam_predecessor_writer_evidence.rs"]
mod custom_record_exam_predecessor_writer_evidence;
#[path = "selector_state/custom_record_index_limit_writer_evidence.rs"]
mod custom_record_index_limit_writer_evidence;
#[path = "selector_state/custom_record_index_zero_writer_evidence.rs"]
mod custom_record_index_zero_writer_evidence;
#[path = "selector_state/custom_record_source_page_index_writer_evidence.rs"]
mod custom_record_source_page_index_writer_evidence;
#[path = "selector_state/custom_record_source_writer_evidence.rs"]
mod custom_record_source_writer_evidence;
#[path = "selector_state/custom_record_writer_evidence.rs"]
mod custom_record_writer_evidence;
#[path = "selector_state/profile_validation.rs"]
mod profile_validation;
#[path = "selector_state/resource_selector_writer_evidence.rs"]
mod resource_selector_writer_evidence;
#[path = "selector_state/source_bound_zero_fill.rs"]
mod source_bound_zero_fill;
#[path = "selector_state/upstream_writer_evidence.rs"]
mod upstream_writer_evidence;
#[path = "selector_state/writer_evidence.rs"]
mod writer_evidence;

use bounded_configuration_copy::validated_bounded_configuration_copies;
use bounded_descending_fill::validated_bounded_descending_fill_addresses;
use bounded_upstream_access::validated_bounded_upstream_access_addresses;
use custom_record_exam_predecessor_writer_evidence::validated_custom_record_exam_predecessor_writer_evidence;
use custom_record_index_limit_writer_evidence::validated_custom_record_index_limit_writer_evidence;
use custom_record_index_zero_writer_evidence::validated_custom_record_index_zero_writer_evidence;
use custom_record_source_page_index_writer_evidence::validated_custom_record_source_page_index_writer_evidence;
use custom_record_source_writer_evidence::validated_custom_record_source_writer_evidence;
use custom_record_writer_evidence::validated_custom_record_writer_evidence;
use resource_selector_writer_evidence::validated_resource_selector_writer_evidence;
use upstream_writer_evidence::validated_upstream_writer_evidence;
use writer_evidence::{ValidatedSelectorWriterEvidence, validated_selector_writer_evidence};

#[derive(Clone, Copy)]
struct StateRangeProfile {
    id: &'static str,
    start_runtime_address: u32,
    end_runtime_address: u32,
}

const LOADER_SELECTOR_STATE_PROFILES: &[StateRangeProfile] = &[
    StateRangeProfile {
        id: "four_slot_resource_and_layout_bytes",
        start_runtime_address: 0x801f_6494,
        end_runtime_address: 0x801f_649c,
    },
    StateRangeProfile {
        id: "four_slot_mode_byte",
        start_runtime_address: 0x801f_64be,
        end_runtime_address: 0x801f_64bf,
    },
    StateRangeProfile {
        id: "four_slot_variant_bytes",
        start_runtime_address: 0x801f_64ed,
        end_runtime_address: 0x801f_64ef,
    },
    StateRangeProfile {
        id: "four_slot_custom_record_indices",
        start_runtime_address: 0x801f_64fc,
        end_runtime_address: 0x801f_6500,
    },
];

const LOADER_SELECTOR_UPSTREAM_STATE_PROFILES: &[StateRangeProfile] = &[
    StateRangeProfile {
        id: "main_forwarded_resource_selector",
        start_runtime_address: 0x801f_5c7f,
        end_runtime_address: 0x801f_5c80,
    },
    StateRangeProfile {
        id: "configuration_resource_and_layout_bytes",
        start_runtime_address: 0x801f_6702,
        end_runtime_address: 0x801f_6706,
    },
    StateRangeProfile {
        id: "configuration_variant_bytes",
        start_runtime_address: 0x801f_6706,
        end_runtime_address: 0x801f_670a,
    },
    StateRangeProfile {
        id: "configuration_custom_record_indices",
        start_runtime_address: 0x801f_6717,
        end_runtime_address: 0x801f_671b,
    },
    StateRangeProfile {
        id: "mode_round_limit_and_counter",
        start_runtime_address: 0x801f_5c38,
        end_runtime_address: 0x801f_5c3a,
    },
    StateRangeProfile {
        id: "plsel5_table_group",
        start_runtime_address: 0x801f_5c90,
        end_runtime_address: 0x801f_5c91,
    },
    StateRangeProfile {
        id: "rkdemo_resource_selector_pair",
        start_runtime_address: 0x801f_6520,
        end_runtime_address: 0x801f_6522,
    },
    StateRangeProfile {
        id: "custom_record_primary_fallback_source",
        start_runtime_address: 0x801f_1854,
        end_runtime_address: 0x801f_1855,
    },
    StateRangeProfile {
        id: "custom_record_primary_fallback_global_source",
        start_runtime_address: 0x801f_1bc7,
        end_runtime_address: 0x801f_1bc8,
    },
    StateRangeProfile {
        id: "custom_record_primary_fallback_global_predecessor",
        start_runtime_address: 0x801f_1bc3,
        end_runtime_address: 0x801f_1bc4,
    },
    StateRangeProfile {
        id: "custom_record_index_zero_runtime_source_low_byte",
        start_runtime_address: 0x801f_1a0e,
        end_runtime_address: 0x801f_1a0f,
    },
    StateRangeProfile {
        id: "custom_record_index_zero_exam_source",
        start_runtime_address: 0x801f_180a,
        end_runtime_address: 0x801f_180b,
    },
    StateRangeProfile {
        id: "custom_record_source_page_index",
        start_runtime_address: 0x801f_1a04,
        end_runtime_address: 0x801f_1a05,
    },
    StateRangeProfile {
        id: "custom_record_index_zero_limit_word",
        start_runtime_address: 0x801f_1a1c,
        end_runtime_address: 0x801f_1a20,
    },
    StateRangeProfile {
        id: "custom_record_index_zero_exam_predecessor",
        start_runtime_address: 0x801f_6302,
        end_runtime_address: 0x801f_6303,
    },
    StateRangeProfile {
        id: "custom_record_primary_table_index_source",
        start_runtime_address: 0x801f_185a,
        end_runtime_address: 0x801f_185b,
    },
    StateRangeProfile {
        id: "custom_record_primary_special_source",
        start_runtime_address: 0x801f_185f,
        end_runtime_address: 0x801f_1860,
    },
    StateRangeProfile {
        id: "custom_record_secondary_near_source",
        start_runtime_address: 0x801f_18dd,
        end_runtime_address: 0x801f_18de,
    },
    StateRangeProfile {
        id: "custom_record_zero_primary_special",
        start_runtime_address: 0x801f_5804,
        end_runtime_address: 0x801f_5805,
    },
    StateRangeProfile {
        id: "custom_record_zero_secondary_near",
        start_runtime_address: 0x801f_5805,
        end_runtime_address: 0x801f_5806,
    },
    StateRangeProfile {
        id: "custom_record_zero_primary_fallback",
        start_runtime_address: 0x801f_5807,
        end_runtime_address: 0x801f_5808,
    },
    StateRangeProfile {
        id: "custom_record_zero_secondary_far",
        start_runtime_address: 0x801f_5813,
        end_runtime_address: 0x801f_5814,
    },
    StateRangeProfile {
        id: "custom_record_one_primary_special",
        start_runtime_address: 0x801f_582c,
        end_runtime_address: 0x801f_582d,
    },
    StateRangeProfile {
        id: "custom_record_one_secondary_near",
        start_runtime_address: 0x801f_582d,
        end_runtime_address: 0x801f_582e,
    },
    StateRangeProfile {
        id: "custom_record_one_primary_fallback",
        start_runtime_address: 0x801f_582f,
        end_runtime_address: 0x801f_5830,
    },
    StateRangeProfile {
        id: "custom_record_two_primary_special",
        start_runtime_address: 0x801f_5854,
        end_runtime_address: 0x801f_5855,
    },
    StateRangeProfile {
        id: "custom_record_two_secondary_near",
        start_runtime_address: 0x801f_5855,
        end_runtime_address: 0x801f_5856,
    },
    StateRangeProfile {
        id: "custom_record_two_primary_fallback",
        start_runtime_address: 0x801f_5857,
        end_runtime_address: 0x801f_5858,
    },
    StateRangeProfile {
        id: "custom_record_three_primary_special",
        start_runtime_address: 0x801f_587c,
        end_runtime_address: 0x801f_587d,
    },
    StateRangeProfile {
        id: "custom_record_three_secondary_near",
        start_runtime_address: 0x801f_587d,
        end_runtime_address: 0x801f_587e,
    },
    StateRangeProfile {
        id: "custom_record_three_primary_fallback",
        start_runtime_address: 0x801f_587f,
        end_runtime_address: 0x801f_5880,
    },
    StateRangeProfile {
        id: "custom_record_four_primary_special",
        start_runtime_address: 0x801f_58a4,
        end_runtime_address: 0x801f_58a5,
    },
    StateRangeProfile {
        id: "custom_record_four_secondary_near",
        start_runtime_address: 0x801f_58a5,
        end_runtime_address: 0x801f_58a6,
    },
    StateRangeProfile {
        id: "custom_record_four_primary_fallback",
        start_runtime_address: 0x801f_58a7,
        end_runtime_address: 0x801f_58a8,
    },
    StateRangeProfile {
        id: "custom_record_five_primary_special",
        start_runtime_address: 0x801f_58cc,
        end_runtime_address: 0x801f_58cd,
    },
    StateRangeProfile {
        id: "custom_record_five_secondary_near",
        start_runtime_address: 0x801f_58cd,
        end_runtime_address: 0x801f_58ce,
    },
    StateRangeProfile {
        id: "custom_record_five_primary_fallback",
        start_runtime_address: 0x801f_58cf,
        end_runtime_address: 0x801f_58d0,
    },
    StateRangeProfile {
        id: "custom_record_six_primary_special",
        start_runtime_address: 0x801f_58f4,
        end_runtime_address: 0x801f_58f5,
    },
    StateRangeProfile {
        id: "custom_record_six_secondary_near",
        start_runtime_address: 0x801f_58f5,
        end_runtime_address: 0x801f_58f6,
    },
    StateRangeProfile {
        id: "custom_record_six_primary_fallback",
        start_runtime_address: 0x801f_58f7,
        end_runtime_address: 0x801f_58f8,
    },
    StateRangeProfile {
        id: "custom_record_seven_primary_special",
        start_runtime_address: 0x801f_591c,
        end_runtime_address: 0x801f_591d,
    },
    StateRangeProfile {
        id: "custom_record_seven_secondary_near",
        start_runtime_address: 0x801f_591d,
        end_runtime_address: 0x801f_591e,
    },
    StateRangeProfile {
        id: "custom_record_seven_primary_fallback",
        start_runtime_address: 0x801f_591f,
        end_runtime_address: 0x801f_5920,
    },
    StateRangeProfile {
        id: "custom_record_eight_primary_special",
        start_runtime_address: 0x801f_5944,
        end_runtime_address: 0x801f_5945,
    },
    StateRangeProfile {
        id: "custom_record_eight_secondary_near",
        start_runtime_address: 0x801f_5945,
        end_runtime_address: 0x801f_5946,
    },
    StateRangeProfile {
        id: "custom_record_eight_primary_fallback",
        start_runtime_address: 0x801f_5947,
        end_runtime_address: 0x801f_5948,
    },
    StateRangeProfile {
        id: "custom_record_nine_primary_special",
        start_runtime_address: 0x801f_596c,
        end_runtime_address: 0x801f_596d,
    },
    StateRangeProfile {
        id: "custom_record_nine_secondary_near",
        start_runtime_address: 0x801f_596d,
        end_runtime_address: 0x801f_596e,
    },
    StateRangeProfile {
        id: "custom_record_nine_primary_fallback",
        start_runtime_address: 0x801f_596f,
        end_runtime_address: 0x801f_5970,
    },
    StateRangeProfile {
        id: "custom_record_ten_primary_special",
        start_runtime_address: 0x801f_5994,
        end_runtime_address: 0x801f_5995,
    },
    StateRangeProfile {
        id: "custom_record_ten_secondary_near",
        start_runtime_address: 0x801f_5995,
        end_runtime_address: 0x801f_5996,
    },
    StateRangeProfile {
        id: "custom_record_ten_primary_fallback",
        start_runtime_address: 0x801f_5997,
        end_runtime_address: 0x801f_5998,
    },
    StateRangeProfile {
        id: "custom_record_eleven_primary_special",
        start_runtime_address: 0x801f_59bc,
        end_runtime_address: 0x801f_59bd,
    },
    StateRangeProfile {
        id: "custom_record_eleven_secondary_near",
        start_runtime_address: 0x801f_59bd,
        end_runtime_address: 0x801f_59be,
    },
    StateRangeProfile {
        id: "custom_record_eleven_primary_fallback",
        start_runtime_address: 0x801f_59bf,
        end_runtime_address: 0x801f_59c0,
    },
    StateRangeProfile {
        id: "custom_record_twelve_primary_special",
        start_runtime_address: 0x801f_59e4,
        end_runtime_address: 0x801f_59e5,
    },
    StateRangeProfile {
        id: "custom_record_twelve_secondary_near",
        start_runtime_address: 0x801f_59e5,
        end_runtime_address: 0x801f_59e6,
    },
    StateRangeProfile {
        id: "custom_record_twelve_primary_fallback",
        start_runtime_address: 0x801f_59e7,
        end_runtime_address: 0x801f_59e8,
    },
    StateRangeProfile {
        id: "custom_record_thirteen_primary_special",
        start_runtime_address: 0x801f_5a0c,
        end_runtime_address: 0x801f_5a0d,
    },
    StateRangeProfile {
        id: "custom_record_thirteen_secondary_near",
        start_runtime_address: 0x801f_5a0d,
        end_runtime_address: 0x801f_5a0e,
    },
    StateRangeProfile {
        id: "custom_record_thirteen_primary_fallback",
        start_runtime_address: 0x801f_5a0f,
        end_runtime_address: 0x801f_5a10,
    },
    StateRangeProfile {
        id: "custom_record_fourteen_primary_special",
        start_runtime_address: 0x801f_5a34,
        end_runtime_address: 0x801f_5a35,
    },
    StateRangeProfile {
        id: "custom_record_fourteen_secondary_near",
        start_runtime_address: 0x801f_5a35,
        end_runtime_address: 0x801f_5a36,
    },
    StateRangeProfile {
        id: "custom_record_fourteen_primary_fallback",
        start_runtime_address: 0x801f_5a37,
        end_runtime_address: 0x801f_5a38,
    },
    StateRangeProfile {
        id: "custom_record_fifteen_primary_special",
        start_runtime_address: 0x801f_5a5c,
        end_runtime_address: 0x801f_5a5d,
    },
    StateRangeProfile {
        id: "custom_record_fifteen_secondary_near",
        start_runtime_address: 0x801f_5a5d,
        end_runtime_address: 0x801f_5a5e,
    },
    StateRangeProfile {
        id: "custom_record_fifteen_primary_fallback",
        start_runtime_address: 0x801f_5a5f,
        end_runtime_address: 0x801f_5a60,
    },
    StateRangeProfile {
        id: "custom_record_sixteen_primary_special",
        start_runtime_address: 0x801f_5a84,
        end_runtime_address: 0x801f_5a85,
    },
    StateRangeProfile {
        id: "custom_record_sixteen_secondary_near",
        start_runtime_address: 0x801f_5a85,
        end_runtime_address: 0x801f_5a86,
    },
    StateRangeProfile {
        id: "custom_record_sixteen_primary_fallback",
        start_runtime_address: 0x801f_5a87,
        end_runtime_address: 0x801f_5a88,
    },
];

pub(super) fn loader_selector_state_accesses(
    image: &LoadedImage,
    flow: &DerivedAddressScan,
    reachable_instruction_offsets: &BTreeSet<usize>,
) -> Result<Vec<StaticProfiledStateAccessAudit>> {
    let bounded_fill_addresses =
        validated_bounded_descending_fill_addresses(image, reachable_instruction_offsets)?;
    let bounded_copy_evidence =
        validated_bounded_configuration_copies(image, reachable_instruction_offsets)?;
    let writer_evidence = validated_selector_writer_evidence(
        image,
        reachable_instruction_offsets,
        &bounded_copy_evidence,
    )?;

    profiled_state_accesses(
        image,
        flow,
        LOADER_SELECTOR_STATE_PROFILES,
        |reference| {
            !bounded_fill_addresses
                .get(&reference.instruction_offset)
                .is_some_and(|addresses| !addresses.contains(&reference.address))
                && !bounded_copy_evidence
                    .get(&reference.instruction_offset)
                    .is_some_and(|evidence| {
                        !evidence.destination_addresses.contains(&reference.address)
                    })
        },
        &writer_evidence,
    )
}

pub(super) fn loader_selector_upstream_state_accesses(
    image: &LoadedImage,
    flow: &DerivedAddressScan,
    reachable_instruction_offsets: &BTreeSet<usize>,
) -> Result<Vec<StaticProfiledStateAccessAudit>> {
    let bounded_fill_addresses =
        validated_bounded_descending_fill_addresses(image, reachable_instruction_offsets)?;
    let bounded_copy_evidence =
        validated_bounded_configuration_copies(image, reachable_instruction_offsets)?;
    let bounded_copy_source_addresses = bounded_copy_evidence
        .values()
        .map(|evidence| {
            (
                evidence.source_instruction_offset,
                &evidence.source_addresses,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let bounded_upstream_addresses =
        validated_bounded_upstream_access_addresses(image, reachable_instruction_offsets)?;
    let mut writer_evidence =
        validated_upstream_writer_evidence(image, reachable_instruction_offsets)?;
    for (offset, evidence) in
        validated_custom_record_index_zero_writer_evidence(image, reachable_instruction_offsets)?
    {
        ensure!(
            writer_evidence.insert(offset, evidence).is_none(),
            "{} repeats custom-record index-zero writer evidence at +0x{offset:x}",
            image.path
        );
    }
    for (offset, evidence) in
        validated_custom_record_index_limit_writer_evidence(image, reachable_instruction_offsets)?
    {
        ensure!(
            writer_evidence.insert(offset, evidence).is_none(),
            "{} repeats custom-record index-limit writer evidence at +0x{offset:x}",
            image.path
        );
    }
    for (offset, evidence) in validated_custom_record_exam_predecessor_writer_evidence(
        image,
        reachable_instruction_offsets,
    )? {
        ensure!(
            writer_evidence.insert(offset, evidence).is_none(),
            "{} repeats custom-record exam-predecessor writer evidence at +0x{offset:x}",
            image.path
        );
    }
    for (offset, evidence) in validated_custom_record_source_page_index_writer_evidence(image)? {
        ensure!(
            writer_evidence.insert(offset, evidence).is_none(),
            "{} repeats custom-record source-page index writer evidence at +0x{offset:x}",
            image.path
        );
    }
    for (offset, evidence) in validated_resource_selector_writer_evidence(image)? {
        ensure!(
            writer_evidence.insert(offset, evidence).is_none(),
            "{} repeats selector upstream-writer evidence at +0x{offset:x}",
            image.path
        );
    }
    for (offset, evidence) in
        validated_custom_record_writer_evidence(image, reachable_instruction_offsets)?
    {
        ensure!(
            writer_evidence.insert(offset, evidence).is_none(),
            "{} repeats custom-record upstream-writer evidence at +0x{offset:x}",
            image.path
        );
    }
    for (offset, evidence) in validated_custom_record_source_writer_evidence(image)? {
        ensure!(
            writer_evidence.insert(offset, evidence).is_none(),
            "{} repeats custom-record source writer evidence at +0x{offset:x}",
            image.path
        );
    }
    profiled_state_accesses(
        image,
        flow,
        LOADER_SELECTOR_UPSTREAM_STATE_PROFILES,
        |reference| {
            !bounded_fill_addresses
                .get(&reference.instruction_offset)
                .is_some_and(|addresses| !addresses.contains(&reference.address))
                && !bounded_copy_evidence
                    .get(&reference.instruction_offset)
                    .is_some_and(|evidence| {
                        !evidence.destination_addresses.contains(&reference.address)
                    })
                && !bounded_copy_source_addresses
                    .get(&reference.instruction_offset)
                    .is_some_and(|addresses| !addresses.contains(&reference.address))
                && !bounded_upstream_addresses
                    .get(&reference.instruction_offset)
                    .is_some_and(|addresses| !addresses.contains(&reference.address))
        },
        &writer_evidence,
    )
}

fn profiled_state_accesses(
    image: &LoadedImage,
    flow: &DerivedAddressScan,
    profiles: &[StateRangeProfile],
    include_reference: impl Fn(&DerivedAddress) -> bool,
    writer_evidence: &BTreeMap<usize, ValidatedSelectorWriterEvidence>,
) -> Result<Vec<StaticProfiledStateAccessAudit>> {
    let runtime_base = image
        .runtime_base
        .context("profiled state source lacks a runtime base")?;
    let mut grouped = BTreeMap::<
        (usize, usize, &'static str, usize),
        (BTreeSet<usize>, BTreeSet<u32>, String),
    >::new();
    for reference in &flow.addresses {
        if reference.kind != DerivedAddressKind::MemoryAccess {
            continue;
        }
        if !include_reference(reference) {
            continue;
        }
        let instruction_end = reference
            .instruction_offset
            .checked_add(4)
            .context("loader selector-state instruction end overflow")?;
        let bytes = image
            .data
            .get(reference.instruction_offset..instruction_end)
            .context("loader selector-state instruction is truncated")?;
        let pc = runtime_base
            .checked_add(u32::try_from(reference.instruction_offset)?)
            .context("loader selector-state instruction address overflow")?;
        let instruction = decode(u32::from_le_bytes(bytes.try_into()?), pc).with_context(|| {
            format!("failed to decode loader selector-state access at {pc:#010x}")
        })?;
        let Some(access) = decoded_memory_access(&instruction) else {
            continue;
        };
        let access_end = reference
            .address
            .checked_add(u32::try_from(access.width_bytes)?)
            .context("loader selector-state access end overflow")?;
        for (profile_index, profile) in profiles.iter().enumerate() {
            if reference.address >= profile.end_runtime_address
                || access_end <= profile.start_runtime_address
            {
                continue;
            }
            let (seed_offsets, access_addresses, _) = grouped
                .entry((
                    profile_index,
                    reference.instruction_offset,
                    access.operation.as_str(),
                    access.width_bytes,
                ))
                .or_insert_with(|| (BTreeSet::new(), BTreeSet::new(), format!("{instruction:?}")));
            seed_offsets.insert(reference.seed_offset);
            access_addresses.insert(reference.address);
        }
    }

    Ok(grouped
        .into_iter()
        .map(
            |(
                (profile_index, instruction_offset, operation, width_bytes),
                (seed_offsets, access_addresses, decoded_instruction),
            )| {
                let profile = &profiles[profile_index];
                StaticProfiledStateAccessAudit {
                    profile_id: profile.id.to_string(),
                    profile_runtime_byte_range: [
                        format!("0x{:08x}", profile.start_runtime_address),
                        format!("0x{:08x}", profile.end_runtime_address),
                    ],
                    seed_instruction_offsets: seed_offsets
                        .into_iter()
                        .map(|offset| format!("0x{offset:x}"))
                        .collect(),
                    instruction_offset: format!("0x{instruction_offset:x}"),
                    instruction_runtime_address: format!(
                        "0x{:08x}",
                        runtime_base + instruction_offset as u32
                    ),
                    decoded_instruction,
                    access_runtime_addresses: access_addresses
                        .into_iter()
                        .map(|address| format!("0x{address:08x}"))
                        .collect(),
                    operation: operation.to_string(),
                    width_bytes,
                    entrypoint_reachable: true,
                    writer_evidence: writer_evidence.get(&instruction_offset).map(|evidence| {
                        StaticSelectorWriterEvidenceAudit {
                            classification: evidence.classification.to_string(),
                            value_resolution: evidence.value_resolution.to_string(),
                            exact_value_candidates: evidence.exact_value_candidates.clone(),
                            bounded_value_range: evidence.bounded_value_range,
                            source_runtime_byte_ranges: evidence
                                .source_runtime_byte_ranges
                                .iter()
                                .map(|range| {
                                    [format!("0x{:08x}", range[0]), format!("0x{:08x}", range[1])]
                                })
                                .collect(),
                            evidence: evidence.evidence.to_string(),
                        }
                    }),
                }
            },
        )
        .collect())
}

#[cfg(test)]
mod tests {
    use psx_r3000a::{Assembler, Instruction, Register};

    use super::*;
    use crate::consumer_analysis::analyze_loaded_program;
    use crate::source_disc::LoadedImageEntrypoint;

    const RUNTIME_BASE: u32 = 0x800a_2000;

    #[test]
    fn separates_reachable_selector_state_reads_and_writes() {
        let image = loaded_image(&[
            Instruction::Lui {
                rt: Register::T0,
                immediate: 0x801f,
            },
            Instruction::Ori {
                rt: Register::T0,
                rs: Register::T0,
                immediate: 0x6494,
            },
            Instruction::Lbu {
                rt: Register::V0,
                base: Register::T0,
                offset: 0,
            },
            Instruction::Sb {
                rt: Register::V0,
                base: Register::T0,
                offset: 1,
            },
            Instruction::Jr { rs: Register::RA },
            Instruction::nop(),
        ]);
        let analysis = analyze_loaded_program(&image, 64).unwrap().unwrap();

        let accesses = loader_selector_state_accesses(
            &image,
            &analysis.value_flow,
            &analysis.closure.scan.instruction_offsets,
        )
        .unwrap();

        assert_eq!(accesses.len(), 2);
        assert_eq!(accesses[0].operation, "read");
        assert_eq!(accesses[0].access_runtime_addresses, ["0x801f6494"]);
        assert_eq!(accesses[1].operation, "write");
        assert_eq!(accesses[1].access_runtime_addresses, ["0x801f6495"]);
        assert!(accesses.iter().all(|access| access.entrypoint_reachable));
    }

    #[test]
    fn excludes_selector_state_accesses_after_the_entrypoint_returns() {
        let image = loaded_image(&[
            Instruction::Jr { rs: Register::RA },
            Instruction::nop(),
            Instruction::Lui {
                rt: Register::T0,
                immediate: 0x801f,
            },
            Instruction::Lbu {
                rt: Register::V0,
                base: Register::T0,
                offset: 0x6494,
            },
        ]);
        let analysis = analyze_loaded_program(&image, 64).unwrap().unwrap();

        let accesses = loader_selector_state_accesses(
            &image,
            &analysis.value_flow,
            &analysis.closure.scan.instruction_offsets,
        )
        .unwrap();

        assert!(accesses.is_empty());
    }

    fn loaded_image(instructions: &[Instruction]) -> LoadedImage {
        let mut assembler = Assembler::new();
        for instruction in instructions {
            assembler.emit(instruction.clone());
        }
        let program = assembler.assemble(RUNTIME_BASE).unwrap();
        LoadedImage {
            path: "DAT1/SYNTH.BIN".to_string(),
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
