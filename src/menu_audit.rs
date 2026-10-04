use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::cue::CueSheet;
use crate::pipeline::{BASELINE_BIN_SHA256, sha256_file};
use anyhow::{Context, Result, ensure};
use psx_r3000a::PROFILE_ID;

#[path = "menu_audit/candidate_references.rs"]
mod candidate_references;
#[path = "menu_audit/known_consumers.rs"]
mod known_consumers;
#[path = "menu_audit/known_non_text.rs"]
mod known_non_text;
#[path = "menu_audit/model.rs"]
mod model;
#[path = "menu_audit/reachable_references.rs"]
mod reachable_references;
#[path = "menu_audit/scanner.rs"]
mod scanner;
#[path = "menu_audit/sources.rs"]
mod sources;

#[cfg(test)]
#[path = "menu_audit/scanner_tests.rs"]
mod scanner_tests;

use crate::consumer_analysis::analyze_loaded_program;
use crate::psx_static_analysis::reachable_control_flow::UnresolvedRegisterTransferKind;
use candidate_references::{
    CandidateClassificationEvidence, address_materialization_reference_count,
    audit_string_candidate, loaded_word_reference_count, memory_access_reference_count,
};
use known_consumers::identify_known_consumer_evidence;
pub(crate) use known_consumers::{
    kanri_school_label_consumers, kanri_source_menu_codes, validate_mgtit_placement_renderer,
};
use known_non_text::identify_known_non_text_evidence;
use model::CodeAccumulation;
pub use model::{
    AddressFlowReferenceAudit, AddressFlowSeedExhaustionAudit, AddressFlowSourceAudit,
    BoundedJumpTableAudit, CodeReachabilityAudit, LoadedWordReferenceAudit, MenuCodeAuditConfig,
    MenuCodeAuditManifest, MenuConsumerEvidence, MenuNonTextEvidence, OverlayAudit, PageAudit,
    ProvisionalCodeCheck, ProvisionalCodeEvidence, ReachableLoadedWordReferenceAudit,
    ResolvedRegisterTransferAudit, SharedReferenceSourceAudit, SharedStringReferences,
    StringCandidate, UnresolvedRegisterTransferAudit, UsedCodeAudit,
};
use scanner::{
    ATLAS_CODE_COUNT, MAX_STRING_LENGTH, MENU_CODE_LIMIT, MENU_CODE_MASK, OVERLAY_BASE,
    ReferenceIndex, SKIP_CODE, index_overlay_with_shared_references,
    index_references_from_value_flow,
};
pub(crate) use scanner::{wrapped_cells_overlap, wrapped_cells_overlap_sized};
use sources::load_audit_inputs;

pub const DEFAULT_MENU_ADDRESS_FLOW_STATE_BUDGET: usize =
    crate::psx_static_analysis::value_flow::DEFAULT_VALUE_FLOW_STATE_BUDGET;

fn address_flow_seed_exhaustion_audit(
    exhaustion: &crate::psx_static_analysis::value_flow::AddressFlowSeedExhaustion,
) -> AddressFlowSeedExhaustionAudit {
    AddressFlowSeedExhaustionAudit {
        seed_offset: hex_offset(exhaustion.seed_offset),
        processed_state_count: exhaustion.processed_state_count,
        discovered_state_count: exhaustion.discovered_state_count,
        distinct_instruction_offset_count: exhaustion.distinct_instruction_offset_count,
        maximum_states_at_instruction_offset: exhaustion.maximum_states_at_instruction_offset,
        pending_state_count: exhaustion.pending_state_count,
        distinct_frontier_instruction_offset_count: exhaustion
            .distinct_frontier_instruction_offset_count,
    }
}

fn resolved_register_transfer_audits(
    transfers: &[crate::psx_static_analysis::value_flow::ResolvedRegisterTransfer],
    reachable_instruction_offsets: &BTreeSet<usize>,
    reachable_lui_seed_offsets: &BTreeSet<usize>,
    runtime_base: u32,
    loaded_size: usize,
) -> Vec<ResolvedRegisterTransferAudit> {
    transfers
        .iter()
        .filter(|transfer| {
            reachable_lui_seed_offsets.contains(&transfer.seed_offset)
                && reachable_instruction_offsets.contains(&transfer.instruction_offset)
        })
        .map(|transfer| ResolvedRegisterTransferAudit {
            seed_offset: hex_offset(transfer.seed_offset),
            instruction_offset: hex_offset(transfer.instruction_offset),
            target: format!("0x{:08x}", transfer.target),
            target_in_loaded_image: transfer
                .target
                .checked_sub(runtime_base)
                .and_then(|offset| usize::try_from(offset).ok())
                .is_some_and(|offset| offset.is_multiple_of(4) && offset + 4 <= loaded_size),
        })
        .collect()
}

pub fn audit_menu_codes(config: &MenuCodeAuditConfig) -> Result<MenuCodeAuditManifest> {
    let manifest = collect_menu_code_audit(&config.cue, config.address_flow_state_budget)?;
    if let Some(parent) = config.output.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    std::fs::write(
        &config.output,
        format!("{}\n", serde_json::to_string_pretty(&manifest)?),
    )
    .with_context(|| format!("failed to write {}", config.output.display()))?;
    Ok(manifest)
}

pub fn collect_menu_code_audit(
    cue_path: &Path,
    address_flow_state_budget: usize,
) -> Result<MenuCodeAuditManifest> {
    ensure!(
        address_flow_state_budget > 0,
        "address-flow state budget must be positive"
    );
    let cue = CueSheet::parse(cue_path)?;
    let source_bin_sha256 = sha256_file(&cue.image_path)?;
    ensure!(
        source_bin_sha256 == BASELINE_BIN_SHA256,
        "unsupported source BIN SHA-256: {source_bin_sha256}"
    );

    let inputs = load_audit_inputs(&cue.image_path, address_flow_state_budget)?;

    let mut overlays = Vec::new();
    let mut used_codes: BTreeMap<u16, CodeAccumulation> = BTreeMap::new();
    let mut all_control_nibbles = BTreeSet::new();
    let mut candidate_pointer_reference_count = 0usize;
    let mut candidate_address_materialization_reference_count = 0usize;
    let mut candidate_memory_access_reference_count = 0usize;
    let mut candidate_loaded_word_reference_count = 0usize;
    let mut candidate_entrypoint_reachable_string_count = 0usize;
    let mut candidate_entrypoint_reachable_address_materialization_reference_count = 0usize;
    let mut candidate_entrypoint_reachable_memory_access_reference_count = 0usize;
    let mut candidate_entrypoint_reachable_loaded_word_reference_count = 0usize;
    let mut candidate_entrypoint_reachable_direct_pointer_load_string_count = 0usize;
    let mut candidate_entrypoint_reachable_direct_pointer_load_reference_count = 0usize;
    let mut candidate_shared_pointer_reference_count = 0usize;
    let mut candidate_shared_address_materialization_reference_count = 0usize;
    let mut candidate_shared_memory_access_reference_count = 0usize;
    let mut candidate_shared_loaded_word_reference_count = 0usize;
    let mut dat1_address_flow_seed_count = 0usize;
    let mut dat1_address_flow_instruction_state_count = 0usize;
    let mut dat1_address_flow_budget_exhausted_seed_count = 0usize;
    let mut dat1_address_flow_sources = Vec::with_capacity(inputs.overlays.len());
    let mut candidate_string_count = 0usize;
    let mut consumer_confirmed_string_count = 0usize;
    let mut consumer_confirmed_reference_count = 0usize;
    let mut confirmed_non_text_candidate_count = 0usize;
    let mut unclassified_entrypoint_reachable_direct_pointer_load_candidate_count = 0usize;
    let mut code_occurrence_count = 0usize;
    let mut skip_occurrence_count = 0usize;

    for overlay in &inputs.overlays {
        let program_analysis = analyze_loaded_program(overlay, address_flow_state_budget)?;
        let local_references = match (overlay.runtime_base, program_analysis.as_ref()) {
            (Some(runtime_base), Some(analysis)) => index_references_from_value_flow(
                &overlay.data,
                runtime_base,
                overlay.data.len(),
                0,
                &analysis.value_flow,
            ),
            _ => ReferenceIndex::default(),
        };
        let shared_references =
            (overlay.runtime_base == Some(OVERLAY_BASE)).then_some(&inputs.shared_references);
        let (candidates, local_references) = index_overlay_with_shared_references(
            &overlay.data,
            local_references,
            shared_references,
        );
        let resolved_register_transfers = program_analysis
            .as_ref()
            .map(|analysis| analysis.value_flow.resolved_register_transfers.clone())
            .unwrap_or_default();
        let declared_entrypoints = overlay.entrypoints.clone();
        let code_reachability = program_analysis.map(|analysis| analysis.closure);
        let reachable_instruction_offsets = code_reachability
            .as_ref()
            .map(|closure| closure.scan.instruction_offsets.clone())
            .unwrap_or_default();
        let reachable_lui_seed_offsets = code_reachability
            .as_ref()
            .map(|closure| closure.scan.lui_instruction_offsets.clone())
            .unwrap_or_default();
        let mut known_consumer_evidence = identify_known_consumer_evidence(
            &overlay.path,
            &overlay.data,
            overlay.runtime_base,
            &reachable_instruction_offsets,
            &reachable_lui_seed_offsets,
            local_references.resolved_direct_call_arguments(),
        )?;
        let mut known_non_text_evidence = identify_known_non_text_evidence(
            &overlay.path,
            &overlay.data,
            overlay.runtime_base,
            &reachable_instruction_offsets,
        )?;
        dat1_address_flow_seed_count += local_references.address_flow_seed_count();
        dat1_address_flow_instruction_state_count +=
            local_references.address_flow_instruction_state_count();
        dat1_address_flow_budget_exhausted_seed_count +=
            local_references.address_flow_budget_exhausted_seed_count();
        dat1_address_flow_sources.push(AddressFlowSourceAudit {
            path: overlay.path.clone(),
            file_size: overlay.data.len(),
            seed_count: local_references.address_flow_seed_count(),
            instruction_state_count: local_references.address_flow_instruction_state_count(),
            budget_exhausted_seed_count: local_references
                .address_flow_budget_exhausted_seed_count(),
            budget_exhausted_seed_offsets: local_references
                .address_flow_budget_exhausted_seed_offsets()
                .iter()
                .copied()
                .map(hex_offset)
                .collect(),
            budget_exhausted_seeds: local_references
                .address_flow_budget_exhausted_seeds()
                .iter()
                .map(address_flow_seed_exhaustion_audit)
                .collect(),
            code_reachability: code_reachability
                .map(|closure| {
                    let scan = closure.scan;
                    let runtime_base = overlay
                        .runtime_base
                        .expect("code reachability requires a runtime base");
                    let resolved_transfers = resolved_register_transfer_audits(
                        &resolved_register_transfers,
                        &scan.instruction_offsets,
                        &scan.lui_instruction_offsets,
                        runtime_base,
                        overlay.data.len(),
                    );
                    let resolved_transfer_instruction_count = resolved_transfers
                        .iter()
                        .map(|transfer| &transfer.instruction_offset)
                        .collect::<BTreeSet<_>>()
                        .len();
                    let unresolved_indirect_jump_count = scan
                        .unresolved_indirect_transfers
                        .values()
                        .filter(|transfer| transfer.kind == UnresolvedRegisterTransferKind::Jump)
                        .count();
                    let unresolved_linked_indirect_call_count = scan
                        .unresolved_indirect_transfers
                        .values()
                        .filter(|transfer| {
                            transfer.kind == UnresolvedRegisterTransferKind::LinkedCall
                        })
                        .count();
                    let bounded_jump_table_entry_count = closure
                        .bounded_jump_tables
                        .values()
                        .map(|table| table.targets.len())
                        .sum();
                    let bounded_jump_table_unique_target_count = closure
                        .bounded_jump_tables
                        .values()
                        .flat_map(|table| table.targets.iter().copied())
                        .collect::<BTreeSet<_>>()
                        .len();
                    let bounded_jump_tables = closure
                        .bounded_jump_tables
                        .into_values()
                        .map(|table| {
                            let unique_target_count =
                                table.targets.iter().copied().collect::<BTreeSet<_>>().len();
                            BoundedJumpTableAudit {
                                bound_instruction_offset: hex_offset(
                                    table.bound_instruction_offset,
                                ),
                                transfer_instruction_offset: hex_offset(
                                    table.transfer_instruction_offset,
                                ),
                                selector_register: table.selector_register.to_string(),
                                table_address: format!("0x{:08x}", table.table_address),
                                table_offset: hex_offset(table.table_offset),
                                entry_count: table.targets.len(),
                                unique_target_count,
                                targets: table
                                    .targets
                                    .into_iter()
                                    .map(|target| format!("0x{target:08x}"))
                                    .collect(),
                            }
                        })
                        .collect::<Vec<_>>();
                    let bounded_jump_table_count = bounded_jump_tables.len();
                    CodeReachabilityAudit {
                        runtime_base: overlay
                            .runtime_base
                            .map(|runtime_base| format!("0x{runtime_base:08x}")),
                        declared_entrypoints: declared_entrypoints
                            .iter()
                            .map(|entrypoint| format!("0x{:08x}", entrypoint.runtime_address))
                            .collect(),
                        declared_entrypoints_in_loaded_image: true,
                        reachable_instruction_count: scan.instruction_offsets.len(),
                        reachable_lui_seed_count: scan.lui_instruction_offsets.len(),
                        decode_failure_count: scan.decode_failure_offsets.len(),
                        unresolved_indirect_transfer_count: scan
                            .unresolved_indirect_transfer_offsets
                            .len(),
                        unresolved_indirect_transfer_offsets: scan
                            .unresolved_indirect_transfer_offsets
                            .into_iter()
                            .map(hex_offset)
                            .collect(),
                        unresolved_indirect_transfers: scan
                            .unresolved_indirect_transfers
                            .into_iter()
                            .map(
                                |(instruction_offset, transfer)| UnresolvedRegisterTransferAudit {
                                    instruction_offset: hex_offset(instruction_offset),
                                    source_register: transfer.source_register.to_string(),
                                    transfer_kind: match transfer.kind {
                                        UnresolvedRegisterTransferKind::Jump => "jump",
                                        UnresolvedRegisterTransferKind::LinkedCall => "linked_call",
                                    }
                                    .to_string(),
                                },
                            )
                            .collect(),
                        unresolved_indirect_jump_count,
                        unresolved_linked_indirect_call_count,
                        bounded_jump_table_count,
                        bounded_jump_table_entry_count,
                        bounded_jump_table_unique_target_count,
                        bounded_jump_tables,
                        abi_return_count: scan.abi_return_offsets.len(),
                        abi_return_offsets: scan
                            .abi_return_offsets
                            .into_iter()
                            .map(hex_offset)
                            .collect(),
                        reachable_seed_resolved_register_transfer_count: resolved_transfers.len(),
                        reachable_seed_resolved_register_transfer_instruction_count:
                            resolved_transfer_instruction_count,
                        reachable_seed_resolved_register_transfers: resolved_transfers,
                        control_transfer_resolution_pass_count: closure
                            .control_transfer_resolution_pass_count,
                        outside_image_transfer_target_count: scan
                            .outside_image_transfer_targets
                            .len(),
                        control_transfer_in_delay_slot_count: scan
                            .control_transfer_in_delay_slot_offsets
                            .len(),
                    }
                })
                .unwrap_or(CodeReachabilityAudit {
                    runtime_base: overlay
                        .runtime_base
                        .map(|runtime_base| format!("0x{runtime_base:08x}")),
                    declared_entrypoints: declared_entrypoints
                        .iter()
                        .map(|entrypoint| format!("0x{:08x}", entrypoint.runtime_address))
                        .collect(),
                    declared_entrypoints_in_loaded_image: false,
                    reachable_instruction_count: 0,
                    reachable_lui_seed_count: 0,
                    decode_failure_count: 0,
                    unresolved_indirect_transfer_count: 0,
                    unresolved_indirect_transfer_offsets: Vec::new(),
                    unresolved_indirect_transfers: Vec::new(),
                    unresolved_indirect_jump_count: 0,
                    unresolved_linked_indirect_call_count: 0,
                    bounded_jump_table_count: 0,
                    bounded_jump_table_entry_count: 0,
                    bounded_jump_table_unique_target_count: 0,
                    bounded_jump_tables: Vec::new(),
                    abi_return_count: 0,
                    abi_return_offsets: Vec::new(),
                    reachable_seed_resolved_register_transfer_count: 0,
                    reachable_seed_resolved_register_transfer_instruction_count: 0,
                    reachable_seed_resolved_register_transfers: Vec::new(),
                    control_transfer_resolution_pass_count: 0,
                    outside_image_transfer_target_count: 0,
                    control_transfer_in_delay_slot_count: 0,
                }),
        });
        if candidates.is_empty() {
            continue;
        }

        let mut overlay_unique_codes = BTreeSet::new();
        let mut overlay_control_nibbles = BTreeSet::new();
        let mut overlay_code_occurrence_count = 0usize;
        let mut overlay_skip_occurrence_count = 0usize;
        let mut strings = Vec::with_capacity(candidates.len());

        for (target_offset, candidate) in candidates {
            candidate_string_count += 1;
            for &raw_code in &candidate.raw_codes {
                let control_nibble = (raw_code >> 12) as u8;
                let code = raw_code & MENU_CODE_MASK;
                overlay_control_nibbles.insert(control_nibble);
                all_control_nibbles.insert(control_nibble);
                if code == SKIP_CODE {
                    overlay_skip_occurrence_count += 1;
                    skip_occurrence_count += 1;
                    continue;
                }
                overlay_code_occurrence_count += 1;
                code_occurrence_count += 1;
                overlay_unique_codes.insert(code);
                let accumulation = used_codes.entry(code).or_default();
                accumulation.occurrence_count += 1;
                accumulation.overlays.insert(overlay.path.clone());
            }
            let audited = audit_string_candidate(
                target_offset,
                candidate,
                CandidateClassificationEvidence {
                    consumer: known_consumer_evidence
                        .remove(&target_offset)
                        .unwrap_or_default(),
                    non_text: known_non_text_evidence
                        .remove(&target_offset)
                        .unwrap_or_default(),
                },
                &inputs.shared_reference_source.path,
                overlay.runtime_base,
                &reachable_instruction_offsets,
                &reachable_lui_seed_offsets,
            );
            ensure!(
                audited.candidate.consumer_reference_count == 0
                    || audited.candidate.non_text_evidence.is_empty(),
                "menu candidate cannot be both a renderer consumer and confirmed non-text in {}",
                overlay.path
            );
            let counts = audited.reference_counts;
            consumer_confirmed_string_count +=
                usize::from(audited.candidate.consumer_reference_count > 0);
            consumer_confirmed_reference_count += audited.candidate.consumer_reference_count;
            confirmed_non_text_candidate_count +=
                usize::from(!audited.candidate.non_text_evidence.is_empty());
            unclassified_entrypoint_reachable_direct_pointer_load_candidate_count += usize::from(
                audited.candidate.consumer_reference_count == 0
                    && audited.candidate.non_text_evidence.is_empty()
                    && !audited
                        .candidate
                        .entrypoint_reachable_direct_pointer_load_references
                        .is_empty(),
            );
            candidate_pointer_reference_count += counts.pointer;
            candidate_address_materialization_reference_count += counts.address_materialization;
            candidate_memory_access_reference_count += counts.memory_access;
            candidate_loaded_word_reference_count += counts.loaded_word;
            candidate_entrypoint_reachable_string_count += counts.entrypoint_reachable_string;
            candidate_entrypoint_reachable_address_materialization_reference_count +=
                counts.entrypoint_reachable_address_materialization;
            candidate_entrypoint_reachable_memory_access_reference_count +=
                counts.entrypoint_reachable_memory_access;
            candidate_entrypoint_reachable_loaded_word_reference_count +=
                counts.entrypoint_reachable_loaded_word;
            candidate_entrypoint_reachable_direct_pointer_load_string_count +=
                counts.entrypoint_reachable_direct_pointer_load_string;
            candidate_entrypoint_reachable_direct_pointer_load_reference_count +=
                counts.entrypoint_reachable_direct_pointer_load;
            candidate_shared_pointer_reference_count += counts.shared_pointer;
            candidate_shared_address_materialization_reference_count +=
                counts.shared_address_materialization;
            candidate_shared_memory_access_reference_count += counts.shared_memory_access;
            candidate_shared_loaded_word_reference_count += counts.shared_loaded_word;
            strings.push(audited.candidate);
        }
        ensure!(
            known_consumer_evidence.is_empty(),
            "known consumer evidence did not resolve to an admissible candidate in {}",
            overlay.path
        );
        ensure!(
            known_non_text_evidence.is_empty(),
            "known non-text evidence did not resolve to an admissible candidate in {}",
            overlay.path
        );

        overlays.push(OverlayAudit {
            path: overlay.path.clone(),
            file_size: overlay.data.len(),
            consumer_confirmed_string_count: strings
                .iter()
                .filter(|string| string.consumer_reference_count > 0)
                .count(),
            consumer_confirmed_reference_count: strings
                .iter()
                .map(|string| string.consumer_reference_count)
                .sum(),
            confirmed_non_text_candidate_count: strings
                .iter()
                .filter(|string| !string.non_text_evidence.is_empty())
                .count(),
            unclassified_entrypoint_reachable_direct_pointer_load_candidate_count: strings
                .iter()
                .filter(|string| {
                    string.consumer_reference_count == 0
                        && string.non_text_evidence.is_empty()
                        && !string
                            .entrypoint_reachable_direct_pointer_load_references
                            .is_empty()
                })
                .count(),
            address_flow_seed_count: local_references.address_flow_seed_count(),
            address_flow_instruction_state_count: local_references
                .address_flow_instruction_state_count(),
            address_flow_budget_exhausted_seed_count: local_references
                .address_flow_budget_exhausted_seed_count(),
            candidate_pointer_reference_count: strings
                .iter()
                .map(|string| string.pointer_offsets.len())
                .sum(),
            candidate_address_materialization_reference_count: strings
                .iter()
                .map(|string| string.address_materialization_offsets.len())
                .sum(),
            candidate_memory_access_reference_count: strings
                .iter()
                .map(|string| string.memory_access_offsets.len())
                .sum(),
            candidate_loaded_word_reference_count: strings
                .iter()
                .map(|string| string.loaded_word_references.len())
                .sum(),
            candidate_entrypoint_reachable_string_count: strings
                .iter()
                .filter(|string| {
                    !string
                        .entrypoint_reachable_address_materialization_references
                        .is_empty()
                        || !string
                            .entrypoint_reachable_memory_access_references
                            .is_empty()
                        || !string
                            .entrypoint_reachable_loaded_word_references
                            .is_empty()
                })
                .count(),
            candidate_entrypoint_reachable_address_materialization_reference_count: strings
                .iter()
                .map(|string| {
                    string
                        .entrypoint_reachable_address_materialization_references
                        .len()
                })
                .sum(),
            candidate_entrypoint_reachable_memory_access_reference_count: strings
                .iter()
                .map(|string| string.entrypoint_reachable_memory_access_references.len())
                .sum(),
            candidate_entrypoint_reachable_loaded_word_reference_count: strings
                .iter()
                .map(|string| string.entrypoint_reachable_loaded_word_references.len())
                .sum(),
            candidate_entrypoint_reachable_direct_pointer_load_string_count: strings
                .iter()
                .filter(|string| {
                    !string
                        .entrypoint_reachable_direct_pointer_load_references
                        .is_empty()
                })
                .count(),
            candidate_entrypoint_reachable_direct_pointer_load_reference_count: strings
                .iter()
                .map(|string| {
                    string
                        .entrypoint_reachable_direct_pointer_load_references
                        .len()
                })
                .sum(),
            candidate_shared_pointer_reference_count: strings
                .iter()
                .flat_map(|string| &string.shared_references)
                .map(|references| references.pointer_offsets.len())
                .sum(),
            candidate_shared_address_materialization_reference_count: strings
                .iter()
                .flat_map(|string| &string.shared_references)
                .map(|references| references.address_materialization_offsets.len())
                .sum(),
            candidate_shared_memory_access_reference_count: strings
                .iter()
                .flat_map(|string| &string.shared_references)
                .map(|references| references.memory_access_offsets.len())
                .sum(),
            candidate_shared_loaded_word_reference_count: strings
                .iter()
                .flat_map(|string| &string.shared_references)
                .map(|references| references.loaded_word_references.len())
                .sum(),
            candidate_string_count: strings.len(),
            code_occurrence_count: overlay_code_occurrence_count,
            skip_occurrence_count: overlay_skip_occurrence_count,
            unique_normalized_code_count: overlay_unique_codes.len(),
            control_nibbles: overlay_control_nibbles.into_iter().collect(),
            strings,
        });
    }

    let unreferenced_code_candidates: Vec<_> = (0..ATLAS_CODE_COUNT)
        .map(|code| code as u16)
        .filter(|code| !used_codes.contains_key(code))
        .map(hex_code)
        .collect();
    let statically_unreferenced_nonoverlapping_codes: Vec<_> = (0..ATLAS_CODE_COUNT)
        .map(|code| code as u16)
        .filter(|code| !used_codes.contains_key(code))
        .filter(|code| {
            used_codes
                .keys()
                .all(|used_code| !wrapped_cells_overlap(*code, *used_code))
        })
        .map(hex_code)
        .collect();
    let pages = (0u8..4)
        .map(|page| {
            let used_code_count = used_codes
                .keys()
                .filter(|code| (**code >> 8) as u8 == page)
                .count();
            PageAudit {
                page,
                used_code_count,
                unreferenced_candidate_count: 256 - used_code_count,
            }
        })
        .collect();
    let used_codes = used_codes
        .into_iter()
        .map(|(code, accumulation)| UsedCodeAudit {
            code: hex_code(code),
            occurrence_count: accumulation.occurrence_count,
            overlays: accumulation.overlays.into_iter().collect(),
        })
        .collect::<Vec<_>>();

    let manifest = MenuCodeAuditManifest {
        kind: "conservative DAT1 menu-code shared static-reference audit".to_string(),
        implementation: "independent Rust original-media analysis".to_string(),
        source_bin_sha256,
        instruction_profile: PROFILE_ID.to_string(),
        overlay_runtime_base: format!("0x{OVERLAY_BASE:08x}"),
        pointer_alignment: 4,
        instruction_alignment: 4,
        address_flow_state_budget,
        control_flow_policy: "typed R3000A static semantics; a conditional branch follows only its feasible successor when tracked operands prove the predicate and otherwise preserves both successors; direct same-image jumps and calls, every linked-transfer return site, resolved same-image register transfers, source-bounded same-image jump-table targets, and mandatory delay slots are followed while preserving path-distinct register states at shared instruction points; direct-call arguments are observed after the delay slot, while a load started in that slot is not admitted as a ready argument; address flow follows an ABI return path that preserves callee-saved tracked values and clears caller-saved values"
            .to_string(),
        address_materialization_operations: vec![
            "lui seed with bounded control-flow register-value propagation".to_string(),
            "addi/addiu, andi/ori/xori, slti/sltiu".to_string(),
            "add/addu, sub/subu, and/or/xor/nor, slt/sltu".to_string(),
            "sll/srl/sra and sllv/srlv/srav".to_string(),
        ],
        memory_access_operations: vec![
            "lb/lbu/lh/lhu/lw/lwl/lwr effective addresses".to_string(),
            "sb/sh/sw/swl/swr effective addresses".to_string(),
            "lwc2/swc2 effective addresses".to_string(),
        ],
        loaded_word_policy: "aligned LW values are resolved only from the same loaded image; direct references retain pointer storage and load coordinates, and derived values become visible only after one load-delay instruction; a delay-slot write to the destination discards the pending provenance"
            .to_string(),
        shared_reference_sources: vec![inputs.shared_reference_source],
        maximum_candidate_string_length: MAX_STRING_LENGTH,
        code_normalization: "raw_u16 & 0x0fff; upper nibble retained as a control value"
            .to_string(),
        accepted_normalized_codes: "0x0000..0x03ff plus skip 0x0fff".to_string(),
        dat1_bin_files_scanned: inputs.overlays.len(),
        dat1_address_flow_seed_count,
        dat1_address_flow_instruction_state_count,
        dat1_address_flow_budget_exhausted_seed_count,
        dat1_address_flow_sources,
        candidate_overlay_count: overlays.len(),
        consumer_confirmed_string_count,
        consumer_confirmed_reference_count,
        confirmed_non_text_candidate_count,
        unclassified_entrypoint_reachable_direct_pointer_load_candidate_count,
        candidate_pointer_reference_count,
        candidate_address_materialization_reference_count,
        candidate_memory_access_reference_count,
        candidate_loaded_word_reference_count,
        candidate_entrypoint_reachable_string_count,
        candidate_entrypoint_reachable_address_materialization_reference_count,
        candidate_entrypoint_reachable_memory_access_reference_count,
        candidate_entrypoint_reachable_loaded_word_reference_count,
        candidate_entrypoint_reachable_direct_pointer_load_string_count,
        candidate_entrypoint_reachable_direct_pointer_load_reference_count,
        candidate_shared_pointer_reference_count,
        candidate_shared_address_materialization_reference_count,
        candidate_shared_memory_access_reference_count,
        candidate_shared_loaded_word_reference_count,
        candidate_string_count,
        code_occurrence_count,
        skip_occurrence_count,
        used_code_count: used_codes.len(),
        unreferenced_code_candidate_count: unreferenced_code_candidates.len(),
        statically_unreferenced_nonoverlapping_code_count:
            statically_unreferenced_nonoverlapping_codes.len(),
        proven_reclaimable_code_count: 0,
        control_nibbles: all_control_nibbles.into_iter().collect(),
        pages,
        overlays,
        used_codes,
        unreferenced_code_candidates,
        statically_unreferenced_nonoverlapping_codes,
        limitations: vec![
            "A pointer-shaped reference is evidence of a candidate string, not proof that its overlay uses MENU.BIZ or the options renderer; shared executable references are conservatively applied to every overlay whose bytes form an admissible target.".to_string(),
            "Typed R3000A address scanning propagates LUI-derived constants through supported register arithmetic, direct control-flow successors, same-image direct calls and architectural return sites, resolved register targets, and source-bounded same-image jump tables under a per-seed flow-state budget. The ABI return path preserves callee-saved tracked values and clears caller-saved values. The scan preserves distinct register states when paths meet, records load/store effective addresses, and resolves aligned LW pointer values from the same loaded image after the architectural load delay. Unknown linked-call and external effects, stack-built arrays, runtime-generated glyph codes, and paths beyond the declared budget remain open.".to_string(),
            "An unreferenced code is not reclaimable until atlas-cell overlap and every consumer of the loaded MOJI2 image are classified.".to_string(),
        ],
    };

    Ok(manifest)
}

pub(crate) fn audit_provisional_codes(
    image_path: &Path,
    provisional_codes: &[u16],
) -> Result<ProvisionalCodeEvidence> {
    ensure!(
        !provisional_codes.is_empty(),
        "no provisional codes requested"
    );
    ensure!(
        provisional_codes
            .iter()
            .all(|code| *code <= MENU_CODE_LIMIT),
        "provisional code is outside the menu atlas domain"
    );
    ensure!(
        provisional_codes
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            .len()
            == provisional_codes.len(),
        "duplicate provisional code"
    );

    let inputs = load_audit_inputs(image_path, DEFAULT_MENU_ADDRESS_FLOW_STATE_BUDGET)?;

    let mut candidate_overlay_count = 0usize;
    let mut candidate_string_count = 0usize;
    let mut candidate_pointer_reference_count = 0usize;
    let mut candidate_address_materialization_reference_count = 0usize;
    let mut candidate_memory_access_reference_count = 0usize;
    let mut candidate_loaded_word_reference_count = 0usize;
    let mut candidate_shared_pointer_reference_count = 0usize;
    let mut candidate_shared_address_materialization_reference_count = 0usize;
    let mut candidate_shared_memory_access_reference_count = 0usize;
    let mut candidate_shared_loaded_word_reference_count = 0usize;
    let mut dat1_address_flow_seed_count = 0usize;
    let mut dat1_address_flow_instruction_state_count = 0usize;
    let mut dat1_address_flow_budget_exhausted_seed_count = 0usize;
    let mut references: BTreeMap<u16, BTreeSet<String>> = BTreeMap::new();
    for overlay in &inputs.overlays {
        let program_analysis =
            analyze_loaded_program(overlay, DEFAULT_MENU_ADDRESS_FLOW_STATE_BUDGET)?;
        let local_references = match (overlay.runtime_base, program_analysis.as_ref()) {
            (Some(runtime_base), Some(analysis)) => index_references_from_value_flow(
                &overlay.data,
                runtime_base,
                overlay.data.len(),
                0,
                &analysis.value_flow,
            ),
            _ => ReferenceIndex::default(),
        };
        let shared_references =
            (overlay.runtime_base == Some(OVERLAY_BASE)).then_some(&inputs.shared_references);
        let (candidates, local_references) = index_overlay_with_shared_references(
            &overlay.data,
            local_references,
            shared_references,
        );
        dat1_address_flow_seed_count += local_references.address_flow_seed_count();
        dat1_address_flow_instruction_state_count +=
            local_references.address_flow_instruction_state_count();
        dat1_address_flow_budget_exhausted_seed_count +=
            local_references.address_flow_budget_exhausted_seed_count();
        if candidates.is_empty() {
            continue;
        }
        candidate_overlay_count += 1;
        candidate_string_count += candidates.len();
        candidate_pointer_reference_count += candidates
            .values()
            .map(|candidate| candidate.pointer_offsets.len())
            .sum::<usize>();
        candidate_address_materialization_reference_count += candidates
            .values()
            .map(|candidate| address_materialization_reference_count(candidate, false))
            .sum::<usize>();
        candidate_memory_access_reference_count += candidates
            .values()
            .map(|candidate| memory_access_reference_count(candidate, false))
            .sum::<usize>();
        candidate_loaded_word_reference_count += candidates
            .values()
            .map(|candidate| loaded_word_reference_count(&candidate.loaded_word_references))
            .sum::<usize>();
        candidate_shared_pointer_reference_count += candidates
            .values()
            .map(|candidate| candidate.shared_pointer_offsets.len())
            .sum::<usize>();
        candidate_shared_address_materialization_reference_count += candidates
            .values()
            .map(|candidate| address_materialization_reference_count(candidate, true))
            .sum::<usize>();
        candidate_shared_memory_access_reference_count += candidates
            .values()
            .map(|candidate| memory_access_reference_count(candidate, true))
            .sum::<usize>();
        candidate_shared_loaded_word_reference_count += candidates
            .values()
            .map(|candidate| loaded_word_reference_count(&candidate.shared_loaded_word_references))
            .sum::<usize>();
        for candidate in candidates.values() {
            for raw_code in &candidate.raw_codes {
                let code = raw_code & MENU_CODE_MASK;
                if code != SKIP_CODE {
                    references
                        .entry(code)
                        .or_default()
                        .insert(overlay.path.clone());
                }
            }
        }
    }

    let used_codes: BTreeSet<_> = references.keys().copied().collect();
    let provisional_set: BTreeSet<_> = provisional_codes.iter().copied().collect();
    let checks = provisional_codes
        .iter()
        .map(|code| ProvisionalCodeCheck {
            code: hex_code(*code),
            static_reference_overlays: references
                .get(code)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .collect(),
            wrapped_cell_overlap_codes: used_codes
                .iter()
                .filter(|used| wrapped_cells_overlap(*code, **used))
                .copied()
                .map(hex_code)
                .collect(),
            other_candidate_overlap_codes: provisional_set
                .iter()
                .filter(|other| **other != *code && wrapped_cells_overlap(*code, **other))
                .copied()
                .map(hex_code)
                .collect(),
        })
        .collect();
    Ok(ProvisionalCodeEvidence {
        dat1_bin_files_scanned: inputs.overlays.len(),
        dat1_address_flow_seed_count,
        dat1_address_flow_instruction_state_count,
        dat1_address_flow_budget_exhausted_seed_count,
        shared_reference_sources: vec![inputs.shared_reference_source],
        candidate_overlay_count,
        candidate_string_count,
        candidate_pointer_reference_count,
        candidate_address_materialization_reference_count,
        candidate_memory_access_reference_count,
        candidate_loaded_word_reference_count,
        candidate_shared_pointer_reference_count,
        candidate_shared_address_materialization_reference_count,
        candidate_shared_memory_access_reference_count,
        candidate_shared_loaded_word_reference_count,
        checks,
    })
}

fn hex_offset(offset: usize) -> String {
    format!("0x{offset:04x}")
}

fn hex_code(code: u16) -> String {
    format!("0x{code:04x}")
}
