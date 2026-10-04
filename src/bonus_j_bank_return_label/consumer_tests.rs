use std::collections::BTreeSet;

use super::consumer::{validate_return_label_composed_consumer, validate_return_label_consumer};
use super::glyph_ownership::allocated_physical_alias_codes;
use super::test_support::{source_overlay_fixture, write_u32};

#[test]
fn source_consumer_binds_the_pointer_both_callers_and_renderer_grammar() {
    let validation = validate_return_label_consumer(&source_overlay_fixture()).unwrap();

    assert!(validation.declared_physical_alias_set_matches);
    assert!(validation.existing_bonus_component_allocations_disjoint);
    assert!(validation.pointer_command_table_parsed_glyphs_disjoint);
    assert!(validation.declared_direct_selector_byte_region_scan_disjoint);
}

#[test]
fn the_declared_glyphs_reserve_their_complete_physical_alias_closure() {
    assert_eq!(
        allocated_physical_alias_codes(),
        [0x03b3, 0x03b5, 0x03bf]
            .into_iter()
            .collect::<BTreeSet<_>>()
    );
}

#[test]
fn pointer_caller_and_renderer_drift_are_each_rejected() {
    for offset in [0x13b4, 0xca90, 0x9634] {
        let mut drifted = source_overlay_fixture();
        write_u32(&mut drifted, offset, 0);
        assert!(validate_return_label_composed_consumer(&drifted).is_err());
    }
}

#[test]
fn a_source_pointer_command_using_an_allocated_alias_is_rejected() {
    let mut drifted = source_overlay_fixture();
    drifted[0x0100..0x0104].copy_from_slice(&[0x03, 0x03, 0x0b, 0x81]);

    assert!(validate_return_label_consumer(&drifted).is_err());
}

#[test]
fn a_direct_selector_using_an_allocated_alias_is_rejected() {
    let mut drifted = source_overlay_fixture();
    drifted[0x1500..0x1503].copy_from_slice(&[0x0b, 0x0f, 0x0b]);

    assert!(validate_return_label_consumer(&drifted).is_err());
}
