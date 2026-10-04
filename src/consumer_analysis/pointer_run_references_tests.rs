use psx_r3000a::{Instruction, Register, encode};

use super::{audit_declared_pointer_runs, validate_pointer_run_profiles};
use crate::consumer_analysis::profiles::{
    StaticPointerRunNonAddressFlowProfile, StaticPointerRunProfile,
};
use crate::psx_static_analysis::value_flow::{
    AddressFlowSeedExhaustion, DerivedAddress, DerivedAddressKind, DerivedAddressScan,
};
use crate::source_disc::LoadedImage;

const RUNTIME_BASE: u32 = 0x800a_2000;
const POINTER_TABLE_ADDRESS: u32 = RUNTIME_BASE + 0x40;
const TARGET_ARENA_ADDRESS: u32 = RUNTIME_BASE + 0x80;

#[test]
fn pointer_run_audit_separates_table_targets_raw_words_and_reachable_flow() {
    let image = fixture_image([TARGET_ARENA_ADDRESS, TARGET_ARENA_ADDRESS + 4]);
    let flow = DerivedAddressScan {
        addresses: vec![
            DerivedAddress {
                seed_offset: 0,
                instruction_offset: 0,
                source_offset: 0,
                address: POINTER_TABLE_ADDRESS,
                kind: DerivedAddressKind::RegisterValue,
            },
            DerivedAddress {
                seed_offset: 0,
                instruction_offset: 4,
                source_offset: 4,
                address: TARGET_ARENA_ADDRESS,
                kind: DerivedAddressKind::LoadedValue {
                    storage_address: POINTER_TABLE_ADDRESS,
                    load_instruction_offset: 4,
                    width_bytes: 4,
                    sign_extended: false,
                },
            },
        ],
        resolved_register_transfers: Vec::new(),
        resolved_direct_call_arguments: Vec::new(),
        seed_count: 1,
        instruction_state_count: 2,
        budget_exhausted_seed_count: 0,
        budget_exhausted_seed_offsets: Vec::new(),
        budget_exhausted_seeds: Vec::new(),
    };

    let audits =
        audit_declared_pointer_runs(&image, Some(&flow), 64, 0, &[profile()], &[]).unwrap();
    let audit = &audits[0];

    assert_eq!(audit.decoded_targets.len(), 2);
    assert_eq!(audit.distinct_target_count, 2);
    assert_eq!(audit.raw_address_references.len(), 4);
    assert_eq!(audit.external_raw_address_reference_count, 1);
    assert_eq!(audit.reachable_derived_references.len(), 2);
    assert_eq!(
        audit.reference_assessment,
        "reachable_derived_reference_observed"
    );
    assert_eq!(
        audit.reachable_derived_references[1]
            .loaded_value_storage_runtime_address
            .as_deref(),
        Some("0x800a2040")
    );
}

#[test]
fn pointer_run_audit_rejects_a_target_outside_the_declared_arena() {
    let image = fixture_image([TARGET_ARENA_ADDRESS, TARGET_ARENA_ADDRESS + 0x20]);

    let error = audit_declared_pointer_runs(&image, None, 64, 0, &[profile()], &[])
        .expect_err("out-of-arena target must fail closed");

    assert!(error.to_string().contains("target 1 leaves its arena"));
}

#[test]
fn pointer_run_audit_keeps_an_empty_observation_open_when_flow_limits_remain() {
    let image = fixture_image([TARGET_ARENA_ADDRESS, TARGET_ARENA_ADDRESS + 4]);

    let audits = audit_declared_pointer_runs(&image, None, 64, 0, &[profile()], &[]).unwrap();

    assert_eq!(
        audits[0].reference_assessment,
        "no_reachable_derived_reference_observed_with_open_declared_entrypoint_flow"
    );
    assert!(audits[0].declared_entrypoint_flow_limits_remain);
}

#[test]
fn pointer_run_profile_validation_rejects_duplicate_physical_runs() {
    let duplicate = StaticPointerRunProfile {
        id: "duplicate",
        ..profile()
    };

    let error = validate_pointer_run_profiles(&[profile(), duplicate], &[])
        .expect_err("duplicate physical run must fail closed");

    assert!(error.to_string().contains("repeats a physical run"));
}

#[test]
fn reviewed_non_address_scalar_flow_closes_only_its_exact_exhausted_seed() {
    let mut image = non_address_fixture_image();
    let flow = non_address_flow(vec![8, 12]);
    let run = profile();
    let flow_profile = non_address_profile();

    let audits = audit_declared_pointer_runs(
        &image,
        Some(&flow),
        64,
        0,
        std::slice::from_ref(&run),
        std::slice::from_ref(&flow_profile),
    )
    .unwrap();

    assert_eq!(
        audits[0].reachable_analysis_profiled_non_address_seed_ids,
        ["synthetic_sprite_coordinate_record_loop"]
    );
    assert_eq!(
        audits[0].reachable_analysis_unprofiled_exhausted_seed_count,
        0
    );
    assert!(!audits[0].declared_entrypoint_flow_limits_remain);
    assert_eq!(
        audits[0].reference_assessment,
        "no_reachable_pointer_run_reference_in_reviewed_declared_entrypoint_flow"
    );

    let different_budget = audit_declared_pointer_runs(
        &image,
        Some(&flow),
        32,
        0,
        std::slice::from_ref(&run),
        std::slice::from_ref(&flow_profile),
    )
    .unwrap();
    assert_eq!(
        different_budget[0].reachable_analysis_profiled_non_address_exhausted_seed_count,
        0
    );
    assert_eq!(
        different_budget[0].reachable_analysis_unprofiled_exhausted_seed_count,
        1
    );
    assert!(different_budget[0].declared_entrypoint_flow_limits_remain);

    set_instruction(
        &mut image,
        0,
        Instruction::Lui {
            rt: Register::A2,
            immediate: 0x800a,
        },
    );
    let seed_error = audit_declared_pointer_runs(
        &image,
        Some(&flow),
        64,
        0,
        std::slice::from_ref(&run),
        std::slice::from_ref(&flow_profile),
    )
    .expect_err("changed seed instruction must reopen the reviewed flow");
    assert!(seed_error.to_string().contains("seed instruction changed"));

    let mut image = non_address_fixture_image();
    set_instruction(
        &mut image,
        4,
        Instruction::Lw {
            rt: Register::A3,
            base: Register::A3,
            offset: 0x2020,
        },
    );
    let load_error = audit_declared_pointer_runs(
        &image,
        Some(&flow),
        64,
        0,
        std::slice::from_ref(&run),
        std::slice::from_ref(&flow_profile),
    )
    .expect_err("changed scalar load must reopen the reviewed flow");
    assert!(
        load_error
            .to_string()
            .contains("scalar load instruction changed")
    );

    let frontier_error = audit_declared_pointer_runs(
        &non_address_fixture_image(),
        Some(&non_address_flow(vec![8])),
        64,
        0,
        &[run],
        &[flow_profile],
    )
    .expect_err("changed frontier must reopen the reviewed flow");
    assert!(frontier_error.to_string().contains("frontier changed"));
}

fn fixture_image(targets: [u32; 2]) -> LoadedImage {
    let mut data = vec![0; 0xa0];
    for (index, instruction) in [Instruction::nop(), Instruction::nop()].iter().enumerate() {
        let offset = index * 4;
        data[offset..offset + 4].copy_from_slice(
            &encode(instruction, RUNTIME_BASE + offset as u32)
                .unwrap()
                .to_le_bytes(),
        );
    }
    data[0x10..0x14].copy_from_slice(&POINTER_TABLE_ADDRESS.to_le_bytes());
    data[0x40..0x44].copy_from_slice(&targets[0].to_le_bytes());
    data[0x44..0x48].copy_from_slice(&targets[1].to_le_bytes());
    data[0x80..0x84].copy_from_slice(&TARGET_ARENA_ADDRESS.to_le_bytes());
    LoadedImage {
        path: "DAT1/SYNTH.BIN".to_string(),
        data,
        runtime_base: Some(RUNTIME_BASE),
        entrypoints: Vec::new(),
    }
}

fn non_address_fixture_image() -> LoadedImage {
    let mut image = fixture_image([TARGET_ARENA_ADDRESS, TARGET_ARENA_ADDRESS + 4]);
    set_instruction(
        &mut image,
        0,
        Instruction::Lui {
            rt: Register::A3,
            immediate: 0x800a,
        },
    );
    set_instruction(
        &mut image,
        4,
        Instruction::Lhu {
            rt: Register::A3,
            base: Register::A3,
            offset: 0x2020,
        },
    );
    set_instruction(
        &mut image,
        8,
        Instruction::Addiu {
            rt: Register::S2,
            rs: Register::S2,
            immediate: 1,
        },
    );
    set_instruction(
        &mut image,
        12,
        Instruction::Lw {
            rt: Register::FP,
            base: Register::SP,
            offset: 0x38,
        },
    );
    image.data[0x20..0x22].copy_from_slice(&7u16.to_le_bytes());
    image
}

fn non_address_flow(frontier_instruction_offsets: Vec<usize>) -> DerivedAddressScan {
    DerivedAddressScan {
        addresses: vec![DerivedAddress {
            seed_offset: 0,
            instruction_offset: 4,
            source_offset: 4,
            address: 7,
            kind: DerivedAddressKind::LoadedValue {
                storage_address: RUNTIME_BASE + 0x20,
                load_instruction_offset: 4,
                width_bytes: 2,
                sign_extended: false,
            },
        }],
        resolved_register_transfers: Vec::new(),
        resolved_direct_call_arguments: Vec::new(),
        seed_count: 1,
        instruction_state_count: 64,
        budget_exhausted_seed_count: 1,
        budget_exhausted_seed_offsets: vec![0],
        budget_exhausted_seeds: vec![AddressFlowSeedExhaustion {
            seed_offset: 0,
            processed_state_count: 64,
            discovered_state_count: 66,
            distinct_instruction_offset_count: 4,
            maximum_states_at_instruction_offset: 16,
            pending_state_count: 2,
            distinct_frontier_instruction_offset_count: frontier_instruction_offsets.len(),
            frontier_instruction_offsets,
        }],
    }
}

fn non_address_profile() -> StaticPointerRunNonAddressFlowProfile {
    StaticPointerRunNonAddressFlowProfile {
        id: "synthetic_sprite_coordinate_record_loop",
        pointer_run_id: "synthetic_pointer_run",
        image_path: "DAT1/SYNTH.BIN",
        analysis_state_budget: 64,
        seed_instruction_offset: 0,
        expected_seed_instruction: Instruction::Lui {
            rt: Register::A3,
            immediate: 0x800a,
        },
        scalar_load_instruction_offset: 4,
        expected_scalar_load_instruction: Instruction::Lhu {
            rt: Register::A3,
            base: Register::A3,
            offset: 0x2020,
        },
        scalar_storage_runtime_address: RUNTIME_BASE + 0x20,
        expected_frontier_instructions: &[
            (
                8,
                Instruction::Addiu {
                    rt: Register::S2,
                    rs: Register::S2,
                    immediate: 1,
                },
            ),
            (
                12,
                Instruction::Lw {
                    rt: Register::FP,
                    base: Register::SP,
                    offset: 0x38,
                },
            ),
        ],
    }
}

fn set_instruction(image: &mut LoadedImage, offset: usize, instruction: Instruction) {
    image.data[offset..offset + 4].copy_from_slice(
        &encode(&instruction, RUNTIME_BASE + offset as u32)
            .unwrap()
            .to_le_bytes(),
    );
}

const fn profile() -> StaticPointerRunProfile {
    StaticPointerRunProfile {
        id: "synthetic_pointer_run",
        image_path: "DAT1/SYNTH.BIN",
        pointer_table_runtime_address: POINTER_TABLE_ADDRESS,
        pointer_count: 2,
        target_arena_runtime_address: TARGET_ARENA_ADDRESS,
        target_arena_size: 0x20,
    }
}
