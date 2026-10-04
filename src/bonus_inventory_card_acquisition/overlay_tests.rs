use super::command_sequences::CARD_ACQUISITION_SEQUENCES;
use super::overlay::{patch_card_acquisition_overlay, patch_composed_card_acquisition_overlay};
use super::test_support::{authored_units, overlay_fixture};

#[test]
#[ignore = "requires assets/"]
fn overlay_patch_owns_only_three_fixed_command_records() {
    let source = overlay_fixture();
    let patched = patch_card_acquisition_overlay(&source, &authored_units()).unwrap();

    assert_eq!(
        patched.expected_write_ranges,
        vec![[0x102c, 0x1034], [0x1034, 0x1050], [0x1050, 0x106c]]
    );
    assert_eq!(patched.bytes.len(), source.len());
    for (offset, (patched_byte, source_byte)) in patched.bytes.iter().zip(&source).enumerate() {
        if !(0x102c..0x106c).contains(&offset) {
            assert_eq!(
                patched_byte, source_byte,
                "unexpected write at +0x{offset:04x}"
            );
        }
    }
    for spec in CARD_ACQUISITION_SEQUENCES {
        assert_eq!(
            &patched.bytes[spec.pointer_storage_offset..spec.pointer_storage_offset + 4],
            &source[spec.pointer_storage_offset..spec.pointer_storage_offset + 4]
        );
    }
}

#[test]
#[ignore = "requires assets/"]
fn composed_patch_preserves_confirmation_and_neighbor_component_writes() {
    let mut composed = overlay_fixture();
    composed[0x1004..0x102c].fill(0xa5);
    composed[0x0814..0x0818].copy_from_slice(&[0x51, 0x52, 0x53, 0x54]);
    composed[0x86dc..0x86e0].copy_from_slice(&[0x61, 0x62, 0x63, 0x64]);
    composed[0x66e0..0x66e4].copy_from_slice(&[0x71, 0x72, 0x73, 0x74]);

    let patched = patch_composed_card_acquisition_overlay(&composed, &authored_units()).unwrap();

    assert!(
        patched.bytes[0x1004..0x102c]
            .iter()
            .all(|byte| *byte == 0xa5)
    );
    assert_eq!(&patched.bytes[0x0814..0x0818], &[0x51, 0x52, 0x53, 0x54]);
    assert_eq!(&patched.bytes[0x86dc..0x86e0], &[0x61, 0x62, 0x63, 0x64]);
    assert_eq!(&patched.bytes[0x66e0..0x66e4], &[0x71, 0x72, 0x73, 0x74]);
}

#[test]
#[ignore = "requires assets/"]
fn each_source_record_is_hash_and_byte_guarded() {
    let source = overlay_fixture();
    for spec in CARD_ACQUISITION_SEQUENCES {
        let mut drifted = source.clone();
        drifted[spec.sequence_offset] ^= 1;
        assert!(patch_card_acquisition_overlay(&drifted, &authored_units()).is_err());
    }
}
