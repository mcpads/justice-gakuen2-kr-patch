use super::*;

fn synthetic_tzz() -> Vec<u8> {
    let mut data = vec![0u8; 0x1800];
    data[0..4].copy_from_slice(&0x800u32.to_le_bytes());
    data[4..8].copy_from_slice(&6u32.to_le_bytes());
    data[8..12].copy_from_slice(&0x1000u32.to_le_bytes());
    data[12..16].copy_from_slice(&10u32.to_le_bytes());
    data[0x800..0x806].copy_from_slice(b"first!");
    data[0x1000..0x100a].copy_from_slice(b"second!!!!");
    data
}

#[test]
fn parses_member_sizes_separately_from_fixed_slots() {
    let data = synthetic_tzz();
    let members = parse_tzz(&data).unwrap();

    assert_eq!(members.len(), 2);
    assert_eq!(members[0].compressed_range(), 0x800..0x806);
    assert_eq!(members[0].slot_range(), 0x800..0x1000);
    assert_eq!(members[1].compressed_range(), 0x1000..0x100a);
    assert_eq!(members[1].slot_range(), 0x1000..0x1800);
}

#[test]
fn rejects_member_that_overruns_the_next_slot() {
    let mut data = synthetic_tzz();
    data[4..8].copy_from_slice(&0x801u32.to_le_bytes());

    assert!(parse_tzz(&data).is_err());
}
