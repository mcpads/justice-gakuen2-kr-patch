use std::collections::BTreeSet;

use psx_r3000a::{Instruction, encode};

use super::minisel::{
    CONSUMER_GRAMMAR, NON_TEXT_CANDIDATE_OFFSETS, PRIMITIVE_RECORD_OFFSETS,
    validate_primitive_record_candidates,
};

const BASE: u32 = 0x800a_2000;
const COUNTS: [u16; 14] = [1, 1, 1, 1, 1, 1, 1, 1, 3, 1, 3, 3, 4, 1];
const REQUIRED_REACHABLE_OFFSETS: [usize; 3] = [0x0cd0, 0x16b8, 0x16e8];

#[test]
fn classifies_every_string_shaped_value_inside_the_primitive_records_as_non_text() {
    let (data, reachable) = fixture();

    let evidence = validate_primitive_record_candidates(&data, BASE, &reachable).unwrap();

    assert_eq!(
        evidence.keys().copied().collect::<Vec<_>>(),
        NON_TEXT_CANDIDATE_OFFSETS
    );
}

#[test]
fn rejects_a_changed_primitive_count() {
    let (mut data, reachable) = fixture();
    data[0x00d0..0x00d2].copy_from_slice(&2u16.to_le_bytes());

    let error = validate_primitive_record_candidates(&data, BASE, &reachable).unwrap_err();

    assert!(error.to_string().contains("count changed"));
}

#[test]
fn rejects_an_unreachable_primitive_consumer() {
    let (data, mut reachable) = fixture();
    reachable.remove(&0x16e8);

    let error = validate_primitive_record_candidates(&data, BASE, &reachable).unwrap_err();

    assert!(error.to_string().contains("not entrypoint-reachable"));
}

#[test]
fn rejects_changed_primitive_parser_grammar() {
    let (mut data, reachable) = fixture();
    write_instruction(&mut data, 0x0d00, Instruction::nop());

    let error = validate_primitive_record_candidates(&data, BASE, &reachable).unwrap_err();

    assert!(error.to_string().contains("consumer grammar changed"));
}

fn fixture() -> (Vec<u8>, BTreeSet<usize>) {
    let mut data = vec![0u8; 0x176c];
    for &offset in &NON_TEXT_CANDIDATE_OFFSETS {
        data[offset..offset + 2].copy_from_slice(&1u16.to_le_bytes());
        data[offset + 2..offset + 4].copy_from_slice(&0u16.to_le_bytes());
    }
    for (index, (&record_offset, &count)) in PRIMITIVE_RECORD_OFFSETS
        .iter()
        .zip(COUNTS.iter())
        .enumerate()
    {
        data[record_offset..record_offset + 2].copy_from_slice(&count.to_le_bytes());
        data[0x0228 + index * 4..0x022c + index * 4]
            .copy_from_slice(&(BASE + record_offset as u32).to_le_bytes());
    }
    for (offset, instruction) in CONSUMER_GRAMMAR {
        write_instruction(&mut data, *offset, instruction.clone());
    }
    (
        data,
        REQUIRED_REACHABLE_OFFSETS
            .into_iter()
            .collect::<BTreeSet<_>>(),
    )
}

fn write_instruction(data: &mut [u8], offset: usize, instruction: Instruction) {
    let word = encode(&instruction, BASE + offset as u32).unwrap();
    data[offset..offset + 4].copy_from_slice(&word.to_le_bytes());
}
