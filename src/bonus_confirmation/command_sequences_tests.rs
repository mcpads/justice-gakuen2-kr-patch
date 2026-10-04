use super::command_sequences::{
    MEMORY_CARD_COPY_PROMPT_LENGTH, MEMORY_CARD_COPY_PROMPT_OFFSET, MEMORY_CARD_DESTINATION_LENGTH,
    MEMORY_CARD_DESTINATION_OFFSET,
};
use super::overlay::patch_confirmation_overlay;
use super::test_support::{authored_units, overlay_fixture};

#[test]
#[ignore = "requires assets/"]
fn memory_card_copy_confirmation_uses_command_blanks() {
    let patched = patch_confirmation_overlay(&overlay_fixture(), &authored_units()).unwrap();

    assert_eq!(
        &patched.bytes[MEMORY_CARD_DESTINATION_OFFSET
            ..MEMORY_CARD_DESTINATION_OFFSET + MEMORY_CARD_DESTINATION_LENGTH],
        &[
            3, 1, 5, 0x63, 0x63, 0x63, 3, 5, 5, 3, 6, 5, 3, 7, 5, 0x63, 0x63, 0x63, 3, 2, 5, 3, 3,
            5, 3, 8, 5, 0x81, 0, 0, 0, 0,
        ]
    );
    assert_eq!(
        &patched.bytes[MEMORY_CARD_COPY_PROMPT_OFFSET
            ..MEMORY_CARD_COPY_PROMPT_OFFSET + MEMORY_CARD_COPY_PROMPT_LENGTH],
        &[
            3, 9, 5, 3, 10, 5, 3, 3, 3, 3, 4, 3, 3, 5, 3, 0, 5, 5, 0x81, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ]
    );
}
