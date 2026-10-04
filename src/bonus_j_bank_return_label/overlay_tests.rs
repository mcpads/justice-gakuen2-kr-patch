use super::command_sequence::RETURN_LABEL_SEQUENCE;
use super::overlay::patch_return_label_overlay;
use super::test_support::{authored_unit, glyph_allocations, read_u32, source_overlay_fixture};

#[test]
#[ignore = "requires assets/"]
fn patch_changes_only_the_owned_record_and_preserves_its_consumer() {
    let source = source_overlay_fixture();
    let patched =
        patch_return_label_overlay(&source, &authored_unit(), &glyph_allocations()).unwrap();

    assert!(patched.source_command_record_matches);
    assert!(patched.changes_confined_to_fixed_command_record);
    assert!(patched.changed_byte_ranges.iter().all(|[start, end]| {
        RETURN_LABEL_SEQUENCE.sequence_offset <= *start
            && *end <= RETURN_LABEL_SEQUENCE.sequence_offset + RETURN_LABEL_SEQUENCE.storage_length
    }));
    assert_eq!(
        read_u32(&patched.bytes, RETURN_LABEL_SEQUENCE.pointer_storage_offset),
        read_u32(&source, RETURN_LABEL_SEQUENCE.pointer_storage_offset)
    );
    assert_eq!(read_u32(&patched.bytes, 0xca98), read_u32(&source, 0xca98));
    assert_eq!(read_u32(&patched.bytes, 0xe4b8), read_u32(&source, 0xe4b8));
    let record = &patched.bytes[RETURN_LABEL_SEQUENCE.sequence_offset
        ..RETURN_LABEL_SEQUENCE.sequence_offset + RETURN_LABEL_SEQUENCE.storage_length];
    assert_eq!(
        record[patched.output_payload_byte_length - 1],
        0x81,
        "the encoded label remains terminated"
    );
    assert!(
        record[patched.output_payload_byte_length..]
            .iter()
            .all(|byte| *byte == 0),
        "unused source storage remains padding"
    );
}

#[test]
#[ignore = "requires assets/"]
fn source_record_drift_fails_before_any_write() {
    let mut source = source_overlay_fixture();
    source[RETURN_LABEL_SEQUENCE.sequence_offset] ^= 1;

    assert!(patch_return_label_overlay(&source, &authored_unit(), &glyph_allocations()).is_err());
}
