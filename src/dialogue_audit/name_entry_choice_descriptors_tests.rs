use super::name_entry_choice_descriptors::audit_name_entry_choice_descriptors;
use super::name_entry_test_fixture::name_entry_overlay;

#[test]
fn choice_descriptor_producers_bind_all_fixed_choice_cells() {
    let surfaces = audit_name_entry_choice_descriptors(&name_entry_overlay()).unwrap();

    assert_eq!(
        surfaces
            .iter()
            .map(|surface| (
                surface.id.as_str(),
                surface.logical_choice_count,
                surface.descriptor_count
            ))
            .collect::<Vec<_>>(),
        [
            ("school-choices", 5, 10),
            ("subject-choices", 8, 8),
            ("favorite-word-choices", 16, 16),
        ]
    );
    assert_eq!(surfaces[0].descriptors[1].cell.x, 32);
    assert_eq!(surfaces[1].descriptors[7].cell.x, 436);
    assert_eq!(surfaces[2].descriptors[15].cell.x, 692);
}

#[test]
fn choice_descriptor_hash_rejects_unbound_source_bytes() {
    let mut overlay = name_entry_overlay();
    overlay[0x1674] ^= 1;

    let error = audit_name_entry_choice_descriptors(&overlay).unwrap_err();

    assert!(
        error
            .to_string()
            .contains("subject-choices descriptor table")
    );
}
