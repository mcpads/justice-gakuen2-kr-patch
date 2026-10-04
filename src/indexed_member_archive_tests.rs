use super::*;
use crate::compression::compress;

fn fixture() -> (Vec<u8>, &'static IndexedMemberArchiveContract) {
    let decoded = [source_member(0x1357), source_member(0x2468)];
    let stored = decoded
        .iter()
        .map(|member| compress(member, 16).unwrap())
        .collect::<Vec<_>>();
    let mut record = vec![0xa5; 0x1800];
    record[..0x800].fill(0);
    record[0..4].copy_from_slice(&0x800u32.to_le_bytes());
    record[4..8].copy_from_slice(&(stored[0].len() as u32).to_le_bytes());
    record[8..12].copy_from_slice(&0x1000u32.to_le_bytes());
    record[12..16].copy_from_slice(&(stored[1].len() as u32).to_le_bytes());
    record[0x800..0x800 + stored[0].len()].copy_from_slice(&stored[0]);
    record[0x1000..0x1000 + stored[1].len()].copy_from_slice(&stored[1]);

    let record = Box::leak(record.into_boxed_slice());
    let members = Box::leak(
        vec![
            IndexedMemberContract {
                id: "first",
                index: 0,
                table_pair_offset: 0,
                stored_offset: 0x800,
                stored_size: stored[0].len(),
                stored_sha256: Box::leak(sha256_bytes(&stored[0]).into_boxed_str()),
                slot_size: 0x800,
                decoded_size: decoded[0].len(),
                decoded_sha256: Box::leak(sha256_bytes(&decoded[0]).into_boxed_str()),
            },
            IndexedMemberContract {
                id: "second",
                index: 1,
                table_pair_offset: 8,
                stored_offset: 0x1000,
                stored_size: stored[1].len(),
                stored_sha256: Box::leak(sha256_bytes(&stored[1]).into_boxed_str()),
                slot_size: 0x800,
                decoded_size: decoded[1].len(),
                decoded_sha256: Box::leak(sha256_bytes(&decoded[1]).into_boxed_str()),
            },
        ]
        .into_boxed_slice(),
    );
    let contract = Box::leak(Box::new(IndexedMemberArchiveContract {
        record_size: record.len(),
        record_sha256: Box::leak(sha256_bytes(record).into_boxed_str()),
        header_size: 0x800,
        header_sha256: Box::leak(sha256_bytes(&record[..0x800]).into_boxed_str()),
        members,
    }));
    (record.to_vec(), contract)
}

#[test]
fn rebuilds_each_selected_member_with_fixed_offsets_and_exact_stream_sizes() {
    let (source, contract) = fixture();
    let mut patched = decode_indexed_member_archive(&source, contract).unwrap();
    patched[..0x400].fill(0x19);
    patched[0x400..].fill(0x91);

    let rebuilt = rebuild_indexed_member_archive(&source, contract, &patched).unwrap();

    assert_eq!(rebuilt.members.len(), 2);
    assert!(
        rebuilt
            .members
            .iter()
            .all(|member| member.changed_stored_byte_count > 0)
    );
    let descriptors = parse_tzz(&rebuilt.physical).unwrap();
    for (index, descriptor) in descriptors.into_iter().enumerate() {
        assert_eq!(descriptor.offset, contract.members[index].stored_offset);
        assert_eq!(descriptor.slot_size, contract.members[index].slot_size);
        assert_eq!(
            descriptor.compressed_size,
            rebuilt.members[index].patched_stored_size
        );
        assert_eq!(
            decompress(&rebuilt.physical[descriptor.compressed_range()], false).unwrap(),
            patched[index * 0x400..(index + 1) * 0x400]
        );
    }
}

fn source_member(seed: u16) -> Vec<u8> {
    let mut words = (0..448u16)
        .map(|index| seed.wrapping_add(index.wrapping_mul(73)) ^ index.rotate_left(5) ^ 0x9e37)
        .collect::<Vec<_>>();
    let repeated = words[48..80].to_vec();
    words.extend_from_slice(&repeated);
    words.extend_from_slice(&repeated);
    words.into_iter().flat_map(u16::to_le_bytes).collect()
}

#[test]
fn rejects_a_member_table_that_no_longer_matches_the_exact_contract() {
    let (mut source, contract) = fixture();
    source[8..12].copy_from_slice(&0x1800u32.to_le_bytes());

    assert!(decode_indexed_member_archive(&source, contract).is_err());
}
