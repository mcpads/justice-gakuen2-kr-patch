use std::collections::{BTreeMap, BTreeSet, VecDeque};

use psx_r3000a::{Instruction, decode};

use super::ExecutableDomain;
use super::bounded_jump_tables::{BoundedJumpTable, read_bounded_jump_table};
use super::control_flow::{FlowTarget, flow_successors};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ReachabilityPoint {
    instruction_offset: usize,
    continuation: Option<ReachabilityContinuation>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ReachabilityContinuation {
    targets: Vec<u32>,
    fallthrough: Option<u32>,
    linked_return: Option<u32>,
}

#[derive(Debug, Default)]
pub(crate) struct ReachableInstructionScan {
    pub(crate) instruction_offsets: BTreeSet<usize>,
    pub(crate) lui_instruction_offsets: BTreeSet<usize>,
    pub(crate) decode_failure_offsets: BTreeSet<usize>,
    pub(crate) unresolved_indirect_transfer_offsets: BTreeSet<usize>,
    pub(crate) unresolved_indirect_transfers: BTreeMap<usize, UnresolvedRegisterTransfer>,
    pub(crate) abi_return_offsets: BTreeSet<usize>,
    pub(crate) outside_image_transfer_targets: BTreeSet<u32>,
    pub(crate) outside_executable_domain_transfer_targets: BTreeSet<u32>,
    pub(crate) control_transfer_in_delay_slot_offsets: BTreeSet<usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct UnresolvedRegisterTransfer {
    pub(crate) source_register: psx_r3000a::Register,
    pub(crate) kind: UnresolvedRegisterTransferKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UnresolvedRegisterTransferKind {
    Jump,
    LinkedCall,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct ResolvedRegisterTransfer {
    pub(crate) seed_offset: usize,
    pub(crate) instruction_offset: usize,
    pub(crate) target: u32,
}

#[derive(Debug)]
pub(crate) struct ReachableInstructionClosure {
    pub(crate) scan: ReachableInstructionScan,
    pub(crate) bounded_jump_tables: BTreeMap<usize, BoundedJumpTable>,
    pub(crate) control_transfer_resolution_pass_count: usize,
}

#[cfg(test)]
pub(crate) fn declared_overlay_entrypoint(
    data: &[u8],
    instruction_base: u32,
    executable_domain: &ExecutableDomain,
) -> Option<u32> {
    if !executable_domain.matches_image_len(data.len()) {
        return None;
    }
    let bytes: [u8; 4] = data.get(..4)?.try_into().ok()?;
    let entrypoint = u32::from_le_bytes(bytes);
    executable_domain
        .instruction_offset(instruction_base, entrypoint)
        .map(|_| entrypoint)
}

#[cfg(test)]
pub(crate) fn scan_reachable_instructions(
    data: &[u8],
    instruction_base: u32,
    executable_domain: &ExecutableDomain,
    entrypoint: u32,
) -> ReachableInstructionScan {
    scan_reachable_instructions_with_register_targets(
        data,
        instruction_base,
        executable_domain,
        entrypoint,
        &BTreeMap::new(),
    )
}

#[cfg(test)]
pub(crate) fn close_reachable_control_transfers(
    data: &[u8],
    instruction_base: u32,
    executable_domain: &ExecutableDomain,
    entrypoint: u32,
    resolved_register_transfers: &[ResolvedRegisterTransfer],
) -> ReachableInstructionClosure {
    close_reachable_control_transfers_from_entrypoints(
        data,
        instruction_base,
        executable_domain,
        &[entrypoint],
        resolved_register_transfers,
    )
}

#[cfg(test)]
pub(crate) fn close_reachable_control_transfers_from_entrypoints(
    data: &[u8],
    instruction_base: u32,
    executable_domain: &ExecutableDomain,
    entrypoints: &[u32],
    resolved_register_transfers: &[ResolvedRegisterTransfer],
) -> ReachableInstructionClosure {
    close_reachable_control_transfers_from_entrypoints_with_declared_targets(
        data,
        instruction_base,
        executable_domain,
        entrypoints,
        resolved_register_transfers,
        &BTreeMap::new(),
    )
}

pub(crate) fn close_reachable_control_transfers_from_entrypoints_with_declared_targets(
    data: &[u8],
    instruction_base: u32,
    executable_domain: &ExecutableDomain,
    entrypoints: &[u32],
    resolved_register_transfers: &[ResolvedRegisterTransfer],
    declared_register_targets: &BTreeMap<usize, BTreeSet<u32>>,
) -> ReachableInstructionClosure {
    let mut scan = scan_reachable_instructions_from_entrypoints_with_register_targets(
        data,
        instruction_base,
        executable_domain,
        entrypoints,
        declared_register_targets,
    );
    let mut admitted_register_targets = BTreeMap::new();
    let mut bounded_jump_tables = BTreeMap::new();
    let mut control_transfer_resolution_pass_count = 0usize;
    loop {
        let register_targets = resolved_register_transfers
            .iter()
            .filter(|transfer| {
                scan.lui_instruction_offsets.contains(&transfer.seed_offset)
                    && scan
                        .instruction_offsets
                        .contains(&transfer.instruction_offset)
            })
            .fold(
                BTreeMap::<usize, BTreeSet<u32>>::new(),
                |mut targets, transfer| {
                    targets
                        .entry(transfer.instruction_offset)
                        .or_default()
                        .insert(transfer.target);
                    targets
                },
            );
        let discovered_jump_tables = scan
            .unresolved_indirect_transfers
            .iter()
            .filter(|(_, transfer)| transfer.kind == UnresolvedRegisterTransferKind::Jump)
            .filter_map(|(&instruction_offset, _)| {
                read_bounded_jump_table(
                    data,
                    instruction_base,
                    executable_domain,
                    instruction_offset,
                )
                .map(|table| (instruction_offset, table))
            })
            .collect::<BTreeMap<_, _>>();
        let previous_jump_table_count = bounded_jump_tables.len();
        bounded_jump_tables.extend(discovered_jump_tables);
        if register_targets == admitted_register_targets
            && bounded_jump_tables.len() == previous_jump_table_count
        {
            break;
        }
        admitted_register_targets = register_targets;
        control_transfer_resolution_pass_count += 1;
        let mut admitted_control_targets = declared_register_targets.clone();
        for (&instruction_offset, targets) in &admitted_register_targets {
            admitted_control_targets
                .entry(instruction_offset)
                .or_default()
                .extend(targets.iter().copied());
        }
        for table in bounded_jump_tables.values() {
            admitted_control_targets
                .entry(table.transfer_instruction_offset)
                .or_default()
                .extend(table.targets.iter().copied());
        }
        scan = scan_reachable_instructions_from_entrypoints_with_register_targets(
            data,
            instruction_base,
            executable_domain,
            entrypoints,
            &admitted_control_targets,
        );
    }
    ReachableInstructionClosure {
        scan,
        bounded_jump_tables,
        control_transfer_resolution_pass_count,
    }
}

#[cfg(test)]
pub(crate) fn scan_reachable_instructions_with_register_targets(
    data: &[u8],
    instruction_base: u32,
    executable_domain: &ExecutableDomain,
    entrypoint: u32,
    register_targets: &BTreeMap<usize, BTreeSet<u32>>,
) -> ReachableInstructionScan {
    scan_reachable_instructions_from_entrypoints_with_register_targets(
        data,
        instruction_base,
        executable_domain,
        &[entrypoint],
        register_targets,
    )
}

fn scan_reachable_instructions_from_entrypoints_with_register_targets(
    data: &[u8],
    instruction_base: u32,
    executable_domain: &ExecutableDomain,
    entrypoints: &[u32],
    register_targets: &BTreeMap<usize, BTreeSet<u32>>,
) -> ReachableInstructionScan {
    if !executable_domain.matches_image_len(data.len()) {
        return ReachableInstructionScan::default();
    }
    let first = entrypoints
        .iter()
        .filter_map(|&entrypoint| {
            executable_domain
                .instruction_offset(instruction_base, entrypoint)
                .map(|instruction_offset| ReachabilityPoint {
                    instruction_offset,
                    continuation: None,
                })
        })
        .collect::<BTreeSet<_>>();
    if first.is_empty() {
        return ReachableInstructionScan::default();
    }
    let mut scan = ReachableInstructionScan::default();
    let mut seen = first.clone();
    let mut work = VecDeque::from_iter(first);
    while let Some(point) = work.pop_front() {
        scan.instruction_offsets.insert(point.instruction_offset);
        let pc = instruction_base.wrapping_add(point.instruction_offset as u32);
        let Ok(instruction) = decode(aligned_word(data, point.instruction_offset), pc) else {
            scan.decode_failure_offsets.insert(point.instruction_offset);
            continue;
        };
        if matches!(instruction, Instruction::Lui { .. }) {
            scan.lui_instruction_offsets
                .insert(point.instruction_offset);
        }
        let flow = flow_successors(&instruction, pc);
        if let Some(continuation) = point.continuation {
            if flow.delay_slot.is_some() {
                scan.control_transfer_in_delay_slot_offsets
                    .insert(point.instruction_offset);
                continue;
            }
            enqueue_continuation(
                instruction_base,
                executable_domain,
                continuation,
                &mut scan,
                &mut seen,
                &mut work,
            );
            continue;
        }

        let targets = match flow.target {
            Some(FlowTarget::Direct(target)) => vec![target],
            Some(FlowTarget::Register(register)) => {
                let targets: Vec<u32> = register_targets
                    .get(&point.instruction_offset)
                    .map(|targets| targets.iter().copied().collect())
                    .unwrap_or_default();
                if targets.is_empty() {
                    if register == psx_r3000a::Register::RA {
                        scan.abi_return_offsets.insert(point.instruction_offset);
                    } else {
                        scan.unresolved_indirect_transfer_offsets
                            .insert(point.instruction_offset);
                        scan.unresolved_indirect_transfers.insert(
                            point.instruction_offset,
                            UnresolvedRegisterTransfer {
                                source_register: register,
                                kind: match instruction {
                                    Instruction::Jr { .. } => UnresolvedRegisterTransferKind::Jump,
                                    Instruction::Jalr { .. } => {
                                        UnresolvedRegisterTransferKind::LinkedCall
                                    }
                                    _ => unreachable!("typed register transfer instruction"),
                                },
                            },
                        );
                    }
                }
                targets
            }
            None => Vec::new(),
        };
        let continuation = ReachabilityContinuation {
            targets,
            fallthrough: flow.fallthrough,
            linked_return: flow.return_site,
        };
        if let Some(delay_slot) = flow.delay_slot {
            enqueue_point(
                instruction_base,
                executable_domain,
                delay_slot,
                Some(continuation),
                &mut scan,
                &mut seen,
                &mut work,
            );
        } else {
            enqueue_continuation(
                instruction_base,
                executable_domain,
                continuation,
                &mut scan,
                &mut seen,
                &mut work,
            );
        }
    }
    scan
}

fn enqueue_continuation(
    instruction_base: u32,
    executable_domain: &ExecutableDomain,
    continuation: ReachabilityContinuation,
    scan: &mut ReachableInstructionScan,
    seen: &mut BTreeSet<ReachabilityPoint>,
    work: &mut VecDeque<ReachabilityPoint>,
) {
    for target in continuation.targets.iter().copied() {
        enqueue_point(
            instruction_base,
            executable_domain,
            target,
            None,
            scan,
            seen,
            work,
        );
    }
    if let Some(fallthrough) = continuation.fallthrough
        && !continuation.targets.contains(&fallthrough)
    {
        enqueue_point(
            instruction_base,
            executable_domain,
            fallthrough,
            None,
            scan,
            seen,
            work,
        );
    }
    if let Some(linked_return) = continuation.linked_return
        && !continuation.targets.contains(&linked_return)
        && Some(linked_return) != continuation.fallthrough
    {
        enqueue_point(
            instruction_base,
            executable_domain,
            linked_return,
            None,
            scan,
            seen,
            work,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn enqueue_point(
    instruction_base: u32,
    executable_domain: &ExecutableDomain,
    pc: u32,
    continuation: Option<ReachabilityContinuation>,
    scan: &mut ReachableInstructionScan,
    seen: &mut BTreeSet<ReachabilityPoint>,
    work: &mut VecDeque<ReachabilityPoint>,
) {
    let Some(instruction_offset) = executable_domain.image_instruction_offset(instruction_base, pc)
    else {
        scan.outside_image_transfer_targets.insert(pc);
        return;
    };
    if !executable_domain.contains_instruction_offset(instruction_offset) {
        scan.outside_executable_domain_transfer_targets.insert(pc);
        return;
    }
    let point = ReachabilityPoint {
        instruction_offset,
        continuation,
    };
    if seen.insert(point.clone()) {
        work.push_back(point);
    }
}

fn aligned_word(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(
        data[offset..offset + 4]
            .try_into()
            .expect("bounded aligned instruction read"),
    )
}

#[cfg(test)]
#[path = "reachable_control_flow_tests.rs"]
mod tests;
