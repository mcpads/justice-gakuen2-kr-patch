use psx_r3000a::{Instruction, Register, decode, encode};

use super::name_entry::OVERLAY_RUNTIME_BASE;
use super::name_entry_confirmation_hook::install_name_entry_confirmation_hook;
use super::name_entry_delete_handler::confirmation_entry_address;

const HOOK_OFFSET: usize = 0x1dac;

#[test]
fn confirmation_hook_routes_staged_state_through_the_typed_handler() {
    let mut source = vec![0_u8; HOOK_OFFSET + 16];
    let source_instructions = [
        Instruction::Beq {
            rs: Register::V0,
            rt: Register::ZERO,
            target: runtime_address(0x1dbc),
        },
        Instruction::Andi {
            rt: Register::V0,
            rs: Register::V1,
            immediate: 0x00a0,
        },
        Instruction::J {
            target: runtime_address(0x1f54),
        },
        Instruction::Addiu {
            rt: Register::V0,
            rs: Register::ZERO,
            immediate: 1,
        },
    ];
    for (index, instruction) in source_instructions.iter().enumerate() {
        let offset = HOOK_OFFSET + index * 4;
        source[offset..offset + 4].copy_from_slice(
            &encode(instruction, runtime_address(offset))
                .unwrap()
                .to_le_bytes(),
        );
    }
    let mut patched = source.clone();

    let report =
        install_name_entry_confirmation_hook(&source, &mut patched, confirmation_entry_address())
            .unwrap();

    assert!(report.source_instructions_verified);
    assert_eq!(report.overwritten_byte_count, 16);
    assert_eq!(
        decode(
            read_word(&patched, HOOK_OFFSET + 8),
            runtime_address(HOOK_OFFSET + 8)
        )
        .unwrap(),
        Instruction::J {
            target: confirmation_entry_address(),
        }
    );
    assert_eq!(
        decode(
            read_word(&patched, HOOK_OFFSET + 12),
            runtime_address(HOOK_OFFSET + 12)
        )
        .unwrap(),
        Instruction::Addu {
            rd: Register::A0,
            rs: Register::S0,
            rt: Register::ZERO,
        }
    );
}

fn read_word(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

const fn runtime_address(offset: usize) -> u32 {
    OVERLAY_RUNTIME_BASE + offset as u32
}
