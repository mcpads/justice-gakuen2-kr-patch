use super::command_sequence::RECORD_SPECS;
use super::overlay::patch_exit_confirmation_overlay;
use super::test_support::{authored_units, glyph_allocations, read_u32, source_overlay_fixture};

#[test]
#[ignore = "requires assets/"]
fn patch_changes_both_clerk_records_and_preserves_their_pointers_and_consumer() {
    let source = source_overlay_fixture();
    let pointers_before = RECORD_SPECS.map(|spec| read_u32(&source, spec.pointer_storage_offset));
    let patched =
        patch_exit_confirmation_overlay(&source, &authored_units(), &glyph_allocations()).unwrap();

    assert_eq!(
        patched.expected_write_ranges,
        RECORD_SPECS
            .iter()
            .map(|spec| [
                spec.record_offset,
                spec.record_offset + spec.source_record.len()
            ])
            .collect::<Vec<_>>()
    );
    assert_eq!(patched.records.len(), 2);
    assert!(patched.source_command_records_match);
    assert!(patched.changes_confined_to_fixed_record);
    assert!(patched.changed_byte_ranges.iter().all(|[start, end]| {
        RECORD_SPECS.iter().any(|spec| {
            spec.record_offset <= *start && *end <= spec.record_offset + spec.source_record.len()
        })
    }));
    assert_eq!(
        RECORD_SPECS.map(|spec| read_u32(&patched.bytes, spec.pointer_storage_offset)),
        pointers_before
    );
}

#[test]
#[ignore = "requires assets/"]
fn either_source_record_drift_is_rejected_before_writing() {
    for spec in RECORD_SPECS {
        let mut source = source_overlay_fixture();
        source[spec.record_offset] ^= 1;
        let error =
            patch_exit_confirmation_overlay(&source, &authored_units(), &glyph_allocations())
                .unwrap_err();
        assert!(error.to_string().contains("source record changed"));
    }
}
