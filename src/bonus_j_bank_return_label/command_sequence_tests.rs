use super::command_sequence::{RETURN_LABEL_SEQUENCE, encode_return_label};
use super::test_support::{authored_unit, glyph_allocations};

#[test]
#[ignore = "requires assets/"]
fn authored_return_label_encodes_within_the_source_command_capacity() {
    let unit = authored_unit();
    let allocations = glyph_allocations();
    let encoded = encode_return_label(&unit, &allocations).unwrap();

    let expected_commands = unit
        .korean_text
        .as_deref()
        .unwrap()
        .chars()
        .flat_map(|character| {
            let code = allocations
                .iter()
                .find(|allocation| allocation.text == character)
                .unwrap()
                .code;
            [
                (code >> 8) as u8,
                (code & 0x000f) as u8,
                ((code >> 4) & 0x000f) as u8,
            ]
        })
        .collect::<Vec<_>>();
    assert_eq!(
        &encoded.bytes[..expected_commands.len()],
        expected_commands,
        "each authored character must use its independently assigned glyph command"
    );

    assert!(encoded.command_count > 0);
    assert!(
        encoded.command_count <= (RETURN_LABEL_SEQUENCE.storage_length - 1) / 3,
        "the authored label must fit the source record's command capacity"
    );
    assert_eq!(
        encoded.payload_byte_length,
        encoded.command_count * 3 + 1,
        "each glyph is one three-byte command followed by the terminator"
    );
    assert_eq!(encoded.bytes[encoded.payload_byte_length - 1], 0x81);
    assert!(
        encoded.bytes[encoded.payload_byte_length..]
            .iter()
            .all(|byte| *byte == 0)
    );
}

#[test]
#[ignore = "requires assets/"]
fn an_unallocated_character_fails_instead_of_being_substituted() {
    let mut unit = authored_unit();
    unit.korean_text.as_mut().unwrap().push('X');

    let error = encode_return_label(&unit, &glyph_allocations()).unwrap_err();
    assert!(error.to_string().contains("no J-BANK return-label glyph"));
}

#[test]
#[ignore = "requires assets/"]
fn allocated_glyphs_cannot_overflow_the_fixed_source_record() {
    let allocations = glyph_allocations();
    let command_capacity = (RETURN_LABEL_SEQUENCE.storage_length - 1) / 3;
    let mut unit = authored_unit();
    unit.korean_text = Some(allocations[0].text.to_string().repeat(command_capacity + 1));

    let error = encode_return_label(&unit, &allocations).unwrap_err();
    assert!(error.to_string().contains("source command capacity"));
}
