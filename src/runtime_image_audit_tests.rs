use crate::pipeline::sha256_bytes;

use super::{
    SELECTOR_TABLE_OFFSET, SELECTOR_TABLE_PREFIX_SIZE, path_manifest_sha256, selector_table_prefix,
};

#[test]
fn selector_table_prefix_requires_a_complete_prefix() {
    assert_eq!(selector_table_prefix(&vec![0; SELECTOR_TABLE_OFFSET]), None);
    assert_eq!(
        selector_table_prefix(&vec![
            0;
            SELECTOR_TABLE_OFFSET + SELECTOR_TABLE_PREFIX_SIZE - 1
        ]),
        None
    );
}

#[test]
fn selector_table_prefix_preserves_the_runtime_comparison_bytes() {
    let mut decoded = vec![0; SELECTOR_TABLE_OFFSET + SELECTOR_TABLE_PREFIX_SIZE];
    decoded[SELECTOR_TABLE_OFFSET..].copy_from_slice(&[
        0x8c, 0xa5, 0x10, 0x80, 0x48, 0x44, 0x11, 0x80, 0xe8, 0x6d, 0x11, 0x80, 0x24, 0x79, 0x11,
        0x80,
    ]);

    assert_eq!(
        selector_table_prefix(&decoded).as_deref(),
        Some("8ca5108048441180e86d118024791180")
    );
}

#[test]
fn path_manifest_hash_includes_order_and_terminal_newline() {
    assert_eq!(
        path_manifest_sha256(&["DAT2/MGC01.BIZ", "DAT2/MGC02.BIZ"]),
        sha256_bytes(b"DAT2/MGC01.BIZ\nDAT2/MGC02.BIZ\n")
    );
}
