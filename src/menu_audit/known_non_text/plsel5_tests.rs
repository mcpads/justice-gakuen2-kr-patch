use std::collections::BTreeSet;

use psx_r3000a::{Instruction, encode};

use super::plsel5::{TABLES, consumer_grammar, validate_counted_parameter_pointer_candidates};

const BASE: u32 = 0x800a_2000;

#[test]
fn classifies_all_six_counted_parameter_pointer_tables_as_non_text() {
    let (data, reachable) = fixture();

    let evidence = validate_counted_parameter_pointer_candidates(&data, BASE, &reachable).unwrap();

    assert_eq!(
        evidence.keys().copied().collect::<Vec<_>>(),
        TABLES.iter().map(|table| table.offset).collect::<Vec<_>>()
    );
}

#[test]
fn rejects_a_changed_parameter_pointer_count() {
    let (mut data, reachable) = fixture();
    data[0x34a0..0x34a4].copy_from_slice(&1u32.to_le_bytes());

    let error = validate_counted_parameter_pointer_candidates(&data, BASE, &reachable).unwrap_err();

    assert!(error.to_string().contains("count changed"));
}

#[test]
fn rejects_a_changed_parameter_pointer() {
    let (mut data, reachable) = fixture();
    data[0x34b8..0x34bc].copy_from_slice(&(BASE + 0x3248).to_le_bytes());

    let error = validate_counted_parameter_pointer_candidates(&data, BASE, &reachable).unwrap_err();

    assert!(error.to_string().contains("target 0 changed"));
}

#[test]
fn rejects_changed_parameter_selector_grammar() {
    let (mut data, reachable) = fixture();
    write_instruction(&mut data, 0x5a18, Instruction::nop());

    let error = validate_counted_parameter_pointer_candidates(&data, BASE, &reachable).unwrap_err();

    assert!(error.to_string().contains("consumer grammar changed"));
}

#[test]
fn rejects_changed_parameter_consumer_grammar() {
    let (mut data, reachable) = fixture();
    write_instruction(&mut data, 0xbc98, Instruction::nop());

    let error = validate_counted_parameter_pointer_candidates(&data, BASE, &reachable).unwrap_err();

    assert!(error.to_string().contains("consumer grammar changed"));
}

#[test]
fn rejects_an_unreachable_parameter_consumer() {
    let (data, mut reachable) = fixture();
    reachable.remove(&0x5e74);

    let error = validate_counted_parameter_pointer_candidates(&data, BASE, &reachable).unwrap_err();

    assert!(error.to_string().contains("not entrypoint-reachable"));
}

fn fixture() -> (Vec<u8>, BTreeSet<usize>) {
    let mut data = vec![0u8; 0xbca0];
    for table in TABLES {
        data[table.offset..table.offset + 4]
            .copy_from_slice(&(table.targets.len() as u32).to_le_bytes());
        for (index, target_offset) in table.targets.iter().copied().enumerate() {
            data[table.offset + 4 + index * 4..table.offset + 8 + index * 4]
                .copy_from_slice(&(BASE + target_offset as u32).to_le_bytes());
        }
        data[table.pointer_storage_offset..table.pointer_storage_offset + 4]
            .copy_from_slice(&(BASE + table.offset as u32).to_le_bytes());
    }
    let grammar = consumer_grammar();
    for (offset, instruction) in &grammar {
        write_instruction(&mut data, *offset, instruction.clone());
    }
    let reachable = TABLES
        .iter()
        .flat_map(|table| [table.initializer_offset, table.selector_offset])
        .chain([0x5e74, 0xbc30, 0xbc40, 0xbc98])
        .collect();
    (data, reachable)
}

fn write_instruction(data: &mut [u8], offset: usize, instruction: Instruction) {
    let word = encode(&instruction, BASE + offset as u32).unwrap();
    data[offset..offset + 4].copy_from_slice(&word.to_le_bytes());
}
