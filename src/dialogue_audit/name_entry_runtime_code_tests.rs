use super::name_entry_runtime_code::{
    RUNTIME_CODE_REGION_OFFSET, RUNTIME_CODE_REGION_SIZE, audit_name_entry_runtime_code_region,
    install_name_entry_runtime_program,
};

#[test]
fn exact_zero_tail_is_bound_without_claiming_a_runtime_consumer() {
    let decoded = vec![0u8; RUNTIME_CODE_REGION_OFFSET + RUNTIME_CODE_REGION_SIZE];

    let audit = audit_name_entry_runtime_code_region(&decoded).unwrap();

    assert_eq!(audit.decoded_byte_range, [0x33340, 0x33800]);
    assert_eq!(audit.runtime_address_range, ["0x80103340", "0x80103800"]);
    assert_eq!(audit.byte_count, 1_216);
    assert!(audit.source_bytes_are_zero);
    assert!(!audit.typed_code_installed);
    assert!(!audit.runtime_execution_verified);
}

#[test]
fn changed_or_extended_tail_is_rejected() {
    let mut decoded = vec![0u8; RUNTIME_CODE_REGION_OFFSET + RUNTIME_CODE_REGION_SIZE];
    decoded[RUNTIME_CODE_REGION_OFFSET] = 1;
    assert!(audit_name_entry_runtime_code_region(&decoded).is_err());

    decoded[RUNTIME_CODE_REGION_OFFSET] = 0;
    decoded.push(0);
    assert!(audit_name_entry_runtime_code_region(&decoded).is_err());
}

#[test]
fn typed_program_install_preserves_unused_tail_bytes() {
    let mut decoded = vec![0u8; RUNTIME_CODE_REGION_OFFSET + RUNTIME_CODE_REGION_SIZE];
    let program = [1_u8, 2, 3, 4];

    install_name_entry_runtime_program(&mut decoded, &program).unwrap();

    assert_eq!(
        &decoded[RUNTIME_CODE_REGION_OFFSET..RUNTIME_CODE_REGION_OFFSET + program.len()],
        &program
    );
    assert!(
        decoded[RUNTIME_CODE_REGION_OFFSET + program.len()..]
            .iter()
            .all(|byte| *byte == 0)
    );
    assert!(install_name_entry_runtime_program(&mut decoded, &program).is_err());
}
