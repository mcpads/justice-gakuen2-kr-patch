use std::collections::BTreeSet;

use super::consumer::{
    POINTER_COMMAND_TABLE_RANGES, validate_exit_confirmation_composed_consumer,
    validate_exit_confirmation_consumer,
};
use super::glyph_ownership::allocated_physical_alias_codes;
use super::test_support::{source_overlay_fixture, write_u32};

#[test]
fn typed_consumer_binds_clerk_pointer_and_renderer_grammar() {
    let validation = validate_exit_confirmation_consumer(&source_overlay_fixture()).unwrap();

    assert!(validation.declared_physical_alias_set_matches);
    assert!(validation.fixed_ui_tim_disjoint);
    assert_eq!(
        validation.pointer_command_record_count,
        POINTER_COMMAND_TABLE_RANGES
            .iter()
            .map(|[start, end]| (end - start) / 4)
            .sum::<usize>()
    );
    assert!(validation.pointer_command_table_parsed_glyphs_disjoint);
    assert!(validation.declared_direct_selector_byte_region_scan_disjoint);
}

#[test]
fn allocated_codes_reserve_their_complete_physical_alias_closure() {
    assert_eq!(
        allocated_physical_alias_codes(),
        [
            0x0390, 0x0391, 0x0392, 0x0393, 0x0394, 0x0395, 0x0396, 0x0397, 0x0398, 0x0399, 0x039a,
            0x039b, 0x039d, 0x039e, 0x039f,
        ]
        .into_iter()
        .collect::<BTreeSet<_>>()
    );
}

#[test]
fn pointer_caller_and_page_base_drift_are_each_rejected() {
    for offset in [0x372c, 0x3754, 0x5a0c, 0x7190] {
        let mut drifted = source_overlay_fixture();
        write_u32(&mut drifted, offset, 0);
        assert!(validate_exit_confirmation_composed_consumer(&drifted).is_err());
    }
}

#[test]
fn a_source_pointer_command_using_an_allocated_alias_is_rejected() {
    let mut drifted = source_overlay_fixture();
    drifted[0x0100..0x0104].copy_from_slice(&[0x03, 0x00, 0x09, 0x81]);
    assert!(validate_exit_confirmation_consumer(&drifted).is_err());
}

#[test]
fn a_page_plus_nine_direct_selector_using_an_alias_is_rejected() {
    let mut drifted = source_overlay_fixture();
    drifted[0x3800..0x3803].copy_from_slice(&[0x0c, 0x00, 0x09]);
    assert!(validate_exit_confirmation_consumer(&drifted).is_err());
}
