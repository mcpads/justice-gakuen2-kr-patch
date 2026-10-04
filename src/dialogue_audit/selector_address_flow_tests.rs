use psx_r3000a::{Instruction, Register, encode};

use crate::psx_static_analysis::value_flow::{
    DerivedAddress, DerivedAddressKind, ResolvedRegisterTransfer,
};

use super::selector_address_flow::{
    direct_calls_to, resolved_register_calls_to, selector_region_high_half_seeds,
    selector_table_base_materializations,
};
use super::selector_address_flow_model::{
    DialogueMessageConstructorCall, DialogueResolvedMessageConstructorCall,
    DialogueSelectorPointerLoad, DialogueSelectorRegionSeed,
    DialogueSelectorTableBaseMaterialization, DialogueTypedInstruction,
};

const MGAME_BASE: u32 = 0x800a_2000;

#[test]
fn reports_each_table_base_seed_once_without_claiming_table_slots() {
    let references = vec![
        reference(0x100, 0x104, 0x8010_1000),
        reference(0x100, 0x108, 0x8010_1000),
        reference(0x200, 0x204, 0x8010_1004),
        reference(0x300, 0x304, 0x8010_1020),
    ];

    assert_eq!(
        selector_table_base_materializations(&references).unwrap(),
        vec![DialogueSelectorTableBaseMaterialization {
            table_base_runtime_address: "0x80101000".to_string(),
            seed_instruction_runtime_address: "0x800a2100".to_string(),
            materialization_instruction_runtime_address: "0x800a2104".to_string(),
        }]
    );
}

#[test]
fn reports_only_resolved_register_transfers_to_the_requested_target() {
    let target = 0x800b_84d4;
    let transfers = vec![
        ResolvedRegisterTransfer {
            seed_offset: 0x100,
            instruction_offset: 0x120,
            target,
        },
        ResolvedRegisterTransfer {
            seed_offset: 0x200,
            instruction_offset: 0x220,
            target: target + 4,
        },
    ];

    assert_eq!(
        resolved_register_calls_to(&transfers, target).unwrap(),
        vec![DialogueResolvedMessageConstructorCall {
            seed_instruction_runtime_address: "0x800a2100".to_string(),
            call_instruction_runtime_address: "0x800a2120".to_string(),
        }]
    );
}

#[test]
fn reports_nonzero_lui_seeds_for_the_selector_runtime_region() {
    let mut mgame = vec![0u8; 16];
    write_instruction(
        &mut mgame,
        0,
        Instruction::Lui {
            rt: Register::A0,
            immediate: 0x8010,
        },
    );
    write_instruction(
        &mut mgame,
        4,
        Instruction::Lui {
            rt: Register::ZERO,
            immediate: 0x8010,
        },
    );
    write_instruction(
        &mut mgame,
        8,
        Instruction::Lui {
            rt: Register::A1,
            immediate: 0x800f,
        },
    );
    write_instruction(
        &mut mgame,
        12,
        Instruction::Addiu {
            rt: Register::A0,
            rs: Register::A0,
            immediate: 0x1000,
        },
    );

    assert_eq!(
        selector_region_high_half_seeds(&mgame).unwrap(),
        vec![DialogueSelectorRegionSeed {
            seed_instruction_runtime_address: "0x800a2000".to_string(),
            diagnostic_use_window: vec![
                DialogueTypedInstruction {
                    runtime_address: "0x800a2000".to_string(),
                    instruction: format!(
                        "{:?}",
                        Instruction::Lui {
                            rt: Register::A0,
                            immediate: 0x8010,
                        }
                    ),
                },
                DialogueTypedInstruction {
                    runtime_address: "0x800a2004".to_string(),
                    instruction: format!(
                        "{:?}",
                        Instruction::Lui {
                            rt: Register::ZERO,
                            immediate: 0x8010,
                        }
                    ),
                },
                DialogueTypedInstruction {
                    runtime_address: "0x800a2008".to_string(),
                    instruction: format!(
                        "{:?}",
                        Instruction::Lui {
                            rt: Register::A1,
                            immediate: 0x800f,
                        }
                    ),
                },
                DialogueTypedInstruction {
                    runtime_address: "0x800a200c".to_string(),
                    instruction: format!(
                        "{:?}",
                        Instruction::Addiu {
                            rt: Register::A0,
                            rs: Register::A0,
                            immediate: 0x1000,
                        }
                    ),
                },
            ],
        }]
    );
}

#[test]
fn reports_only_direct_calls_to_the_requested_runtime_target() {
    let target = 0x800b_84d4;
    let mut mgame = vec![0u8; 12];
    write_instruction(&mut mgame, 0, Instruction::Jal { target });
    write_instruction(&mut mgame, 4, Instruction::Jal { target: target + 4 });
    write_instruction(
        &mut mgame,
        8,
        Instruction::Jalr {
            rd: Register::RA,
            rs: Register::V0,
        },
    );

    assert_eq!(
        direct_calls_to(&mgame, target, &[]).unwrap(),
        vec![DialogueMessageConstructorCall {
            call_instruction_runtime_address: "0x800a2000".to_string(),
            diagnostic_setup_window: vec![
                DialogueTypedInstruction {
                    runtime_address: "0x800a2000".to_string(),
                    instruction: format!("{:?}", Instruction::Jal { target }),
                },
                DialogueTypedInstruction {
                    runtime_address: "0x800a2004".to_string(),
                    instruction: format!("{:?}", Instruction::Jal { target: target + 4 }),
                },
            ],
            enclosing_function_runtime_address: None,
            direct_enclosing_function_call_count: 0,
            direct_enclosing_function_calls: Vec::new(),
            enclosing_function_selector_pointer_load_count: 0,
            enclosing_function_selector_pointer_loads: Vec::new(),
        }]
    );
}

#[test]
fn binds_selector_pointer_loads_from_the_enclosing_function_to_a_direct_call() {
    let target = 0x800b_8534;
    let mut mgame = vec![0u8; 24];
    write_instruction(
        &mut mgame,
        0,
        Instruction::Addiu {
            rt: Register::SP,
            rs: Register::SP,
            immediate: -24,
        },
    );
    write_instruction(
        &mut mgame,
        4,
        Instruction::Lui {
            rt: Register::V1,
            immediate: 0x8010,
        },
    );
    write_instruction(
        &mut mgame,
        8,
        Instruction::Lw {
            rt: Register::V1,
            base: Register::V1,
            offset: 0x1008,
        },
    );
    write_instruction(&mut mgame, 12, Instruction::Jal { target });
    let addresses = vec![DerivedAddress {
        seed_offset: 4,
        instruction_offset: 8,
        source_offset: 8,
        address: 0x8010_1008,
        kind: DerivedAddressKind::MemoryAccess,
    }];

    let calls = direct_calls_to(&mgame, target, &addresses).unwrap();

    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].enclosing_function_selector_pointer_load_count, 1);
    assert_eq!(
        calls[0].enclosing_function_selector_pointer_loads,
        vec![DialogueSelectorPointerLoad {
            selector_index: 2,
            pointer_storage_runtime_address: "0x80101008".to_string(),
            load_instruction_runtime_address: "0x800a2008".to_string(),
        }]
    );
}

fn reference(seed_offset: usize, instruction_offset: usize, address: u32) -> DerivedAddress {
    DerivedAddress {
        seed_offset,
        instruction_offset,
        source_offset: seed_offset,
        address,
        kind: DerivedAddressKind::RegisterValue,
    }
}

fn write_instruction(data: &mut [u8], offset: usize, instruction: Instruction) {
    let word = encode(&instruction, MGAME_BASE + offset as u32).unwrap();
    data[offset..offset + 4].copy_from_slice(&word.to_le_bytes());
}
