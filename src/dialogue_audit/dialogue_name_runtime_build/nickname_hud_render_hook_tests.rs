use psx_r3000a::{Instruction, Register, decode};

use super::nickname_hud_render_hook::install_nickname_hud_render_hook;

const MGAME_SIZE: usize = 0x0002_e000;
const RENDER_OFFSET: usize = 0x0000_d5e0;
const RENDER_ADDRESS: u32 = 0x800a_f5e0;
const RENDER_WRAPPER_ADDRESS: u32 = 0x800c_c200;

#[test]
fn exact_nickname_renderer_uploads_after_each_native_texture_load_and_resumes() {
    let source = synthetic_mgame();
    let mut patched = source.clone();

    let report =
        install_nickname_hud_render_hook(&source, &mut patched, RENDER_WRAPPER_ADDRESS).unwrap();

    assert!(report.source_verified);
    assert!(report.installed);
    assert!(report.native_loader_called_by_wrapper);
    assert!(report.returns_to_native_renderer_continuation);
    assert!(report.uploads_after_each_native_texture_load);
    assert!(report.caller_return_address_preserved);
    assert_eq!(report.wrapper_address, "0x800cc200");
    assert_eq!(
        decode(read_word(&patched, RENDER_OFFSET), RENDER_ADDRESS).unwrap(),
        Instruction::Jal {
            target: RENDER_WRAPPER_ADDRESS,
        }
    );
    assert_eq!(
        decode(read_word(&patched, RENDER_OFFSET + 4), RENDER_ADDRESS + 4).unwrap(),
        Instruction::Addu {
            rd: Register::S0,
            rs: Register::S0,
            rt: Register::V1,
        }
    );
}

#[test]
fn changed_nickname_renderer_is_rejected_before_writing() {
    let mut source = synthetic_mgame();
    source[RENDER_OFFSET] ^= 1;
    let mut patched = source.clone();

    assert!(
        install_nickname_hud_render_hook(&source, &mut patched, RENDER_WRAPPER_ADDRESS).is_err()
    );
}

fn synthetic_mgame() -> Vec<u8> {
    let mut source = vec![0_u8; MGAME_SIZE];
    source[RENDER_OFFSET..RENDER_OFFSET + 8]
        .copy_from_slice(&[0x09, 0xf8, 0x40, 0x00, 0x21, 0x80, 0x03, 0x02]);
    source
}

fn read_word(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}
