use super::{SourceChange, compose_disjoint_source_changes};

#[test]
fn disjoint_changes_are_composed_from_the_immutable_source() {
    let source = [0, 0, 0, 0, 0, 0];
    let first = [1, 1, 0, 0, 0, 0];
    let second = [0, 0, 0, 2, 2, 0];

    let composed = compose_disjoint_source_changes(
        "record",
        &source,
        &[
            SourceChange {
                owner: "first",
                bytes: &first,
            },
            SourceChange {
                owner: "second",
                bytes: &second,
            },
        ],
    )
    .unwrap();

    assert_eq!(composed.bytes, [1, 1, 0, 2, 2, 0]);
    assert_eq!(composed.changed_byte_ranges, [[0, 2], [3, 5]]);
    assert_eq!(composed.contributions[0].changed_byte_ranges, [[0, 2]]);
    assert_eq!(composed.contributions[1].changed_byte_ranges, [[3, 5]]);
}

#[test]
fn overlapping_changes_are_rejected_before_record_replacement() {
    let source = [0, 0, 0];
    let first = [0, 1, 0];
    let second = [0, 2, 0];

    let error = compose_disjoint_source_changes(
        "decoded MGTIT",
        &source,
        &[
            SourceChange {
                owner: "title-adjacent menu",
                bytes: &first,
            },
            SourceChange {
                owner: "title notice",
                bytes: &second,
            },
        ],
    )
    .unwrap_err();

    assert!(error.to_string().contains("overlap at 0x1"));
    assert!(
        error
            .to_string()
            .contains("title-adjacent menu and title notice")
    );
}

#[test]
fn a_change_cannot_resize_the_source() {
    let error = compose_disjoint_source_changes(
        "record",
        &[0, 0, 0],
        &[SourceChange {
            owner: "short",
            bytes: &[1, 0],
        }],
    )
    .unwrap_err();

    assert!(error.to_string().contains("length changed from 3 to 2"));
}
