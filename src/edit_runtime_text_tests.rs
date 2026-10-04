use super::build::centered_x_for_test;
use super::pass_fixed_presentation::{password_value_display_codes, validate_choice_columns};

#[test]
fn translated_choices_keep_cursor_columns_when_a_label_is_shorter() {
    let row = [0x72, 0xfff, 0xfff, 0xfff, 0xfff, 0x73, 0x74, 0x75];
    validate_choice_columns("pass_register_another_choices", &row).unwrap();
    // Removing padding would move the second choice away from its cursor.
    let collapsed = [0x72, 0xfff, 0x73, 0x74, 0x75];
    assert!(validate_choice_columns("pass_register_another_choices", &collapsed).is_err());
    let mut shifted = row;
    shifted.swap(4, 5);
    assert!(validate_choice_columns("pass_register_another_choices", &shifted).is_err());
}

#[test]
fn acceptance_controls_leave_the_dynamic_name_field_empty() {
    let mut row = [0xfff; 18];
    for column in [0, 1, 10, 11, 13, 14, 16, 17] {
        row[column] = 0x72;
    }
    validate_choice_columns("pass_character_acceptance_controls", &row).unwrap();
    for column in [2, 9, 12, 15] {
        let mut overlap = row;
        overlap[column] = 0x73;
        assert!(validate_choice_columns("pass_character_acceptance_controls", &overlap).is_err());
    }
}

#[test]
fn shorter_korean_notices_keep_the_source_center() {
    assert_eq!(centered_x_for_test(380, 6, 5, 20).unwrap(), 390);
    assert_eq!(centered_x_for_test(404, 3, 2, 20).unwrap(), 414);
    assert_eq!(centered_x_for_test(102, 10, 9, 20).unwrap(), 112);
}

#[test]
fn password_display_lookup_includes_the_sparse_high_value_codes() {
    let mut pass = vec![0u8; 0x03d4];
    let values = (0u8..=0x4b).chain([0x80, 0x81, 0x82, 0x83]);
    for (slot, value) in values.enumerate() {
        pass[0x036c + slot] = value;
        let code = u16::try_from(slot + 1).unwrap();
        let offset = 0x02cc + usize::from(value) * 2;
        pass[offset..offset + 2].copy_from_slice(&code.to_le_bytes());
    }

    let codes = password_value_display_codes(&pass).unwrap();
    assert_eq!(codes.len(), 80);
    assert_eq!(&codes[76..], &[77, 78, 79, 80]);

    pass[0x03cc..0x03ce].copy_from_slice(&0xffffu16.to_le_bytes());
    assert_ne!(password_value_display_codes(&pass).unwrap()[76], 77);
}
