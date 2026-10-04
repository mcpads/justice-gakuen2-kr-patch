use psx_r3000a::{Assembler, Instruction, Register, load_address, verify_placed_program};

use super::mgame_reload_wrapper::{
    DIALOGUE_RUNTIME_RELOAD_SIGNATURE, MGAME_ENTRY_RESUME_ADDRESS, emit_mgame_reload_wrapper,
};
use super::{
    MGAME_RUNTIME_REPAIR_HELPER_ORIGIN, NAME_DIALOGUE_RUNTIME_ORIGIN,
    NAME_DIALOGUE_RUNTIME_RELOAD_SOURCE_ORIGIN,
};

const WRAPPER_ORIGIN: u32 = 0x8001_0cac;

#[test]
fn mgame_reload_preserves_private_nickname_storage_without_rewriting_scene_ram() {
    let mut assembler = Assembler::new();
    emit_mgame_reload_wrapper(&mut assembler, 2_296).unwrap();
    let program = assembler.assemble(WRAPPER_ORIGIN).unwrap();
    let instructions = verify_placed_program(program.bytes(), WRAPPER_ORIGIN).unwrap();

    let repair = instructions
        .iter()
        .position(|instruction| {
            *instruction
                == Instruction::Jal {
                    target: MGAME_RUNTIME_REPAIR_HELPER_ORIGIN,
                }
        })
        .unwrap();
    let enter = instructions
        .iter()
        .position(|instruction| {
            *instruction
                == Instruction::J {
                    target: MGAME_ENTRY_RESUME_ADDRESS,
                }
        })
        .unwrap();

    assert!(program.bytes().len() <= 0x007c);
    assert!(repair < enter);
    assert_eq!(
        instructions
            .iter()
            .filter(|instruction| matches!(instruction, Instruction::Jal { .. }))
            .collect::<Vec<_>>(),
        [&Instruction::Jal {
            target: MGAME_RUNTIME_REPAIR_HELPER_ORIGIN,
        }]
    );
    assert!(contains_sequence(
        &instructions[..repair],
        &load_address(Register::T0, NAME_DIALOGUE_RUNTIME_RELOAD_SOURCE_ORIGIN)
    ));
    assert!(contains_sequence(
        &instructions[..repair],
        &load_address(Register::T1, NAME_DIALOGUE_RUNTIME_ORIGIN)
    ));
    assert!(contains_sequence(
        &instructions[..repair],
        &load_address(Register::T4, DIALOGUE_RUNTIME_RELOAD_SIGNATURE)
    ));
    assert!(instructions[..repair].contains(&Instruction::Ori {
        rt: Register::T2,
        rs: Register::ZERO,
        immediate: 573,
    }));
    assert!(instructions[..repair].contains(&Instruction::Sw {
        rt: Register::T3,
        base: Register::T1,
        offset: 0,
    }));
    assert!(
        instructions[..repair]
            .iter()
            .any(|instruction| matches!(instruction, Instruction::Beq { .. }))
    );
    assert!(
        instructions[..repair]
            .iter()
            .any(|instruction| matches!(instruction, Instruction::Bne { .. }))
    );
    assert!(instructions[..repair].windows(2).any(|window| {
        matches!(
            window[0],
            Instruction::Bgtz {
                rs: Register::T2,
                ..
            }
        ) && window[1]
            == Instruction::Addiu {
                rt: Register::T2,
                rs: Register::T2,
                immediate: -1,
            }
    }));
    assert!(instructions[..repair].contains(&Instruction::Sw {
        rt: Register::RA,
        base: Register::SP,
        offset: 4,
    }));
    assert!(instructions[repair..enter].contains(&Instruction::Lw {
        rt: Register::RA,
        base: Register::SP,
        offset: 4,
    }));
    assert_eq!(
        instructions[enter - 2],
        Instruction::Lui {
            rt: Register::V1,
            immediate: 0x801f,
        }
    );
    assert_eq!(
        instructions[enter - 1],
        Instruction::Lbu {
            rt: Register::V1,
            base: Register::V1,
            offset: 0x1801,
        }
    );
    assert_eq!(instructions[enter + 1], Instruction::nop());
}

fn contains_sequence(haystack: &[Instruction], needle: &[Instruction]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

#[test]
fn fresh_overlay_restores_all_code_even_when_combat_preserves_the_old_signature() {
    use crate::name_input::runtime_test_machine::execute_with_callbacks;
    let size = 2296;
    let mut assembler = Assembler::new();
    emit_mgame_reload_wrapper(&mut assembler, size).unwrap();
    let program = assembler.assemble(WRAPPER_ORIGIN).unwrap();
    let source = (NAME_DIALOGUE_RUNTIME_RELOAD_SOURCE_ORIGIN & 0x1fff_ffff) as usize;
    let destination = (NAME_DIALOGUE_RUNTIME_ORIGIN & 0x1fff_ffff) as usize;
    for source_valid in [false, true] {
        for destination_valid in [false, true] {
            let mut memory = vec![0xa5; 0x200000];
            let mut payload: Vec<u8> = (0..size).map(|i| (i * 37 + 11) as u8).collect();
            payload[4..8].copy_from_slice(&DIALOGUE_RUNTIME_RELOAD_SIGNATURE.to_le_bytes());
            if source_valid {
                memory[source..source + size].copy_from_slice(&payload);
            }
            // Worst-case damage: all destination bytes are stale except its
            // old marker. This includes arrays and GPU scratch, not just aa80.
            if destination_valid {
                memory[destination + 4..destination + 8]
                    .copy_from_slice(&DIALOGUE_RUNTIME_RELOAD_SIGNATURE.to_le_bytes());
            }
            memory[0x9aa80..0x9aa84].copy_from_slice(&0x801efcc0_u32.to_le_bytes());
            memory[0x1f1801] = 7;
            let original = memory.clone();
            let mut r = [0; 32];
            r[29] = 0x801ff000;
            r[31] = 0x80010000;
            let saved = r;
            let mut repairs = 0;
            let mut resumed = false;
            execute_with_callbacks(
                program.bytes(),
                WRAPPER_ORIGIN,
                &mut r,
                &mut memory,
                None,
                &mut |pc, r, m| {
                    if pc == MGAME_RUNTIME_REPAIR_HELPER_ORIGIN {
                        repairs += 1;
                        if source_valid {
                            assert_eq!(&m[destination..destination + size], &payload);
                        }
                        for register in [8, 9, 10, 11, 12] {
                            r[register] = 0xdeadbeef;
                        }
                        true
                    } else if pc == MGAME_ENTRY_RESUME_ADDRESS {
                        resumed = true;
                        assert_eq!(r[3], 7);
                        true
                    } else {
                        false
                    }
                },
            );
            assert!(resumed);
            assert_eq!(repairs, usize::from(source_valid || destination_valid));
            assert_eq!(r[29], saved[29]);
            assert_eq!(r[31], saved[31]);
            let mut expected = original;
            if source_valid {
                expected[destination..destination + size].copy_from_slice(&payload);
            }
            expected[0x1feffc..0x1ff000].copy_from_slice(&saved[31].to_le_bytes());
            assert!(
                memory == expected,
                "reload touched data outside runtime and its stack frame"
            );
        }
    }
}
