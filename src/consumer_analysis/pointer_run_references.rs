//! Investigation-only census for explicitly declared pointer runs.

use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};
use psx_r3000a::{Instruction, decode};

use super::model::{
    StaticDerivedAddressReferenceAudit, StaticPointerRunAudit, StaticPointerTargetAudit,
    StaticRawAddressReferenceAudit, StaticValueFlowSeedExhaustionAudit,
};
use super::profiles::{StaticPointerRunNonAddressFlowProfile, StaticPointerRunProfile};
use crate::psx_static_analysis::value_flow::{
    DerivedAddress, DerivedAddressKind, DerivedAddressScan,
};
use crate::source_disc::LoadedImage;

pub(super) fn validate_pointer_run_profiles(
    profiles: &[StaticPointerRunProfile],
    non_address_flow_profiles: &[StaticPointerRunNonAddressFlowProfile],
) -> Result<()> {
    let mut ids = BTreeSet::new();
    let mut physical_runs = BTreeSet::new();
    for profile in profiles {
        ensure!(
            !profile.id.trim().is_empty() && ids.insert(profile.id),
            "duplicate or empty static pointer-run profile {}",
            profile.id
        );
        ensure!(
            !profile.image_path.trim().is_empty()
                && profile.pointer_count > 0
                && profile.target_arena_size > 0,
            "static pointer-run profile {} has an empty path or range",
            profile.id
        );
        ensure!(
            physical_runs.insert((
                profile.image_path,
                profile.pointer_table_runtime_address,
                profile.target_arena_runtime_address,
            )),
            "static pointer-run profile {} repeats a physical run",
            profile.id
        );
        let pointer_table_size = pointer_table_size(profile)?;
        let pointer_table_end = profile
            .pointer_table_runtime_address
            .checked_add(pointer_table_size)
            .context("static pointer table range overflow")?;
        let target_arena_end = profile
            .target_arena_runtime_address
            .checked_add(u32::try_from(profile.target_arena_size)?)
            .context("static pointer target arena range overflow")?;
        ensure!(
            pointer_table_end <= profile.target_arena_runtime_address
                || target_arena_end <= profile.pointer_table_runtime_address,
            "static pointer-run profile {} overlaps its table and target arena",
            profile.id
        );
    }
    let pointer_runs_by_id = profiles
        .iter()
        .map(|profile| (profile.id, profile))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut non_address_ids = BTreeSet::new();
    let mut non_address_seeds = BTreeSet::new();
    for profile in non_address_flow_profiles {
        ensure!(
            !profile.id.trim().is_empty() && non_address_ids.insert(profile.id),
            "duplicate or empty static pointer-run non-address flow profile {}",
            profile.id
        );
        let pointer_run = pointer_runs_by_id
            .get(profile.pointer_run_id)
            .with_context(|| {
                format!(
                    "static pointer-run non-address flow profile {} names an unknown run {}",
                    profile.id, profile.pointer_run_id
                )
            })?;
        ensure!(
            profile.image_path == pointer_run.image_path,
            "static pointer-run non-address flow profile {} image does not match its run",
            profile.id
        );
        ensure!(
            non_address_seeds.insert((profile.pointer_run_id, profile.seed_instruction_offset)),
            "static pointer-run non-address flow profile {} repeats a run seed",
            profile.id
        );
        ensure!(
            profile.analysis_state_budget > 0
                && profile.scalar_load_instruction_offset > profile.seed_instruction_offset,
            "static pointer-run non-address flow profile {} has an invalid budget or load order",
            profile.id
        );
        ensure!(
            !profile.expected_frontier_instructions.is_empty(),
            "static pointer-run non-address flow profile {} has no frontier instructions",
            profile.id
        );
        ensure!(
            profile
                .expected_frontier_instructions
                .iter()
                .map(|(offset, _)| offset)
                .collect::<BTreeSet<_>>()
                .len()
                == profile.expected_frontier_instructions.len(),
            "static pointer-run non-address flow profile {} repeats a frontier offset",
            profile.id
        );
    }
    Ok(())
}

pub(super) fn audit_declared_pointer_runs(
    image: &LoadedImage,
    value_flow: Option<&DerivedAddressScan>,
    reachable_analysis_state_budget: usize,
    reachable_analysis_unresolved_indirect_jump_count: usize,
    profiles: &[StaticPointerRunProfile],
    non_address_flow_profiles: &[StaticPointerRunNonAddressFlowProfile],
) -> Result<Vec<StaticPointerRunAudit>> {
    profiles
        .iter()
        .filter(|profile| profile.image_path == image.path)
        .map(|profile| {
            audit_pointer_run(
                image,
                value_flow,
                reachable_analysis_state_budget,
                reachable_analysis_unresolved_indirect_jump_count,
                profile,
                non_address_flow_profiles,
            )
        })
        .collect()
}

fn audit_pointer_run(
    image: &LoadedImage,
    value_flow: Option<&DerivedAddressScan>,
    reachable_analysis_state_budget: usize,
    reachable_analysis_unresolved_indirect_jump_count: usize,
    profile: &StaticPointerRunProfile,
    non_address_flow_profiles: &[StaticPointerRunNonAddressFlowProfile],
) -> Result<StaticPointerRunAudit> {
    let runtime_base = image
        .runtime_base
        .with_context(|| format!("pointer-run image {} lacks a runtime base", image.path))?;
    let pointer_table_size = pointer_table_size(profile)?;
    let pointer_table_end = checked_address_end(
        profile.pointer_table_runtime_address,
        pointer_table_size,
        "pointer table",
    )?;
    let target_arena_end = checked_address_end(
        profile.target_arena_runtime_address,
        u32::try_from(profile.target_arena_size)?,
        "pointer target arena",
    )?;
    let pointer_table_offset = loaded_image_offset(
        image,
        runtime_base,
        profile.pointer_table_runtime_address,
        usize::try_from(pointer_table_size)?,
        "pointer table",
    )?;
    let _target_arena_offset = loaded_image_offset(
        image,
        runtime_base,
        profile.target_arena_runtime_address,
        profile.target_arena_size,
        "pointer target arena",
    )?;

    let decoded_targets = (0..profile.pointer_count)
        .map(|index| {
            decode_pointer_target(
                image,
                runtime_base,
                pointer_table_offset,
                profile,
                target_arena_end,
                index,
            )
        })
        .collect::<Result<Vec<_>>>()?;
    let distinct_target_count = decoded_targets
        .iter()
        .map(|target| target.target_runtime_address.as_str())
        .collect::<BTreeSet<_>>()
        .len();
    let raw_address_references = raw_address_references(
        image,
        runtime_base,
        profile.pointer_table_runtime_address,
        pointer_table_end,
        profile.target_arena_runtime_address,
        target_arena_end,
    );
    let external_raw_address_reference_count = raw_address_references
        .iter()
        .filter(|reference| reference.source_region == "other_loaded_image_word")
        .count();
    let reachable_derived_references = match value_flow {
        Some(value_flow) => derived_address_references(
            image,
            runtime_base,
            value_flow,
            profile.pointer_table_runtime_address,
            pointer_table_end,
            profile.target_arena_runtime_address,
            target_arena_end,
        )?,
        None => Vec::new(),
    };
    let reachable_pointer_table_reference_count = reachable_derived_references
        .iter()
        .filter(|reference| {
            reference.derived_target_region == "pointer_table"
                || reference.loaded_value_storage_region.as_deref() == Some("pointer_table")
        })
        .count();
    let reachable_target_arena_reference_count = reachable_derived_references
        .iter()
        .filter(|reference| {
            reference.derived_target_region == "target_arena"
                || reference.loaded_value_storage_region.as_deref() == Some("target_arena")
        })
        .count();
    let reachable_derived_instruction_site_count = reachable_derived_references
        .iter()
        .map(|reference| {
            (
                reference.seed_instruction_runtime_address.as_str(),
                reference.instruction_runtime_address.as_str(),
            )
        })
        .collect::<BTreeSet<_>>()
        .len();
    let reachable_analysis_budget_exhausted_seed_count = value_flow
        .map(|flow| flow.budget_exhausted_seed_count)
        .unwrap_or(0);
    let reachable_analysis_profiled_non_address_seed_ids = value_flow
        .map(|flow| {
            validate_non_address_exhausted_seeds(
                image,
                flow,
                reachable_analysis_state_budget,
                profile,
                non_address_flow_profiles,
            )
        })
        .transpose()?
        .unwrap_or_default();
    ensure!(
        reachable_derived_references.is_empty()
            || reachable_analysis_profiled_non_address_seed_ids.is_empty(),
        "{} reviewed non-address flow now reaches the declared pointer run",
        profile.id
    );
    let reachable_analysis_profiled_non_address_exhausted_seed_count =
        reachable_analysis_profiled_non_address_seed_ids.len();
    let reachable_analysis_unprofiled_exhausted_seed_count =
        reachable_analysis_budget_exhausted_seed_count
            .checked_sub(reachable_analysis_profiled_non_address_exhausted_seed_count)
            .context("profiled non-address exhaustion count exceeds all exhausted seeds")?;
    let declared_entrypoint_flow_limits_remain = value_flow.is_none()
        || reachable_analysis_unprofiled_exhausted_seed_count > 0
        || reachable_analysis_unresolved_indirect_jump_count > 0;
    let reachable_analysis_budget_exhaustions = value_flow
        .into_iter()
        .flat_map(|flow| &flow.budget_exhausted_seeds)
        .map(|exhaustion| {
            Ok(StaticValueFlowSeedExhaustionAudit {
                seed_instruction_offset: hex_offset(exhaustion.seed_offset),
                seed_instruction_runtime_address: hex_address(
                    runtime_base
                        .checked_add(u32::try_from(exhaustion.seed_offset)?)
                        .context("value-flow exhaustion seed address overflow")?,
                ),
                processed_state_count: exhaustion.processed_state_count,
                discovered_state_count: exhaustion.discovered_state_count,
                distinct_instruction_offset_count: exhaustion.distinct_instruction_offset_count,
                maximum_states_at_instruction_offset: exhaustion
                    .maximum_states_at_instruction_offset,
                pending_state_count: exhaustion.pending_state_count,
                distinct_frontier_instruction_offset_count: exhaustion
                    .distinct_frontier_instruction_offset_count,
                frontier_instruction_offsets: exhaustion
                    .frontier_instruction_offsets
                    .iter()
                    .copied()
                    .map(hex_offset)
                    .collect(),
                frontier_instruction_runtime_addresses: exhaustion
                    .frontier_instruction_offsets
                    .iter()
                    .copied()
                    .map(|offset| {
                        runtime_base
                            .checked_add(u32::try_from(offset)?)
                            .map(hex_address)
                            .context("value-flow frontier address overflow")
                    })
                    .collect::<Result<Vec<_>>>()?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let reference_assessment = if !reachable_derived_references.is_empty() {
        "reachable_derived_reference_observed"
    } else if declared_entrypoint_flow_limits_remain {
        "no_reachable_derived_reference_observed_with_open_declared_entrypoint_flow"
    } else {
        "no_reachable_pointer_run_reference_in_reviewed_declared_entrypoint_flow"
    };

    Ok(StaticPointerRunAudit {
        id: profile.id.to_string(),
        pointer_table_runtime_byte_range: [
            hex_address(profile.pointer_table_runtime_address),
            hex_address(pointer_table_end),
        ],
        pointer_count: profile.pointer_count,
        target_arena_runtime_byte_range: [
            hex_address(profile.target_arena_runtime_address),
            hex_address(target_arena_end),
        ],
        target_arena_size: profile.target_arena_size,
        distinct_target_count,
        decoded_targets,
        raw_address_references,
        external_raw_address_reference_count,
        reachable_derived_references,
        reachable_pointer_table_reference_count,
        reachable_target_arena_reference_count,
        reachable_derived_instruction_site_count,
        reachable_analysis_state_budget,
        reachable_analysis_budget_exhausted_seed_count,
        reachable_analysis_budget_exhaustions,
        reachable_analysis_profiled_non_address_exhausted_seed_count,
        reachable_analysis_profiled_non_address_seed_ids,
        reachable_analysis_unprofiled_exhausted_seed_count,
        reachable_analysis_unresolved_indirect_jump_count,
        declared_entrypoint_flow_limits_remain,
        reference_assessment: reference_assessment.to_string(),
    })
}

fn validate_non_address_exhausted_seeds(
    image: &LoadedImage,
    value_flow: &DerivedAddressScan,
    analysis_state_budget: usize,
    pointer_run: &StaticPointerRunProfile,
    profiles: &[StaticPointerRunNonAddressFlowProfile],
) -> Result<Vec<String>> {
    let matching_profiles = profiles
        .iter()
        .filter(|profile| {
            profile.pointer_run_id == pointer_run.id
                && profile.analysis_state_budget == analysis_state_budget
        })
        .collect::<Vec<_>>();
    let exhausted_seed_offsets = value_flow
        .budget_exhausted_seeds
        .iter()
        .map(|exhaustion| exhaustion.seed_offset)
        .collect::<BTreeSet<_>>();
    let profiled_seed_offsets = matching_profiles
        .iter()
        .map(|profile| profile.seed_instruction_offset)
        .collect::<BTreeSet<_>>();
    ensure!(
        profiled_seed_offsets.is_subset(&exhausted_seed_offsets),
        "{} reviewed non-address seed census changed: expected {profiled_seed_offsets:?}, exhausted {exhausted_seed_offsets:?}",
        pointer_run.id
    );

    matching_profiles
        .into_iter()
        .map(|profile| {
            ensure!(
                profile.image_path == image.path,
                "{} reviewed non-address flow image changed",
                profile.id
            );
            ensure_instruction(
                image,
                profile.seed_instruction_offset,
                &profile.expected_seed_instruction,
                profile.id,
                "seed",
            )?;
            ensure_instruction(
                image,
                profile.scalar_load_instruction_offset,
                &profile.expected_scalar_load_instruction,
                profile.id,
                "scalar load",
            )?;
            let exhaustion = value_flow
                .budget_exhausted_seeds
                .iter()
                .find(|exhaustion| exhaustion.seed_offset == profile.seed_instruction_offset)
                .expect("profiled seed subset was validated");
            let actual_frontier = exhaustion
                .frontier_instruction_offsets
                .iter()
                .copied()
                .collect::<BTreeSet<_>>();
            let expected_frontier = profile
                .expected_frontier_instructions
                .iter()
                .map(|(offset, _)| *offset)
                .collect::<BTreeSet<_>>();
            ensure!(
                actual_frontier == expected_frontier,
                "{} reviewed non-address frontier changed: expected {expected_frontier:?}, found {actual_frontier:?}",
                profile.id
            );
            for (offset, instruction) in profile.expected_frontier_instructions {
                ensure_instruction(image, *offset, instruction, profile.id, "frontier")?;
            }
            let scalar_load_observed = value_flow.addresses.iter().any(|address| {
                address.seed_offset == profile.seed_instruction_offset
                    && address.instruction_offset == profile.scalar_load_instruction_offset
                    && matches!(
                        address.kind,
                        DerivedAddressKind::LoadedValue {
                            storage_address,
                            load_instruction_offset,
                            width_bytes: 2,
                            sign_extended: false,
                        } if storage_address == profile.scalar_storage_runtime_address
                            && load_instruction_offset == profile.scalar_load_instruction_offset
                    )
            });
            ensure!(
                scalar_load_observed,
                "{} reviewed non-address unsigned-halfword load evidence changed",
                profile.id
            );
            Ok(profile.id.to_string())
        })
        .collect()
}

fn ensure_instruction(
    image: &LoadedImage,
    offset: usize,
    expected: &Instruction,
    profile_id: &str,
    role: &str,
) -> Result<()> {
    let runtime_base = image
        .runtime_base
        .with_context(|| format!("{} lacks a runtime base", image.path))?;
    let end = offset
        .checked_add(4)
        .context("pointer-run instruction range overflow")?;
    let bytes = image
        .data
        .get(offset..end)
        .with_context(|| format!("{profile_id} {role} instruction is truncated"))?;
    let pc = runtime_base
        .checked_add(u32::try_from(offset)?)
        .context("pointer-run instruction address overflow")?;
    let actual = decode(u32::from_le_bytes(bytes.try_into()?), pc)
        .with_context(|| format!("failed to decode {profile_id} {role} instruction"))?;
    ensure!(
        &actual == expected,
        "{profile_id} {role} instruction changed at {offset:#x}: expected {expected:?}, found {actual:?}"
    );
    Ok(())
}

fn decode_pointer_target(
    image: &LoadedImage,
    runtime_base: u32,
    pointer_table_offset: usize,
    profile: &StaticPointerRunProfile,
    target_arena_end: u32,
    index: usize,
) -> Result<StaticPointerTargetAudit> {
    let storage_offset = pointer_table_offset
        .checked_add(index.checked_mul(4).context("pointer index overflow")?)
        .context("pointer storage offset overflow")?;
    let storage_end = storage_offset
        .checked_add(4)
        .context("pointer storage end overflow")?;
    let target = u32::from_le_bytes(
        image.data[storage_offset..storage_end]
            .try_into()
            .expect("validated pointer table range"),
    );
    ensure!(
        address_in_range(
            target,
            profile.target_arena_runtime_address,
            target_arena_end
        ),
        "static pointer-run profile {} target {} leaves its arena: {target:#010x}",
        profile.id,
        index
    );
    let target_loaded_image_offset = target
        .checked_sub(runtime_base)
        .context("pointer target precedes loaded image")?;
    let target_arena_offset = target
        .checked_sub(profile.target_arena_runtime_address)
        .context("pointer target precedes target arena")?;
    Ok(StaticPointerTargetAudit {
        index,
        storage_offset: hex_offset(storage_offset),
        storage_runtime_address: hex_address(
            runtime_base
                .checked_add(u32::try_from(storage_offset)?)
                .context("pointer storage address overflow")?,
        ),
        target_runtime_address: hex_address(target),
        target_loaded_image_offset: hex_offset(usize::try_from(target_loaded_image_offset)?),
        target_arena_offset: hex_offset(usize::try_from(target_arena_offset)?),
    })
}

fn raw_address_references(
    image: &LoadedImage,
    runtime_base: u32,
    pointer_table_start: u32,
    pointer_table_end: u32,
    target_arena_start: u32,
    target_arena_end: u32,
) -> Vec<StaticRawAddressReferenceAudit> {
    image
        .data
        .as_chunks::<4>()
        .0
        .iter()
        .enumerate()
        .filter_map(|(word_index, bytes)| {
            let value = u32::from_le_bytes(*bytes);
            let target_region = if address_in_range(value, pointer_table_start, pointer_table_end) {
                "pointer_table"
            } else if address_in_range(value, target_arena_start, target_arena_end) {
                "target_arena"
            } else {
                return None;
            };
            let source_offset = word_index * 4;
            let source_runtime_address = runtime_base + u32::try_from(source_offset).ok()?;
            let source_region = if address_in_range(
                source_runtime_address,
                pointer_table_start,
                pointer_table_end,
            ) {
                "pointer_table_entry"
            } else if address_in_range(source_runtime_address, target_arena_start, target_arena_end)
            {
                "target_arena_word"
            } else {
                "other_loaded_image_word"
            };
            Some(StaticRawAddressReferenceAudit {
                source_offset: hex_offset(source_offset),
                source_runtime_address: hex_address(source_runtime_address),
                value_runtime_address: hex_address(value),
                target_region: target_region.to_string(),
                source_region: source_region.to_string(),
            })
        })
        .collect()
}

fn derived_address_references(
    image: &LoadedImage,
    runtime_base: u32,
    value_flow: &DerivedAddressScan,
    pointer_table_start: u32,
    pointer_table_end: u32,
    target_arena_start: u32,
    target_arena_end: u32,
) -> Result<Vec<StaticDerivedAddressReferenceAudit>> {
    value_flow
        .addresses
        .iter()
        .filter(|reference| {
            address_in_either_range(
                reference.address,
                pointer_table_start,
                pointer_table_end,
                target_arena_start,
                target_arena_end,
            ) || loaded_value_storage_address(reference).is_some_and(|address| {
                address_in_either_range(
                    address,
                    pointer_table_start,
                    pointer_table_end,
                    target_arena_start,
                    target_arena_end,
                )
            })
        })
        .map(|reference| {
            derived_address_reference(
                image,
                runtime_base,
                reference,
                pointer_table_start,
                pointer_table_end,
                target_arena_start,
                target_arena_end,
            )
        })
        .collect()
}

fn derived_address_reference(
    image: &LoadedImage,
    runtime_base: u32,
    reference: &DerivedAddress,
    pointer_table_start: u32,
    pointer_table_end: u32,
    target_arena_start: u32,
    target_arena_end: u32,
) -> Result<StaticDerivedAddressReferenceAudit> {
    let instruction_runtime_address = runtime_base
        .checked_add(u32::try_from(reference.instruction_offset)?)
        .context("derived-reference instruction address overflow")?;
    let instruction_end = reference
        .instruction_offset
        .checked_add(4)
        .context("derived-reference instruction end overflow")?;
    let instruction_bytes = image
        .data
        .get(reference.instruction_offset..instruction_end)
        .context("derived-reference instruction is truncated")?;
    let instruction = decode(
        u32::from_le_bytes(instruction_bytes.try_into()?),
        instruction_runtime_address,
    )
    .with_context(|| {
        format!(
            "failed to decode derived-reference instruction at {instruction_runtime_address:#010x}"
        )
    })?;
    let (
        derived_address_kind,
        loaded_value_storage_runtime_address,
        loaded_value_storage_region,
        loaded_value_instruction_offset,
        loaded_value_width_bytes,
        loaded_value_sign_extended,
    ) = match reference.kind {
        DerivedAddressKind::RegisterValue => ("register_value", None, None, None, None, None),
        DerivedAddressKind::MemoryAccess => ("memory_access", None, None, None, None, None),
        DerivedAddressKind::LoadedValue {
            storage_address,
            load_instruction_offset,
            width_bytes,
            sign_extended,
        } => (
            "loaded_value",
            Some(hex_address(storage_address)),
            Some(
                address_region(
                    storage_address,
                    pointer_table_start,
                    pointer_table_end,
                    target_arena_start,
                    target_arena_end,
                )
                .to_string(),
            ),
            Some(hex_offset(load_instruction_offset)),
            Some(width_bytes),
            Some(sign_extended),
        ),
    };
    Ok(StaticDerivedAddressReferenceAudit {
        seed_instruction_offset: hex_offset(reference.seed_offset),
        seed_instruction_runtime_address: hex_address(
            runtime_base
                .checked_add(u32::try_from(reference.seed_offset)?)
                .context("derived-reference seed address overflow")?,
        ),
        instruction_offset: hex_offset(reference.instruction_offset),
        instruction_runtime_address: hex_address(instruction_runtime_address),
        decoded_instruction: format!("{instruction:?}"),
        derived_runtime_address: hex_address(reference.address),
        derived_target_region: address_region(
            reference.address,
            pointer_table_start,
            pointer_table_end,
            target_arena_start,
            target_arena_end,
        )
        .to_string(),
        derived_address_kind: derived_address_kind.to_string(),
        loaded_value_storage_runtime_address,
        loaded_value_storage_region,
        loaded_value_instruction_offset,
        loaded_value_width_bytes,
        loaded_value_sign_extended,
    })
}

fn pointer_table_size(profile: &StaticPointerRunProfile) -> Result<u32> {
    u32::try_from(
        profile
            .pointer_count
            .checked_mul(4)
            .context("static pointer table size overflow")?,
    )
    .context("static pointer table is too large")
}

fn checked_address_end(start: u32, size: u32, label: &str) -> Result<u32> {
    start
        .checked_add(size)
        .with_context(|| format!("{label} runtime range overflow"))
}

fn loaded_image_offset(
    image: &LoadedImage,
    runtime_base: u32,
    runtime_address: u32,
    size: usize,
    label: &str,
) -> Result<usize> {
    let offset = runtime_address
        .checked_sub(runtime_base)
        .and_then(|offset| usize::try_from(offset).ok())
        .with_context(|| format!("{} {label} precedes its loaded image", image.path))?;
    let end = offset
        .checked_add(size)
        .with_context(|| format!("{} {label} range overflow", image.path))?;
    ensure!(
        end <= image.data.len(),
        "{} {label} leaves its loaded image",
        image.path
    );
    Ok(offset)
}

fn loaded_value_storage_address(reference: &DerivedAddress) -> Option<u32> {
    match reference.kind {
        DerivedAddressKind::LoadedValue {
            storage_address, ..
        } => Some(storage_address),
        DerivedAddressKind::RegisterValue | DerivedAddressKind::MemoryAccess => None,
    }
}

fn address_in_either_range(
    address: u32,
    first_start: u32,
    first_end: u32,
    second_start: u32,
    second_end: u32,
) -> bool {
    address_in_range(address, first_start, first_end)
        || address_in_range(address, second_start, second_end)
}

fn address_in_range(address: u32, start: u32, end: u32) -> bool {
    address >= start && address < end
}

fn address_region(
    address: u32,
    pointer_table_start: u32,
    pointer_table_end: u32,
    target_arena_start: u32,
    target_arena_end: u32,
) -> &'static str {
    if address_in_range(address, pointer_table_start, pointer_table_end) {
        "pointer_table"
    } else if address_in_range(address, target_arena_start, target_arena_end) {
        "target_arena"
    } else {
        "outside_declared_pointer_run"
    }
}

fn hex_address(value: u32) -> String {
    format!("0x{value:08x}")
}

fn hex_offset(value: usize) -> String {
    format!("0x{value:x}")
}

#[cfg(test)]
#[path = "pointer_run_references_tests.rs"]
mod tests;
