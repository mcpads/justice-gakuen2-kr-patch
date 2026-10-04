use std::collections::BTreeMap;

use super::build::rebuild_record_for_test;

#[test]
fn shorter_notice_clears_its_owned_record_tail() {
    let source = [0xff; 12];
    let codes = BTreeMap::from([('가', 0x1234), ('나', 0x2345)]);
    let rebuilt = rebuild_record_for_test(&source, "가나", &codes).unwrap();
    assert_eq!(&rebuilt[..6], &[2, 0, 0x34, 0x12, 0x45, 0x23]);
    assert!(rebuilt[6..].iter().all(|byte| *byte == 0));
}

#[test]
fn notice_spaces_use_the_renderer_advance_code() {
    let source = [0xff; 12];
    let codes = BTreeMap::from([('가', 0x1234), ('나', 0x2345)]);
    let rebuilt = rebuild_record_for_test(&source, "가 나", &codes).unwrap();
    assert_eq!(&rebuilt[..8], &[3, 0, 0x34, 0x12, 0xff, 0x0f, 0x45, 0x23]);
}
