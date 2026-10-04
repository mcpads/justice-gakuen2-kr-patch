use psx_r3000a::{Instruction, Register};

use super::ExecutableDomain;
use super::value_flow::{
    DEFAULT_VALUE_FLOW_STATE_BUDGET, DerivedAddress, DerivedAddressKind,
    scan_derived_address_flow as scan_derived_address_flow_in_domain,
    scan_derived_address_flow_with_budget as scan_derived_address_flow_with_budget_in_domain,
    scan_derived_addresses as scan_derived_addresses_in_domain,
};

const OVERLAY_BASE: u32 = 0x800a_2000;

fn scan_derived_addresses(data: &[u8], instruction_base: u32) -> Vec<DerivedAddress> {
    let executable_domain = ExecutableDomain::full_image(data.len());
    scan_derived_addresses_in_domain(data, instruction_base, &executable_domain)
}

fn scan_derived_address_flow(
    data: &[u8],
    instruction_base: u32,
) -> super::value_flow::DerivedAddressScan {
    let executable_domain = ExecutableDomain::full_image(data.len());
    scan_derived_address_flow_in_domain(data, instruction_base, &executable_domain)
}

fn scan_derived_address_flow_with_budget(
    data: &[u8],
    instruction_base: u32,
    state_budget: usize,
) -> super::value_flow::DerivedAddressScan {
    let executable_domain = ExecutableDomain::full_image(data.len());
    scan_derived_address_flow_with_budget_in_domain(
        data,
        instruction_base,
        &executable_domain,
        state_budget,
    )
}

#[test]
fn materialized_address_survives_register_copy_and_addition() {
    let data = encode_instructions(&[
        Instruction::Lui {
            rt: Register::S0,
            immediate: 0x800a,
        },
        Instruction::Addu {
            rd: Register::S1,
            rs: Register::S0,
            rt: Register::ZERO,
        },
        Instruction::Addiu {
            rt: Register::A0,
            rs: Register::S1,
            immediate: 0x2040,
        },
    ]);

    assert!(contains_register_value(
        &scan_derived_addresses(&data, OVERLAY_BASE),
        OVERLAY_BASE + 0x40,
    ));
}

#[test]
fn unrelated_instruction_can_separate_the_address_operations() {
    let data = encode_instructions(&[
        Instruction::Lui {
            rt: Register::S0,
            immediate: 0x800a,
        },
        Instruction::Addiu {
            rt: Register::A0,
            rs: Register::ZERO,
            immediate: 2,
        },
        Instruction::Ori {
            rt: Register::S1,
            rs: Register::S0,
            immediate: 0x2040,
        },
    ]);

    assert!(contains_register_value(
        &scan_derived_addresses(&data, OVERLAY_BASE),
        OVERLAY_BASE + 0x40,
    ));
}

#[test]
fn materialized_address_can_follow_more_than_four_instructions_after_the_seed() {
    let data = encode_instructions(&[
        Instruction::Lui {
            rt: Register::S0,
            immediate: 0x800a,
        },
        Instruction::nop(),
        Instruction::nop(),
        Instruction::nop(),
        Instruction::nop(),
        Instruction::Addiu {
            rt: Register::A0,
            rs: Register::S0,
            immediate: 0x2040,
        },
    ]);

    assert!(contains_register_value(
        &scan_derived_addresses(&data, OVERLAY_BASE),
        OVERLAY_BASE + 0x40,
    ));
}

#[test]
fn flow_budget_exhaustion_reports_the_source_seed_offset() {
    let mut instructions = vec![Instruction::Lui {
        rt: Register::S0,
        immediate: 0x800a,
    }];
    instructions.extend(std::iter::repeat_n(
        Instruction::nop(),
        DEFAULT_VALUE_FLOW_STATE_BUDGET + 1,
    ));

    let scan = scan_derived_address_flow(&encode_instructions(&instructions), OVERLAY_BASE);
    assert_eq!(
        scan.instruction_state_count,
        DEFAULT_VALUE_FLOW_STATE_BUDGET
    );
    assert_eq!(scan.budget_exhausted_seed_count, 1);
    assert_eq!(scan.budget_exhausted_seed_offsets, [0]);
    assert_eq!(
        scan.budget_exhausted_seeds[0].distinct_instruction_offset_count,
        DEFAULT_VALUE_FLOW_STATE_BUDGET + 1
    );
    assert_eq!(
        scan.budget_exhausted_seeds[0].maximum_states_at_instruction_offset,
        1
    );
}

#[test]
fn cyclic_state_changes_report_many_states_at_the_same_instruction() {
    let data = encode_instructions(&[
        Instruction::Lui {
            rt: Register::S0,
            immediate: 0x800a,
        },
        Instruction::Addiu {
            rt: Register::T0,
            rs: Register::ZERO,
            immediate: 0,
        },
        Instruction::Addiu {
            rt: Register::T0,
            rs: Register::T0,
            immediate: 1,
        },
        Instruction::J {
            target: OVERLAY_BASE + 8,
        },
        Instruction::nop(),
    ]);

    let scan = scan_derived_address_flow_with_budget(&data, OVERLAY_BASE, 32);
    let exhaustion = &scan.budget_exhausted_seeds[0];
    assert_eq!(exhaustion.seed_offset, 0);
    assert!(exhaustion.distinct_instruction_offset_count <= 4);
    assert!(exhaustion.maximum_states_at_instruction_offset > 1);
}

#[test]
fn larger_state_budget_reaches_a_reference_after_a_smaller_budget_stops() {
    let mut instructions = vec![Instruction::Lui {
        rt: Register::S0,
        immediate: 0x800a,
    }];
    instructions.extend(std::iter::repeat_n(Instruction::nop(), 8));
    instructions.push(Instruction::Addiu {
        rt: Register::A0,
        rs: Register::S0,
        immediate: 0x2040,
    });
    let data = encode_instructions(&instructions);

    let bounded = scan_derived_address_flow_with_budget(&data, OVERLAY_BASE, 4);
    assert_eq!(bounded.budget_exhausted_seed_offsets, [0]);
    assert!(
        !bounded
            .addresses
            .iter()
            .any(|reference| is_register_value(reference, OVERLAY_BASE + 0x40))
    );

    let saturated = scan_derived_address_flow_with_budget(&data, OVERLAY_BASE, 16);
    assert!(saturated.budget_exhausted_seed_offsets.is_empty());
    assert!(
        saturated
            .addresses
            .iter()
            .any(|reference| is_register_value(reference, OVERLAY_BASE + 0x40))
    );
}

#[test]
fn register_transfer_preserves_the_seed_and_consumer_coordinates() {
    let data = encode_instructions(&[
        Instruction::Lui {
            rt: Register::T0,
            immediate: 0x800a,
        },
        Instruction::Ori {
            rt: Register::T0,
            rs: Register::T0,
            immediate: 0x2040,
        },
        Instruction::Jr { rs: Register::T0 },
        Instruction::nop(),
    ]);

    let scan = scan_derived_address_flow(&data, OVERLAY_BASE);

    assert_eq!(scan.resolved_register_transfers.len(), 1);
    let transfer = scan.resolved_register_transfers[0];
    assert_eq!(transfer.seed_offset, 0);
    assert_eq!(transfer.instruction_offset, 8);
    assert_eq!(transfer.target, OVERLAY_BASE + 0x40);
}

#[test]
fn direct_jump_preserves_values_at_a_same_image_target() {
    let data = encode_instructions(&[
        Instruction::Lui {
            rt: Register::S0,
            immediate: 0x800a,
        },
        Instruction::J {
            target: OVERLAY_BASE + 0x10,
        },
        Instruction::nop(),
        Instruction::nop(),
        Instruction::Addiu {
            rt: Register::A0,
            rs: Register::S0,
            immediate: 0x2040,
        },
    ]);

    assert!(contains_register_value(
        &scan_derived_addresses(&data, OVERLAY_BASE),
        OVERLAY_BASE + 0x40,
    ));
}

#[test]
fn conditional_branch_preserves_both_possible_reference_paths() {
    let data = encode_instructions(&[
        Instruction::Lui {
            rt: Register::S0,
            immediate: 0x800a,
        },
        Instruction::Beq {
            rs: Register::S1,
            rt: Register::S2,
            target: OVERLAY_BASE + 0x18,
        },
        Instruction::nop(),
        Instruction::Addiu {
            rt: Register::A0,
            rs: Register::S0,
            immediate: 0x2040,
        },
        Instruction::J {
            target: OVERLAY_BASE + 0x80,
        },
        Instruction::nop(),
        Instruction::Addiu {
            rt: Register::A1,
            rs: Register::S0,
            immediate: 0x2060,
        },
    ]);

    let addresses = scan_derived_addresses(&data, OVERLAY_BASE);
    assert!(contains_register_value(&addresses, OVERLAY_BASE + 0x40));
    assert!(contains_register_value(&addresses, OVERLAY_BASE + 0x60));
}

#[test]
fn known_taken_branch_discards_the_infeasible_fallthrough() {
    let data = encode_instructions(&[
        Instruction::Lui {
            rt: Register::S0,
            immediate: 0x800a,
        },
        Instruction::Addiu {
            rt: Register::T0,
            rs: Register::ZERO,
            immediate: 1,
        },
        Instruction::Bne {
            rs: Register::T0,
            rt: Register::ZERO,
            target: OVERLAY_BASE + 0x20,
        },
        Instruction::nop(),
        Instruction::Addiu {
            rt: Register::A0,
            rs: Register::S0,
            immediate: 0x2040,
        },
        Instruction::J {
            target: OVERLAY_BASE + 0x30,
        },
        Instruction::nop(),
        Instruction::nop(),
        Instruction::Addiu {
            rt: Register::A1,
            rs: Register::S0,
            immediate: 0x2060,
        },
    ]);

    let addresses = scan_derived_addresses(&data, OVERLAY_BASE);
    assert!(!contains_register_value(&addresses, OVERLAY_BASE + 0x40));
    assert!(contains_register_value(&addresses, OVERLAY_BASE + 0x60));
}

#[test]
fn known_not_taken_branch_discards_the_infeasible_target() {
    let data = encode_instructions(&[
        Instruction::Lui {
            rt: Register::S0,
            immediate: 0x800a,
        },
        Instruction::Addiu {
            rt: Register::T0,
            rs: Register::ZERO,
            immediate: 0,
        },
        Instruction::Bne {
            rs: Register::T0,
            rt: Register::ZERO,
            target: OVERLAY_BASE + 0x20,
        },
        Instruction::nop(),
        Instruction::Addiu {
            rt: Register::A0,
            rs: Register::S0,
            immediate: 0x2040,
        },
        Instruction::J {
            target: OVERLAY_BASE + 0x30,
        },
        Instruction::nop(),
        Instruction::nop(),
        Instruction::Addiu {
            rt: Register::A1,
            rs: Register::S0,
            immediate: 0x2060,
        },
    ]);

    let addresses = scan_derived_addresses(&data, OVERLAY_BASE);
    assert!(contains_register_value(&addresses, OVERLAY_BASE + 0x40));
    assert!(!contains_register_value(&addresses, OVERLAY_BASE + 0x60));
}

#[test]
fn shared_instruction_keeps_a_reference_that_exists_on_only_one_incoming_path() {
    let data = encode_instructions(&[
        Instruction::Lui {
            rt: Register::S0,
            immediate: 0x800a,
        },
        Instruction::Beq {
            rs: Register::S1,
            rt: Register::S2,
            target: OVERLAY_BASE + 0x18,
        },
        Instruction::nop(),
        Instruction::Addiu {
            rt: Register::S0,
            rs: Register::ZERO,
            immediate: 1,
        },
        Instruction::J {
            target: OVERLAY_BASE + 0x28,
        },
        Instruction::nop(),
        Instruction::nop(),
        Instruction::nop(),
        Instruction::nop(),
        Instruction::nop(),
        Instruction::Addiu {
            rt: Register::A0,
            rs: Register::S0,
            immediate: 0x2040,
        },
    ]);

    assert!(contains_register_value(
        &scan_derived_addresses(&data, OVERLAY_BASE),
        OVERLAY_BASE + 0x40,
    ));
}

#[test]
fn same_image_direct_call_propagates_into_the_callee_and_back_to_the_return_site() {
    let data = encode_instructions(&[
        Instruction::Lui {
            rt: Register::S0,
            immediate: 0x800a,
        },
        Instruction::Jal {
            target: OVERLAY_BASE + 0x18,
        },
        Instruction::nop(),
        Instruction::Addiu {
            rt: Register::A1,
            rs: Register::S0,
            immediate: 0x2060,
        },
        Instruction::J {
            target: OVERLAY_BASE + 0x80,
        },
        Instruction::nop(),
        Instruction::Addiu {
            rt: Register::A0,
            rs: Register::S0,
            immediate: 0x2040,
        },
        Instruction::Jr { rs: Register::RA },
        Instruction::nop(),
    ]);

    let addresses = scan_derived_addresses(&data, OVERLAY_BASE);
    assert!(contains_register_value(&addresses, OVERLAY_BASE + 0x40));
    assert!(contains_register_value(&addresses, OVERLAY_BASE + 0x60));
}

#[test]
fn direct_call_argument_preserves_its_seed_call_and_value() {
    let data = encode_instructions(&[
        Instruction::Lui {
            rt: Register::A1,
            immediate: 0x800a,
        },
        Instruction::Addiu {
            rt: Register::A1,
            rs: Register::A1,
            immediate: 0x22a0,
        },
        Instruction::Jal {
            target: OVERLAY_BASE + 0x20,
        },
        Instruction::nop(),
    ]);

    let scan = scan_derived_address_flow(&data, OVERLAY_BASE);

    assert!(scan.resolved_direct_call_arguments.iter().any(|argument| {
        argument.seed_offset == 0
            && argument.instruction_offset == 8
            && argument.target == OVERLAY_BASE + 0x20
            && argument.argument_register == Register::A1
            && argument.value == OVERLAY_BASE + 0x02a0
    }));
}

#[test]
fn direct_call_argument_uses_the_value_written_by_the_delay_slot() {
    let data = encode_instructions(&[
        Instruction::Lui {
            rt: Register::A1,
            immediate: 0x800a,
        },
        Instruction::Jal {
            target: OVERLAY_BASE + 0x20,
        },
        Instruction::Addiu {
            rt: Register::A1,
            rs: Register::A1,
            immediate: 0x22a0,
        },
    ]);

    let scan = scan_derived_address_flow(&data, OVERLAY_BASE);

    assert!(scan.resolved_direct_call_arguments.iter().any(|argument| {
        argument.seed_offset == 0
            && argument.instruction_offset == 4
            && argument.target == OVERLAY_BASE + 0x20
            && argument.argument_register == Register::A1
            && argument.value == OVERLAY_BASE + 0x02a0
    }));
    assert!(
        !scan
            .resolved_direct_call_arguments
            .iter()
            .any(|argument| argument.argument_register == Register::A1
                && argument.value == OVERLAY_BASE)
    );
}

#[test]
fn direct_call_argument_rejects_a_load_started_in_the_delay_slot() {
    let mut data = encode_instructions(&[
        Instruction::Lui {
            rt: Register::S0,
            immediate: 0x800a,
        },
        Instruction::Jal {
            target: OVERLAY_BASE + 0x20,
        },
        Instruction::Lw {
            rt: Register::A1,
            base: Register::S0,
            offset: 0x10,
        },
    ]);
    data.resize(0x14, 0);
    data[0x10..0x14].copy_from_slice(&(OVERLAY_BASE + 0x80).to_le_bytes());

    let scan = scan_derived_address_flow(&data, OVERLAY_BASE);

    assert!(
        !scan
            .resolved_direct_call_arguments
            .iter()
            .any(|argument| argument.argument_register == Register::A1)
    );
}

#[test]
fn call_return_preserves_a_callee_saved_address_for_the_next_call() {
    let mut data = vec![0u8; 0x40];
    install_instructions(
        &mut data,
        &[
            Instruction::Lui {
                rt: Register::S0,
                immediate: 0x800a,
            },
            Instruction::Jal {
                target: OVERLAY_BASE + 0x20,
            },
            Instruction::nop(),
            Instruction::Addiu {
                rt: Register::A1,
                rs: Register::S0,
                immediate: 0x22ac,
            },
            Instruction::Jal {
                target: OVERLAY_BASE + 0x30,
            },
            Instruction::nop(),
            Instruction::J {
                target: OVERLAY_BASE + 0x3c,
            },
            Instruction::nop(),
            Instruction::Addiu {
                rt: Register::S0,
                rs: Register::ZERO,
                immediate: 0,
            },
            Instruction::Jr { rs: Register::RA },
            Instruction::nop(),
        ],
    );

    let scan = scan_derived_address_flow(&data, OVERLAY_BASE);

    assert!(scan.resolved_direct_call_arguments.iter().any(|argument| {
        argument.seed_offset == 0
            && argument.instruction_offset == 0x10
            && argument.target == OVERLAY_BASE + 0x30
            && argument.argument_register == Register::A1
            && argument.value == OVERLAY_BASE + 0x02ac
    }));
}

#[test]
fn overwritten_seed_register_does_not_create_a_reference() {
    let data = encode_instructions(&[
        Instruction::Lui {
            rt: Register::S0,
            immediate: 0x800a,
        },
        Instruction::Addiu {
            rt: Register::S0,
            rs: Register::ZERO,
            immediate: 1,
        },
        Instruction::Addiu {
            rt: Register::A0,
            rs: Register::S0,
            immediate: 0x2040,
        },
    ]);

    assert!(
        !scan_derived_addresses(&data, OVERLAY_BASE)
            .iter()
            .any(|reference| reference.address == OVERLAY_BASE + 0x40)
    );
}

#[test]
fn control_transfer_stops_propagation_after_its_delay_slot() {
    let data = encode_instructions(&[
        Instruction::Lui {
            rt: Register::S0,
            immediate: 0x800a,
        },
        Instruction::J {
            target: OVERLAY_BASE + 0x40,
        },
        Instruction::nop(),
        Instruction::Addiu {
            rt: Register::A0,
            rs: Register::S0,
            immediate: 0x2040,
        },
    ]);

    assert!(
        !scan_derived_addresses(&data, OVERLAY_BASE)
            .iter()
            .any(|reference| reference.address == OVERLAY_BASE + 0x40)
    );
}

#[test]
fn delay_slot_can_finish_the_address_materialization() {
    let data = encode_instructions(&[
        Instruction::Lui {
            rt: Register::S0,
            immediate: 0x800a,
        },
        Instruction::J {
            target: OVERLAY_BASE + 0x40,
        },
        Instruction::Addiu {
            rt: Register::A0,
            rs: Register::S0,
            immediate: 0x2040,
        },
    ]);

    assert!(contains_register_value(
        &scan_derived_addresses(&data, OVERLAY_BASE),
        OVERLAY_BASE + 0x40,
    ));
}

#[test]
fn memory_access_reports_the_effective_address_and_instruction_offset() {
    let data = encode_instructions(&[
        Instruction::Lui {
            rt: Register::S0,
            immediate: 0x800a,
        },
        Instruction::Lhu {
            rt: Register::V0,
            base: Register::S0,
            offset: 0x2040,
        },
    ]);

    assert!(
        scan_derived_addresses(&data, OVERLAY_BASE).contains(&DerivedAddress {
            seed_offset: 0,
            instruction_offset: 4,
            source_offset: 4,
            address: OVERLAY_BASE + 0x40,
            kind: DerivedAddressKind::MemoryAccess,
        })
    );
}

#[test]
fn loaded_word_reports_the_pointer_and_its_storage_address() {
    let mut data = vec![0u8; 0x80];
    install_instructions(
        &mut data,
        &[
            Instruction::Lui {
                rt: Register::S0,
                immediate: 0x800a,
            },
            Instruction::Lw {
                rt: Register::A0,
                base: Register::S0,
                offset: 0x2020,
            },
        ],
    );
    data[0x20..0x24].copy_from_slice(&(OVERLAY_BASE + 0x40).to_le_bytes());

    assert!(
        scan_derived_addresses(&data, OVERLAY_BASE).contains(&DerivedAddress {
            seed_offset: 0,
            instruction_offset: 4,
            source_offset: 4,
            address: OVERLAY_BASE + 0x40,
            kind: DerivedAddressKind::LoadedValue {
                storage_address: OVERLAY_BASE + 0x20,
                load_instruction_offset: 4,
                width_bytes: 4,
                sign_extended: false,
            },
        })
    );
}

#[test]
fn scalar_loads_preserve_width_and_sign_extension() {
    let mut data = vec![0u8; 0xa0];
    install_instructions(
        &mut data,
        &[
            Instruction::Lui {
                rt: Register::S0,
                immediate: 0x800a,
            },
            Instruction::Addiu {
                rt: Register::S0,
                rs: Register::S0,
                immediate: 0x2080,
            },
            Instruction::Lb {
                rt: Register::T0,
                base: Register::S0,
                offset: 0,
            },
            Instruction::Lbu {
                rt: Register::T1,
                base: Register::S0,
                offset: 0,
            },
            Instruction::Lh {
                rt: Register::T2,
                base: Register::S0,
                offset: 2,
            },
            Instruction::Lhu {
                rt: Register::T3,
                base: Register::S0,
                offset: 2,
            },
        ],
    );
    data[0x80] = 0x80;
    data[0x82..0x84].copy_from_slice(&0x8000u16.to_le_bytes());

    let references = scan_derived_addresses(&data, OVERLAY_BASE);

    assert!(contains_loaded_value(&references, 0xffff_ff80, 1, true));
    assert!(contains_loaded_value(&references, 0x80, 1, false));
    assert!(contains_loaded_value(&references, 0xffff_8000, 2, true));
    assert!(contains_loaded_value(&references, 0x8000, 2, false));
}

#[test]
fn loaded_halfword_loop_count_bounds_reachable_memory_addresses() {
    let mut data = vec![0u8; 0xa0];
    install_instructions(
        &mut data,
        &[
            Instruction::Lui {
                rt: Register::S0,
                immediate: 0x800a,
            },
            Instruction::Addiu {
                rt: Register::S0,
                rs: Register::S0,
                immediate: 0x2080,
            },
            Instruction::Lh {
                rt: Register::S1,
                base: Register::S0,
                offset: 0,
            },
            Instruction::Addiu {
                rt: Register::S0,
                rs: Register::S0,
                immediate: 2,
            },
            Instruction::Addiu {
                rt: Register::T0,
                rs: Register::ZERO,
                immediate: 0,
            },
            Instruction::Lh {
                rt: Register::V0,
                base: Register::S0,
                offset: 0,
            },
            Instruction::Addiu {
                rt: Register::S0,
                rs: Register::S0,
                immediate: 2,
            },
            Instruction::Addiu {
                rt: Register::T0,
                rs: Register::T0,
                immediate: 1,
            },
            Instruction::Slt {
                rd: Register::V1,
                rs: Register::T0,
                rt: Register::S1,
            },
            Instruction::Bne {
                rs: Register::V1,
                rt: Register::ZERO,
                target: OVERLAY_BASE + 0x14,
            },
            Instruction::nop(),
            Instruction::Jr { rs: Register::RA },
            Instruction::nop(),
        ],
    );
    data[0x80..0x82].copy_from_slice(&2u16.to_le_bytes());
    data[0x82..0x84].copy_from_slice(&10u16.to_le_bytes());
    data[0x84..0x86].copy_from_slice(&20u16.to_le_bytes());

    let scan = scan_derived_address_flow(&data, OVERLAY_BASE);
    let loop_addresses = scan
        .addresses
        .iter()
        .filter(|reference| {
            reference.instruction_offset == 0x14
                && reference.kind == DerivedAddressKind::MemoryAccess
        })
        .map(|reference| reference.address)
        .collect::<Vec<_>>();

    assert_eq!(loop_addresses, [OVERLAY_BASE + 0x82, OVERLAY_BASE + 0x84]);
    assert_eq!(scan.budget_exhausted_seed_count, 0);
}

#[test]
fn load_does_not_report_the_destination_registers_stale_value_as_new_materialization() {
    let data = encode_instructions(&[
        Instruction::Lui {
            rt: Register::S0,
            immediate: 0x800a,
        },
        Instruction::Ori {
            rt: Register::S0,
            rs: Register::S0,
            immediate: 0x2040,
        },
        Instruction::Lw {
            rt: Register::S0,
            base: Register::SP,
            offset: 0,
        },
    ]);

    assert!(
        !scan_derived_addresses(&data, OVERLAY_BASE)
            .iter()
            .any(|reference| reference.instruction_offset == 8
                && reference.kind == DerivedAddressKind::RegisterValue)
    );
}

#[test]
fn loaded_word_is_not_propagated_into_the_load_delay_slot() {
    let mut data = vec![0u8; 0x80];
    install_instructions(
        &mut data,
        &[
            Instruction::Lui {
                rt: Register::S0,
                immediate: 0x800a,
            },
            Instruction::Lw {
                rt: Register::A0,
                base: Register::S0,
                offset: 0x2020,
            },
            Instruction::Addiu {
                rt: Register::A1,
                rs: Register::A0,
                immediate: 0x20,
            },
        ],
    );
    data[0x20..0x24].copy_from_slice(&(OVERLAY_BASE + 0x40).to_le_bytes());

    assert!(
        !scan_derived_addresses(&data, OVERLAY_BASE)
            .iter()
            .any(|reference| reference.address == OVERLAY_BASE + 0x60)
    );
}

#[test]
fn loaded_word_is_propagated_after_the_load_delay_slot() {
    let mut data = vec![0u8; 0x90];
    install_instructions(
        &mut data,
        &[
            Instruction::Lui {
                rt: Register::S0,
                immediate: 0x800a,
            },
            Instruction::Lw {
                rt: Register::A0,
                base: Register::S0,
                offset: 0x2020,
            },
            Instruction::nop(),
            Instruction::Addiu {
                rt: Register::A1,
                rs: Register::A0,
                immediate: 0x20,
            },
        ],
    );
    data[0x20..0x24].copy_from_slice(&(OVERLAY_BASE + 0x40).to_le_bytes());

    assert!(
        scan_derived_addresses(&data, OVERLAY_BASE).contains(&DerivedAddress {
            seed_offset: 0,
            instruction_offset: 12,
            source_offset: 12,
            address: OVERLAY_BASE + 0x60,
            kind: DerivedAddressKind::LoadedValue {
                storage_address: OVERLAY_BASE + 0x20,
                load_instruction_offset: 4,
                width_bytes: 4,
                sign_extended: false,
            },
        })
    );
}

#[test]
fn delay_slot_write_discards_the_pending_loaded_word() {
    let mut data = vec![0u8; 0x90];
    install_instructions(
        &mut data,
        &[
            Instruction::Lui {
                rt: Register::S0,
                immediate: 0x800a,
            },
            Instruction::Lw {
                rt: Register::A0,
                base: Register::S0,
                offset: 0x2020,
            },
            Instruction::Addiu {
                rt: Register::A0,
                rs: Register::ZERO,
                immediate: 1,
            },
            Instruction::Addiu {
                rt: Register::A1,
                rs: Register::A0,
                immediate: 0x20,
            },
        ],
    );
    data[0x20..0x24].copy_from_slice(&(OVERLAY_BASE + 0x40).to_le_bytes());

    assert!(
        !scan_derived_addresses(&data, OVERLAY_BASE)
            .iter()
            .any(|reference| reference.address == OVERLAY_BASE + 0x60)
    );
}

#[test]
fn value_flow_does_not_decode_data_outside_the_executable_domain() {
    let data = encode_instructions(&[
        Instruction::Lui {
            rt: Register::S0,
            immediate: 0x800a,
        },
        Instruction::Addiu {
            rt: Register::S0,
            rs: Register::S0,
            immediate: 0x2040,
        },
        Instruction::Addiu {
            rt: Register::ZERO,
            rs: Register::ZERO,
            immediate: 0,
        },
        Instruction::Addiu {
            rt: Register::ZERO,
            rs: Register::ZERO,
            immediate: 0,
        },
        Instruction::Lui {
            rt: Register::S1,
            immediate: 0x800a,
        },
        Instruction::Addiu {
            rt: Register::S1,
            rs: Register::S1,
            immediate: 0x2060,
        },
    ]);
    let executable_domain =
        ExecutableDomain::from_instruction_offsets(data.len(), [0, 4, 8]).unwrap();

    let scan = scan_derived_address_flow_in_domain(&data, OVERLAY_BASE, &executable_domain);

    assert_eq!(scan.seed_count, 1);
    assert!(contains_register_value(
        &scan.addresses,
        OVERLAY_BASE + 0x40
    ));
    assert!(!contains_register_value(
        &scan.addresses,
        OVERLAY_BASE + 0x60
    ));
}

fn contains_register_value(addresses: &[DerivedAddress], address: u32) -> bool {
    addresses
        .iter()
        .any(|reference| is_register_value(reference, address))
}

fn is_register_value(reference: &DerivedAddress, address: u32) -> bool {
    reference.address == address && reference.kind == DerivedAddressKind::RegisterValue
}

fn contains_loaded_value(
    references: &[DerivedAddress],
    value: u32,
    width_bytes: u8,
    sign_extended: bool,
) -> bool {
    references.iter().any(|reference| {
        reference.address == value
            && matches!(
                reference.kind,
                DerivedAddressKind::LoadedValue {
                    width_bytes: actual_width,
                    sign_extended: actual_sign_extension,
                    ..
                } if actual_width == width_bytes && actual_sign_extension == sign_extended
            )
    })
}

fn encode_instructions(instructions: &[Instruction]) -> Vec<u8> {
    let mut data = Vec::with_capacity(instructions.len() * 4);
    for (index, instruction) in instructions.iter().enumerate() {
        let pc = OVERLAY_BASE + (index * 4) as u32;
        let encoded = psx_r3000a::encode(instruction, pc).unwrap();
        data.extend_from_slice(&encoded.to_le_bytes());
    }
    data
}

fn install_instructions(data: &mut [u8], instructions: &[Instruction]) {
    let encoded = encode_instructions(instructions);
    data[..encoded.len()].copy_from_slice(&encoded);
}
