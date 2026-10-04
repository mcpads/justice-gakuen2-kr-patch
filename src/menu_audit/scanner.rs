use std::collections::{BTreeMap, BTreeSet};

#[cfg(test)]
use crate::psx_static_analysis::value_flow::scan_derived_address_flow_with_budget;
use crate::psx_static_analysis::value_flow::{
    AddressFlowSeedExhaustion, DerivedAddressKind, ResolvedDirectCallArgument,
};

pub(super) const OVERLAY_BASE: u32 = 0x800a_2000;
pub(super) const MAX_STRING_LENGTH: usize = 64;
pub(super) const MENU_CODE_LIMIT: u16 = 0x03ff;
pub(super) const MENU_CODE_MASK: u16 = 0x0fff;
pub(super) const SKIP_CODE: u16 = 0x0fff;
pub(super) const ATLAS_CODE_COUNT: usize = 0x0400;

#[derive(Debug, Default)]
pub(super) struct ReferenceIndex {
    references: BTreeMap<usize, ReferenceLocations>,
    address_flow_seed_count: usize,
    address_flow_instruction_state_count: usize,
    address_flow_budget_exhausted_seed_count: usize,
    address_flow_budget_exhausted_seed_offsets: Vec<usize>,
    address_flow_budget_exhausted_seeds: Vec<AddressFlowSeedExhaustion>,
    resolved_direct_call_arguments: Vec<ResolvedDirectCallArgument>,
}

impl ReferenceIndex {
    pub(super) fn pointer_reference_count(&self) -> usize {
        self.references
            .values()
            .map(|locations| locations.pointer_offsets.len())
            .sum()
    }

    pub(super) fn address_materialization_reference_count(&self) -> usize {
        self.references
            .values()
            .map(|locations| locations.address_materialization_references.len())
            .sum()
    }

    pub(super) fn memory_access_reference_count(&self) -> usize {
        self.references
            .values()
            .map(|locations| locations.memory_access_references.len())
            .sum()
    }

    pub(super) fn loaded_word_reference_count(&self) -> usize {
        self.references
            .values()
            .map(|locations| locations.loaded_word_references.len())
            .sum()
    }

    pub(super) fn address_flow_seed_count(&self) -> usize {
        self.address_flow_seed_count
    }

    pub(super) fn address_flow_instruction_state_count(&self) -> usize {
        self.address_flow_instruction_state_count
    }

    pub(super) fn address_flow_budget_exhausted_seed_count(&self) -> usize {
        self.address_flow_budget_exhausted_seed_count
    }

    pub(super) fn address_flow_budget_exhausted_seed_offsets(&self) -> &[usize] {
        &self.address_flow_budget_exhausted_seed_offsets
    }

    pub(super) fn address_flow_budget_exhausted_seeds(&self) -> &[AddressFlowSeedExhaustion] {
        &self.address_flow_budget_exhausted_seeds
    }

    pub(super) fn resolved_direct_call_arguments(&self) -> &[ResolvedDirectCallArgument] {
        &self.resolved_direct_call_arguments
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct AddressFlowReference {
    pub(super) seed_offset: usize,
    pub(super) instruction_offset: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct LoadedWordReference {
    pub(super) seed_offset: usize,
    pub(super) instruction_offset: usize,
    pub(super) load_instruction_offset: usize,
    pub(super) storage_address: u32,
}

#[derive(Debug, Default)]
struct ReferenceLocations {
    pointer_offsets: Vec<usize>,
    address_materialization_references: Vec<AddressFlowReference>,
    memory_access_references: Vec<AddressFlowReference>,
    loaded_word_references: Vec<LoadedWordReference>,
}

#[derive(Debug)]
pub(super) struct CandidateString {
    pub(super) pointer_offsets: Vec<usize>,
    pub(super) address_materialization_references: Vec<AddressFlowReference>,
    pub(super) memory_access_references: Vec<AddressFlowReference>,
    pub(super) loaded_word_references: Vec<LoadedWordReference>,
    pub(super) shared_pointer_offsets: Vec<usize>,
    pub(super) shared_address_materialization_references: Vec<AddressFlowReference>,
    pub(super) shared_memory_access_references: Vec<AddressFlowReference>,
    pub(super) shared_loaded_word_references: Vec<LoadedWordReference>,
    pub(super) raw_codes: Vec<u16>,
}

#[cfg(test)]
pub(super) fn scan_overlay_with_shared_references_report(
    data: &[u8],
    shared_references: &ReferenceIndex,
    address_flow_state_budget: usize,
) -> (BTreeMap<usize, CandidateString>, ReferenceIndex) {
    let local_references = scan_references(
        data,
        OVERLAY_BASE,
        OVERLAY_BASE,
        data.len(),
        0,
        address_flow_state_budget,
    );
    index_overlay_with_shared_references(data, local_references, Some(shared_references))
}

pub(super) fn index_overlay_with_shared_references(
    data: &[u8],
    local_references: ReferenceIndex,
    shared_references: Option<&ReferenceIndex>,
) -> (BTreeMap<usize, CandidateString>, ReferenceIndex) {
    let target_offsets: BTreeSet<_> = local_references
        .references
        .keys()
        .chain(
            shared_references
                .into_iter()
                .flat_map(|references| references.references.keys()),
        )
        .copied()
        .collect();
    let mut candidates = BTreeMap::new();
    for target_offset in target_offsets {
        let Some(raw_codes) = candidate_codes(data, target_offset) else {
            continue;
        };
        let local = local_references.references.get(&target_offset);
        let shared =
            shared_references.and_then(|references| references.references.get(&target_offset));
        candidates.insert(
            target_offset,
            CandidateString {
                pointer_offsets: local
                    .map(|locations| locations.pointer_offsets.clone())
                    .unwrap_or_default(),
                address_materialization_references: local
                    .map(|locations| locations.address_materialization_references.clone())
                    .unwrap_or_default(),
                memory_access_references: local
                    .map(|locations| locations.memory_access_references.clone())
                    .unwrap_or_default(),
                loaded_word_references: local
                    .map(|locations| locations.loaded_word_references.clone())
                    .unwrap_or_default(),
                shared_pointer_offsets: shared
                    .map(|locations| locations.pointer_offsets.clone())
                    .unwrap_or_default(),
                shared_address_materialization_references: shared
                    .map(|locations| locations.address_materialization_references.clone())
                    .unwrap_or_default(),
                shared_memory_access_references: shared
                    .map(|locations| locations.memory_access_references.clone())
                    .unwrap_or_default(),
                shared_loaded_word_references: shared
                    .map(|locations| locations.loaded_word_references.clone())
                    .unwrap_or_default(),
                raw_codes,
            },
        );
    }
    (candidates, local_references)
}

#[cfg(test)]
pub(super) fn scan_references(
    data: &[u8],
    instruction_base: u32,
    target_runtime_base: u32,
    target_size: usize,
    source_offset_bias: usize,
    address_flow_state_budget: usize,
) -> ReferenceIndex {
    let mut references = BTreeMap::<usize, ReferenceLocations>::new();
    for pointer_offset in (0..data.len().saturating_sub(3)).step_by(4) {
        let pointer = aligned_word(data, pointer_offset);
        let Some(target_offset) =
            loaded_image_target_offset(pointer, target_runtime_base, target_size)
        else {
            continue;
        };
        references
            .entry(target_offset)
            .or_default()
            .pointer_offsets
            .push(source_offset_bias + pointer_offset);
    }

    let executable_domain = crate::psx_static_analysis::ExecutableDomain::full_image(data.len());
    let address_flow = scan_derived_address_flow_with_budget(
        data,
        instruction_base,
        &executable_domain,
        address_flow_state_budget,
    );
    index_references(
        target_runtime_base,
        target_size,
        source_offset_bias,
        &address_flow,
        references,
    )
}

pub(super) fn index_references_from_value_flow(
    data: &[u8],
    target_runtime_base: u32,
    target_size: usize,
    source_offset_bias: usize,
    address_flow: &crate::psx_static_analysis::value_flow::DerivedAddressScan,
) -> ReferenceIndex {
    let mut references = BTreeMap::<usize, ReferenceLocations>::new();
    for pointer_offset in (0..data.len().saturating_sub(3)).step_by(4) {
        let pointer = aligned_word(data, pointer_offset);
        let Some(target_offset) =
            loaded_image_target_offset(pointer, target_runtime_base, target_size)
        else {
            continue;
        };
        references
            .entry(target_offset)
            .or_default()
            .pointer_offsets
            .push(source_offset_bias + pointer_offset);
    }
    index_references(
        target_runtime_base,
        target_size,
        source_offset_bias,
        address_flow,
        references,
    )
}

fn index_references(
    target_runtime_base: u32,
    target_size: usize,
    source_offset_bias: usize,
    address_flow: &crate::psx_static_analysis::value_flow::DerivedAddressScan,
    mut references: BTreeMap<usize, ReferenceLocations>,
) -> ReferenceIndex {
    for derived_address in &address_flow.addresses {
        let Some(target_offset) =
            loaded_image_target_offset(derived_address.address, target_runtime_base, target_size)
        else {
            continue;
        };
        let locations = references.entry(target_offset).or_default();
        match derived_address.kind {
            DerivedAddressKind::RegisterValue => {
                locations
                    .address_materialization_references
                    .push(AddressFlowReference {
                        seed_offset: source_offset_bias + derived_address.seed_offset,
                        instruction_offset: source_offset_bias + derived_address.instruction_offset,
                    })
            }
            DerivedAddressKind::MemoryAccess => {
                locations
                    .memory_access_references
                    .push(AddressFlowReference {
                        seed_offset: source_offset_bias + derived_address.seed_offset,
                        instruction_offset: source_offset_bias + derived_address.instruction_offset,
                    })
            }
            DerivedAddressKind::LoadedValue {
                storage_address,
                load_instruction_offset,
                width_bytes: 4,
                ..
            } => locations.loaded_word_references.push(LoadedWordReference {
                seed_offset: source_offset_bias + derived_address.seed_offset,
                instruction_offset: source_offset_bias + derived_address.source_offset,
                load_instruction_offset: source_offset_bias + load_instruction_offset,
                storage_address,
            }),
            DerivedAddressKind::LoadedValue { .. } => {}
        }
    }
    ReferenceIndex {
        references,
        address_flow_seed_count: address_flow.seed_count,
        address_flow_instruction_state_count: address_flow.instruction_state_count,
        address_flow_budget_exhausted_seed_count: address_flow.budget_exhausted_seed_count,
        address_flow_budget_exhausted_seed_offsets: address_flow
            .budget_exhausted_seed_offsets
            .iter()
            .copied()
            .map(|offset| source_offset_bias + offset)
            .collect(),
        address_flow_budget_exhausted_seeds: address_flow
            .budget_exhausted_seeds
            .iter()
            .cloned()
            .map(|mut exhaustion| {
                exhaustion.seed_offset = source_offset_bias
                    .checked_add(exhaustion.seed_offset)
                    .expect("source offset bias fits usize");
                exhaustion
            })
            .collect(),
        resolved_direct_call_arguments: address_flow
            .resolved_direct_call_arguments
            .iter()
            .copied()
            .map(|mut argument| {
                argument.seed_offset = source_offset_bias
                    .checked_add(argument.seed_offset)
                    .expect("source offset bias fits usize");
                argument.instruction_offset = source_offset_bias
                    .checked_add(argument.instruction_offset)
                    .expect("source offset bias fits usize");
                argument
            })
            .collect(),
    }
}

fn loaded_image_target_offset(
    pointer: u32,
    target_runtime_base: u32,
    target_size: usize,
) -> Option<usize> {
    let target_offset = usize::try_from(pointer.checked_sub(target_runtime_base)?).ok()?;
    (target_offset.is_multiple_of(2) && target_offset + 2 <= target_size).then_some(target_offset)
}

pub(super) fn candidate_codes(data: &[u8], target_offset: usize) -> Option<Vec<u16>> {
    if target_offset + 2 > data.len() {
        return None;
    }
    let length = usize::from(u16::from_le_bytes(
        data[target_offset..target_offset + 2].try_into().ok()?,
    ));
    if !(1..=MAX_STRING_LENGTH).contains(&length) {
        return None;
    }
    let end = target_offset.checked_add(2 + length * 2)?;
    if end > data.len() {
        return None;
    }
    let raw_codes: Vec<_> = data[target_offset + 2..end]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();
    raw_codes
        .iter()
        .all(|raw_code| {
            let code = raw_code & MENU_CODE_MASK;
            code <= MENU_CODE_LIMIT || code == SKIP_CODE
        })
        .then_some(raw_codes)
}

fn aligned_word(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(
        data[offset..offset + 4]
            .try_into()
            .expect("bounded aligned instruction read"),
    )
}

pub(crate) fn wrapped_cells_overlap(left: u16, right: u16) -> bool {
    wrapped_cells_overlap_sized(left, 20, right, 20)
}

pub(crate) fn wrapped_cells_overlap_sized(
    left: u16,
    left_size: usize,
    right: u16,
    right_size: usize,
) -> bool {
    crate::menu_atlas::logical_regions_overlap(
        left, left_size, left_size, right, right_size, right_size,
    )
}
