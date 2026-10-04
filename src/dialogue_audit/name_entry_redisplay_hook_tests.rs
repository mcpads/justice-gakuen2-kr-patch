use psx_r3000a::{Instruction, Register, decode};

use super::name_entry::OVERLAY_RUNTIME_BASE;
use super::name_entry_redisplay_hook::{
    install_name_entry_redisplay_hook, install_name_entry_redisplay_hook_fixture,
};
use super::name_entry_test_fixture::name_entry_overlay;

const RESOLVER_ADDRESS: u32 = 0x8010_12e0;

#[test]
fn exact_redisplay_lookup_prefix_is_replaced_with_a_typed_tag_resolver_call() {
    let mut overlay = name_entry_overlay();
    install_name_entry_redisplay_hook_fixture(&mut overlay);
    let report = install_name_entry_redisplay_hook(&mut overlay, RESOLVER_ADDRESS).unwrap();

    assert_eq!(report.hook_file_offset, "0x7514");
    assert_eq!(report.overwritten_byte_count, 28);
    assert_eq!(report.typed_hook_instruction_count, 7);
    assert!(report.source_instructions_verified);
    assert!(report.installed);
    assert!(!report.runtime_execution_verified);
    assert_eq!(
        decode(word(&overlay, 0x751c), OVERLAY_RUNTIME_BASE + 0x751c).unwrap(),
        Instruction::Jal {
            target: RESOLVER_ADDRESS,
        }
    );
    assert_eq!(
        decode(word(&overlay, 0x7520), OVERLAY_RUNTIME_BASE + 0x7520).unwrap(),
        Instruction::Addu {
            rd: Register::A2,
            rs: Register::S3,
            rt: Register::ZERO,
        }
    );
}

#[test]
fn changed_source_or_out_of_range_resolver_is_rejected() {
    let mut changed = name_entry_overlay();
    install_name_entry_redisplay_hook_fixture(&mut changed);
    changed[0x7514..0x7518].copy_from_slice(&0_u32.to_le_bytes());
    assert!(install_name_entry_redisplay_hook(&mut changed, RESOLVER_ADDRESS).is_err());

    let mut overlay = name_entry_overlay();
    install_name_entry_redisplay_hook_fixture(&mut overlay);
    assert!(install_name_entry_redisplay_hook(&mut overlay, 0x8010_1800).is_err());
}

fn word(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}
