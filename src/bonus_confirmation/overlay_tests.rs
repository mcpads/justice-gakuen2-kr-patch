use psx_r3000a::{Instruction, Register};

use super::consumer::OVERLAY_RUNTIME_BASE;
use super::overlay::patch_composed_confirmation_overlay;
use super::test_support::{authored_units, overlay_fixture, write_instruction, write_u32};

#[test]
#[ignore = "requires assets/"]
fn overlay_patch_owns_only_the_three_strings_two_records_choices_and_choice_geometry() {
    let source = overlay_fixture();
    let patched = patch_composed_confirmation_overlay(&source, &authored_units()).unwrap();

    assert_eq!(
        patched.expected_write_ranges,
        [
            [0x0fe4, 0x1004],
            [0x1004, 0x102c],
            [0x167c, 0x1694],
            [0x1694, 0x16c0],
            [0x16c0, 0x16d0],
            [0x8d70, 0x8d74],
            [0x8ed4, 0x8ed8],
            [0x8ee8, 0x8eec],
        ]
    );
    assert!(patched.changed_byte_ranges.iter().all(|[start, end]| {
        patched
            .expected_write_ranges
            .iter()
            .any(|[allowed_start, allowed_end]| allowed_start <= start && end <= allowed_end)
    }));
}

#[test]
#[ignore = "requires assets/"]
fn composed_patch_preserves_page_action_and_stock_owned_writes() {
    let mut composed = overlay_fixture();
    composed[0x07e8..0x07ec].copy_from_slice(&[0x5a, 0xa5, 0x5a, 0xa5]);
    composed[0x86dc..0x86e0].copy_from_slice(&[0xa5, 0x5a, 0xa5, 0x5a]);
    write_instruction(
        &mut composed,
        0x66e0,
        Instruction::Addiu {
            rt: Register::S4,
            rs: Register::ZERO,
            immediate: 4,
        },
    );
    let stock_selector = composed[0x66e0..0x66e4].to_vec();

    let patched = patch_composed_confirmation_overlay(&composed, &authored_units()).unwrap();

    assert_eq!(&patched.bytes[0x07e8..0x07ec], &[0x5a, 0xa5, 0x5a, 0xa5]);
    assert_eq!(&patched.bytes[0x86dc..0x86e0], &[0xa5, 0x5a, 0xa5, 0x5a]);
    assert_eq!(&patched.bytes[0x66e0..0x66e4], stock_selector);
}

#[test]
#[ignore = "requires assets/"]
fn composed_patch_preserves_a_rebound_following_command_sequence() {
    const FOLLOWING_COMMAND_OFFSET: usize = 0x102c;
    const FOLLOWING_COMMAND_POINTER_OFFSET: usize = 0x13f4;
    const FOLLOWING_COMMAND: [u8; 8] = [0x02, 0x01, 0x01, 0x02, 0x02, 0x01, 0x81, 0x00];

    let mut composed = overlay_fixture();
    composed[FOLLOWING_COMMAND_OFFSET..FOLLOWING_COMMAND_OFFSET + FOLLOWING_COMMAND.len()]
        .copy_from_slice(&FOLLOWING_COMMAND);
    write_u32(
        &mut composed,
        FOLLOWING_COMMAND_POINTER_OFFSET,
        OVERLAY_RUNTIME_BASE + FOLLOWING_COMMAND_OFFSET as u32,
    );
    let following_pointer =
        composed[FOLLOWING_COMMAND_POINTER_OFFSET..FOLLOWING_COMMAND_POINTER_OFFSET + 4].to_vec();

    let patched = patch_composed_confirmation_overlay(&composed, &authored_units()).unwrap();

    assert_eq!(
        &patched.bytes
            [FOLLOWING_COMMAND_OFFSET..FOLLOWING_COMMAND_OFFSET + FOLLOWING_COMMAND.len()],
        FOLLOWING_COMMAND
    );
    assert_eq!(
        &patched.bytes[FOLLOWING_COMMAND_POINTER_OFFSET..FOLLOWING_COMMAND_POINTER_OFFSET + 4],
        following_pointer
    );
}
