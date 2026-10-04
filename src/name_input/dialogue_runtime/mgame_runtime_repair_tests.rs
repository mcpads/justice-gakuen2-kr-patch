use psx_r3000a::{Instruction, Register, load_address, verify_placed_program};

use super::NAME_DIALOGUE_RUNTIME_ORIGIN;
use super::mgame_runtime_repair::{
    MGAME_RUNTIME_REPAIR_HELPER_BYTE_CAPACITY, MGAME_RUNTIME_REPAIR_HELPER_ORIGIN,
    MGAME_RUNTIME_REPAIR_LITERAL_BYTE_CAPACITY, MGAME_RUNTIME_REPAIR_LITERAL_ORIGIN,
    apply_mgame_runtime_destination_repair, build_mgame_runtime_repair_program,
};

const TEST_NICKNAME_HUD_UPLOADER_ADDRESS: u32 = 0x8009_ab98;

#[test]
fn repair_program_models_first_copy_and_later_destination_repair() {
    let expected = synthetic_runtime();
    let program =
        build_mgame_runtime_repair_program(&expected, TEST_NICKNAME_HUD_UPLOADER_ADDRESS).unwrap();
    let first_entry_destination = expected.clone();
    let mut later_destination = first_entry_destination.clone();
    later_destination[..4].copy_from_slice(&0x800d_0180_u32.to_le_bytes());
    later_destination[0x0578..0x057c].fill(0);
    later_destination[0x0688..0x06b0].fill(0);

    apply_mgame_runtime_destination_repair(
        &mut later_destination,
        u32::from_le_bytes(expected[0x578..0x57c].try_into().unwrap()),
        &program.literal_bytes,
    )
    .unwrap();

    assert_eq!(first_entry_destination, expected);
    assert_eq!(&later_destination[..4], &0x800d_0180_u32.to_le_bytes());
    assert_eq!(&later_destination[4..], &expected[4..]);
    assert_eq!(program.literal_bytes.len(), 40);
    assert_eq!(
        program.report.repair_ranges[0].destination_address_range,
        ["0x8009a978", "0x8009a97c"]
    );
    assert_eq!(
        program.report.repair_ranges[1].destination_address_range,
        ["0x8009aa88", "0x8009aab0"]
    );
    assert!(program.report.first_entry_copy_roundtrip_verified);
    assert!(program.report.destination_repair_roundtrip_verified);
}

#[test]
fn destination_repair_fits_its_gaps_and_uploads_nickname_before_mgame_returns() {
    let program = build_mgame_runtime_repair_program(
        &synthetic_runtime(),
        TEST_NICKNAME_HUD_UPLOADER_ADDRESS,
    )
    .unwrap();
    let repair_instructions =
        verify_placed_program(&program.helper_bytes, MGAME_RUNTIME_REPAIR_HELPER_ORIGIN).unwrap();

    assert_eq!(
        program.helper_bytes.len(),
        MGAME_RUNTIME_REPAIR_HELPER_BYTE_CAPACITY
    );
    assert_eq!(
        program.literal_bytes.len(),
        MGAME_RUNTIME_REPAIR_LITERAL_BYTE_CAPACITY
    );
    assert!(contains_sequence(
        &repair_instructions,
        &load_address(Register::T1, NAME_DIALOGUE_RUNTIME_ORIGIN + 0x0688)
    ));
    assert!(contains_sequence(
        &repair_instructions,
        &load_address(Register::T3, 0x8fa8_0070)
    ));
    assert!(contains_sequence(
        &repair_instructions,
        &load_address(Register::T0, MGAME_RUNTIME_REPAIR_LITERAL_ORIGIN)
    ));
    assert!(repair_instructions.contains(&Instruction::Ori {
        rt: Register::T2,
        rs: Register::ZERO,
        immediate: 10,
    }));
    assert!(repair_instructions.contains(&Instruction::Sw {
        rt: Register::T3,
        base: Register::T1,
        offset: 0,
    }));
    assert_eq!(
        repair_instructions[repair_instructions.len() - 4],
        Instruction::J {
            target: TEST_NICKNAME_HUD_UPLOADER_ADDRESS,
        }
    );
    assert_eq!(repair_instructions.last(), Some(&Instruction::nop()));
    assert!(program.report.nickname_hud_upload_before_mgame_entry);
    assert!(program.report.nickname_hud_upload_skips_profile);
    let guard = repair_instructions.len() - 9;
    assert_eq!(
        repair_instructions[guard + 1],
        Instruction::Lbu {
            rt: Register::T0,
            base: Register::T0,
            offset: 0x194d
        }
    );
    assert_eq!(
        repair_instructions[guard + 2],
        Instruction::Ori {
            rt: Register::T1,
            rs: Register::ZERO,
            immediate: 2
        }
    );
    assert_eq!(
        repair_instructions[guard + 3],
        Instruction::Beq {
            rs: Register::T0,
            rt: Register::T1,
            target: MGAME_RUNTIME_REPAIR_HELPER_ORIGIN + ((guard + 7) * 4) as u32
        }
    );
    assert_eq!(
        repair_instructions[guard + 7],
        Instruction::Jr { rs: Register::RA }
    );
    assert_eq!(NAME_DIALOGUE_RUNTIME_ORIGIN, 0x8009_a400);
}

fn contains_sequence(haystack: &[Instruction], needle: &[Instruction]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

fn synthetic_runtime() -> Vec<u8> {
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
    runtime
}

#[test]
fn emitted_repair_restores_the_selected_runtime_after_decoder_layout_changes() {
    use crate::name_input::runtime_test_machine::execute;
    for word in [0x8fa80070u32, 0x34080001] {
        let mut expected_runtime = synthetic_runtime();
        expected_runtime[0x578..0x57c].copy_from_slice(&word.to_le_bytes());
        // This overwritten range need not be a function epilogue in a new decoder.
        expected_runtime[0x6ac..0x6b0].copy_from_slice(&0x34090002u32.to_le_bytes());
        let program = build_mgame_runtime_repair_program(
            &expected_runtime,
            TEST_NICKNAME_HUD_UPLOADER_ADDRESS,
        )
        .unwrap();
        let mut memory = vec![0x55; 0x200000];
        let base = (NAME_DIALOGUE_RUNTIME_ORIGIN & 0x1fff_ffff) as usize;
        memory[base..base + expected_runtime.len()].copy_from_slice(&expected_runtime);
        let literals = (MGAME_RUNTIME_REPAIR_LITERAL_ORIGIN & 0x1fff_ffff) as usize;
        memory[literals..literals + program.literal_bytes.len()]
            .copy_from_slice(&program.literal_bytes);
        memory[0x1f194d] = 2; // Preserve profile texture: no external HUD upload.
        let expected = memory.clone();
        memory[base + 0x578..base + 0x57c].fill(0);
        memory[base + 0x688..base + 0x6b0].fill(0);
        let mut r = [0; 32];
        r[31] = 0x800f0000;
        execute(
            &program.helper_bytes,
            MGAME_RUNTIME_REPAIR_HELPER_ORIGIN,
            &mut r,
            &mut memory,
        );
        assert!(
            memory == expected,
            "repair changed memory outside the native clobber ranges"
        );
    }
}
