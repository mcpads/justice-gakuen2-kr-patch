use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};
use psx_r3000a::{Instruction, Register, decode};

use super::declared_selector_domain;
use super::writer_domain::{DeclaredSelectorDomain, declared_upstream_state_domain};
use crate::consumer_analysis::catalog::SourceCatalog;
use crate::consumer_analysis::model::{LoadedImageConsumerAudit, StaticCustomRecordSelectorAudit};
use crate::consumer_analysis::profiles::MAIN_DISC_RECORD_LOADER_ADDRESS;
use crate::source_disc::{LoadedImage, MAIN_EXECUTABLE_PATH};

const RECORD_BASE_RUNTIME_ADDRESS: u32 = 0x801f_5804;
const RECORD_STRIDE_BYTES: u32 = 40;
const SPECIAL_RESOURCE_SELECTOR: u8 = 30;

#[derive(Clone, Copy)]
struct CustomRecordSelectorProfile {
    id: &'static str,
    function_runtime_address: u32,
    record_index_state_runtime_address: u32,
    resource_selector_state_runtime_address: u32,
    secondary_record_byte_offset: u32,
    primary_table_base_when_selector_zero: u32,
    secondary_table_base_when_selector_zero: u32,
}

struct SelectorCallBinding<'a> {
    record_byte_offsets: &'a BTreeSet<u32>,
    offset_condition_state_runtime_address: Option<u32>,
    table_load_runtime_address: u32,
    loader_call_runtime_address: u32,
    table_base_when_selector_zero: u32,
    selector_stride_bytes: u32,
}

struct CustomRecordLookupContext<'a> {
    image: &'a LoadedImage,
    catalog: &'a SourceCatalog,
}

const CUSTOM_RECORD_SELECTOR_PROFILES: &[CustomRecordSelectorProfile] = &[
    CustomRecordSelectorProfile {
        id: "resource_state_6494_custom_record",
        function_runtime_address: 0x8002_8808,
        record_index_state_runtime_address: 0x801f_64fc,
        resource_selector_state_runtime_address: 0x801f_6494,
        secondary_record_byte_offset: 1,
        primary_table_base_when_selector_zero: 0x8008_a70e,
        secondary_table_base_when_selector_zero: 0x8008_a4f8,
    },
    CustomRecordSelectorProfile {
        id: "resource_state_6496_custom_record",
        function_runtime_address: 0x8002_89a4,
        record_index_state_runtime_address: 0x801f_64fe,
        resource_selector_state_runtime_address: 0x801f_6496,
        secondary_record_byte_offset: 1,
        primary_table_base_when_selector_zero: 0x8008_a70e,
        secondary_table_base_when_selector_zero: 0x8008_a4fa,
    },
    CustomRecordSelectorProfile {
        id: "resource_state_6495_custom_record",
        function_runtime_address: 0x8002_8b54,
        record_index_state_runtime_address: 0x801f_64fd,
        resource_selector_state_runtime_address: 0x801f_6495,
        secondary_record_byte_offset: 15,
        primary_table_base_when_selector_zero: 0x8008_a70e,
        secondary_table_base_when_selector_zero: 0x8008_a628,
    },
    CustomRecordSelectorProfile {
        id: "resource_state_6497_custom_record",
        function_runtime_address: 0x8002_8d04,
        record_index_state_runtime_address: 0x801f_64ff,
        resource_selector_state_runtime_address: 0x801f_6497,
        secondary_record_byte_offset: 15,
        primary_table_base_when_selector_zero: 0x8008_a70e,
        secondary_table_base_when_selector_zero: 0x8008_a62a,
    },
];

pub(super) fn annotate_custom_record_selector_loads(
    image: &LoadedImage,
    reports: &mut [LoadedImageConsumerAudit],
    catalog: &SourceCatalog,
) -> Result<()> {
    ensure!(
        image.path == MAIN_EXECUTABLE_PATH,
        "custom-record selector profiles require the main executable"
    );
    let runtime_base = image
        .runtime_base
        .context("main executable lacks a runtime base")?;
    let domains = CUSTOM_RECORD_SELECTOR_PROFILES
        .iter()
        .map(|profile| {
            let record_index_domain =
                declared_selector_domain(reports, profile.record_index_state_runtime_address)?;
            let resource_selector_domain =
                declared_selector_domain(reports, profile.resource_selector_state_runtime_address)?;
            let primary_offsets = primary_record_byte_offsets(&resource_selector_domain);
            let primary_selector_domain =
                record_byte_domain(reports, &record_index_domain, &primary_offsets)?;
            let secondary_offsets = BTreeSet::from([profile.secondary_record_byte_offset]);
            let secondary_selector_domain =
                record_byte_domain(reports, &record_index_domain, &secondary_offsets)?;
            Ok((
                *profile,
                record_index_domain,
                primary_offsets,
                primary_selector_domain,
                secondary_offsets,
                secondary_selector_domain,
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    let main_report = exactly_one(
        reports
            .iter_mut()
            .filter(|report| report.path == MAIN_EXECUTABLE_PATH),
        "main executable consumer report",
    )?;
    let mut profiled_calls = BTreeSet::new();
    let lookup_context = CustomRecordLookupContext { image, catalog };

    for (
        profile,
        record_index_domain,
        primary_offsets,
        primary_selector_domain,
        secondary_offsets,
        secondary_selector_domain,
    ) in &domains
    {
        validate_profile(image, runtime_base, profile)?;
        annotate_call(
            &lookup_context,
            main_report,
            profile,
            record_index_domain,
            &SelectorCallBinding {
                record_byte_offsets: primary_offsets,
                offset_condition_state_runtime_address: Some(
                    profile.resource_selector_state_runtime_address,
                ),
                table_load_runtime_address: profile.function_runtime_address + 0x74,
                loader_call_runtime_address: profile.function_runtime_address + 0x78,
                table_base_when_selector_zero: profile.primary_table_base_when_selector_zero,
                selector_stride_bytes: 4,
            },
            primary_selector_domain,
            &mut profiled_calls,
        )?;
        annotate_call(
            &lookup_context,
            main_report,
            profile,
            record_index_domain,
            &SelectorCallBinding {
                record_byte_offsets: secondary_offsets,
                offset_condition_state_runtime_address: None,
                table_load_runtime_address: profile.function_runtime_address + 0x104,
                loader_call_runtime_address: profile.function_runtime_address + 0x108,
                table_base_when_selector_zero: profile.secondary_table_base_when_selector_zero,
                selector_stride_bytes: 8,
            },
            secondary_selector_domain,
            &mut profiled_calls,
        )?;
    }

    ensure!(
        profiled_calls.len() == CUSTOM_RECORD_SELECTOR_PROFILES.len() * 2,
        "custom-record selector call count changed"
    );
    Ok(())
}

fn annotate_call(
    lookup_context: &CustomRecordLookupContext<'_>,
    main_report: &mut LoadedImageConsumerAudit,
    profile: &CustomRecordSelectorProfile,
    record_index_domain: &super::writer_domain::DeclaredSelectorDomain,
    binding: &SelectorCallBinding<'_>,
    selector_value_domain: &DeclaredSelectorDomain,
    profiled_calls: &mut BTreeSet<u32>,
) -> Result<()> {
    let loader_call_runtime_address = binding.loader_call_runtime_address;
    ensure!(
        profiled_calls.insert(loader_call_runtime_address),
        "custom-record loader call {loader_call_runtime_address:#010x} has multiple profiles"
    );
    let load = exactly_one(
        main_report
            .disc_record_load_call_candidates
            .iter_mut()
            .filter(|candidate| {
                candidate.call_runtime_address == format!("0x{loader_call_runtime_address:08x}")
            }),
        "custom-record selector loader call",
    )?;
    ensure!(
        load.static_owner_id.as_deref() == Some("four_slot_resource_setup"),
        "custom-record selector call lost its four-slot owner at {loader_call_runtime_address:#010x}"
    );
    ensure!(
        load.catalog_index_candidates.is_empty(),
        "custom-record selector call was unexpectedly catalog-resolved at {loader_call_runtime_address:#010x}"
    );
    let memory_load = load
        .catalog_index_memory_load
        .as_mut()
        .context("custom-record selector call lost its preceding table load")?;
    ensure!(
        memory_load.load_runtime_address == format!("0x{:08x}", binding.table_load_runtime_address),
        "custom-record table load address changed before {loader_call_runtime_address:#010x}"
    );
    ensure!(
        memory_load.effective_address_when_selector_zero.as_deref()
            == Some(format!("0x{:08x}", binding.table_base_when_selector_zero).as_str()),
        "custom-record table base changed before {loader_call_runtime_address:#010x}"
    );
    ensure!(
        memory_load.selector_stride_bytes == Some(binding.selector_stride_bytes),
        "custom-record table stride changed before {loader_call_runtime_address:#010x}"
    );
    ensure!(
        memory_load.load_kind == "unsigned_halfword"
            && memory_load.address_resolution == "affine"
            && memory_load.selector_register.as_deref() == Some("v0")
            && memory_load.selector_origin_kind.as_deref() == Some("unsigned_byte_load"),
        "custom-record table load semantics changed before {loader_call_runtime_address:#010x}"
    );
    let expected_selector_origin = if binding.selector_stride_bytes == 4 {
        profile.function_runtime_address + 0x5c
    } else {
        profile.function_runtime_address + 0xf0
    };
    ensure!(
        memory_load.selector_origin_runtime_address.as_deref()
            == Some(format!("0x{expected_selector_origin:08x}").as_str()),
        "custom-record selector origin changed before {loader_call_runtime_address:#010x}"
    );
    ensure!(
        memory_load.declared_selector_domain.is_none()
            && memory_load.custom_record_selector.is_none(),
        "custom-record selector call already has a domain at {loader_call_runtime_address:#010x}"
    );

    let selector_addresses =
        selector_runtime_addresses(record_index_domain, binding.record_byte_offsets)?;
    let mut unresolved_selector_sources = selector_value_domain.unresolved_runtime_sources.clone();
    if !record_index_domain.unresolved_runtime_sources.is_empty() {
        unresolved_selector_sources.insert(format!(
            "dynamic_record_index@0x{:08x}",
            profile.record_index_state_runtime_address
        ));
    }
    let (effective_addresses, table_values) = table_candidates_for_selector_values(
        lookup_context.image,
        lookup_context
            .image
            .runtime_base
            .context("custom-record table image lacks a runtime base")?,
        binding.table_base_when_selector_zero,
        binding.selector_stride_bytes,
        &selector_value_domain.known_values,
    )?;
    let known_catalog_record_paths = table_values
        .iter()
        .flat_map(|value| lookup_context.catalog.record_paths(*value))
        .collect::<BTreeSet<_>>();
    memory_load.custom_record_selector = Some(StaticCustomRecordSelectorAudit {
        profile_id: profile.id.to_string(),
        record_index_state_runtime_address: format!(
            "0x{:08x}",
            profile.record_index_state_runtime_address
        ),
        record_index_value_resolution: if record_index_domain.is_finite() {
            "finite_declared_writer_domain"
        } else {
            "partial_dynamic_declared_writer_domain"
        }
        .to_string(),
        known_record_index_candidates: record_index_domain
            .known_values
            .iter()
            .copied()
            .collect(),
        unresolved_record_index_sources: record_index_domain
            .unresolved_runtime_sources
            .iter()
            .cloned()
            .collect(),
        record_base_runtime_address: format!("0x{RECORD_BASE_RUNTIME_ADDRESS:08x}"),
        record_stride_bytes: RECORD_STRIDE_BYTES,
        selector_record_byte_offsets: binding.record_byte_offsets.iter().copied().collect(),
        selector_offset_condition_state_runtime_address: binding
            .offset_condition_state_runtime_address
            .map(|address| format!("0x{address:08x}")),
        selector_runtime_address_candidates: selector_addresses
            .iter()
            .map(|address| format!("0x{address:08x}"))
            .collect(),
        selector_value_resolution: if selector_value_domain.is_finite()
            && record_index_domain.is_finite()
        {
            "finite_declared_writer_domain"
        } else {
            "partial_dynamic_declared_writer_domain"
        }
        .to_string(),
        known_selector_value_candidates: selector_value_domain
            .known_values
            .iter()
            .copied()
            .collect(),
        unresolved_selector_sources: unresolved_selector_sources.into_iter().collect(),
        effective_address_candidates: effective_addresses
            .into_iter()
            .map(|address| format!("0x{address:08x}"))
            .collect(),
        table_value_candidates: table_values.into_iter().collect(),
        known_catalog_record_paths: known_catalog_record_paths.into_iter().collect(),
        evidence: "validated explicit custom-record index load, forty-byte record addressing, conditional or fixed field load, selector transform, table load, and disc-loader call"
            .to_string(),
    });
    load.evidence =
        "full_loaded_image_direct_jal_candidate_with_custom_record_selector_domain".to_string();
    Ok(())
}

fn table_candidates_for_selector_values(
    image: &LoadedImage,
    runtime_base: u32,
    table_base: u32,
    selector_stride_bytes: u32,
    selector_values: &BTreeSet<u8>,
) -> Result<(BTreeSet<u32>, BTreeSet<u16>)> {
    let mut addresses = BTreeSet::new();
    let mut values = BTreeSet::new();
    for selector in selector_values {
        let address = table_base
            .checked_add(u32::from(*selector) * selector_stride_bytes)
            .context("custom-record table address overflow")?;
        let offset = usize::try_from(
            address
                .checked_sub(runtime_base)
                .context("custom-record table address precedes the main executable")?,
        )?;
        let bytes = image
            .data
            .get(offset..offset + 2)
            .context("custom-record table halfword is outside the main executable")?;
        addresses.insert(address);
        values.insert(u16::from_le_bytes(bytes.try_into()?));
    }
    Ok((addresses, values))
}

fn record_byte_domain(
    reports: &[LoadedImageConsumerAudit],
    record_index_domain: &DeclaredSelectorDomain,
    record_byte_offsets: &BTreeSet<u32>,
) -> Result<DeclaredSelectorDomain> {
    let mut domain = DeclaredSelectorDomain::default();
    for address in selector_runtime_addresses(record_index_domain, record_byte_offsets)? {
        let address_domain = declared_upstream_state_domain(reports, address)?;
        domain.known_values.extend(address_domain.known_values);
        domain
            .unresolved_runtime_sources
            .extend(address_domain.unresolved_runtime_sources);
    }
    Ok(domain)
}

fn primary_record_byte_offsets(
    resource_selector_domain: &super::writer_domain::DeclaredSelectorDomain,
) -> BTreeSet<u32> {
    let mut offsets = BTreeSet::new();
    if resource_selector_domain
        .known_values
        .contains(&SPECIAL_RESOURCE_SELECTOR)
        || !resource_selector_domain
            .unresolved_runtime_sources
            .is_empty()
    {
        offsets.insert(0);
    }
    if resource_selector_domain
        .known_values
        .iter()
        .any(|value| *value != SPECIAL_RESOURCE_SELECTOR)
        || !resource_selector_domain
            .unresolved_runtime_sources
            .is_empty()
    {
        offsets.insert(3);
    }
    offsets
}

fn selector_runtime_addresses(
    record_index_domain: &super::writer_domain::DeclaredSelectorDomain,
    record_byte_offsets: &BTreeSet<u32>,
) -> Result<BTreeSet<u32>> {
    let mut addresses = BTreeSet::new();
    for index in &record_index_domain.known_values {
        let record_address = RECORD_BASE_RUNTIME_ADDRESS
            .checked_add(u32::from(*index) * RECORD_STRIDE_BYTES)
            .context("custom-record address overflow")?;
        for byte_offset in record_byte_offsets {
            addresses.insert(
                record_address
                    .checked_add(*byte_offset)
                    .context("custom-record field address overflow")?,
            );
        }
    }
    Ok(addresses)
}

fn validate_profile(
    image: &LoadedImage,
    runtime_base: u32,
    profile: &CustomRecordSelectorProfile,
) -> Result<()> {
    let entry = profile.function_runtime_address;
    let record_index_low = profile.record_index_state_runtime_address as u16;
    let resource_selector_low = profile.resource_selector_state_runtime_address as u16;
    let expected = [
        (
            entry + 0x04,
            Instruction::Lui {
                rt: Register::V1,
                immediate: 0x801f,
            },
        ),
        (
            entry + 0x08,
            Instruction::Lbu {
                rt: Register::V1,
                base: Register::V1,
                offset: record_index_low as i16,
            },
        ),
        (
            entry + 0x0c,
            Instruction::Lui {
                rt: Register::A0,
                immediate: 0x801f,
            },
        ),
        (
            entry + 0x1c,
            Instruction::Ori {
                rt: Register::A0,
                rs: Register::A0,
                immediate: RECORD_BASE_RUNTIME_ADDRESS as u16,
            },
        ),
        (
            entry + 0x28,
            Instruction::Sll {
                rd: Register::V0,
                rt: Register::V1,
                shift: 2,
            },
        ),
        (
            entry + 0x2c,
            Instruction::Addu {
                rd: Register::V0,
                rs: Register::V0,
                rt: Register::V1,
            },
        ),
        (
            entry + 0x30,
            Instruction::Sll {
                rd: Register::A1,
                rt: Register::V0,
                shift: 3,
            },
        ),
        (
            entry + 0x34,
            Instruction::Lui {
                rt: Register::V0,
                immediate: 0x801f,
            },
        ),
        (
            entry + 0x38,
            Instruction::Lbu {
                rt: Register::V0,
                base: Register::V0,
                offset: resource_selector_low as i16,
            },
        ),
        (
            entry + 0x3c,
            Instruction::Addiu {
                rt: Register::V1,
                rs: Register::ZERO,
                immediate: i16::from(SPECIAL_RESOURCE_SELECTOR),
            },
        ),
        (
            entry + 0x40,
            Instruction::Bne {
                rs: Register::V0,
                rt: Register::V1,
                target: entry + 0x54,
            },
        ),
        (
            entry + 0x44,
            Instruction::Addu {
                rd: Register::S0,
                rs: Register::A1,
                rt: Register::A0,
            },
        ),
        (
            entry + 0x48,
            Instruction::Lbu {
                rt: Register::V0,
                base: Register::S0,
                offset: 0,
            },
        ),
        (
            entry + 0x4c,
            Instruction::J {
                target: entry + 0x68,
            },
        ),
        (
            entry + 0x54,
            Instruction::Lui {
                rt: Register::AT,
                immediate: 0x801f,
            },
        ),
        (
            entry + 0x58,
            Instruction::Addu {
                rd: Register::AT,
                rs: Register::A1,
                rt: Register::AT,
            },
        ),
        (
            entry + 0x5c,
            Instruction::Lbu {
                rt: Register::V0,
                base: Register::AT,
                offset: 0x5807,
            },
        ),
        (
            entry + 0x64,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::V0,
                immediate: 11,
            },
        ),
        (
            entry + 0x68,
            Instruction::Sll {
                rd: Register::V0,
                rt: Register::V0,
                shift: 2,
            },
        ),
        (
            entry + 0x6c,
            Instruction::Lui {
                rt: Register::AT,
                immediate: 0x8009,
            },
        ),
        (
            entry + 0x70,
            Instruction::Addu {
                rd: Register::AT,
                rs: Register::AT,
                rt: Register::V0,
            },
        ),
        (
            entry + 0x74,
            Instruction::Lhu {
                rt: Register::A1,
                base: Register::AT,
                offset: signed_low(
                    profile
                        .primary_table_base_when_selector_zero
                        .wrapping_sub(11 * 4),
                ),
            },
        ),
        (
            entry + 0x78,
            Instruction::Jal {
                target: MAIN_DISC_RECORD_LOADER_ADDRESS,
            },
        ),
        (
            entry + 0xf0,
            Instruction::Lbu {
                rt: Register::V0,
                base: Register::S0,
                offset: profile.secondary_record_byte_offset as i16,
            },
        ),
        (
            entry + 0xf8,
            Instruction::Sll {
                rd: Register::V0,
                rt: Register::V0,
                shift: 3,
            },
        ),
        (
            entry + 0xfc,
            Instruction::Lui {
                rt: Register::AT,
                immediate: 0x8009,
            },
        ),
        (
            entry + 0x100,
            Instruction::Addu {
                rd: Register::AT,
                rs: Register::AT,
                rt: Register::V0,
            },
        ),
        (
            entry + 0x104,
            Instruction::Lhu {
                rt: Register::A1,
                base: Register::AT,
                offset: signed_low(profile.secondary_table_base_when_selector_zero),
            },
        ),
        (
            entry + 0x108,
            Instruction::Jal {
                target: MAIN_DISC_RECORD_LOADER_ADDRESS,
            },
        ),
    ];
    for (address, instruction) in expected {
        ensure_instruction(image, runtime_base, address, instruction)?;
    }
    Ok(())
}

fn signed_low(address: u32) -> i16 {
    address as u16 as i16
}

fn ensure_instruction(
    image: &LoadedImage,
    runtime_base: u32,
    runtime_address: u32,
    expected: Instruction,
) -> Result<()> {
    let offset = runtime_address
        .checked_sub(runtime_base)
        .and_then(|offset| usize::try_from(offset).ok())
        .context("custom-record instruction leaves the loaded image")?;
    let bytes = image
        .data
        .get(offset..offset.checked_add(4).context("instruction end overflow")?)
        .context("custom-record instruction is truncated")?;
    let actual =
        decode(u32::from_le_bytes(bytes.try_into()?), runtime_address).with_context(|| {
            format!("failed to decode custom-record instruction at {runtime_address:#010x}")
        })?;
    ensure!(
        actual == expected,
        "custom-record selector instruction changed at {runtime_address:#010x}: expected {expected:?}, found {actual:?}"
    );
    Ok(())
}

fn exactly_one<T>(mut values: impl Iterator<Item = T>, description: &str) -> Result<T> {
    let value = values
        .next()
        .with_context(|| format!("missing {description}"))?;
    ensure!(
        values.next().is_none(),
        "multiple values match {description}"
    );
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consumer_analysis::catalog_selector_domain::writer_domain::DeclaredSelectorDomain;

    #[test]
    fn primary_field_offsets_follow_the_special_resource_condition() {
        let ordinary = DeclaredSelectorDomain {
            known_values: BTreeSet::from([0, 2]),
            unresolved_runtime_sources: BTreeSet::new(),
        };
        let special = DeclaredSelectorDomain {
            known_values: BTreeSet::from([SPECIAL_RESOURCE_SELECTOR]),
            unresolved_runtime_sources: BTreeSet::new(),
        };
        let dynamic = DeclaredSelectorDomain {
            known_values: BTreeSet::from([0]),
            unresolved_runtime_sources: BTreeSet::from(["runtime".to_string()]),
        };

        assert_eq!(primary_record_byte_offsets(&ordinary), BTreeSet::from([3]));
        assert_eq!(primary_record_byte_offsets(&special), BTreeSet::from([0]));
        assert_eq!(
            primary_record_byte_offsets(&dynamic),
            BTreeSet::from([0, 3])
        );
    }

    #[test]
    fn selector_addresses_use_declared_indices_and_explicit_field_offsets() {
        let domain = DeclaredSelectorDomain {
            known_values: BTreeSet::from([0, 2]),
            unresolved_runtime_sources: BTreeSet::new(),
        };

        assert_eq!(
            selector_runtime_addresses(&domain, &BTreeSet::from([1, 15])).unwrap(),
            BTreeSet::from([0x801f_5805, 0x801f_5813, 0x801f_5855, 0x801f_5863])
        );
    }
}
