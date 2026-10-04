use std::collections::BTreeSet;

use psx_r3000a::{Instruction, encode};

use super::koubai2::{SEQUENCES, consumer_grammar, validate_command_byte_sequence_candidates};

const BASE: u32 = 0x800a_2000;
const REQUIRED_REACHABLE: [usize; 18] = [
    0x6f04, 0x6fdc, 0x6fe0, 0x7680, 0x76dc, 0x76e0, 0xd35c, 0xd368, 0xd370, 0xd420, 0xd424, 0xd42c,
    0x986c, 0x9874, 0x98a8, 0x9374, 0x937c, 0x93b4,
];

#[test]
fn classifies_all_six_command_byte_sequences_as_non_text() {
    let (data, reachable) = fixture();

    let evidence = validate_command_byte_sequence_candidates(&data, BASE, &reachable).unwrap();

    assert_eq!(
        evidence.keys().copied().collect::<Vec<_>>(),
        SEQUENCES
            .iter()
            .map(|sequence| sequence.offset)
            .collect::<Vec<_>>()
    );
}

#[test]
fn rejects_a_changed_command_sequence_pointer() {
    let (mut data, reachable) = fixture();
    data[0x12f8..0x12fc].copy_from_slice(&(BASE + 0x0628).to_le_bytes());

    let error = validate_command_byte_sequence_candidates(&data, BASE, &reachable).unwrap_err();

    assert!(error.to_string().contains("pointer"));
}

#[test]
fn rejects_a_changed_command_sequence_boundary() {
    let (mut data, reachable) = fixture();
    data[0x1308..0x130c].copy_from_slice(&(BASE + 0x0674).to_le_bytes());

    let error = validate_command_byte_sequence_candidates(&data, BASE, &reachable).unwrap_err();

    assert!(error.to_string().contains("boundary changed"));
}

#[test]
fn rejects_changed_primary_command_interpreter_grammar() {
    let (mut data, reachable) = fixture();
    write_instruction(&mut data, 0x98a8, Instruction::nop());

    let error = validate_command_byte_sequence_candidates(&data, BASE, &reachable).unwrap_err();

    assert!(error.to_string().contains("consumer grammar changed"));
}

#[test]
fn rejects_changed_secondary_command_interpreter_grammar() {
    let (mut data, reachable) = fixture();
    write_instruction(&mut data, 0x93b4, Instruction::nop());

    let error = validate_command_byte_sequence_candidates(&data, BASE, &reachable).unwrap_err();

    assert!(error.to_string().contains("consumer grammar changed"));
}

#[test]
fn rejects_an_unreachable_command_interpreter() {
    let (data, mut reachable) = fixture();
    reachable.remove(&0x986c);

    let error = validate_command_byte_sequence_candidates(&data, BASE, &reachable).unwrap_err();

    assert!(error.to_string().contains("not entrypoint-reachable"));
}

fn fixture() -> (Vec<u8>, BTreeSet<usize>) {
    let mut data = vec![0u8; 0xd430];
    for sequence in SEQUENCES {
        data[sequence.offset..sequence.offset + 4].copy_from_slice(&[1, 0, 0x0a, 1]);
        data[sequence.terminator_offset] = 0x81;
        data[sequence.pointer_storage_offset..sequence.pointer_storage_offset + 4]
            .copy_from_slice(&(BASE + sequence.offset as u32).to_le_bytes());
        let next_offset = (sequence.terminator_offset + 4) & !3;
        data[sequence.pointer_storage_offset + 4..sequence.pointer_storage_offset + 8]
            .copy_from_slice(&(BASE + next_offset as u32).to_le_bytes());
    }
    for (offset, instruction) in consumer_grammar() {
        write_instruction(&mut data, offset, instruction);
    }
    (data, REQUIRED_REACHABLE.into_iter().collect())
}

fn write_instruction(data: &mut [u8], offset: usize, instruction: Instruction) {
    let word = encode(&instruction, BASE + offset as u32).unwrap();
    data[offset..offset + 4].copy_from_slice(&word.to_le_bytes());
}
