use std::collections::BTreeSet;

use super::script::validate_unique_script_partition;
use super::script_opcode::{ScriptFlow, opcode_spec};
use super::script_topology::{
    canonical_route_table_offsets, is_zero_tail_padding, resolve_bank_table_aliases,
    runtime_pointer_offset,
};

#[test]
fn admitted_command_widths_match_cursor_advances() {
    let expected = [
        (0x03, 4),
        (0x04, 4),
        (0x14, 4),
        (0x15, 8),
        (0x16, 4),
        (0x17, 8),
        (0x18, 4),
        (0x1e, 8),
        (0x1f, 4),
        (0x21, 12),
        (0x47, 4),
        (0x4c, 4),
    ];

    for (opcode, width) in expected {
        assert_eq!(opcode_spec(opcode).unwrap().width, width);
    }
    assert!(opcode_spec(0x77).is_err());
}

#[test]
fn script_pointer_conversion_is_bounded() {
    assert_eq!(
        runtime_pointer_offset(0x8011_a418, 0x51_800).unwrap(),
        0x4_a418
    );
    assert!(runtime_pointer_offset(0x800c_ffff, 0x51_800).is_err());
    assert!(runtime_pointer_offset(0x8012_1800, 0x51_800).is_err());
}

#[test]
fn exact_bank_table_pointer_alias_becomes_one_logical_bank() {
    assert_eq!(
        resolve_bank_table_aliases([0x4c200, 0x4c200], 0x4c240).unwrap(),
        [0x4c200]
    );
    assert_eq!(
        resolve_bank_table_aliases([0x4e090, 0x4e0e8], 0x4e140).unwrap(),
        [0x4e090, 0x4e0e8]
    );
    assert!(resolve_bank_table_aliases([0x4e0e8, 0x4e090], 0x4e140).is_err());
}

#[test]
fn trailing_single_word_route_records_can_alias_an_earlier_route_table() {
    let mut decoded = vec![0u8; 0x51_800];
    decoded[0x4c1f8..0x4c1fc].copy_from_slice(&0x8011_c05cu32.to_le_bytes());
    decoded[0x4c1fc..0x4c200].copy_from_slice(&0x8011_c05cu32.to_le_bytes());
    let starts = BTreeSet::from([0x4c05c, 0x4c1f8, 0x4c1fc]);

    let canonical = canonical_route_table_offsets(&decoded, &starts, 0x4c200).unwrap();

    assert_eq!(canonical[&0x4c05c], 0x4c05c);
    assert_eq!(canonical[&0x4c1f8], 0x4c05c);
    assert_eq!(canonical[&0x4c1fc], 0x4c05c);

    decoded[0x4c1f8..0x4c1fc].copy_from_slice(&0x8011_a008u32.to_le_bytes());
    let ordinary =
        canonical_route_table_offsets(&decoded, &BTreeSet::from([0x4c05c, 0x4c1f8]), 0x4c1fc)
            .unwrap();
    assert_eq!(ordinary[&0x4c1f8], 0x4c1f8);
}

#[test]
fn zero_tail_padding_ends_before_a_pointer_table() {
    let bytes = [0x63, 0x00, 0x00, 0x08, 0xa0];
    assert!(is_zero_tail_padding(&bytes, 1, 3, 4));
    assert!(!is_zero_tail_padding(&bytes, 0, 3, 4));
    assert!(!is_zero_tail_padding(&bytes, 1, 5, 4));
}

#[test]
fn dialogue_message_index_is_big_endian_inside_command() {
    let raw = [0x1e, 0x00, 0x12, 0x34, 0x04, 0x00, 0x00, 0x00];
    assert_eq!(u16::from_be_bytes([raw[2], raw[3]]), 0x1234);

    let choice = [
        0x21, 0x12, 0x34, 0x07, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x00,
    ];
    assert_eq!(u16::from_be_bytes([choice[1], choice[2]]), 0x1234);
}

#[test]
fn message_opcode_shapes_match_mgame_cursor_reads() {
    let expected = [
        (0x1e, 8, (2, 3)),
        (0x1f, 4, (2, 3)),
        (0x20, 12, (1, 2)),
        (0x21, 12, (1, 2)),
        (0x22, 4, (2, 3)),
        (0x24, 16, (2, 3)),
        (0x25, 12, (1, 2)),
    ];
    for (opcode, width, message_index_bytes) in expected {
        let spec = opcode_spec(opcode).unwrap();
        assert_eq!(spec.width, width);
        assert_eq!(spec.message_index_bytes, Some(message_index_bytes));
    }
}

#[test]
fn route_transfers_stop_only_unconditional_linear_walks() {
    assert_eq!(
        opcode_spec(0x1d).unwrap().flow,
        ScriptFlow::ConditionalRouteTransfer
    );
    assert!(!opcode_spec(0x1d).unwrap().flow.ends_linear_stream());
    assert_eq!(opcode_spec(0x47).unwrap().flow, ScriptFlow::RouteTransfer);
    assert!(opcode_spec(0x47).unwrap().flow.ends_linear_stream());
    assert_eq!(opcode_spec(0x4a).unwrap().flow, ScriptFlow::Terminal);
    assert_eq!(opcode_spec(0x4b).unwrap().flow, ScriptFlow::Terminal);
    assert_eq!(opcode_spec(0x4b).unwrap().width, 7);
}

#[test]
fn one_byte_handlers_keep_distinct_fallthrough_and_outer_state_flows() {
    assert_eq!(opcode_spec(0x10).unwrap().width, 1);
    assert_eq!(opcode_spec(0x10).unwrap().flow, ScriptFlow::Fallthrough);
    assert_eq!(opcode_spec(0x63).unwrap().width, 1);
    assert_eq!(opcode_spec(0x63).unwrap().flow, ScriptFlow::OuterStateYield);
    assert!(opcode_spec(0x63).unwrap().flow.ends_linear_stream());
}

#[test]
fn script_classification_partitions_each_source_asset_once() {
    let paths = |values: &[&str]| {
        values
            .iter()
            .map(|value| (*value).to_string())
            .collect::<Vec<_>>()
    };
    let sources = paths(&["a", "b", "c"]);

    assert!(
        validate_unique_script_partition(&sources, &paths(&["a", "b"]), &paths(&["c"])).is_ok()
    );
    assert!(
        validate_unique_script_partition(&sources, &paths(&["a", "a"]), &paths(&["b", "c"]))
            .is_err()
    );
    assert!(
        validate_unique_script_partition(&sources, &paths(&["a", "b"]), &paths(&["b", "c"]))
            .is_err()
    );
    assert!(validate_unique_script_partition(&sources, &paths(&["a"]), &paths(&["c"])).is_err());
}
