use psx_r3000a::{Instruction, Register};

use super::{
    ReachableInstructionClosure, ReachableInstructionScan, ResolvedRegisterTransfer,
    UnresolvedRegisterTransferKind, close_reachable_control_transfers as close_transfers_in_domain,
    close_reachable_control_transfers_from_entrypoints_with_declared_targets,
    declared_overlay_entrypoint as read_entrypoint_in_domain,
    scan_reachable_instructions as scan_in_domain,
    scan_reachable_instructions_with_register_targets as scan_with_targets_in_domain,
};
use crate::psx_static_analysis::ExecutableDomain;

const BASE: u32 = 0x800a_2000;

fn scan_reachable_instructions(
    data: &[u8],
    instruction_base: u32,
    entrypoint: u32,
) -> ReachableInstructionScan {
    let executable_domain = ExecutableDomain::full_image(data.len());
    scan_in_domain(data, instruction_base, &executable_domain, entrypoint)
}

fn scan_reachable_instructions_with_register_targets(
    data: &[u8],
    instruction_base: u32,
    entrypoint: u32,
    register_targets: &std::collections::BTreeMap<usize, std::collections::BTreeSet<u32>>,
) -> ReachableInstructionScan {
    let executable_domain = ExecutableDomain::full_image(data.len());
    scan_with_targets_in_domain(
        data,
        instruction_base,
        &executable_domain,
        entrypoint,
        register_targets,
    )
}

fn close_reachable_control_transfers(
    data: &[u8],
    instruction_base: u32,
    entrypoint: u32,
    resolved_register_transfers: &[ResolvedRegisterTransfer],
) -> ReachableInstructionClosure {
    let executable_domain = ExecutableDomain::full_image(data.len());
    close_transfers_in_domain(
        data,
        instruction_base,
        &executable_domain,
        entrypoint,
        resolved_register_transfers,
    )
}

#[test]
fn declared_indirect_targets_extend_the_entrypoint_closure() {
    let data = overlay(&[
        Instruction::Jr { rs: Register::T0 },
        Instruction::nop(),
        Instruction::Lui {
            rt: Register::S0,
            immediate: 0x800a,
        },
    ]);
    let executable_domain = ExecutableDomain::full_image(data.len());
    let declared_targets = [(4, [BASE + 12].into_iter().collect())]
        .into_iter()
        .collect();

    let closure = close_reachable_control_transfers_from_entrypoints_with_declared_targets(
        &data,
        BASE,
        &executable_domain,
        &[BASE + 4],
        &[],
        &declared_targets,
    );

    assert!(closure.scan.unresolved_indirect_transfers.is_empty());
    assert!(closure.scan.lui_instruction_offsets.contains(&12));
}

fn declared_overlay_entrypoint(data: &[u8], instruction_base: u32) -> Option<u32> {
    let executable_domain = ExecutableDomain::full_image(data.len());
    read_entrypoint_in_domain(data, instruction_base, &executable_domain)
}

#[test]
fn direct_control_flow_excludes_bytes_after_an_unconditional_transfer() {
    let data = overlay(&[
        Instruction::J {
            target: BASE + 0x18,
        },
        Instruction::nop(),
        Instruction::Lui {
            rt: Register::S0,
            immediate: 0x800a,
        },
        Instruction::nop(),
        Instruction::nop(),
        Instruction::Lui {
            rt: Register::S1,
            immediate: 0x800a,
        },
    ]);

    let scan = scan_reachable_instructions(&data, BASE, BASE + 4);

    assert!(scan.instruction_offsets.contains(&4));
    assert!(scan.instruction_offsets.contains(&8));
    assert!(!scan.instruction_offsets.contains(&12));
    assert!(!scan.instruction_offsets.contains(&16));
    assert!(scan.lui_instruction_offsets.contains(&24));
}

#[test]
fn conditional_control_flow_preserves_target_and_fallthrough_after_the_delay_slot() {
    let data = overlay(&[
        Instruction::Beq {
            rs: Register::S0,
            rt: Register::S1,
            target: BASE + 0x18,
        },
        Instruction::nop(),
        Instruction::Lui {
            rt: Register::S2,
            immediate: 0x800a,
        },
        Instruction::J {
            target: BASE + 0x40,
        },
        Instruction::nop(),
        Instruction::Lui {
            rt: Register::S3,
            immediate: 0x800a,
        },
    ]);

    let scan = scan_reachable_instructions(&data, BASE, BASE + 4);

    assert!(scan.lui_instruction_offsets.contains(&12));
    assert!(scan.lui_instruction_offsets.contains(&24));
}

#[test]
fn linked_transfer_reaches_both_the_callee_and_its_return_site() {
    let data = overlay(&[
        Instruction::Jal {
            target: BASE + 0x14,
        },
        Instruction::nop(),
        Instruction::Lui {
            rt: Register::S0,
            immediate: 0x800a,
        },
        Instruction::J {
            target: BASE + 0x40,
        },
        Instruction::Jr { rs: Register::RA },
        Instruction::nop(),
    ]);

    let scan = scan_reachable_instructions(&data, BASE, BASE + 4);

    assert!(scan.instruction_offsets.contains(&0x14));
    assert!(scan.lui_instruction_offsets.contains(&0x0c));
}

#[test]
fn register_transfer_remains_an_explicit_reachability_gap() {
    let data = overlay(&[Instruction::Jr { rs: Register::T0 }, Instruction::nop()]);

    let scan = scan_reachable_instructions(&data, BASE, BASE + 4);

    assert_eq!(
        scan.unresolved_indirect_transfer_offsets,
        [4].into_iter().collect()
    );
    let transfer = scan.unresolved_indirect_transfers.get(&4).unwrap();
    assert_eq!(transfer.source_register, Register::T0);
    assert_eq!(transfer.kind, UnresolvedRegisterTransferKind::Jump);
}

#[test]
fn linked_register_transfer_is_classified_as_an_indirect_call() {
    let data = overlay(&[
        Instruction::Jalr {
            rd: Register::RA,
            rs: Register::V0,
        },
        Instruction::nop(),
    ]);

    let scan = scan_reachable_instructions(&data, BASE, BASE + 4);

    let transfer = scan.unresolved_indirect_transfers.get(&4).unwrap();
    assert_eq!(transfer.source_register, Register::V0);
    assert_eq!(transfer.kind, UnresolvedRegisterTransferKind::LinkedCall);
}

#[test]
fn return_address_transfer_is_a_terminal_abi_return_after_its_delay_slot() {
    let data = overlay(&[
        Instruction::Jr { rs: Register::RA },
        Instruction::nop(),
        Instruction::Lui {
            rt: Register::S0,
            immediate: 0x800a,
        },
    ]);

    let scan = scan_reachable_instructions(&data, BASE, BASE + 4);

    assert_eq!(scan.abi_return_offsets, [4].into_iter().collect());
    assert!(scan.unresolved_indirect_transfer_offsets.is_empty());
    assert!(scan.instruction_offsets.contains(&8));
    assert!(!scan.instruction_offsets.contains(&12));
}

#[test]
fn resolved_register_transfer_admits_its_target_after_the_delay_slot() {
    let data = overlay(&[
        Instruction::Jr { rs: Register::T0 },
        Instruction::nop(),
        Instruction::Lui {
            rt: Register::S0,
            immediate: 0x800a,
        },
        Instruction::nop(),
        Instruction::nop(),
        Instruction::Lui {
            rt: Register::S1,
            immediate: 0x800a,
        },
    ]);
    let register_targets =
        std::collections::BTreeMap::from([(4, std::collections::BTreeSet::from([BASE + 0x18]))]);

    let scan =
        scan_reachable_instructions_with_register_targets(&data, BASE, BASE + 4, &register_targets);

    assert!(scan.unresolved_indirect_transfer_offsets.is_empty());
    assert!(scan.unresolved_indirect_transfers.is_empty());
    assert!(!scan.lui_instruction_offsets.contains(&12));
    assert!(scan.lui_instruction_offsets.contains(&24));

    let executable_domain =
        ExecutableDomain::from_instruction_ranges(data.len(), std::iter::once(4..12))
            .expect("fixture code range is aligned and bounded");
    let bounded_scan =
        scan_with_targets_in_domain(&data, BASE, &executable_domain, BASE + 4, &register_targets);
    assert_eq!(
        bounded_scan.outside_executable_domain_transfer_targets,
        [BASE + 0x18].into_iter().collect()
    );
    assert!(bounded_scan.outside_image_transfer_targets.is_empty());
}

#[test]
fn bounded_jump_table_targets_become_reachable_after_the_delay_slot() {
    let mut data = vec![0; 0x88];
    data[..4].copy_from_slice(&(BASE + 4).to_le_bytes());
    let instructions = [
        (
            4,
            Instruction::Sltiu {
                rt: Register::V0,
                rs: Register::V1,
                immediate: 2,
            },
        ),
        (
            8,
            Instruction::Beq {
                rs: Register::V0,
                rt: Register::ZERO,
                target: BASE + 0x60,
            },
        ),
        (
            0x0c,
            Instruction::Sll {
                rd: Register::V0,
                rt: Register::V1,
                shift: 2,
            },
        ),
        (
            0x10,
            Instruction::Lui {
                rt: Register::AT,
                immediate: 0x800a,
            },
        ),
        (
            0x14,
            Instruction::Addu {
                rd: Register::AT,
                rs: Register::AT,
                rt: Register::V0,
            },
        ),
        (
            0x18,
            Instruction::Lw {
                rt: Register::V0,
                base: Register::AT,
                offset: 0x2080,
            },
        ),
        (0x1c, Instruction::nop()),
        (0x20, Instruction::Jr { rs: Register::V0 }),
        (0x24, Instruction::nop()),
        (
            0x40,
            Instruction::Lui {
                rt: Register::S0,
                immediate: 0x800a,
            },
        ),
        (0x44, Instruction::Jr { rs: Register::RA }),
        (0x48, Instruction::nop()),
        (
            0x50,
            Instruction::Lui {
                rt: Register::S1,
                immediate: 0x800a,
            },
        ),
        (0x54, Instruction::Jr { rs: Register::RA }),
        (0x58, Instruction::nop()),
        (0x60, Instruction::Jr { rs: Register::RA }),
        (0x64, Instruction::nop()),
    ];
    for (offset, instruction) in instructions {
        write_instruction(&mut data, offset, instruction);
    }
    data[0x80..0x84].copy_from_slice(&(BASE + 0x40).to_le_bytes());
    data[0x84..0x88].copy_from_slice(&(BASE + 0x50).to_le_bytes());

    let closure = close_reachable_control_transfers(&data, BASE, BASE + 4, &[]);

    assert_eq!(closure.bounded_jump_tables.len(), 1);
    assert!(closure.scan.unresolved_indirect_transfer_offsets.is_empty());
    assert!(closure.scan.lui_instruction_offsets.contains(&0x40));
    assert!(closure.scan.lui_instruction_offsets.contains(&0x50));
}

#[test]
fn register_transfer_closure_repeats_when_one_target_reveals_another() {
    let data = overlay(&[
        Instruction::Lui {
            rt: Register::T0,
            immediate: 0x800a,
        },
        Instruction::Ori {
            rt: Register::T0,
            rs: Register::T0,
            immediate: 0x2014,
        },
        Instruction::Jr { rs: Register::T0 },
        Instruction::nop(),
        Instruction::Lui {
            rt: Register::T1,
            immediate: 0x800a,
        },
        Instruction::Ori {
            rt: Register::T1,
            rs: Register::T1,
            immediate: 0x2024,
        },
        Instruction::Jr { rs: Register::T1 },
        Instruction::nop(),
        Instruction::Lui {
            rt: Register::S1,
            immediate: 0x800a,
        },
    ]);
    let transfers = [
        ResolvedRegisterTransfer {
            seed_offset: 4,
            instruction_offset: 12,
            target: BASE + 0x14,
        },
        ResolvedRegisterTransfer {
            seed_offset: 20,
            instruction_offset: 28,
            target: BASE + 0x24,
        },
    ];

    let closure = close_reachable_control_transfers(&data, BASE, BASE + 4, &transfers);

    assert_eq!(closure.control_transfer_resolution_pass_count, 2);
    assert!(closure.scan.unresolved_indirect_transfer_offsets.is_empty());
    assert!(closure.scan.unresolved_indirect_transfers.is_empty());
    assert!(closure.scan.lui_instruction_offsets.contains(&36));
}

#[test]
fn first_word_is_admitted_only_when_it_points_inside_the_executable_domain() {
    let valid = overlay(&[Instruction::nop()]);
    assert_eq!(declared_overlay_entrypoint(&valid, BASE), Some(BASE + 4));

    let executable_domain =
        ExecutableDomain::from_instruction_ranges(valid.len(), std::iter::once(0..4))
            .expect("fixture code range is aligned and bounded");
    assert_eq!(
        read_entrypoint_in_domain(&valid, BASE, &executable_domain),
        None
    );
}

fn overlay(instructions: &[Instruction]) -> Vec<u8> {
    let mut data = (BASE + 4).to_le_bytes().to_vec();
    for (index, instruction) in instructions.iter().enumerate() {
        let pc = BASE + 4 + (index * 4) as u32;
        data.extend_from_slice(&psx_r3000a::encode(instruction, pc).unwrap().to_le_bytes());
    }
    data
}

fn write_instruction(data: &mut [u8], offset: usize, instruction: Instruction) {
    let pc = BASE + offset as u32;
    data[offset..offset + 4]
        .copy_from_slice(&psx_r3000a::encode(&instruction, pc).unwrap().to_le_bytes());
}
