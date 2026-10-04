use std::collections::{BTreeMap, BTreeSet};

use super::dialogue_code_allocation::allocate_character_codes;

#[test]
fn allocation_reuses_exact_source_codes_before_rewriting_cells() {
    let required = BTreeSet::from(['가', '나', ' ']);
    let reusable = BTreeMap::from([('가', vec![9, 3]), (' ', vec![2])]);

    let first = allocate_character_codes(&required, &reusable, &BTreeSet::new(), 8, 8).unwrap();
    let second = allocate_character_codes(&required, &reusable, &BTreeSet::new(), 8, 8).unwrap();

    assert_eq!(first, second);
    assert_eq!(first[&' '].code, 2);
    assert!(first[&' '].source_glyph_reused);
    assert_eq!(first[&'가'].code, 3);
    assert!(first[&'가'].source_glyph_reused);
    assert_eq!(first[&'나'].code, 0);
    assert!(!first[&'나'].source_glyph_reused);
}

#[test]
fn allocation_never_consumes_the_global_name_reservation() {
    let required = BTreeSet::from(['가', '나', '다']);
    let allocated =
        allocate_character_codes(&required, &BTreeMap::new(), &BTreeSet::new(), 3, 3).unwrap();

    assert!(allocated.values().all(|assignment| assignment.code < 3));
    assert!(
        allocate_character_codes(
            &BTreeSet::from(['가', '나', '다', '라']),
            &BTreeMap::new(),
            &BTreeSet::new(),
            3,
            3
        )
        .is_err()
    );
}

#[test]
fn allocation_reuses_but_never_replaces_a_protected_source_glyph() {
    let required = BTreeSet::from(['가', '나']);
    let reusable = BTreeMap::from([('가', vec![4, 1])]);
    let protected = BTreeSet::from([0, 1, 2]);

    let allocated = allocate_character_codes(&required, &reusable, &protected, 5, 5).unwrap();

    assert_eq!(allocated[&'가'].code, 1);
    assert!(allocated[&'가'].source_glyph_reused);
    assert_eq!(allocated[&'나'].code, 3);
    assert!(!allocated[&'나'].source_glyph_reused);
}

#[test]
fn allocation_can_share_an_exact_global_name_code_without_repurposing_it() {
    let required = BTreeSet::from(['김']);
    let reusable = BTreeMap::from([('김', vec![5])]);

    let allocated = allocate_character_codes(&required, &reusable, &BTreeSet::new(), 3, 6).unwrap();

    assert_eq!(allocated[&'김'].code, 5);
    assert!(allocated[&'김'].source_glyph_reused);
}
