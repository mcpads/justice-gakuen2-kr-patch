use std::collections::{BTreeMap, BTreeSet, VecDeque};

use psx_r3000a::{Instruction, Register, decode};

#[path = "value_flow/memory.rs"]
mod memory;

use super::ExecutableDomain;
use super::control_flow::{FlowSuccessors, FlowTarget, flow_successors};
pub(crate) use super::reachable_control_flow::ResolvedRegisterTransfer;
use memory::{memory_operand, read_scalar_value};

pub(crate) const DEFAULT_VALUE_FLOW_STATE_BUDGET: usize = 8192;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum DerivedAddressKind {
    RegisterValue,
    MemoryAccess,
    LoadedValue {
        storage_address: u32,
        load_instruction_offset: usize,
        width_bytes: u8,
        sign_extended: bool,
    },
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct DerivedAddress {
    pub(crate) seed_offset: usize,
    pub(crate) instruction_offset: usize,
    pub(crate) source_offset: usize,
    pub(crate) address: u32,
    pub(crate) kind: DerivedAddressKind,
}

#[derive(Debug)]
pub(crate) struct DerivedAddressScan {
    pub(crate) addresses: Vec<DerivedAddress>,
    pub(crate) resolved_register_transfers: Vec<ResolvedRegisterTransfer>,
    pub(crate) resolved_direct_call_arguments: Vec<ResolvedDirectCallArgument>,
    pub(crate) seed_count: usize,
    pub(crate) instruction_state_count: usize,
    pub(crate) budget_exhausted_seed_count: usize,
    pub(crate) budget_exhausted_seed_offsets: Vec<usize>,
    pub(crate) budget_exhausted_seeds: Vec<AddressFlowSeedExhaustion>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct ResolvedDirectCallArgument {
    // The value is observed after the call delay slot. A load started in that
    // slot is not admitted because its result is not ready at callee entry.
    pub(crate) seed_offset: usize,
    pub(crate) instruction_offset: usize,
    pub(crate) target: u32,
    pub(crate) argument_register: Register,
    pub(crate) value: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AddressFlowSeedExhaustion {
    pub(crate) seed_offset: usize,
    pub(crate) processed_state_count: usize,
    pub(crate) discovered_state_count: usize,
    pub(crate) distinct_instruction_offset_count: usize,
    pub(crate) maximum_states_at_instruction_offset: usize,
    pub(crate) pending_state_count: usize,
    pub(crate) distinct_frontier_instruction_offset_count: usize,
    pub(crate) frontier_instruction_offsets: Vec<usize>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct KnownRegisterValue {
    value: u32,
    origin: ValueOrigin,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum ValueOrigin {
    Independent,
    Seed,
    LoadedValue {
        storage_address: u32,
        load_instruction_offset: usize,
        width_bytes: u8,
        sign_extended: bool,
    },
    Mixed,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct PendingLoad {
    destination: Register,
    value: Option<KnownRegisterValue>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct AnalysisState {
    values: [Option<KnownRegisterValue>; 32],
    pending_load: Option<PendingLoad>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct Continuation {
    target: Option<u32>,
    fallthrough: Option<u32>,
    linked_return: Option<u32>,
    direct_call: Option<DirectCallContinuation>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct DirectCallContinuation {
    instruction_offset: usize,
    target: u32,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct FlowPoint {
    instruction_offset: usize,
    continuation: Option<Continuation>,
}

#[cfg(test)]
pub(crate) fn scan_derived_addresses(
    data: &[u8],
    instruction_base: u32,
    executable_domain: &ExecutableDomain,
) -> Vec<DerivedAddress> {
    scan_derived_address_flow(data, instruction_base, executable_domain).addresses
}

pub(crate) fn scan_derived_address_flow(
    data: &[u8],
    instruction_base: u32,
    executable_domain: &ExecutableDomain,
) -> DerivedAddressScan {
    scan_derived_address_flow_with_budget(
        data,
        instruction_base,
        executable_domain,
        DEFAULT_VALUE_FLOW_STATE_BUDGET,
    )
}

pub(crate) fn scan_derived_address_flow_with_budget(
    data: &[u8],
    instruction_base: u32,
    executable_domain: &ExecutableDomain,
    state_budget: usize,
) -> DerivedAddressScan {
    assert!(
        executable_domain.matches_image_len(data.len()),
        "executable domain must describe the analyzed image"
    );
    assert!(
        state_budget > 0,
        "address-flow state budget must be positive"
    );
    let mut addresses = BTreeSet::new();
    let mut resolved_register_transfers = BTreeSet::new();
    let mut resolved_direct_call_arguments = BTreeSet::new();
    let mut seed_count = 0usize;
    let mut instruction_state_count = 0usize;
    let mut budget_exhausted_seed_count = 0usize;
    let mut budget_exhausted_seed_offsets = Vec::new();
    let mut budget_exhausted_seeds = Vec::new();
    for seed_offset in (0..data.len().saturating_sub(3)).step_by(4) {
        if !executable_domain.contains_instruction_offset(seed_offset) {
            continue;
        }
        let seed_pc = instruction_base.wrapping_add(seed_offset as u32);
        let Ok(Instruction::Lui {
            rt: seed_register,
            immediate,
        }) = decode(aligned_word(data, seed_offset), seed_pc)
        else {
            continue;
        };
        if seed_register == Register::ZERO {
            continue;
        }
        seed_count += 1;

        let mut values = [None; 32];
        write_value(
            &mut values,
            seed_register,
            Some(KnownRegisterValue {
                value: u32::from(immediate) << 16,
                origin: ValueOrigin::Seed,
            }),
        );
        let summary = scan_seed_flow(
            data,
            instruction_base,
            executable_domain,
            seed_offset,
            AnalysisState {
                values,
                pending_load: None,
            },
            state_budget,
            &mut SeedFlowEvidence {
                addresses: &mut addresses,
                resolved_register_transfers: &mut resolved_register_transfers,
                resolved_direct_call_arguments: &mut resolved_direct_call_arguments,
            },
        );
        instruction_state_count += summary.instruction_state_count;
        budget_exhausted_seed_count += usize::from(summary.budget_exhausted);
        if let Some(exhaustion) = summary.exhaustion {
            budget_exhausted_seed_offsets.push(seed_offset);
            budget_exhausted_seeds.push(AddressFlowSeedExhaustion {
                seed_offset,
                processed_state_count: summary.instruction_state_count,
                discovered_state_count: exhaustion.discovered_state_count,
                distinct_instruction_offset_count: exhaustion.distinct_instruction_offset_count,
                maximum_states_at_instruction_offset: exhaustion
                    .maximum_states_at_instruction_offset,
                pending_state_count: exhaustion.pending_state_count,
                distinct_frontier_instruction_offset_count: exhaustion
                    .distinct_frontier_instruction_offset_count,
                frontier_instruction_offsets: exhaustion.frontier_instruction_offsets,
            });
        }
    }
    DerivedAddressScan {
        addresses: addresses.into_iter().collect(),
        resolved_register_transfers: resolved_register_transfers.into_iter().collect(),
        resolved_direct_call_arguments: resolved_direct_call_arguments.into_iter().collect(),
        seed_count,
        instruction_state_count,
        budget_exhausted_seed_count,
        budget_exhausted_seed_offsets,
        budget_exhausted_seeds,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SeedFlowSummary {
    instruction_state_count: usize,
    budget_exhausted: bool,
    exhaustion: Option<SeedFlowExhaustion>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SeedFlowExhaustion {
    discovered_state_count: usize,
    distinct_instruction_offset_count: usize,
    maximum_states_at_instruction_offset: usize,
    pending_state_count: usize,
    distinct_frontier_instruction_offset_count: usize,
    frontier_instruction_offsets: Vec<usize>,
}

struct SeedFlowEvidence<'a> {
    addresses: &'a mut BTreeSet<DerivedAddress>,
    resolved_register_transfers: &'a mut BTreeSet<ResolvedRegisterTransfer>,
    resolved_direct_call_arguments: &'a mut BTreeSet<ResolvedDirectCallArgument>,
}

fn scan_seed_flow(
    data: &[u8],
    instruction_base: u32,
    executable_domain: &ExecutableDomain,
    seed_offset: usize,
    initial_state: AnalysisState,
    state_budget: usize,
    evidence: &mut SeedFlowEvidence<'_>,
) -> SeedFlowSummary {
    let Some(first_offset) = seed_offset.checked_add(4) else {
        return SeedFlowSummary {
            instruction_state_count: 0,
            budget_exhausted: false,
            exhaustion: None,
        };
    };
    if !executable_domain.contains_instruction_offset(first_offset) {
        return SeedFlowSummary {
            instruction_state_count: 0,
            budget_exhausted: false,
            exhaustion: None,
        };
    }
    let first = FlowPoint {
        instruction_offset: first_offset,
        continuation: None,
    };
    let mut seen = BTreeSet::from([(first, initial_state)]);
    let mut work = VecDeque::from([(first, initial_state)]);
    let mut processed = 0usize;

    while processed < state_budget {
        let Some((point, mut state)) = work.pop_front() else {
            break;
        };
        processed += 1;
        if !executable_domain.contains_instruction_offset(point.instruction_offset) {
            continue;
        }
        let pc = instruction_base.wrapping_add(point.instruction_offset as u32);
        let Ok(instruction) = decode(aligned_word(data, point.instruction_offset), pc) else {
            continue;
        };
        let flow = flow_successors(&instruction, pc);
        let resolved_target = resolve_target(flow, &state.values);
        if point.continuation.is_none()
            && matches!(flow.target, Some(FlowTarget::Register(_)))
            && let Some(target) = resolved_target
        {
            evidence
                .resolved_register_transfers
                .insert(ResolvedRegisterTransfer {
                    seed_offset,
                    instruction_offset: point.instruction_offset,
                    target,
                });
        }
        let known_branch_taken = known_non_link_branch_taken(&instruction, &state.values);
        let continuation = Continuation {
            target: match known_branch_taken {
                Some(false) => None,
                Some(true) | None => resolved_target,
            },
            fallthrough: match known_branch_taken {
                Some(true) => None,
                Some(false) | None => flow.fallthrough,
            },
            linked_return: flow.return_site,
            direct_call: if point.continuation.is_none()
                && let Instruction::Jal { target } = instruction
            {
                Some(DirectCallContinuation {
                    instruction_offset: point.instruction_offset,
                    target,
                })
            } else {
                None
            },
        };
        execute_instruction(
            data,
            instruction_base,
            seed_offset,
            point.instruction_offset,
            pc,
            &instruction,
            &mut state,
            evidence.addresses,
        );
        if !has_tracked_values(&state) {
            continue;
        }

        if let Some(after_delay_slot) = point.continuation {
            if flow.delay_slot.is_none() {
                if let Some(call) = after_delay_slot.direct_call {
                    record_direct_call_arguments(seed_offset, call, &state, evidence);
                }
                enqueue_continuation(
                    instruction_base,
                    executable_domain,
                    after_delay_slot,
                    state,
                    &mut seen,
                    &mut work,
                );
            }
            continue;
        }
        if let Some(delay_slot) = flow.delay_slot {
            enqueue(
                instruction_base,
                executable_domain,
                delay_slot,
                Some(continuation),
                state,
                &mut seen,
                &mut work,
            );
        } else {
            enqueue_continuation(
                instruction_base,
                executable_domain,
                continuation,
                state,
                &mut seen,
                &mut work,
            );
        }
    }
    let budget_exhausted = !work.is_empty();
    let exhaustion = budget_exhausted.then(|| {
        let mut states_per_instruction = BTreeMap::<usize, usize>::new();
        for (point, _) in &seen {
            *states_per_instruction
                .entry(point.instruction_offset)
                .or_default() += 1;
        }
        let frontier_instruction_offsets: BTreeSet<_> = work
            .iter()
            .map(|(point, _)| point.instruction_offset)
            .collect();
        SeedFlowExhaustion {
            discovered_state_count: seen.len(),
            distinct_instruction_offset_count: states_per_instruction.len(),
            maximum_states_at_instruction_offset: states_per_instruction
                .values()
                .copied()
                .max()
                .unwrap_or(0),
            pending_state_count: work.len(),
            distinct_frontier_instruction_offset_count: frontier_instruction_offsets.len(),
            frontier_instruction_offsets: frontier_instruction_offsets.into_iter().collect(),
        }
    });
    SeedFlowSummary {
        instruction_state_count: processed,
        budget_exhausted,
        exhaustion,
    }
}

fn record_direct_call_arguments(
    seed_offset: usize,
    call: DirectCallContinuation,
    state: &AnalysisState,
    evidence: &mut SeedFlowEvidence<'_>,
) {
    for argument_register in [Register::A0, Register::A1, Register::A2, Register::A3] {
        if state
            .pending_load
            .is_some_and(|load| load.destination == argument_register)
        {
            continue;
        }
        if let Some(value) = read_value(&state.values, argument_register)
            && tracked_origin(value.origin)
        {
            evidence
                .resolved_direct_call_arguments
                .insert(ResolvedDirectCallArgument {
                    seed_offset,
                    instruction_offset: call.instruction_offset,
                    target: call.target,
                    argument_register,
                    value: value.value,
                });
        }
    }
}

fn known_non_link_branch_taken(
    instruction: &Instruction,
    values: &[Option<KnownRegisterValue>; 32],
) -> Option<bool> {
    match instruction {
        Instruction::Beq { rs, rt, .. } => {
            Some(read_value(values, *rs)?.value == read_value(values, *rt)?.value)
        }
        Instruction::Bne { rs, rt, .. } => {
            Some(read_value(values, *rs)?.value != read_value(values, *rt)?.value)
        }
        Instruction::Blez { rs, .. } => Some((read_value(values, *rs)?.value as i32) <= 0),
        Instruction::Bgtz { rs, .. } => Some((read_value(values, *rs)?.value as i32) > 0),
        Instruction::Bltz { rs, .. } => Some((read_value(values, *rs)?.value as i32) < 0),
        Instruction::Bgez { rs, .. } => Some((read_value(values, *rs)?.value as i32) >= 0),
        Instruction::Bltzal { .. } | Instruction::Bgezal { .. } => None,
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn execute_instruction(
    data: &[u8],
    instruction_base: u32,
    seed_offset: usize,
    instruction_offset: usize,
    pc: u32,
    instruction: &Instruction,
    state: &mut AnalysisState,
    addresses: &mut BTreeSet<DerivedAddress>,
) {
    let operand = memory_operand(instruction);
    let effective_address = memory_effective_address(&state.values, instruction);
    if let Some((address, origin)) = effective_address
        && let Some(kind) = derived_kind(origin, DerivedAddressKind::MemoryAccess)
    {
        addresses.insert(DerivedAddress {
            seed_offset,
            instruction_offset,
            source_offset: instruction_offset,
            address,
            kind,
        });
    }

    let loaded_value = operand
        .and_then(|operand| operand.scalar_load)
        .filter(|load| load.destination != Register::ZERO)
        .zip(effective_address)
        .and_then(|(load, (address, _))| {
            read_scalar_value(data, instruction_base, address, load)
                .map(|value| (address, value, load))
        });
    if let Some((storage_address, value, load)) = loaded_value {
        addresses.insert(DerivedAddress {
            seed_offset,
            instruction_offset,
            source_offset: instruction_offset,
            address: value,
            kind: DerivedAddressKind::LoadedValue {
                storage_address,
                load_instruction_offset: instruction_offset,
                width_bytes: load.width_bytes,
                sign_extended: load.sign_extended,
            },
        });
    }

    let current_load = operand.and_then(|operand| {
        operand.load_destination.map(|destination| PendingLoad {
            destination,
            value: loaded_value.map(|(storage_address, value, load)| KnownRegisterValue {
                value,
                origin: ValueOrigin::LoadedValue {
                    storage_address,
                    load_instruction_offset: instruction_offset,
                    width_bytes: load.width_bytes,
                    sign_extended: load.sign_extended,
                },
            }),
        })
    });
    let written_register = current_load
        .map(|load| load.destination)
        .or_else(|| apply_instruction(&mut state.values, instruction, pc));
    if current_load.is_none()
        && let Some(written_register) = written_register
        && let Some(value) = read_value(&state.values, written_register)
        && let Some(kind) = derived_kind(value.origin, DerivedAddressKind::RegisterValue)
    {
        addresses.insert(DerivedAddress {
            seed_offset,
            instruction_offset,
            source_offset: match kind {
                DerivedAddressKind::RegisterValue => seed_offset,
                DerivedAddressKind::MemoryAccess | DerivedAddressKind::LoadedValue { .. } => {
                    instruction_offset
                }
            },
            address: value.value,
            kind,
        });
    }

    if let Some(completed_load) = state.pending_load
        && written_register != Some(completed_load.destination)
    {
        write_value(
            &mut state.values,
            completed_load.destination,
            completed_load.value,
        );
    }
    state.pending_load = current_load;
}

fn has_tracked_values(state: &AnalysisState) -> bool {
    state
        .values
        .iter()
        .flatten()
        .any(|value| tracked_origin(value.origin))
        || state
            .pending_load
            .and_then(|load| load.value)
            .is_some_and(|value| tracked_origin(value.origin))
}

fn resolve_target(flow: FlowSuccessors, values: &[Option<KnownRegisterValue>; 32]) -> Option<u32> {
    match flow.target? {
        FlowTarget::Direct(target) => Some(target),
        FlowTarget::Register(register) => read_value(values, register).map(|value| value.value),
    }
}

fn enqueue_continuation(
    instruction_base: u32,
    executable_domain: &ExecutableDomain,
    continuation: Continuation,
    state: AnalysisState,
    seen: &mut BTreeSet<(FlowPoint, AnalysisState)>,
    work: &mut VecDeque<(FlowPoint, AnalysisState)>,
) {
    if let Some(target) = continuation.target {
        enqueue(
            instruction_base,
            executable_domain,
            target,
            None,
            state,
            seen,
            work,
        );
    }
    if let Some(fallthrough) = continuation.fallthrough
        && Some(fallthrough) != continuation.target
    {
        enqueue(
            instruction_base,
            executable_domain,
            fallthrough,
            None,
            state,
            seen,
            work,
        );
    }
    if let Some(call_return) = continuation.linked_return
        && Some(call_return) != continuation.target
        && Some(call_return) != continuation.fallthrough
    {
        enqueue(
            instruction_base,
            executable_domain,
            call_return,
            None,
            abi_call_return_state(state),
            seen,
            work,
        );
    }
}

fn abi_call_return_state(mut state: AnalysisState) -> AnalysisState {
    if let Some(load) = state.pending_load.take()
        && is_callee_saved(load.destination)
    {
        write_value(&mut state.values, load.destination, load.value);
    }
    let previous_values = state.values;
    state.values = [None; 32];
    for register in [
        Register::S0,
        Register::S1,
        Register::S2,
        Register::S3,
        Register::S4,
        Register::S5,
        Register::S6,
        Register::S7,
        Register::SP,
        Register::FP,
    ] {
        state.values[usize::from(register.index())] =
            previous_values[usize::from(register.index())];
    }
    state
}

fn is_callee_saved(register: Register) -> bool {
    matches!(
        register,
        Register::S0
            | Register::S1
            | Register::S2
            | Register::S3
            | Register::S4
            | Register::S5
            | Register::S6
            | Register::S7
            | Register::SP
            | Register::FP
    )
}

#[allow(clippy::too_many_arguments)]
fn enqueue(
    instruction_base: u32,
    executable_domain: &ExecutableDomain,
    pc: u32,
    continuation: Option<Continuation>,
    state: AnalysisState,
    seen: &mut BTreeSet<(FlowPoint, AnalysisState)>,
    work: &mut VecDeque<(FlowPoint, AnalysisState)>,
) {
    let Some(instruction_offset) = executable_domain.instruction_offset(instruction_base, pc)
    else {
        return;
    };
    let point = FlowPoint {
        instruction_offset,
        continuation,
    };
    if seen.insert((point, state)) {
        work.push_back((point, state));
    }
}

fn memory_effective_address(
    values: &[Option<KnownRegisterValue>; 32],
    instruction: &Instruction,
) -> Option<(u32, ValueOrigin)> {
    let operand = memory_operand(instruction)?;
    let base = read_value(values, operand.base)?;
    tracked_origin(base.origin).then(|| {
        (
            base.value
                .wrapping_add_signed(i32::from(operand.displacement)),
            base.origin,
        )
    })
}

fn derived_kind(origin: ValueOrigin, seed_kind: DerivedAddressKind) -> Option<DerivedAddressKind> {
    match origin {
        ValueOrigin::Seed => Some(seed_kind),
        ValueOrigin::LoadedValue {
            storage_address,
            load_instruction_offset,
            width_bytes,
            sign_extended,
        } => Some(DerivedAddressKind::LoadedValue {
            storage_address,
            load_instruction_offset,
            width_bytes,
            sign_extended,
        }),
        ValueOrigin::Independent | ValueOrigin::Mixed => None,
    }
}

fn tracked_origin(origin: ValueOrigin) -> bool {
    matches!(origin, ValueOrigin::Seed | ValueOrigin::LoadedValue { .. })
}

fn apply_instruction(
    values: &mut [Option<KnownRegisterValue>; 32],
    instruction: &Instruction,
    pc: u32,
) -> Option<Register> {
    let resolved = match instruction {
        Instruction::Lui { rt, immediate } => Some((
            *rt,
            Some(KnownRegisterValue {
                value: u32::from(*immediate) << 16,
                origin: ValueOrigin::Independent,
            }),
        )),
        Instruction::Addi { rt, rs, immediate } => Some((
            *rt,
            unary_value(values, *rs, |value| {
                (value as i32)
                    .checked_add(i32::from(*immediate))
                    .map(|sum| sum as u32)
            }),
        )),
        Instruction::Addiu { rt, rs, immediate } => Some((
            *rt,
            unary_value(values, *rs, |value| {
                Some(value.wrapping_add_signed(i32::from(*immediate)))
            }),
        )),
        Instruction::Andi { rt, rs, immediate } => Some((
            *rt,
            unary_value(values, *rs, |value| Some(value & u32::from(*immediate))),
        )),
        Instruction::Ori { rt, rs, immediate } => Some((
            *rt,
            unary_value(values, *rs, |value| Some(value | u32::from(*immediate))),
        )),
        Instruction::Xori { rt, rs, immediate } => Some((
            *rt,
            unary_value(values, *rs, |value| Some(value ^ u32::from(*immediate))),
        )),
        Instruction::Slti { rt, rs, immediate } => Some((
            *rt,
            unary_value(values, *rs, |value| {
                Some(u32::from((value as i32) < i32::from(*immediate)))
            }),
        )),
        Instruction::Sltiu { rt, rs, immediate } => Some((
            *rt,
            unary_value(values, *rs, |value| {
                Some(u32::from(value < i32::from(*immediate) as u32))
            }),
        )),
        Instruction::Add { rd, rs, rt } => Some((
            *rd,
            binary_value(values, *rs, *rt, |left, right| {
                (left as i32)
                    .checked_add(right as i32)
                    .map(|sum| sum as u32)
            }),
        )),
        Instruction::Addu { rd, rs, rt } => Some((
            *rd,
            binary_value(values, *rs, *rt, |left, right| {
                Some(left.wrapping_add(right))
            }),
        )),
        Instruction::Sub { rd, rs, rt } => Some((
            *rd,
            binary_value(values, *rs, *rt, |left, right| {
                (left as i32)
                    .checked_sub(right as i32)
                    .map(|difference| difference as u32)
            }),
        )),
        Instruction::Subu { rd, rs, rt } => Some((
            *rd,
            binary_value(values, *rs, *rt, |left, right| {
                Some(left.wrapping_sub(right))
            }),
        )),
        Instruction::And { rd, rs, rt } => Some((
            *rd,
            binary_value(values, *rs, *rt, |left, right| Some(left & right)),
        )),
        Instruction::Or { rd, rs, rt } => Some((
            *rd,
            binary_value(values, *rs, *rt, |left, right| Some(left | right)),
        )),
        Instruction::Xor { rd, rs, rt } => Some((
            *rd,
            binary_value(values, *rs, *rt, |left, right| Some(left ^ right)),
        )),
        Instruction::Nor { rd, rs, rt } => Some((
            *rd,
            binary_value(values, *rs, *rt, |left, right| Some(!(left | right))),
        )),
        Instruction::Slt { rd, rs, rt } => Some((
            *rd,
            binary_value(values, *rs, *rt, |left, right| {
                Some(u32::from((left as i32) < (right as i32)))
            }),
        )),
        Instruction::Sltu { rd, rs, rt } => Some((
            *rd,
            binary_value(values, *rs, *rt, |left, right| {
                Some(u32::from(left < right))
            }),
        )),
        Instruction::Sll { rd, rt, shift } => Some((
            *rd,
            unary_value(values, *rt, |value| Some(value << u32::from(*shift))),
        )),
        Instruction::Srl { rd, rt, shift } => Some((
            *rd,
            unary_value(values, *rt, |value| Some(value >> u32::from(*shift))),
        )),
        Instruction::Sra { rd, rt, shift } => Some((
            *rd,
            unary_value(values, *rt, |value| {
                Some(((value as i32) >> u32::from(*shift)) as u32)
            }),
        )),
        Instruction::Sllv { rd, rt, rs } => Some((
            *rd,
            binary_value(values, *rt, *rs, |value, shift| {
                Some(value << (shift & 0x1f))
            }),
        )),
        Instruction::Srlv { rd, rt, rs } => Some((
            *rd,
            binary_value(values, *rt, *rs, |value, shift| {
                Some(value >> (shift & 0x1f))
            }),
        )),
        Instruction::Srav { rd, rt, rs } => Some((
            *rd,
            binary_value(values, *rt, *rs, |value, shift| {
                Some(((value as i32) >> (shift & 0x1f)) as u32)
            }),
        )),
        Instruction::Jal { .. } => Some((
            Register::RA,
            Some(KnownRegisterValue {
                value: pc.wrapping_add(8),
                origin: ValueOrigin::Independent,
            }),
        )),
        Instruction::Jalr { rd, .. } => Some((
            *rd,
            Some(KnownRegisterValue {
                value: pc.wrapping_add(8),
                origin: ValueOrigin::Independent,
            }),
        )),
        _ => instruction.written_gpr().map(|register| (register, None)),
    };
    let (register, value) = resolved?;
    write_value(values, register, value);
    Some(register)
}

fn unary_value(
    values: &[Option<KnownRegisterValue>; 32],
    source: Register,
    operation: impl FnOnce(u32) -> Option<u32>,
) -> Option<KnownRegisterValue> {
    let source = read_value(values, source)?;
    operation(source.value).map(|value| KnownRegisterValue {
        value,
        origin: source.origin,
    })
}

fn binary_value(
    values: &[Option<KnownRegisterValue>; 32],
    left: Register,
    right: Register,
    operation: impl FnOnce(u32, u32) -> Option<u32>,
) -> Option<KnownRegisterValue> {
    let left = read_value(values, left)?;
    let right = read_value(values, right)?;
    operation(left.value, right.value).map(|value| KnownRegisterValue {
        value,
        origin: combine_origins(left.origin, right.origin),
    })
}

fn combine_origins(left: ValueOrigin, right: ValueOrigin) -> ValueOrigin {
    match (left, right) {
        (ValueOrigin::Independent, origin) | (origin, ValueOrigin::Independent) => origin,
        (left, right) if left == right => left,
        _ => ValueOrigin::Mixed,
    }
}

fn read_value(
    values: &[Option<KnownRegisterValue>; 32],
    register: Register,
) -> Option<KnownRegisterValue> {
    if register == Register::ZERO {
        return Some(KnownRegisterValue {
            value: 0,
            origin: ValueOrigin::Independent,
        });
    }
    values[usize::from(register.index())]
}

fn write_value(
    values: &mut [Option<KnownRegisterValue>; 32],
    register: Register,
    value: Option<KnownRegisterValue>,
) {
    if register != Register::ZERO {
        values[usize::from(register.index())] = value;
    }
}

fn aligned_word(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(
        data[offset..offset + 4]
            .try_into()
            .expect("bounded aligned instruction read"),
    )
}
