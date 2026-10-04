use psx_r3000a::{Instruction, decode};

use super::mgame_reload_entry_hook::install_mgame_reload_entry_hook;

const MGAME_SIZE: usize = 0x0002_e000;
const ENTRY_OFFSET: usize = 0x0000_3cdc;
const ENTRY_ADDRESS: u32 = 0x800a_5cdc;
const RELOAD_WRAPPER_ADDRESS: u32 = 0x8001_0cac;

#[test]
fn exact_mgame_entry_keeps_the_loader_pointer_and_routes_through_the_reload_wrapper() {
    let source = synthetic_mgame();
    let mut patched = source.clone();

    let report =
        install_mgame_reload_entry_hook(&source, &mut patched, RELOAD_WRAPPER_ADDRESS).unwrap();

    assert!(report.source_verified);
    assert!(report.installed);
    assert!(report.source_entry_pointer_preserved);
    assert!(report.selects_valid_source_copy_or_destination_repair_before_entry_continuation);
    assert!(report.displaced_instructions_preserved_by_wrapper);
    assert!(report.caller_return_address_preserved);
    assert_eq!(report.source_entry_pointer, "0x800a5cdc");
    assert_eq!(report.entry_address, "0x800a5cdc");
    assert_eq!(report.resume_address, "0x800a5ce4");
    assert_eq!(report.wrapper_address, "0x80010cac");
    assert_eq!(read_word(&patched, 0), ENTRY_ADDRESS);
    assert_eq!(
        decode(read_word(&patched, ENTRY_OFFSET), ENTRY_ADDRESS).unwrap(),
        Instruction::J {
            target: RELOAD_WRAPPER_ADDRESS,
        }
    );
    assert_eq!(
        decode(read_word(&patched, ENTRY_OFFSET + 4), ENTRY_ADDRESS + 4).unwrap(),
        Instruction::nop()
    );
}

#[test]
fn changed_mgame_entry_pointer_is_rejected_before_writing() {
    let mut source = synthetic_mgame();
    source[0] ^= 1;
    let mut patched = source.clone();

    assert!(
        install_mgame_reload_entry_hook(&source, &mut patched, RELOAD_WRAPPER_ADDRESS).is_err()
    );
}

#[test]
fn changed_mgame_entry_instructions_are_rejected_before_writing() {
    let mut source = synthetic_mgame();
    source[ENTRY_OFFSET] ^= 1;
    let mut patched = source.clone();

    assert!(
        install_mgame_reload_entry_hook(&source, &mut patched, RELOAD_WRAPPER_ADDRESS).is_err()
    );
}

fn synthetic_mgame() -> Vec<u8> {
    let mut source = vec![0_u8; MGAME_SIZE];
    source[..4].copy_from_slice(&ENTRY_ADDRESS.to_le_bytes());
    source[ENTRY_OFFSET..ENTRY_OFFSET + 8]
        .copy_from_slice(&[0x1f, 0x80, 0x03, 0x3c, 0x01, 0x18, 0x63, 0x90]);
    source
}

fn read_word(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}
