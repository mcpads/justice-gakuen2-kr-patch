use super::{
    AdditionalEntrypoint, AdditionalEntrypointSource, IndexedTaskDispatch, LoadedImageProfile,
    MainDispatchedEntrypoint, TaskSelectorStore, dat1_runtime_base, loaded_image_entrypoints,
    validated_main_dispatched_entrypoints,
};
use psx_r3000a::{Instruction, Register, encode};

#[test]
fn source_bound_overlay_layout_keeps_distinct_runtime_regions() {
    assert_eq!(
        dat1_runtime_base("DAT1/NEWOPT.BIN").unwrap(),
        Some(0x800a_2000)
    );
    assert_eq!(
        dat1_runtime_base("DAT1/MGAME061.BIN").unwrap(),
        Some(0x8015_0000)
    );
    assert_eq!(
        dat1_runtime_base("DAT1/SIKENG21.BIN").unwrap(),
        Some(0x8015_2000)
    );
    assert_eq!(
        dat1_runtime_base("DAT1/SIKENGO1.BIN").unwrap(),
        Some(0x8015_2000)
    );
    assert_eq!(
        dat1_runtime_base("DAT1/MGENT.BIN").unwrap(),
        Some(0x8017_a000)
    );
    assert_eq!(
        dat1_runtime_base("DAT1/PASS.BIN").unwrap(),
        Some(0x8017_a000)
    );
    assert_eq!(
        dat1_runtime_base("DAT1/SIKENG22.BIN").unwrap(),
        Some(0x8017_a000)
    );
    assert_eq!(
        dat1_runtime_base("DAT1/SIKENGO2.BIN").unwrap(),
        Some(0x8017_a000)
    );
    assert_eq!(dat1_runtime_base("DAT1/EM_ETBL.BIN").unwrap(), None);
    assert!(dat1_runtime_base("DAT1/NEW_FILE.BIN").is_err());
}

#[test]
fn additional_entrypoint_requires_its_exact_header_pointer() {
    const BASE: u32 = 0x8015_2000;
    let profile = LoadedImageProfile {
        path: "DAT1/SYNTH.BIN",
        runtime_base: Some(BASE),
        additional_entrypoints: &[AdditionalEntrypoint {
            role: "callback",
            instruction_offset: 0x20,
            source: AdditionalEntrypointSource::LoadedImageHeaderPointer { pointer_offset: 4 },
        }],
    };
    let mut data = vec![0; 0x24];
    data[..4].copy_from_slice(&(BASE + 0x10).to_le_bytes());
    data[4..8].copy_from_slice(&(BASE + 0x20).to_le_bytes());

    let entrypoints = loaded_image_entrypoints(&profile, &data).unwrap();
    assert_eq!(
        entrypoints
            .iter()
            .map(|entrypoint| (
                entrypoint.role,
                entrypoint.source_reference_kind,
                entrypoint.source_reference_offset,
                entrypoint.runtime_address,
            ))
            .collect::<Vec<_>>(),
        [
            (
                "primary_header_entrypoint",
                "loaded_image_header_pointer",
                0,
                BASE + 0x10,
            ),
            ("callback", "loaded_image_header_pointer", 4, BASE + 0x20,),
        ]
    );

    data[4..8].copy_from_slice(&(BASE + 0x24).to_le_bytes());
    assert!(loaded_image_entrypoints(&profile, &data).is_err());
}

#[test]
fn indexed_task_entrypoint_requires_the_dispatch_and_selector_sources() {
    const BASE: u32 = 0x8010_0000;
    const TABLE_OFFSET: usize = 0x40;
    const SELECTOR: u8 = 3;
    const POINTER_OFFSET: usize = TABLE_OFFSET + SELECTOR as usize * 4;
    const CALL_OFFSET: usize = 0x90;
    const DISPATCHER_OFFSET: usize = 0x100;
    const CALLBACK_OFFSET: usize = 0x180;
    const LITERAL_OFFSET: usize = 0x1bc;
    const STORE_OFFSET: usize = 0x1c0;
    let profile = LoadedImageProfile {
        path: "DAT1/SYNTH.BIN",
        runtime_base: Some(BASE),
        additional_entrypoints: &[AdditionalEntrypoint {
            role: "task_callback",
            instruction_offset: CALLBACK_OFFSET,
            source: AdditionalEntrypointSource::IndexedTaskDispatchTablePointer {
                pointer_offset: POINTER_OFFSET,
                dispatch: IndexedTaskDispatch {
                    dispatcher_call_offset: CALL_OFFSET,
                    dispatcher_offset: DISPATCHER_OFFSET,
                    table_base_offset: TABLE_OFFSET,
                    selector: SELECTOR,
                    selector_stores: &[TaskSelectorStore {
                        literal_offset: LITERAL_OFFSET,
                        store_offset: STORE_OFFSET,
                    }],
                },
            },
        }],
    };
    let mut data = vec![0; STORE_OFFSET + 4];
    data[..4].copy_from_slice(&(BASE + 0x20).to_le_bytes());
    data[POINTER_OFFSET..POINTER_OFFSET + 4]
        .copy_from_slice(&(BASE + CALLBACK_OFFSET as u32).to_le_bytes());
    for (offset, instruction) in [
        (
            CALL_OFFSET,
            Instruction::Jal {
                target: BASE + DISPATCHER_OFFSET as u32,
            },
        ),
        (
            DISPATCHER_OFFSET + 0x1c,
            Instruction::Lui {
                rt: Register::S1,
                immediate: (BASE >> 16) as u16,
            },
        ),
        (
            DISPATCHER_OFFSET + 0x20,
            Instruction::Addiu {
                rt: Register::S1,
                rs: Register::S1,
                immediate: TABLE_OFFSET as i16,
            },
        ),
        (
            DISPATCHER_OFFSET + 0x34,
            Instruction::Lbu {
                rt: Register::V0,
                base: Register::S0,
                offset: 2,
            },
        ),
        (
            DISPATCHER_OFFSET + 0x3c,
            Instruction::Sll {
                rd: Register::V0,
                rt: Register::V0,
                shift: 2,
            },
        ),
        (
            DISPATCHER_OFFSET + 0x40,
            Instruction::Addu {
                rd: Register::V0,
                rs: Register::V0,
                rt: Register::S1,
            },
        ),
        (
            DISPATCHER_OFFSET + 0x44,
            Instruction::Lw {
                rt: Register::V0,
                base: Register::V0,
                offset: 0,
            },
        ),
        (
            DISPATCHER_OFFSET + 0x4c,
            Instruction::Jalr {
                rd: Register::RA,
                rs: Register::V0,
            },
        ),
        (
            LITERAL_OFFSET,
            Instruction::Addiu {
                rt: Register::V0,
                rs: Register::ZERO,
                immediate: i16::from(SELECTOR),
            },
        ),
        (
            STORE_OFFSET,
            Instruction::Sb {
                rt: Register::V0,
                base: Register::V1,
                offset: 2,
            },
        ),
    ] {
        data[offset..offset + 4].copy_from_slice(
            &encode(&instruction, BASE + offset as u32)
                .unwrap()
                .to_le_bytes(),
        );
    }

    let entrypoints = loaded_image_entrypoints(&profile, &data).unwrap();
    assert_eq!(entrypoints.len(), 2);
    assert_eq!(
        entrypoints[1].source_reference_kind,
        "indexed_task_dispatch_table_pointer"
    );
    assert_eq!(entrypoints[1].source_reference_offset, POINTER_OFFSET);
    assert_eq!(
        entrypoints[1].runtime_address,
        BASE + CALLBACK_OFFSET as u32
    );

    data[DISPATCHER_OFFSET + 0x34..DISPATCHER_OFFSET + 0x38].fill(0);
    assert!(loaded_image_entrypoints(&profile, &data).is_err());
}

#[test]
fn main_dispatched_entrypoint_requires_its_exact_direct_call() {
    const CALL_OFFSET: usize = 0x10;
    const TARGET_OFFSET: usize = 0x20;
    let profile = [MainDispatchedEntrypoint {
        role: "mode_setup",
        call_instruction_offset: CALL_OFFSET,
        instruction_offset: TARGET_OFFSET,
    }];
    let target = super::MAIN_TEXT_RUNTIME_BASE + TARGET_OFFSET as u32;
    let mut text = vec![0; TARGET_OFFSET + 4];
    text[CALL_OFFSET..CALL_OFFSET + 4].copy_from_slice(
        &encode(
            &Instruction::Jal { target },
            super::MAIN_TEXT_RUNTIME_BASE + CALL_OFFSET as u32,
        )
        .unwrap()
        .to_le_bytes(),
    );

    let entrypoints = validated_main_dispatched_entrypoints(&text, &profile).unwrap();

    assert_eq!(entrypoints.len(), 1);
    assert_eq!(entrypoints[0].source_reference_kind, "direct_dispatch_call");
    assert_eq!(entrypoints[0].source_reference_offset, CALL_OFFSET);
    assert_eq!(entrypoints[0].runtime_address, target);

    text[CALL_OFFSET..CALL_OFFSET + 4].fill(0);
    assert!(validated_main_dispatched_entrypoints(&text, &profile).is_err());
}
