use super::{DecodedMenuChange, compose_disjoint_menu_changes};

#[test]
fn disjoint_surface_changes_are_composed_from_one_source() {
    let source = [0, 0, 0, 0, 0, 0];
    let mode_select = [1, 1, 0, 0, 0, 0];
    let options = [0, 0, 0, 2, 2, 0];

    let composed = compose_disjoint_menu_changes(
        &source,
        &[
            DecodedMenuChange {
                owner: "MODE SELECT",
                decoded: &mode_select,
            },
            DecodedMenuChange {
                owner: "options",
                decoded: &options,
            },
        ],
    )
    .unwrap();

    assert_eq!(composed.decoded, [1, 1, 0, 2, 2, 0]);
    assert_eq!(composed.changed_byte_ranges, [[0, 2], [3, 5]]);
    assert_eq!(composed.contributions[0].changed_byte_ranges, [[0, 2]]);
    assert_eq!(composed.contributions[1].changed_byte_ranges, [[3, 5]]);
}

#[test]
fn overlapping_surface_changes_are_rejected_before_repacking() {
    let source = [0, 0, 0];
    let first = [0, 1, 0];
    let second = [0, 2, 0];

    let error = compose_disjoint_menu_changes(
        &source,
        &[
            DecodedMenuChange {
                owner: "MODE SELECT",
                decoded: &first,
            },
            DecodedMenuChange {
                owner: "options",
                decoded: &second,
            },
        ],
    )
    .unwrap_err();

    assert!(error.to_string().contains("overlap at 0x1"));
    assert!(error.to_string().contains("MODE SELECT and options"));
}

#[test]
fn a_surface_cannot_change_the_decoded_menu_length() {
    let error = compose_disjoint_menu_changes(
        &[0, 0, 0],
        &[DecodedMenuChange {
            owner: "options",
            decoded: &[1, 0],
        }],
    )
    .unwrap_err();

    assert!(error.to_string().contains("length changed from 3 to 2"));
}
