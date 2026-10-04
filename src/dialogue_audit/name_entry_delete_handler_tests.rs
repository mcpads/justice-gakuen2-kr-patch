use psx_r3000a::{Instruction, Register, verify_placed_program};

use super::name_entry::OVERLAY_RUNTIME_BASE;
use super::name_entry_delete_handler::{
    DELETE_ROUTINE_BYTE_COUNT, DELETE_ROUTINE_OFFSET, build_name_entry_delete_handler,
};

const COMPOUND_FINAL_BACKSPACE_ADDRESS: u32 = 0x8010_1700;

#[test]
fn typed_handler_preserves_delete_and_confirmation_control_flow() {
    let (bytes, typed_instruction_count) =
        build_name_entry_delete_handler(COMPOUND_FINAL_BACKSPACE_ADDRESS).unwrap();
    let instructions =
        verify_placed_program(&bytes, OVERLAY_RUNTIME_BASE + DELETE_ROUTINE_OFFSET as u32).unwrap();

    assert_eq!(bytes.len(), DELETE_ROUTINE_BYTE_COUNT);
    assert_eq!(instructions.len(), typed_instruction_count);
    assert!(instructions.contains(&Instruction::J {
        target: 0x8018_2c08
    }));
    assert!(instructions.contains(&Instruction::J {
        target: 0x8017_bf54
    }));
    let compound_backspace = instructions
        .iter()
        .position(|instruction| {
            *instruction
                == Instruction::J {
                    target: COMPOUND_FINAL_BACKSPACE_ADDRESS,
                }
        })
        .unwrap();
    assert_eq!(instructions[compound_backspace + 1], Instruction::nop());

    let incomplete_confirmation = instructions
        .iter()
        .position(|instruction| {
            *instruction
                == Instruction::Beq {
                    rs: Register::T4,
                    rt: Register::T6,
                    target: 0x8017_bf54,
                }
        })
        .unwrap();
    assert_eq!(
        instructions[incomplete_confirmation + 1],
        Instruction::Addu {
            rd: Register::V0,
            rs: Register::ZERO,
            rt: Register::ZERO,
        }
    );

    let field_load = instructions
        .iter()
        .position(|instruction| {
            *instruction
                == Instruction::Lbu {
                    rt: Register::T0,
                    base: Register::A0,
                    offset: 15,
                }
        })
        .unwrap();
    assert_eq!(
        instructions[field_load + 1],
        Instruction::Lbu {
            rt: Register::T1,
            base: Register::A0,
            offset: 10,
        }
    );
    assert_eq!(
        instructions[field_load + 2],
        Instruction::Sll {
            rd: Register::T0,
            rt: Register::T0,
            shift: 4,
        }
    );
    for window in instructions.windows(2) {
        if let Instruction::Lhu { rt, base, offset } = window[0] {
            if (rt, base, offset) == (Register::T3, Register::T2, 0) {
                assert_eq!(
                    window[1],
                    Instruction::Ori {
                        rt: Register::T5,
                        rs: Register::ZERO,
                        immediate: 0x0fff,
                    }
                );
            } else {
                assert_eq!(window[1], Instruction::nop());
            }
        }
    }
}

#[test]
#[ignore = "requires assets/ and fonts in ../fonts/"]
fn emitted_delete_and_confirmation_preserve_field_boundaries_and_reverse_composition() {
    use crate::name_input::{EMPTY_NAME_SLOT, editing_runtime_tests::Editor};
    let mut editor = Editor::new();
    let (bytes, _) =
        build_name_entry_delete_handler(editor.runtime.compound_final_backspace_address).unwrap();
    let origin = crate::name_input::NAME_INPUT_REDISPLAY_RUNTIME_ORIGIN - 8;
    let target = OVERLAY_RUNTIME_BASE + DELETE_ROUTINE_OFFSET as u32;
    let offset = (target - origin) as usize;
    editor.code[offset..offset + bytes.len()].copy_from_slice(&bytes);
    crate::psx_machine_code_sources::verify_r3000a_load_delays(
        &verify_placed_program(&bytes, target).unwrap(),
        target,
        "delete stages",
    )
    .unwrap();
    const OBJECT: usize = 0x1c0000;
    for field in 0..3 {
        editor.reset(field, 0);
        // Empty fields, including repeated deletion, must not wrap the cursor.
        let before = editor.memory[OBJECT..OBJECT + 0x50].to_vec();
        for _ in 0..8 {
            let mut r = [0; 32];
            r[4] = OBJECT as u32 | 0x80000000;
            r[31] = 0x80182c08;
            editor.call(target, r);
            assert_eq!(&editor.memory[OBJECT..OBJECT + 0x50], before);
        }
        for key in [0, 19, 5, 0] {
            editor.select(key);
        } // 갉
        for expected in [0x8008, 0x8000, 0xc000, EMPTY_NAME_SLOT] {
            // 갈, 가, ㄱ, empty
            let mut r = [0; 32];
            r[4] = OBJECT as u32 | 0x80000000;
            r[31] = 0x80182c08;
            editor.call(target, r);
            assert_eq!(editor.words()[0], expected);
            assert_eq!(editor.memory[OBJECT + 10], 0);
        }
        for (keys, stages) in [
            (
                vec![0, 27, 19, 39],
                vec![0x8000 + 9 * 28, 0x8000 + 8 * 28, 0xc000, EMPTY_NAME_SLOT],
            ),
            (
                vec![0, 32, 23, 39],
                vec![0x8000 + 14 * 28, 0x8000 + 13 * 28, 0xc000, EMPTY_NAME_SLOT],
            ),
        ] {
            for key in keys {
                editor.select(key);
            }
            for expected in stages {
                let mut r = [0; 32];
                r[4] = OBJECT as u32 | 0x80000000;
                r[31] = 0x80182c08;
                editor.call(target, r);
                assert_eq!(editor.words()[0], expected);
                assert_eq!(editor.memory[OBJECT + 10], 0);
            }
        }
        editor.select(0); // Reject confirmation of a visible unfinished initial.
        let before = editor.memory[OBJECT..OBJECT + 0x50].to_vec();
        let mut r = [0; 32];
        r[4] = OBJECT as u32 | 0x80000000;
        r[31] = 0x8017bf54;
        let r = editor.call(target + 8, r);
        assert_eq!(r[2], 0);
        assert_eq!(&editor.memory[OBJECT..OBJECT + 0x50], before);
        editor.select(19);
        let mut r = [0; 32];
        r[4] = OBJECT as u32 | 0x80000000;
        r[31] = 0x8017bf54;
        let r = editor.call(target + 8, r);
        assert_eq!(r[2], 1);
        // At an empty position, backspace reaches and edits the preceding glyph.
        editor.memory[OBJECT + 10] = 1;
        let mut r = [0; 32];
        r[4] = OBJECT as u32 | 0x80000000;
        r[31] = 0x80182c08;
        editor.call(target, r);
        assert_eq!(editor.words()[0], 0xc000);
        assert_eq!(editor.words()[1], EMPTY_NAME_SLOT);
        assert_eq!(editor.memory[OBJECT + 10], 0);
    }
}

#[test]
#[ignore = "requires assets/ and fonts in ../fonts/"]
fn deletion_from_every_empty_slot_stops_at_the_field_boundary_and_preserves_right_hand_text() {
    use crate::name_input::{EMPTY_NAME_SLOT, editing_runtime_tests::Editor};
    const OBJECT: usize = 0x1c0000;
    let mut editor = Editor::new();
    let (bytes, _) =
        build_name_entry_delete_handler(editor.runtime.compound_final_backspace_address).unwrap();
    let origin = crate::name_input::NAME_INPUT_REDISPLAY_RUNTIME_ORIGIN - 8;
    let target = OVERLAY_RUNTIME_BASE + DELETE_ROUTINE_OFFSET as u32;
    let offset = (target - origin) as usize;
    editor.code[offset..offset + bytes.len()].copy_from_slice(&bytes);

    for field in 0..3u8 {
        let capacity = if field == 2 { 4 } else { 6 };
        for cursor in 0..capacity {
            // None covers a completely empty prefix. Every possible occupied
            // position to the left covers adjacent and multiple intervening blanks.
            for previous in std::iter::once(None).chain((0..cursor).map(Some)) {
                for (word, deleted) in [
                    (0xc000u16, EMPTY_NAME_SLOT), // ㄱ -> empty
                    (0x8000, 0xc000),             // 가 -> ㄱ
                    (0x8001, 0x8000),             // 각 -> 가
                    (0x8009, 0x8008),             // 갉 -> 갈
                    (0x0041, EMPTY_NAME_SLOT),    // native non-Hangul code -> empty
                ] {
                    editor.reset(field, cursor);
                    let base = OBJECT + 0x12 + usize::from(field) * 16;
                    // Other fields and terminators are guards, not disposable padding.
                    for other in 0..3usize {
                        let other_base = OBJECT + 0x12 + other * 16;
                        if other != usize::from(field) {
                            editor.memory[other_base..other_base + 12].fill(0x5a);
                        }
                        let end = other_base + if other == 2 { 8 } else { 12 };
                        editor.memory[end..end + 2].copy_from_slice(&0x3001u16.to_le_bytes());
                    }
                    if let Some(slot) = previous {
                        let address = base + usize::from(slot) * 2;
                        editor.memory[address..address + 2].copy_from_slice(&word.to_le_bytes());
                    }
                    for slot in cursor + 1..capacity {
                        let address = base + usize::from(slot) * 2;
                        editor.memory[address..address + 2]
                            .copy_from_slice(&0x8001u16.to_le_bytes());
                    }
                    let mut expected = editor.memory[OBJECT..OBJECT + 0x50].to_vec();
                    expected[10] = previous.unwrap_or(0);
                    if let Some(slot) = previous {
                        let offset = base - OBJECT + usize::from(slot) * 2;
                        expected[offset..offset + 2].copy_from_slice(&deleted.to_le_bytes());
                    }
                    let mut registers = [0; 32];
                    registers[4] = OBJECT as u32 | 0x80000000;
                    registers[31] = 0x80182c08;
                    editor.call(target, registers);
                    assert_eq!(
                        &editor.memory[OBJECT..OBJECT + 0x50],
                        expected,
                        "field {field}, empty slot {cursor}, previous {previous:?}, word {word:04x}"
                    );
                }
            }
        }
    }
}
