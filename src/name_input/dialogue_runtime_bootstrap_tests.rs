use psx_r3000a::{Instruction, Register, verify_placed_program};

use super::dialogue_runtime::build_mgame_runtime_repair_program;
use super::dialogue_runtime_bootstrap::{
    NICKNAME_HUD_PERSISTENT_CELL_ORIGIN, nickname_hud_persistent_cell_address,
    nickname_hud_reused_scene_cell_address,
};
use super::{
    MgameRuntimeRepairProgram, NAME_DIALOGUE_RUNTIME_BOOTSTRAP_ORIGIN,
    NAME_DIALOGUE_RUNTIME_ORIGIN, NAME_DIALOGUE_RUNTIME_STORAGE_ORIGIN,
    NICKNAME_HUD_STORAGE_OFFSET, build_name_dialogue_runtime_bootstrap_program,
};

const NICKNAME_HUD_SCALER_ADDRESS: u32 = 0x800c_bf00;

#[test]
fn dialogue_runtime_bootstrap_copies_words_and_restores_the_entry_delay_slot() {
    let repair = runtime_repair();
    let program =
        build_name_dialogue_runtime_bootstrap_program(1_704, NICKNAME_HUD_SCALER_ADDRESS, &repair)
            .unwrap();
    let instructions =
        verify_placed_program(&program.bytes, NAME_DIALOGUE_RUNTIME_BOOTSTRAP_ORIGIN).unwrap();

    assert_eq!(program.report.source_address, "0x80097ad0");
    assert_eq!(program.report.destination_address, "0x8009a400");
    assert_eq!(program.report.copied_byte_count, 1_704);
    assert!(program.report.restores_entry_delay_slot);
    assert!(instructions.contains(&Instruction::Lw {
        rt: Register::T3,
        base: Register::T0,
        offset: 0,
    }));
    assert!(instructions.contains(&Instruction::Sw {
        rt: Register::T3,
        base: Register::T1,
        offset: 0,
    }));
    assert!(instructions.contains(&Instruction::Addiu {
        rt: Register::V0,
        rs: Register::V0,
        immediate: -26_680,
    }));
    assert_eq!(program.report.typed_instruction_count, instructions.len());
    assert_eq!(NAME_DIALOGUE_RUNTIME_STORAGE_ORIGIN, 0x8009_7ad0);
    assert_eq!(NAME_DIALOGUE_RUNTIME_ORIGIN, 0x8009_a400);
}

#[test]
fn nickname_hud_cells_distinguish_private_storage_from_reused_scene_ram() {
    assert_eq!(nickname_hud_reused_scene_cell_address(11), None);
    assert_eq!(nickname_hud_persistent_cell_address(11), None);
    assert_eq!(
        (12..=15)
            .map(|slot| nickname_hud_reused_scene_cell_address(slot).unwrap())
            .collect::<Vec<_>>(),
        [0x8016_63a0, 0x8016_6468, 0x8016_6530, 0x8016_66c0]
    );
    assert_eq!(
        (12..=15)
            .map(|slot| nickname_hud_persistent_cell_address(slot).unwrap())
            .collect::<Vec<_>>(),
        [0x8009_84b0, 0x8009_8578, 0x8009_8640, 0x8009_8708]
    );
    assert_eq!(nickname_hud_reused_scene_cell_address(16), None);
    assert_eq!(nickname_hud_persistent_cell_address(16), None);
}

#[test]
fn nickname_hud_store_writes_private_persistent_cells_only() {
    let repair = runtime_repair();
    let program =
        build_name_dialogue_runtime_bootstrap_program(1_704, NICKNAME_HUD_SCALER_ADDRESS, &repair)
            .unwrap();
    let instructions =
        verify_placed_program(&program.bytes, NAME_DIALOGUE_RUNTIME_BOOTSTRAP_ORIGIN).unwrap();

    assert_eq!(
        instructions
            .iter()
            .filter(|instruction| {
                **instruction
                    == Instruction::Jal {
                        target: NICKNAME_HUD_SCALER_ADDRESS,
                    }
            })
            .count(),
        1
    );
    assert_eq!(
        program.nickname_hud_scaler_address,
        NICKNAME_HUD_SCALER_ADDRESS
    );
    assert_eq!(program.report.nickname_hud_scaler_address, "0x800cbf00");
    assert!(!program.report.nickname_hud_writes_reused_scene_cells);
    assert!(instructions.windows(2).any(|window| {
        window == psx_r3000a::load_address(Register::T1, NICKNAME_HUD_PERSISTENT_CELL_ORIGIN)
    }));
    assert!(
        !instructions
            .windows(2)
            .any(|window| { window == psx_r3000a::load_address(Register::T4, 0x8014_d148) })
    );
}

#[test]
fn dialogue_runtime_cannot_overlap_the_persistent_nickname_cells() {
    let repair = runtime_repair();
    assert!(
        build_name_dialogue_runtime_bootstrap_program(
            NICKNAME_HUD_STORAGE_OFFSET,
            NICKNAME_HUD_SCALER_ADDRESS,
            &repair,
        )
        .is_ok()
    );
    assert!(
        build_name_dialogue_runtime_bootstrap_program(
            NICKNAME_HUD_STORAGE_OFFSET + 4,
            NICKNAME_HUD_SCALER_ADDRESS,
            &repair,
        )
        .is_err()
    );
}

fn runtime_repair() -> MgameRuntimeRepairProgram {
    let mut runtime = vec![0_u8; 2_296];
    runtime[0x0578..0x057c].copy_from_slice(&0x8fa8_0070_u32.to_le_bytes());
    for (index, word) in [
        0x8fb6_0040_u32,
        0x8fb5_0044,
        0x8fb4_0048,
        0x8fb3_004c,
        0x8fb2_0050,
        0x8fb1_0054,
        0x8fb0_0058,
        0x8fbf_005c,
        0x27bd_0060,
        0x03e0_0008,
    ]
    .into_iter()
    .enumerate()
    {
        let offset = 0x0688 + index * 4;
        runtime[offset..offset + 4].copy_from_slice(&word.to_le_bytes());
    }
    build_mgame_runtime_repair_program(&runtime, 0x8009_ab98).unwrap()
}
