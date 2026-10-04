use super::dialogue_bundle::{DialogueBundleSlot, parse_dialogue_bundle_slots};

fn bundle_with_slots(slots: &[(u32, u32)], byte_count: usize) -> Vec<u8> {
    let mut data = vec![0u8; byte_count];
    for (index, &(offset, size)) in slots.iter().enumerate() {
        let header = index * 8;
        data[header..header + 4].copy_from_slice(&offset.to_le_bytes());
        data[header + 4..header + 8].copy_from_slice(&size.to_le_bytes());
    }
    data
}

#[test]
fn parses_aligned_dialogue_bundle_members() {
    let data = bundle_with_slots(&[(0x800, 5), (0x1000, 7)], 0x1800);
    assert_eq!(
        parse_dialogue_bundle_slots(&data).unwrap(),
        vec![
            DialogueBundleSlot {
                offset: 0x800,
                byte_count: 5,
            },
            DialogueBundleSlot {
                offset: 0x1000,
                byte_count: 7,
            },
        ]
    );
}

#[test]
fn rejects_unaligned_or_overlapping_dialogue_bundle_members() {
    let unaligned = bundle_with_slots(&[(0x801, 5)], 0x1000);
    assert!(parse_dialogue_bundle_slots(&unaligned).is_err());

    let overlapping = bundle_with_slots(&[(0x800, 0x900), (0x1000, 7)], 0x1800);
    assert!(parse_dialogue_bundle_slots(&overlapping).is_err());
}

#[test]
fn rejects_out_of_range_dialogue_bundle_member() {
    let data = bundle_with_slots(&[(0x800, 0x801)], 0x1000);
    assert!(parse_dialogue_bundle_slots(&data).is_err());
}
