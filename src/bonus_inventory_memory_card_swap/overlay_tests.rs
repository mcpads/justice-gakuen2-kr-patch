use super::command_sequences::MEMORY_CARD_SWAP_SEQUENCES;
use super::overlay::{patch_composed_memory_card_swap_overlay, patch_memory_card_swap_overlay};
use super::test_support::{authored_units, overlay_fixture};

#[test]
#[ignore = "requires assets/"]
fn overlay_patch_owns_only_two_fixed_command_records() {
    let source = overlay_fixture();
    let patched = patch_memory_card_swap_overlay(&source, &authored_units()).unwrap();
    assert_eq!(
        patched.expected_write_ranges,
        vec![[0x1138, 0x1188], [0x1188, 0x11e0]]
    );
    assert_eq!(patched.bytes.len(), source.len());
    for (offset, (patched_byte, source_byte)) in patched.bytes.iter().zip(&source).enumerate() {
        if !(0x1138..0x11e0).contains(&offset) {
            assert_eq!(patched_byte, source_byte);
        }
    }
    for spec in MEMORY_CARD_SWAP_SEQUENCES {
        assert_eq!(
            &patched.bytes[spec.pointer_storage_offset..spec.pointer_storage_offset + 4],
            &source[spec.pointer_storage_offset..spec.pointer_storage_offset + 4]
        );
    }
}

#[test]
#[ignore = "requires assets/"]
fn composed_patch_preserves_prior_component_writes() {
    let mut composed = overlay_fixture();
    composed[0x1004..0x106c].fill(0xa5);
    composed[0x0814..0x0818].copy_from_slice(&[0x51, 0x52, 0x53, 0x54]);
    composed[0x86dc..0x86e0].copy_from_slice(&[0x61, 0x62, 0x63, 0x64]);
    let patched = patch_composed_memory_card_swap_overlay(&composed, &authored_units()).unwrap();
    assert!(
        patched.bytes[0x1004..0x106c]
            .iter()
            .all(|byte| *byte == 0xa5)
    );
    assert_eq!(&patched.bytes[0x0814..0x0818], &[0x51, 0x52, 0x53, 0x54]);
    assert_eq!(&patched.bytes[0x86dc..0x86e0], &[0x61, 0x62, 0x63, 0x64]);
}

#[test]
#[ignore = "requires assets/"]
fn each_source_record_is_hash_and_byte_guarded() {
    let source = overlay_fixture();
    for spec in MEMORY_CARD_SWAP_SEQUENCES {
        let mut drifted = source.clone();
        drifted[spec.sequence_offset] ^= 1;
        assert!(patch_memory_card_swap_overlay(&drifted, &authored_units()).is_err());
    }
}
