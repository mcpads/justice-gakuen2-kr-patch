use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, bail, ensure};
use psx_r3000a::{Instruction, Register, decode};

use super::model::{StaticCatalogIndexMemoryLoadAudit, StaticDiscRecordLoadAudit};
use super::profiles::{MAIN_DISC_RECORD_LOADER_ADDRESS, StaticConsumerSourceRegionProfile};
use crate::disc::{RawTrack, iso9660::Iso9660};
use crate::psx_static_analysis::value_flow::ResolvedDirectCallArgument;
use crate::source_disc::{LoadedImage, MAIN_EXECUTABLE_PATH, SupportedSourceDisc};

const MAIN_CATALOG_FILE_OFFSET: usize = 0x76ee8;
const MAIN_CATALOG_ENTRY_SIZE: usize = 12;
const LOCAL_REGISTER_SCAN_INSTRUCTION_COUNT: usize = 96;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct DiscRecordLoaderForwarderProfile {
    pub(super) image_path: &'static str,
    pub(super) entrypoint_runtime_address: u32,
    pub(super) source_argument_register: Register,
    pub(super) saved_argument_register: Register,
    pub(super) save_runtime_address: u32,
    pub(super) loader_call_runtime_address: u32,
    pub(super) forward_runtime_address: u32,
    pub(super) forwarded_argument_register: Register,
    pub(super) mask: u16,
}

const DISC_RECORD_LOADER_FORWARDERS: &[DiscRecordLoaderForwarderProfile] =
    &[DiscRecordLoaderForwarderProfile {
        image_path: MAIN_EXECUTABLE_PATH,
        entrypoint_runtime_address: 0x8001_3748,
        source_argument_register: Register::A0,
        saved_argument_register: Register::S0,
        save_runtime_address: 0x8001_3750,
        loader_call_runtime_address: 0x8001_3788,
        forward_runtime_address: 0x8001_378c,
        forwarded_argument_register: Register::A1,
        mask: u16::MAX,
    }];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct DiscRecordLoaderBoundedIndexProfile {
    pub(super) image_path: &'static str,
    pub(super) call_runtime_address: u32,
    pub(super) validated_instructions: &'static [(u32, Instruction)],
    pub(super) catalog_indices: &'static [u16],
    pub(super) resolution: &'static str,
    pub(super) argument_producer: &'static str,
}

const CST_CATALOG_INDICES: &[u16] = &[
    76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91, 92, 93, 94, 95, 96, 97,
];

const CST_INDEX_INSTRUCTIONS: &[(u32, Instruction)] = &[
    (
        0x8002_7f24,
        Instruction::Lui {
            rt: Register::V0,
            immediate: 0x801f,
        },
    ),
    (
        0x8002_7f28,
        Instruction::Lbu {
            rt: Register::V0,
            base: Register::V0,
            offset: 0x64bd,
        },
    ),
    (
        0x8002_7f30,
        Instruction::Sltiu {
            rt: Register::V0,
            rs: Register::V0,
            immediate: 22,
        },
    ),
    (
        0x8002_7f34,
        Instruction::Bne {
            rs: Register::V0,
            rt: Register::ZERO,
            target: 0x8002_7f48,
        },
    ),
    (
        0x8002_7f3c,
        Instruction::Addiu {
            rt: Register::V0,
            rs: Register::ZERO,
            immediate: 21,
        },
    ),
    (
        0x8002_7f40,
        Instruction::Lui {
            rt: Register::AT,
            immediate: 0x801f,
        },
    ),
    (
        0x8002_7f44,
        Instruction::Sb {
            rt: Register::V0,
            base: Register::AT,
            offset: 0x64bd,
        },
    ),
    (
        0x8002_7f48,
        Instruction::Lui {
            rt: Register::A1,
            immediate: 0x801f,
        },
    ),
    (
        0x8002_7f4c,
        Instruction::Lbu {
            rt: Register::A1,
            base: Register::A1,
            offset: 0x64bd,
        },
    ),
    (
        0x8002_7f58,
        Instruction::Addiu {
            rt: Register::A1,
            rs: Register::A1,
            immediate: 76,
        },
    ),
];

const DISC_RECORD_LOADER_BOUNDED_INDICES: &[DiscRecordLoaderBoundedIndexProfile] =
    &[DiscRecordLoaderBoundedIndexProfile {
        image_path: MAIN_EXECUTABLE_PATH,
        call_runtime_address: 0x8002_7f54,
        validated_instructions: CST_INDEX_INSTRUCTIONS,
        catalog_indices: CST_CATALOG_INDICES,
        resolution: "validated_bounded_range",
        argument_producer: "clamped_byte_plus_literal",
    }];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct DiscRecordLoaderOwnerCallProfile {
    pub(super) call_runtime_address: u32,
    pub(super) destination_runtime_address: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct DiscRecordLoaderOwnerProfile {
    pub(super) image_path: &'static str,
    pub(super) owner_id: &'static str,
    pub(super) function_entry_runtime_address: u32,
    pub(super) loader_calls: &'static [DiscRecordLoaderOwnerCallProfile],
    pub(super) direct_caller_runtime_addresses: &'static [u32],
}

#[derive(Clone, Copy)]
pub(super) struct DiscRecordLoaderProfiles<'a> {
    pub(super) forwarders: &'a [DiscRecordLoaderForwarderProfile],
    pub(super) bounded_indices: &'a [DiscRecordLoaderBoundedIndexProfile],
    pub(super) owners: &'a [DiscRecordLoaderOwnerProfile],
}

const DISC_RECORD_LOADER_OWNERS: &[DiscRecordLoaderOwnerProfile] = &[
    DiscRecordLoaderOwnerProfile {
        image_path: MAIN_EXECUTABLE_PATH,
        owner_id: "four_slot_resource_setup",
        function_entry_runtime_address: 0x8002_8238,
        loader_calls: &[
            owner_call(0x8002_828c, 0x800d_4000),
            owner_call(0x8002_8300, 0x800d_4000),
            owner_call(0x8002_8344, 0x8015_2000),
            owner_call(0x8002_83b8, 0x8015_2000),
        ],
        direct_caller_runtime_addresses: &[0x8002_51ec, 0x8002_8180],
    },
    DiscRecordLoaderOwnerProfile {
        image_path: MAIN_EXECUTABLE_PATH,
        owner_id: "four_slot_resource_setup",
        function_entry_runtime_address: 0x8002_840c,
        loader_calls: &[
            owner_call(0x8002_8460, 0x8011_3000),
            owner_call(0x8002_84d4, 0x8011_3000),
            owner_call(0x8002_8528, 0x8017_a000),
            owner_call(0x8002_859c, 0x8017_a000),
        ],
        direct_caller_runtime_addresses: &[0x8002_5208, 0x8002_8188],
    },
    DiscRecordLoaderOwnerProfile {
        image_path: MAIN_EXECUTABLE_PATH,
        owner_id: "four_slot_resource_setup",
        function_entry_runtime_address: 0x8002_8808,
        loader_calls: &[
            owner_call(0x8002_8880, 0x800d_4000),
            owner_call(0x8002_8910, 0x800d_4000),
        ],
        direct_caller_runtime_addresses: &[0x8002_831c],
    },
    DiscRecordLoaderOwnerProfile {
        image_path: MAIN_EXECUTABLE_PATH,
        owner_id: "four_slot_resource_setup",
        function_entry_runtime_address: 0x8002_89a4,
        loader_calls: &[
            owner_call(0x8002_8a1c, 0x8011_3000),
            owner_call(0x8002_8aac, 0x8011_3000),
        ],
        direct_caller_runtime_addresses: &[0x8002_8500],
    },
    DiscRecordLoaderOwnerProfile {
        image_path: MAIN_EXECUTABLE_PATH,
        owner_id: "four_slot_resource_setup",
        function_entry_runtime_address: 0x8002_8b54,
        loader_calls: &[
            owner_call(0x8002_8bcc, 0x8015_2000),
            owner_call(0x8002_8c5c, 0x8015_2000),
        ],
        direct_caller_runtime_addresses: &[0x8002_83e4],
    },
    DiscRecordLoaderOwnerProfile {
        image_path: MAIN_EXECUTABLE_PATH,
        owner_id: "four_slot_resource_setup",
        function_entry_runtime_address: 0x8002_8d04,
        loader_calls: &[
            owner_call(0x8002_8d7c, 0x8017_a000),
            owner_call(0x8002_8e0c, 0x8017_a000),
        ],
        direct_caller_runtime_addresses: &[0x8002_85c8],
    },
];

const fn owner_call(
    call_runtime_address: u32,
    destination_runtime_address: u32,
) -> DiscRecordLoaderOwnerCallProfile {
    DiscRecordLoaderOwnerCallProfile {
        call_runtime_address,
        destination_runtime_address,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AffineRegisterValue {
    constant: u32,
    selector: Option<AffineSelector>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AffineSelector {
    register: Register,
    stride: u32,
    origin: SelectorOrigin,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SelectorOrigin {
    LocalScanBoundary,
    ControlFlowBoundary {
        instruction_offset: usize,
    },
    MemoryLoad {
        instruction_offset: usize,
        kind: &'static str,
    },
}

pub(super) struct SourceCatalog {
    executable: Vec<u8>,
    record_paths_by_entry: BTreeMap<[u8; 12], Vec<String>>,
}

impl SourceCatalog {
    pub(super) fn read(source: &SupportedSourceDisc) -> Result<Self> {
        let (_, executable) = source.read_record(MAIN_EXECUTABLE_PATH)?;
        let mut track = RawTrack::open(source.image_path())?;
        let files = {
            let mut iso = Iso9660::open(&mut track)?;
            iso.files()?
        };
        let mut record_paths_by_entry = BTreeMap::<_, Vec<_>>::new();
        for (path, record) in files {
            if record.size < 4 {
                continue;
            }
            let (first_sector, _) = track.user_sector(
                record
                    .extent_lba
                    .checked_add(u32::from(record.extended_attribute_blocks))
                    .context("source record data LBA overflow")?,
            )?;
            let mut entry = [0; MAIN_CATALOG_ENTRY_SIZE];
            entry[..8].copy_from_slice(&catalog_key(record.extent_lba, record.size)?);
            entry[8..].copy_from_slice(&first_sector[..4]);
            record_paths_by_entry.entry(entry).or_default().push(path);
        }
        Ok(Self {
            executable,
            record_paths_by_entry,
        })
    }

    pub(super) fn record_paths(&self, index: u16) -> Vec<String> {
        self.entry(index)
            .and_then(|entry| {
                let entry: [u8; MAIN_CATALOG_ENTRY_SIZE] = entry.try_into().ok()?;
                self.record_paths_by_entry.get(&entry)
            })
            .cloned()
            .unwrap_or_default()
    }

    fn entry(&self, index: u16) -> Option<&[u8]> {
        let offset =
            MAIN_CATALOG_FILE_OFFSET.checked_add(usize::from(index) * MAIN_CATALOG_ENTRY_SIZE)?;
        self.executable
            .get(offset..offset.checked_add(MAIN_CATALOG_ENTRY_SIZE)?)
    }
}

pub(super) fn validate_source_catalog_bindings(
    source: &SupportedSourceDisc,
    catalog: &SourceCatalog,
    profiles: &[StaticConsumerSourceRegionProfile],
) -> Result<()> {
    let mut path_by_index = BTreeMap::new();
    let mut verified = BTreeSet::new();
    for profile in profiles {
        if let Some(previous) =
            path_by_index.insert(profile.source_catalog_index, profile.source_record_path)
        {
            ensure!(
                previous == profile.source_record_path,
                "source catalog index {} is assigned to both {} and {}",
                profile.source_catalog_index,
                previous,
                profile.source_record_path
            );
        }
        if !verified.insert((profile.source_record_path, profile.source_catalog_index)) {
            continue;
        }
        let (record, stored) = source.read_record(profile.source_record_path)?;
        let entry = catalog
            .entry(profile.source_catalog_index)
            .with_context(|| {
                format!(
                    "source catalog entry {} for {} leaves the main executable",
                    profile.source_catalog_index, profile.source_record_path
                )
            })?;
        ensure!(
            entry[..8] == catalog_key(record.extent_lba, record.size)?,
            "source catalog index {} no longer identifies {}",
            profile.source_catalog_index,
            profile.source_record_path
        );
        ensure!(
            catalog
                .record_paths(profile.source_catalog_index)
                .iter()
                .any(|path| path == profile.source_record_path),
            "source catalog index {} does not resolve back to {} in the exact ISO file population",
            profile.source_catalog_index,
            profile.source_record_path
        );
        ensure!(
            stored
                .get(..4)
                .is_some_and(|first_word| first_word == &entry[8..12]),
            "source catalog entry {} first-word oracle changed for {}",
            profile.source_catalog_index,
            profile.source_record_path
        );
    }
    Ok(())
}

pub(super) fn disc_record_load_call_candidates(
    image: &LoadedImage,
    reachable_instruction_offsets: &BTreeSet<usize>,
    direct_call_arguments: &[ResolvedDirectCallArgument],
    catalog: &SourceCatalog,
    profiles: &[StaticConsumerSourceRegionProfile],
) -> Result<Vec<StaticDiscRecordLoadAudit>> {
    disc_record_load_call_candidates_with_profiles(
        image,
        reachable_instruction_offsets,
        direct_call_arguments,
        catalog,
        profiles,
        DiscRecordLoaderProfiles {
            forwarders: DISC_RECORD_LOADER_FORWARDERS,
            bounded_indices: DISC_RECORD_LOADER_BOUNDED_INDICES,
            owners: DISC_RECORD_LOADER_OWNERS,
        },
    )
}

pub(super) fn disc_record_load_call_candidates_with_profiles(
    image: &LoadedImage,
    reachable_instruction_offsets: &BTreeSet<usize>,
    direct_call_arguments: &[ResolvedDirectCallArgument],
    catalog: &SourceCatalog,
    profiles: &[StaticConsumerSourceRegionProfile],
    analysis_profiles: DiscRecordLoaderProfiles<'_>,
) -> Result<Vec<StaticDiscRecordLoadAudit>> {
    let Some(runtime_base) = image.runtime_base else {
        return Ok(Vec::new());
    };
    validate_loader_owner_profiles(image, runtime_base, analysis_profiles.owners)?;
    let mut candidates = Vec::new();
    for instruction_offset in (0..image.data.len().saturating_sub(3)).step_by(4) {
        let Some(instruction) = decode_at(image, runtime_base, instruction_offset) else {
            continue;
        };
        if instruction
            != (Instruction::Jal {
                target: MAIN_DISC_RECORD_LOADER_ADDRESS,
            })
        {
            continue;
        }
        let delay_slot = decode_at(image, runtime_base, instruction_offset + 4);
        let literal_index = delay_slot.as_ref().and_then(catalog_index_literal);
        let preceding = instruction_offset
            .checked_sub(4)
            .and_then(|offset| decode_at(image, runtime_base, offset));
        let preceding_literal_index = (delay_slot
            .as_ref()
            .and_then(|instruction| instruction.written_gpr())
            != Some(Register::A1))
        .then(|| preceding.as_ref().and_then(catalog_index_literal))
        .flatten();
        let value_flow_indices = direct_call_arguments
            .iter()
            .filter(|argument| {
                argument.instruction_offset == instruction_offset
                    && argument.target == MAIN_DISC_RECORD_LOADER_ADDRESS
                    && argument.argument_register == Register::A1
            })
            .filter_map(|argument| u16::try_from(argument.value).ok())
            .collect::<BTreeSet<_>>();
        let forwarding = forwarded_catalog_indices(
            image,
            runtime_base,
            instruction_offset,
            analysis_profiles.forwarders,
        )?;
        let bounded = bounded_catalog_indices(
            image,
            runtime_base,
            instruction_offset,
            analysis_profiles.bounded_indices,
        )?;
        let (catalog_indices, resolution, argument_producer) = if let Some(index) = literal_index {
            (vec![index], "delay_slot_literal", "delay_slot_literal")
        } else if let Some(index) = preceding_literal_index {
            (vec![index], "preceding_literal", "preceding_literal")
        } else if !value_flow_indices.is_empty() {
            (
                value_flow_indices.into_iter().collect(),
                "value_flow",
                "reachable_value_flow",
            )
        } else if !forwarding.catalog_indices.is_empty() {
            (
                forwarding.catalog_indices.iter().copied().collect(),
                "forwarded_call_argument",
                "validated_wrapper_call_argument",
            )
        } else if let Some(profile) = bounded {
            (
                profile.catalog_indices.to_vec(),
                profile.resolution,
                profile.argument_producer,
            )
        } else {
            (
                Vec::new(),
                "unresolved",
                unresolved_argument_producer(delay_slot.as_ref(), preceding.as_ref()),
            )
        };
        let catalog_record_paths = catalog_indices
            .iter()
            .flat_map(|index| catalog.record_paths(*index))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let declared_source_region_paths = catalog_indices
            .iter()
            .flat_map(|index| {
                profiles
                    .iter()
                    .filter(move |profile| profile.source_catalog_index == *index)
                    .map(|profile| profile.source_record_path.to_string())
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let memory_load = preceding.as_ref().and_then(|instruction| {
            catalog_index_memory_load_audit(
                image,
                runtime_base,
                instruction_offset.checked_sub(4)?,
                instruction,
            )
        });
        let static_owner = loader_owner(
            image,
            runtime_base,
            instruction_offset,
            analysis_profiles.owners,
        );
        let static_owner_destination_runtime_address = static_owner.and_then(|profile| {
            profile
                .loader_calls
                .iter()
                .find(|call| call.call_runtime_address == runtime_base + instruction_offset as u32)
                .map(|call| format!("0x{:08x}", call.destination_runtime_address))
        });
        candidates.push(StaticDiscRecordLoadAudit {
            call_runtime_address: format!("0x{:08x}", runtime_base + instruction_offset as u32),
            instruction_offset: format!("0x{instruction_offset:x}"),
            catalog_index_candidates: catalog_indices,
            catalog_index_resolution: resolution.to_string(),
            catalog_index_argument_producer: argument_producer.to_string(),
            catalog_index_memory_load: memory_load,
            forwarding_call_runtime_addresses: forwarding
                .call_instruction_offsets
                .iter()
                .map(|offset| format!("0x{:08x}", runtime_base + *offset as u32))
                .collect(),
            static_owner_id: static_owner.map(|profile| profile.owner_id.to_string()),
            static_owner_function_runtime_address: static_owner
                .map(|profile| format!("0x{:08x}", profile.function_entry_runtime_address)),
            static_owner_destination_runtime_address,
            static_owner_direct_caller_runtime_addresses: static_owner
                .into_iter()
                .flat_map(|profile| profile.direct_caller_runtime_addresses)
                .map(|address| format!("0x{address:08x}"))
                .collect(),
            catalog_record_paths,
            declared_source_region_paths,
            entrypoint_reachable: reachable_instruction_offsets.contains(&instruction_offset),
            evidence: "full_loaded_image_direct_jal_candidate".to_string(),
        });
    }
    Ok(candidates)
}

fn bounded_catalog_indices<'a>(
    image: &LoadedImage,
    runtime_base: u32,
    loader_call_instruction_offset: usize,
    profiles: &'a [DiscRecordLoaderBoundedIndexProfile],
) -> Result<Option<&'a DiscRecordLoaderBoundedIndexProfile>> {
    let loader_call_runtime_address = runtime_base
        .checked_add(u32::try_from(loader_call_instruction_offset)?)
        .context("bounded disc-loader call runtime address overflow")?;
    let matching = profiles
        .iter()
        .filter(|profile| {
            profile.image_path == image.path
                && profile.call_runtime_address == loader_call_runtime_address
        })
        .collect::<Vec<_>>();
    ensure!(
        matching.len() <= 1,
        "multiple bounded catalog-index profiles own {loader_call_runtime_address:#010x}"
    );
    let Some(profile) = matching.into_iter().next() else {
        return Ok(None);
    };
    ensure!(
        !profile.catalog_indices.is_empty(),
        "bounded catalog-index profile is empty at {loader_call_runtime_address:#010x}"
    );
    let unique_indices = profile
        .catalog_indices
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    ensure!(
        unique_indices.len() == profile.catalog_indices.len(),
        "bounded catalog-index profile repeats an index at {loader_call_runtime_address:#010x}"
    );
    for (runtime_address, expected) in profile.validated_instructions {
        let instruction_offset = runtime_address
            .checked_sub(runtime_base)
            .and_then(|offset| usize::try_from(offset).ok())
            .with_context(|| {
                format!(
                    "bounded catalog-index instruction {runtime_address:#010x} leaves {}",
                    image.path
                )
            })?;
        ensure!(
            decode_at(image, runtime_base, instruction_offset).as_ref() == Some(expected),
            "bounded catalog-index instruction changed at {runtime_address:#010x}"
        );
    }
    Ok(Some(profile))
}

fn validate_loader_owner_profiles(
    image: &LoadedImage,
    runtime_base: u32,
    profiles: &[DiscRecordLoaderOwnerProfile],
) -> Result<()> {
    let mut owned_loader_calls = BTreeSet::new();
    for profile in profiles
        .iter()
        .filter(|profile| profile.image_path == image.path)
    {
        ensure!(
            !profile.owner_id.is_empty(),
            "disc-loader owner profile has an empty owner id"
        );
        ensure!(
            !profile.loader_calls.is_empty(),
            "disc-loader owner {} has no loader calls",
            profile.owner_id
        );
        for call in profile.loader_calls {
            let call_runtime_address = call.call_runtime_address;
            ensure!(
                owned_loader_calls.insert(call_runtime_address),
                "disc-loader call {call_runtime_address:#010x} has multiple static owners"
            );
            let instruction_offset = call_runtime_address
                .checked_sub(runtime_base)
                .and_then(|offset| usize::try_from(offset).ok())
                .with_context(|| {
                    format!(
                        "owned disc-loader call {call_runtime_address:#010x} leaves {}",
                        image.path
                    )
                })?;
            ensure!(
                decode_at(image, runtime_base, instruction_offset)
                    == Some(Instruction::Jal {
                        target: MAIN_DISC_RECORD_LOADER_ADDRESS,
                    }),
                "owned disc-loader call changed at {call_runtime_address:#010x}"
            );
            let delay_slot = decode_at(image, runtime_base, instruction_offset + 4);
            let destination_low = match delay_slot {
                Some(Instruction::Ori {
                    rt: Register::A0,
                    rs: Register::A0,
                    immediate,
                }) => immediate,
                _ => {
                    bail!(
                        "owned disc-loader destination delay slot changed at {call_runtime_address:#010x}"
                    )
                }
            };
            let destination_base = resolve_affine_register_before(
                image,
                runtime_base,
                instruction_offset,
                Register::A0,
                0,
            )
            .with_context(|| {
                format!(
                    "owned disc-loader destination base is unresolved at {call_runtime_address:#010x}"
                )
            })?;
            ensure!(
                destination_base.selector.is_none(),
                "owned disc-loader destination base is not constant at {call_runtime_address:#010x}"
            );
            let actual_destination = destination_base.constant | u32::from(destination_low);
            ensure!(
                actual_destination == call.destination_runtime_address,
                "owned disc-loader destination changed at {call_runtime_address:#010x}: expected {:#010x}, found {actual_destination:#010x}",
                call.destination_runtime_address
            );
        }

        let actual_callers = direct_call_runtime_addresses(
            image,
            runtime_base,
            profile.function_entry_runtime_address,
        );
        let expected_callers = profile
            .direct_caller_runtime_addresses
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        ensure!(
            actual_callers == expected_callers,
            "direct caller census changed for disc-loader owner {} at {:#010x}: expected {:?}, found {:?}",
            profile.owner_id,
            profile.function_entry_runtime_address,
            expected_callers,
            actual_callers
        );
    }
    Ok(())
}

fn direct_call_runtime_addresses(
    image: &LoadedImage,
    runtime_base: u32,
    target: u32,
) -> BTreeSet<u32> {
    (0..image.data.len().saturating_sub(3))
        .step_by(4)
        .filter(|offset| {
            decode_at(image, runtime_base, *offset) == Some(Instruction::Jal { target })
        })
        .filter_map(|offset| runtime_base.checked_add(u32::try_from(offset).ok()?))
        .collect()
}

fn loader_owner<'a>(
    image: &LoadedImage,
    runtime_base: u32,
    loader_call_instruction_offset: usize,
    profiles: &'a [DiscRecordLoaderOwnerProfile],
) -> Option<&'a DiscRecordLoaderOwnerProfile> {
    let loader_call_runtime_address =
        runtime_base.checked_add(u32::try_from(loader_call_instruction_offset).ok()?)?;
    profiles.iter().find(|profile| {
        profile.image_path == image.path
            && profile
                .loader_calls
                .iter()
                .any(|call| call.call_runtime_address == loader_call_runtime_address)
    })
}

#[derive(Default)]
struct ForwardedCatalogIndices {
    catalog_indices: BTreeSet<u16>,
    call_instruction_offsets: BTreeSet<usize>,
}

fn forwarded_catalog_indices(
    image: &LoadedImage,
    runtime_base: u32,
    loader_call_instruction_offset: usize,
    profiles: &[DiscRecordLoaderForwarderProfile],
) -> Result<ForwardedCatalogIndices> {
    let loader_call_runtime_address = runtime_base
        .checked_add(u32::try_from(loader_call_instruction_offset)?)
        .context("disc-loader call runtime address overflow")?;
    let mut output = ForwardedCatalogIndices::default();
    for profile in profiles.iter().filter(|profile| {
        profile.image_path == image.path
            && profile.loader_call_runtime_address == loader_call_runtime_address
    }) {
        validate_forwarder(image, runtime_base, profile)?;
        for caller_offset in (0..image.data.len().saturating_sub(7)).step_by(4) {
            if decode_at(image, runtime_base, caller_offset)
                != Some(Instruction::Jal {
                    target: profile.entrypoint_runtime_address,
                })
            {
                continue;
            }
            let delay = decode_at(image, runtime_base, caller_offset + 4);
            let preceding = caller_offset
                .checked_sub(4)
                .and_then(|offset| decode_at(image, runtime_base, offset));
            let literal = delay
                .as_ref()
                .and_then(|instruction| {
                    register_literal(instruction, profile.source_argument_register)
                })
                .or_else(|| {
                    (delay.as_ref().and_then(Instruction::written_gpr)
                        != Some(profile.source_argument_register))
                    .then(|| {
                        preceding.as_ref().and_then(|instruction| {
                            register_literal(instruction, profile.source_argument_register)
                        })
                    })
                    .flatten()
                });
            let Some(value) = literal else {
                continue;
            };
            output
                .catalog_indices
                .insert(u16::try_from(value & u32::from(profile.mask))?);
            output.call_instruction_offsets.insert(caller_offset);
        }
    }
    Ok(output)
}

fn validate_forwarder(
    image: &LoadedImage,
    runtime_base: u32,
    profile: &DiscRecordLoaderForwarderProfile,
) -> Result<()> {
    let offset = |address: u32| {
        address
            .checked_sub(runtime_base)
            .and_then(|offset| usize::try_from(offset).ok())
            .context("disc-loader forwarder leaves its loaded image")
    };
    ensure!(
        decode_at(image, runtime_base, offset(profile.save_runtime_address)?)
            == Some(Instruction::Addu {
                rd: profile.saved_argument_register,
                rs: profile.source_argument_register,
                rt: Register::ZERO,
            }),
        "disc-loader forwarder argument save changed at {:#010x}",
        profile.save_runtime_address
    );
    ensure!(
        decode_at(
            image,
            runtime_base,
            offset(profile.loader_call_runtime_address)?
        ) == Some(Instruction::Jal {
            target: MAIN_DISC_RECORD_LOADER_ADDRESS,
        }),
        "disc-loader forwarder call changed at {:#010x}",
        profile.loader_call_runtime_address
    );
    ensure!(
        decode_at(
            image,
            runtime_base,
            offset(profile.forward_runtime_address)?
        ) == Some(Instruction::Andi {
            rt: profile.forwarded_argument_register,
            rs: profile.saved_argument_register,
            immediate: profile.mask,
        }),
        "disc-loader forwarder argument transfer changed at {:#010x}",
        profile.forward_runtime_address
    );
    Ok(())
}

fn catalog_index_memory_load_audit(
    image: &LoadedImage,
    runtime_base: u32,
    load_instruction_offset: usize,
    instruction: &Instruction,
) -> Option<StaticCatalogIndexMemoryLoadAudit> {
    let (base, displacement, load_kind) = match instruction {
        Instruction::Lh {
            rt: Register::A1,
            base,
            offset,
        } => (*base, *offset, "signed_halfword"),
        Instruction::Lhu {
            rt: Register::A1,
            base,
            offset,
        } => (*base, *offset, "unsigned_halfword"),
        _ => return None,
    };
    let affine =
        resolve_affine_register_before(image, runtime_base, load_instruction_offset, base, 0).map(
            |mut value| {
                value.constant = value.constant.wrapping_add_signed(i32::from(displacement));
                value
            },
        );
    let selector = affine.and_then(|value| value.selector);
    Some(StaticCatalogIndexMemoryLoadAudit {
        load_runtime_address: format!(
            "0x{:08x}",
            runtime_base + u32::try_from(load_instruction_offset).ok()?
        ),
        load_kind: load_kind.to_string(),
        address_resolution: if affine.is_some() {
            "affine"
        } else {
            "unresolved"
        }
        .to_string(),
        effective_address_when_selector_zero: affine
            .map(|value| format!("0x{:08x}", value.constant)),
        selector_register: selector.map(|selector| register_name(selector.register).to_string()),
        selector_stride_bytes: selector.map(|selector| selector.stride),
        selector_origin_kind: selector.map(|selector| match selector.origin {
            SelectorOrigin::LocalScanBoundary => "local_scan_boundary".to_string(),
            SelectorOrigin::ControlFlowBoundary { .. } => "control_flow_boundary".to_string(),
            SelectorOrigin::MemoryLoad { kind, .. } => kind.to_string(),
        }),
        selector_origin_runtime_address: selector.and_then(|selector| match selector.origin {
            SelectorOrigin::LocalScanBoundary => None,
            SelectorOrigin::ControlFlowBoundary { instruction_offset }
            | SelectorOrigin::MemoryLoad {
                instruction_offset, ..
            } => Some(format!(
                "0x{:08x}",
                runtime_base + u32::try_from(instruction_offset).ok()?
            )),
        }),
        declared_selector_domain: None,
        custom_record_selector: None,
    })
}

fn resolve_affine_register_before(
    image: &LoadedImage,
    runtime_base: u32,
    before: usize,
    register: Register,
    depth: usize,
) -> Option<AffineRegisterValue> {
    if register == Register::ZERO {
        return Some(AffineRegisterValue {
            constant: 0,
            selector: None,
        });
    }
    if depth > 12 {
        return None;
    }
    let first = before.saturating_sub(LOCAL_REGISTER_SCAN_INSTRUCTION_COUNT * 4);
    for offset in (first..before).step_by(4).rev() {
        let instruction = decode_at(image, runtime_base, offset)?;
        if is_linked_call(&instruction) {
            if is_call_clobbered(register) {
                return None;
            }
            continue;
        }
        if instruction.has_delay_slot() {
            return Some(boundary_selector(
                register,
                SelectorOrigin::ControlFlowBoundary {
                    instruction_offset: offset,
                },
            ));
        }
        if instruction.written_gpr() != Some(register) {
            continue;
        }
        let source =
            |source| resolve_affine_register_before(image, runtime_base, offset, source, depth + 1);
        return match instruction {
            Instruction::Lui { immediate, .. } => Some(AffineRegisterValue {
                constant: u32::from(immediate) << 16,
                selector: None,
            }),
            Instruction::Addi { rs, immediate, .. } | Instruction::Addiu { rs, immediate, .. } => {
                source(rs).map(|mut value| {
                    value.constant = value.constant.wrapping_add_signed(i32::from(immediate));
                    value
                })
            }
            Instruction::Sll { rt, shift, .. } => source(rt)
                .and_then(|value| scale_affine(value, 1_u32.checked_shl(u32::from(shift))?)),
            Instruction::Addu { rs, rt, .. } | Instruction::Add { rs, rt, .. } => {
                combine_affine(source(rs)?, source(rt)?)
            }
            Instruction::Lb { rt, .. } => Some(memory_selector(rt, offset, "signed_byte_load")),
            Instruction::Lbu { rt, .. } => Some(memory_selector(rt, offset, "unsigned_byte_load")),
            Instruction::Lh { rt, .. } => Some(memory_selector(rt, offset, "signed_halfword_load")),
            Instruction::Lhu { rt, .. } => {
                Some(memory_selector(rt, offset, "unsigned_halfword_load"))
            }
            Instruction::Lw { rt, .. } => Some(memory_selector(rt, offset, "word_load")),
            _ => None,
        };
    }
    Some(boundary_selector(
        register,
        SelectorOrigin::LocalScanBoundary,
    ))
}

fn boundary_selector(register: Register, origin: SelectorOrigin) -> AffineRegisterValue {
    AffineRegisterValue {
        constant: 0,
        selector: Some(AffineSelector {
            register,
            stride: 1,
            origin,
        }),
    }
}

fn memory_selector(
    register: Register,
    instruction_offset: usize,
    kind: &'static str,
) -> AffineRegisterValue {
    AffineRegisterValue {
        constant: 0,
        selector: Some(AffineSelector {
            register,
            stride: 1,
            origin: SelectorOrigin::MemoryLoad {
                instruction_offset,
                kind,
            },
        }),
    }
}

fn scale_affine(mut value: AffineRegisterValue, scale: u32) -> Option<AffineRegisterValue> {
    value.constant = value.constant.wrapping_mul(scale);
    if let Some(selector) = &mut value.selector {
        selector.stride = selector.stride.checked_mul(scale)?;
    }
    Some(value)
}

fn combine_affine(
    left: AffineRegisterValue,
    right: AffineRegisterValue,
) -> Option<AffineRegisterValue> {
    let selector = match (left.selector, right.selector) {
        (None, selector) | (selector, None) => selector,
        (Some(left), Some(right))
            if left.register == right.register && left.origin == right.origin =>
        {
            Some(AffineSelector {
                stride: left.stride.checked_add(right.stride)?,
                ..left
            })
        }
        (Some(_), Some(_)) => return None,
    };
    Some(AffineRegisterValue {
        constant: left.constant.wrapping_add(right.constant),
        selector,
    })
}

fn is_linked_call(instruction: &Instruction) -> bool {
    matches!(
        instruction,
        Instruction::Jal { .. }
            | Instruction::Jalr { .. }
            | Instruction::Bltzal { .. }
            | Instruction::Bgezal { .. }
    )
}

fn is_call_clobbered(register: Register) -> bool {
    matches!(
        register,
        Register::AT
            | Register::V0
            | Register::V1
            | Register::A0
            | Register::A1
            | Register::A2
            | Register::A3
            | Register::T0
            | Register::T1
            | Register::T2
            | Register::T3
            | Register::T4
            | Register::T5
            | Register::T6
            | Register::T7
            | Register::T8
            | Register::T9
            | Register::K0
            | Register::K1
            | Register::RA
    )
}

fn register_name(register: Register) -> &'static str {
    match register {
        Register::ZERO => "zero",
        Register::AT => "at",
        Register::V0 => "v0",
        Register::V1 => "v1",
        Register::A0 => "a0",
        Register::A1 => "a1",
        Register::A2 => "a2",
        Register::A3 => "a3",
        Register::T0 => "t0",
        Register::T1 => "t1",
        Register::T2 => "t2",
        Register::T3 => "t3",
        Register::T4 => "t4",
        Register::T5 => "t5",
        Register::T6 => "t6",
        Register::T7 => "t7",
        Register::S0 => "s0",
        Register::S1 => "s1",
        Register::S2 => "s2",
        Register::S3 => "s3",
        Register::S4 => "s4",
        Register::S5 => "s5",
        Register::S6 => "s6",
        Register::S7 => "s7",
        Register::T8 => "t8",
        Register::T9 => "t9",
        Register::K0 => "k0",
        Register::K1 => "k1",
        Register::GP => "gp",
        Register::SP => "sp",
        Register::FP => "fp",
        Register::RA => "ra",
        _ => "out_of_profile",
    }
}

fn unresolved_argument_producer(
    delay_slot: Option<&Instruction>,
    preceding: Option<&Instruction>,
) -> &'static str {
    if delay_slot.and_then(|instruction| instruction.written_gpr()) == Some(Register::A1) {
        return "delay_slot_computation";
    }
    match preceding {
        Some(
            Instruction::Lb {
                rt: Register::A1, ..
            }
            | Instruction::Lbu {
                rt: Register::A1, ..
            }
            | Instruction::Lh {
                rt: Register::A1, ..
            }
            | Instruction::Lhu {
                rt: Register::A1, ..
            }
            | Instruction::Lw {
                rt: Register::A1, ..
            }
            | Instruction::Lwl {
                rt: Register::A1, ..
            }
            | Instruction::Lwr {
                rt: Register::A1, ..
            },
        ) => "preceding_memory_load",
        Some(instruction) if instruction.written_gpr() == Some(Register::A1) => {
            "preceding_computation"
        }
        _ => "no_local_argument_writer",
    }
}

fn decode_at(image: &LoadedImage, runtime_base: u32, offset: usize) -> Option<Instruction> {
    let word = image.data.get(offset..offset.checked_add(4)?)?;
    decode(
        u32::from_le_bytes(word.try_into().ok()?),
        runtime_base.checked_add(u32::try_from(offset).ok()?)?,
    )
    .ok()
}

fn catalog_index_literal(instruction: &Instruction) -> Option<u16> {
    register_literal(instruction, Register::A1).and_then(|value| u16::try_from(value).ok())
}

fn register_literal(instruction: &Instruction, register: Register) -> Option<u32> {
    match instruction {
        Instruction::Addiu {
            rt,
            rs: Register::ZERO,
            immediate,
        } if *rt == register => u32::try_from(*immediate).ok(),
        Instruction::Ori {
            rt,
            rs: Register::ZERO,
            immediate,
        } if *rt == register => Some(u32::from(*immediate)),
        _ => None,
    }
}

fn catalog_key(extent_lba: u32, byte_count: u32) -> Result<[u8; 8]> {
    let absolute_sector = extent_lba
        .checked_add(150)
        .context("source catalog absolute sector overflow")?;
    let minute = absolute_sector / (75 * 60);
    let remainder = absolute_sector % (75 * 60);
    let second = remainder / 75;
    let frame = remainder % 75;
    let mut key = [0u8; 8];
    key[..4].copy_from_slice(&[bcd(minute)?, bcd(second)?, bcd(frame)?, 0]);
    key[4..].copy_from_slice(&byte_count.to_le_bytes());
    Ok(key)
}

fn bcd(value: u32) -> Result<u8> {
    ensure!(
        value <= 99,
        "source catalog BCD value is too large: {value}"
    );
    Ok(u8::try_from(((value / 10) << 4) | (value % 10))?)
}

#[cfg(test)]
impl SourceCatalog {
    pub(super) fn fixture(index: u16, path: &str) -> Self {
        let entry = [1, 2, 3, 0, 4, 5, 6, 7, 8, 9, 10, 11];
        let offset = MAIN_CATALOG_FILE_OFFSET + usize::from(index) * MAIN_CATALOG_ENTRY_SIZE;
        let mut executable = vec![0; offset + MAIN_CATALOG_ENTRY_SIZE];
        executable[offset..offset + MAIN_CATALOG_ENTRY_SIZE].copy_from_slice(&entry);
        Self {
            executable,
            record_paths_by_entry: BTreeMap::from([(entry, vec![path.to_string()])]),
        }
    }
}
