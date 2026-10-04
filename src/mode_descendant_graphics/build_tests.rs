use super::{derive_palette_roles, index_histogram};

#[test]
fn palette_roles_follow_the_source_cell_population_deterministically() {
    let histogram = index_histogram(&[0, 0, 0, 4, 4, 7], 16).unwrap();
    assert_eq!(derive_palette_roles(&histogram).unwrap(), (0, Some(7), 4));
}

#[test]
fn palette_roles_reject_a_cell_without_source_ink() {
    let histogram = index_histogram(&[3, 3, 3], 16).unwrap();
    assert!(derive_palette_roles(&histogram).is_err());
}
