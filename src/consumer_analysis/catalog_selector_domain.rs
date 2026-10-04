use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};
use psx_r3000a::{Instruction, Register, decode};

use super::catalog::SourceCatalog;
use super::model::{LoadedImageConsumerAudit, StaticDeclaredSelectorDomainAudit};
use super::profiles::{MAIN_DISC_RECORD_LOADER_ADDRESS, StaticConsumerSourceRegionProfile};
use crate::source_disc::{LoadedImage, MAIN_EXECUTABLE_PATH};

#[path = "catalog_selector_domain/custom_record.rs"]
mod custom_record;
#[path = "catalog_selector_domain/writer_domain.rs"]
mod writer_domain;

use custom_record::annotate_custom_record_selector_loads;
use writer_domain::declared_selector_domain;

#[derive(Clone, Copy)]
struct SelectorTableProfile {
    selector_state_runtime_address: u32,
    selector_load_runtime_address: u32,
    selector_register: Register,
    scale_runtime_address: u32,
    scaled_register: Register,
    table_loads: &'static [SelectorTableLoadProfile],
}

#[derive(Clone, Copy)]
struct SelectorTableLoadProfile {
    load_runtime_address: u32,
    call_runtime_address: u32,
    table_base_when_selector_zero: u32,
}

const SELECTOR_TABLE_PROFILES: &[SelectorTableProfile] = &[
    SelectorTableProfile {
        selector_state_runtime_address: 0x801f_6494,
        selector_load_runtime_address: 0x8002_823c,
        selector_register: Register::V1,
        scale_runtime_address: 0x8002_827c,
        scaled_register: Register::S2,
        table_loads: &[
            table_load(0x8002_8288, 0x8002_828c, 0x8008_a270),
            table_load(0x8002_82fc, 0x8002_8300, 0x8008_a268),
        ],
    },
    SelectorTableProfile {
        selector_state_runtime_address: 0x801f_6495,
        selector_load_runtime_address: 0x8002_8258,
        selector_register: Register::S0,
        scale_runtime_address: 0x8002_8334,
        scaled_register: Register::S0,
        table_loads: &[
            table_load(0x8002_8340, 0x8002_8344, 0x8008_a270),
            table_load(0x8002_83b4, 0x8002_83b8, 0x8008_a26c),
        ],
    },
    SelectorTableProfile {
        selector_state_runtime_address: 0x801f_6496,
        selector_load_runtime_address: 0x8002_8410,
        selector_register: Register::V1,
        scale_runtime_address: 0x8002_8450,
        scaled_register: Register::S2,
        table_loads: &[
            table_load(0x8002_845c, 0x8002_8460, 0x8008_a270),
            table_load(0x8002_84d0, 0x8002_84d4, 0x8008_a26a),
        ],
    },
    SelectorTableProfile {
        selector_state_runtime_address: 0x801f_6497,
        selector_load_runtime_address: 0x8002_842c,
        selector_register: Register::S0,
        scale_runtime_address: 0x8002_8518,
        scaled_register: Register::S0,
        table_loads: &[
            table_load(0x8002_8524, 0x8002_8528, 0x8008_a270),
            table_load(0x8002_8598, 0x8002_859c, 0x8008_a26e),
        ],
    },
];

const fn table_load(
    load_runtime_address: u32,
    call_runtime_address: u32,
    table_base_when_selector_zero: u32,
) -> SelectorTableLoadProfile {
    SelectorTableLoadProfile {
        load_runtime_address,
        call_runtime_address,
        table_base_when_selector_zero,
    }
}

pub(super) fn resolve_declared_selector_table_loads(
    images: &[LoadedImage],
    image_reports: &mut [LoadedImageConsumerAudit],
    catalog: &SourceCatalog,
    source_region_profiles: &[StaticConsumerSourceRegionProfile],
) -> Result<()> {
    let domains = SELECTOR_TABLE_PROFILES
        .iter()
        .map(|profile| {
            Ok((
                profile.selector_state_runtime_address,
                declared_selector_domain(image_reports, profile.selector_state_runtime_address)?,
            ))
        })
        .collect::<Result<BTreeMap<_, _>>>()?;
    let main_image = exactly_one(
        images
            .iter()
            .filter(|image| image.path == MAIN_EXECUTABLE_PATH),
        "main executable loaded image",
    )?;
    validate_selector_table_profiles(main_image)?;

    let main_report = exactly_one(
        image_reports
            .iter_mut()
            .filter(|report| report.path == MAIN_EXECUTABLE_PATH),
        "main executable consumer report",
    )?;
    let profiled_call_count = SELECTOR_TABLE_PROFILES
        .iter()
        .map(|profile| profile.table_loads.len())
        .sum::<usize>();
    let mut resolved_calls = BTreeSet::new();

    for profile in SELECTOR_TABLE_PROFILES {
        let domain = domains
            .get(&profile.selector_state_runtime_address)
            .context("selector domain disappeared after collection")?;
        for table_load in profile.table_loads {
            ensure!(
                resolved_calls.insert(table_load.call_runtime_address),
                "selector table call {:#010x} has multiple profiles",
                table_load.call_runtime_address
            );
            let load = exactly_one(
                main_report
                    .disc_record_load_call_candidates
                    .iter_mut()
                    .filter(|candidate| {
                        candidate.call_runtime_address
                            == format!("0x{:08x}", table_load.call_runtime_address)
                    }),
                "profiled selector-table loader call",
            )?;
            ensure!(
                load.static_owner_id.as_deref() == Some("four_slot_resource_setup"),
                "profiled selector-table loader call lost its four-slot owner at {:#010x}",
                table_load.call_runtime_address
            );
            let (effective_addresses, table_values) = table_candidates_for_domain(
                main_image,
                table_load.table_base_when_selector_zero,
                16,
                &domain.known_values,
            )?;
            let domain_audit = StaticDeclaredSelectorDomainAudit {
                state_runtime_address: format!(
                    "0x{:08x}",
                    profile.selector_state_runtime_address
                ),
                value_resolution: if domain.is_finite() {
                    "finite_declared_writer_domain"
                } else {
                    "partial_dynamic_declared_writer_domain"
                }
                .to_string(),
                known_value_candidates: domain.known_values.iter().copied().collect(),
                unresolved_runtime_sources: domain
                    .unresolved_runtime_sources
                    .iter()
                    .cloned()
                    .collect(),
                effective_address_candidates: effective_addresses
                    .iter()
                    .map(|address| format!("0x{address:08x}"))
                    .collect(),
                table_value_candidates: table_values.iter().copied().collect(),
                evidence: "classified declared final writers with byte-aligned propagation through classified identity-preserving runtime sources"
                    .to_string(),
            };
            let memory_load = load
                .catalog_index_memory_load
                .as_mut()
                .context("profiled selector-table call lost its preceding memory load")?;
            ensure!(
                memory_load.load_runtime_address
                    == format!("0x{:08x}", table_load.load_runtime_address),
                "profiled selector-table load address changed before {:#010x}",
                table_load.call_runtime_address
            );
            ensure!(
                memory_load.effective_address_when_selector_zero.as_deref()
                    == Some(format!("0x{:08x}", table_load.table_base_when_selector_zero).as_str()),
                "profiled selector-table base changed before {:#010x}",
                table_load.call_runtime_address
            );
            memory_load.declared_selector_domain = Some(domain_audit);

            if domain.is_finite() {
                ensure!(
                    load.catalog_index_candidates.is_empty(),
                    "profiled selector-table call was already catalog-resolved at {:#010x}",
                    table_load.call_runtime_address
                );
                load.catalog_index_candidates = table_values.iter().copied().collect();
                load.catalog_index_resolution =
                    "finite_declared_writer_domain_table_lookup".to_string();
                load.catalog_index_argument_producer =
                    "validated_selector_state_table_lookup".to_string();
                load.catalog_record_paths = table_values
                    .iter()
                    .flat_map(|index| catalog.record_paths(*index))
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect();
                load.declared_source_region_paths = table_values
                    .iter()
                    .flat_map(|index| {
                        source_region_profiles
                            .iter()
                            .filter(move |profile| profile.source_catalog_index == *index)
                            .map(|profile| profile.source_record_path.to_string())
                    })
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect();
                load.evidence =
                    "full_loaded_image_direct_jal_candidate_with_declared_writer_domain"
                        .to_string();
            }
        }
    }
    ensure!(
        resolved_calls.len() == profiled_call_count,
        "selector-table profile call count changed"
    );
    annotate_custom_record_selector_loads(main_image, image_reports, catalog)?;
    Ok(())
}

fn validate_selector_table_profiles(image: &LoadedImage) -> Result<()> {
    let runtime_base = image
        .runtime_base
        .context("main executable lacks runtime base")?;
    let mut calls = BTreeSet::new();
    for profile in SELECTOR_TABLE_PROFILES {
        ensure_instruction(
            image,
            runtime_base,
            profile.selector_load_runtime_address - 4,
            Instruction::Lui {
                rt: profile.selector_register,
                immediate: 0x801f,
            },
        )?;
        ensure_instruction(
            image,
            runtime_base,
            profile.selector_load_runtime_address,
            Instruction::Lbu {
                rt: profile.selector_register,
                base: profile.selector_register,
                offset: profile.selector_state_runtime_address as u16 as i16,
            },
        )?;
        ensure_instruction(
            image,
            runtime_base,
            profile.scale_runtime_address,
            Instruction::Sll {
                rd: profile.scaled_register,
                rt: profile.selector_register,
                shift: 4,
            },
        )?;
        for table_load in profile.table_loads {
            ensure!(
                calls.insert(table_load.call_runtime_address),
                "selector table call has multiple profiles at {:#010x}",
                table_load.call_runtime_address
            );
            ensure_register_is_preserved(
                image,
                runtime_base,
                profile.scale_runtime_address + 4,
                table_load.load_runtime_address - 4,
                profile.scaled_register,
            )?;
            ensure_instruction(
                image,
                runtime_base,
                table_load.load_runtime_address - 8,
                Instruction::Lui {
                    rt: Register::AT,
                    immediate: 0x8009,
                },
            )?;
            ensure_instruction(
                image,
                runtime_base,
                table_load.load_runtime_address - 4,
                Instruction::Addu {
                    rd: Register::AT,
                    rs: Register::AT,
                    rt: profile.scaled_register,
                },
            )?;
            ensure_instruction(
                image,
                runtime_base,
                table_load.load_runtime_address,
                Instruction::Lhu {
                    rt: Register::A1,
                    base: Register::AT,
                    offset: table_load.table_base_when_selector_zero as u16 as i16,
                },
            )?;
            ensure_instruction(
                image,
                runtime_base,
                table_load.call_runtime_address,
                Instruction::Jal {
                    target: MAIN_DISC_RECORD_LOADER_ADDRESS,
                },
            )?;
        }
    }
    Ok(())
}

fn ensure_register_is_preserved(
    image: &LoadedImage,
    runtime_base: u32,
    start_runtime_address: u32,
    end_runtime_address: u32,
    register: Register,
) -> Result<()> {
    for runtime_address in (start_runtime_address..end_runtime_address).step_by(4) {
        let instruction = decode_runtime(image, runtime_base, runtime_address)?;
        ensure!(
            instruction.written_gpr() != Some(register),
            "scaled selector register changed at {runtime_address:#010x}"
        );
    }
    Ok(())
}

fn ensure_instruction(
    image: &LoadedImage,
    runtime_base: u32,
    runtime_address: u32,
    expected: Instruction,
) -> Result<()> {
    let actual = decode_runtime(image, runtime_base, runtime_address)?;
    ensure!(
        actual == expected,
        "selector table instruction changed at {runtime_address:#010x}: expected {expected:?}, found {actual:?}"
    );
    Ok(())
}

fn decode_runtime(
    image: &LoadedImage,
    runtime_base: u32,
    runtime_address: u32,
) -> Result<Instruction> {
    let offset = runtime_address
        .checked_sub(runtime_base)
        .and_then(|offset| usize::try_from(offset).ok())
        .context("selector table instruction leaves the loaded image")?;
    let word = image
        .data
        .get(offset..offset.checked_add(4).context("instruction end overflow")?)
        .context("selector table instruction is truncated")?;
    decode(u32::from_le_bytes(word.try_into()?), runtime_address).with_context(|| {
        format!("failed to decode selector table instruction at {runtime_address:#010x}")
    })
}

fn table_candidates_for_domain(
    image: &LoadedImage,
    table_base_when_selector_zero: u32,
    selector_stride_bytes: u32,
    selector_values: &BTreeSet<u8>,
) -> Result<(BTreeSet<u32>, BTreeSet<u16>)> {
    let runtime_base = image
        .runtime_base
        .context("table image lacks runtime base")?;
    let mut addresses = BTreeSet::new();
    let mut values = BTreeSet::new();
    for selector in selector_values {
        let address = table_base_when_selector_zero
            .checked_add(u32::from(*selector) * selector_stride_bytes)
            .context("selector table address overflow")?;
        let offset = address
            .checked_sub(runtime_base)
            .and_then(|offset| usize::try_from(offset).ok())
            .context("selector table address leaves the loaded image")?;
        let bytes = image
            .data
            .get(offset..offset.checked_add(2).context("table value end overflow")?)
            .context("selector table value is truncated")?;
        addresses.insert(address);
        values.insert(u16::from_le_bytes(bytes.try_into()?));
    }
    Ok((addresses, values))
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

    #[test]
    fn table_candidates_follow_fixture_bytes() {
        let mut image = LoadedImage {
            path: "SYNTH.BIN".to_string(),
            data: vec![0; 16],
            runtime_base: Some(0x8001_0000),
            entrypoints: vec![],
        };
        image.data[2..4].copy_from_slice(&17_u16.to_le_bytes());
        image.data[6..8].copy_from_slice(&29_u16.to_le_bytes());
        let selector_values = BTreeSet::from([0, 2]);

        let (_, first) =
            table_candidates_for_domain(&image, 0x8001_0002, 2, &selector_values).unwrap();
        image.data[6..8].copy_from_slice(&31_u16.to_le_bytes());
        let (_, changed) =
            table_candidates_for_domain(&image, 0x8001_0002, 2, &selector_values).unwrap();

        assert_eq!(first, BTreeSet::from([17, 29]));
        assert_eq!(changed, BTreeSet::from([17, 31]));
    }
}
