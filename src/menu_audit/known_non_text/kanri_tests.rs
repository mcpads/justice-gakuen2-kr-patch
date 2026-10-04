use std::collections::BTreeSet;

use psx_r3000a::{Instruction, encode};

use super::kanri::{
    RECORD_TABLES, REQUIRED_REACHABLE_OFFSETS, caller_grammar, parser_grammar,
    validate_counted_object_record_candidates,
};

const BASE: u32 = 0x800a_2000;

#[test]
fn classifies_the_three_counted_object_tables_as_non_text() {
    let (data, reachable) = fixture();

    let evidence = validate_counted_object_record_candidates(&data, BASE, &reachable).unwrap();

    assert_eq!(
        evidence.keys().copied().collect::<Vec<_>>(),
        RECORD_TABLES
            .iter()
            .map(|table| table.offset)
            .collect::<Vec<_>>()
    );
}

#[test]
fn rejects_a_changed_object_record_count() {
    let (mut data, reachable) = fixture();
    data[0x0fd0..0x0fd2].copy_from_slice(&3u16.to_le_bytes());

    let error = validate_counted_object_record_candidates(&data, BASE, &reachable).unwrap_err();

    assert!(error.to_string().contains("count changed"));
}

#[test]
fn rejects_a_changed_object_record_parser_stride() {
    let (mut data, reachable) = fixture();
    write_instruction(&mut data, 0x69e8, Instruction::nop());

    let error = validate_counted_object_record_candidates(&data, BASE, &reachable).unwrap_err();

    assert!(error.to_string().contains("parser grammar changed"));
}

#[test]
fn rejects_an_unreachable_object_record_caller() {
    let (data, mut reachable) = fixture();
    reachable.remove(&0x4db8);

    let error = validate_counted_object_record_candidates(&data, BASE, &reachable).unwrap_err();

    assert!(error.to_string().contains("not entrypoint-reachable"));
}

fn fixture() -> (Vec<u8>, BTreeSet<usize>) {
    let mut data = vec![0u8; 0x6a20];
    for table in RECORD_TABLES {
        data[table.offset..table.offset + 2].copy_from_slice(&table.count.to_le_bytes());
        for code_index in 0..usize::from(table.count) {
            let code_offset = table.offset + 2 + code_index * 2;
            data[code_offset..code_offset + 2].copy_from_slice(&0x0047u16.to_le_bytes());
        }
        data[table.pointer_storage_offset..table.pointer_storage_offset + 4]
            .copy_from_slice(&(BASE + table.offset as u32).to_le_bytes());
    }
    for (offset, instruction) in caller_grammar().into_iter().chain(parser_grammar()) {
        write_instruction(&mut data, offset, instruction);
    }
    (data, REQUIRED_REACHABLE_OFFSETS.into_iter().collect())
}

fn write_instruction(data: &mut [u8], offset: usize, instruction: Instruction) {
    let word = encode(&instruction, BASE + offset as u32).unwrap();
    data[offset..offset + 4].copy_from_slice(&word.to_le_bytes());
}
