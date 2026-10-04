use super::parser::{
    DECODED_IMAGE_SIZE, DECODED_RUNTIME_BASE, SELECTOR_SLOT_COUNT, SELECTOR_TABLE_OFFSET,
    parse_dialogue_banks,
};

#[test]
fn enumerates_messages_between_selector_tables() {
    let mut decoded = fixture();
    install_u32(&mut decoded, SELECTOR_TABLE_OFFSET, runtime(0x31_040));
    install_u32(&mut decoded, SELECTOR_TABLE_OFFSET + 4, runtime(0x31_080));

    decoded[0x31_020..0x31_026].copy_from_slice(&[1, 0, 2, 0, 3, 0]);
    decoded[0x31_026..0x31_040].fill(4);
    install_u32(&mut decoded, 0x31_040, runtime(0x31_020));
    install_u32(&mut decoded, 0x31_044, runtime(0x31_026));
    install_u32(&mut decoded, 0x31_048, u32::MAX);

    decoded[0x31_04c..0x31_060].fill(5);
    decoded[0x31_060..0x31_080].fill(6);
    install_u32(&mut decoded, 0x31_080, runtime(0x31_04c));
    install_u32(&mut decoded, 0x31_084, runtime(0x31_060));
    install_u32(&mut decoded, 0x31_088, u32::MAX);

    let banks = parse_dialogue_banks(&decoded).unwrap();
    assert_eq!(banks.len(), 2);
    assert_eq!(banks[0].selector_index, 0);
    assert_eq!(banks[0].message_data_start, 0x31_020);
    assert_eq!(banks[0].message_data_end, 0x31_040);
    assert_eq!(banks[0].pointer_table_end, 0x31_04c);
    assert_eq!(banks[0].messages.len(), 2);
    assert_eq!(banks[0].messages[0].decoded_offset, 0x31_020);
    assert_eq!(banks[0].messages[0].data, [1, 0, 2, 0, 3, 0]);
    assert_eq!(banks[1].message_data_start, 0x31_04c);
    assert_eq!(banks[1].messages.len(), 2);
}

#[test]
fn preserves_sparse_selector_indices() {
    let mut decoded = fixture();
    install_u32(&mut decoded, SELECTOR_TABLE_OFFSET + 4, runtime(0x31_040));
    decoded[0x31_020..0x31_040].fill(4);
    install_u32(&mut decoded, 0x31_040, runtime(0x31_020));
    install_u32(&mut decoded, 0x31_044, u32::MAX);

    let banks = parse_dialogue_banks(&decoded).unwrap();
    assert_eq!(banks.len(), 1);
    assert_eq!(banks[0].selector_index, 1);
}

#[test]
fn rejects_message_pointer_outside_its_bank() {
    let mut decoded = fixture();
    install_u32(&mut decoded, SELECTOR_TABLE_OFFSET, runtime(0x31_040));
    install_u32(&mut decoded, 0x31_040, runtime(0x31_044));
    install_u32(&mut decoded, 0x31_044, u32::MAX);
    let error = parse_dialogue_banks(&decoded).unwrap_err();
    assert!(error.to_string().contains("first message does not begin"));
}

#[test]
fn rejects_unterminated_pointer_table() {
    let mut decoded = fixture();
    install_u32(
        &mut decoded,
        SELECTOR_TABLE_OFFSET,
        runtime(DECODED_IMAGE_SIZE - 4),
    );
    install_u32(
        &mut decoded,
        DECODED_IMAGE_SIZE - 4,
        runtime(SELECTOR_TABLE_OFFSET + SELECTOR_SLOT_COUNT * 4),
    );
    let error = parse_dialogue_banks(&decoded).unwrap_err();
    assert!(error.to_string().contains("lacks a terminator"));
}

fn fixture() -> Vec<u8> {
    vec![0; DECODED_IMAGE_SIZE]
}

fn runtime(offset: usize) -> u32 {
    DECODED_RUNTIME_BASE + offset as u32
}

fn install_u32(data: &mut [u8], offset: usize, value: u32) {
    data[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
