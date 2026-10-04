use std::collections::BTreeSet;

use super::{
    NAME_CANDIDATE_POSITION_COUNT, NAME_NAVIGATION_MAP_BYTES, NAME_NAVIGATION_POSITION_COUNT,
    build_sparse_name_navigation,
};

const FIRST_ACTION: u8 = NAME_CANDIDATE_POSITION_COUNT as u8;

#[test]
fn rewritten_navigation_skips_inactive_candidates_in_every_direction() {
    let active = (0..25).collect::<BTreeSet<_>>();

    let source = rectangular_navigation_fixture();
    let rewritten = build_sparse_name_navigation(&source, &active).unwrap();

    assert!(rewritten.redirected_edge_count > 0);
    assert!(rewritten.bytes.iter().all(|target| {
        usize::from(*target) >= NAME_CANDIDATE_POSITION_COUNT || active.contains(target)
    }));
    assert_eq!(rewritten.bytes[20 * 4 + 3], 21);
    assert_eq!(rewritten.bytes[24 * 4 + 3], FIRST_ACTION + 2);
}

#[test]
fn wholly_inactive_column_uses_deterministic_active_fallback() {
    let active = [0u8].into_iter().collect::<BTreeSet<_>>();

    let source = rectangular_navigation_fixture();
    let rewritten = build_sparse_name_navigation(&source, &active).unwrap();

    assert!(rewritten.cycle_fallback_edge_count > 0);
    assert!(
        rewritten.bytes.iter().all(|target| {
            usize::from(*target) >= NAME_CANDIDATE_POSITION_COUNT || *target == 0
        })
    );
}

#[test]
fn empty_or_out_of_grid_active_positions_are_rejected() {
    let source = rectangular_navigation_fixture();
    assert!(build_sparse_name_navigation(&source, &BTreeSet::new()).is_err());
    assert!(
        build_sparse_name_navigation(
            &source,
            &[NAME_CANDIDATE_POSITION_COUNT as u8].into_iter().collect(),
        )
        .is_err()
    );
}

#[test]
fn malformed_source_navigation_is_rejected() {
    let active = BTreeSet::from([0]);
    assert!(build_sparse_name_navigation(&[0; 4], &active).is_err());

    let mut source = rectangular_navigation_fixture();
    source[0] = NAME_NAVIGATION_POSITION_COUNT as u8;
    assert!(build_sparse_name_navigation(&source, &active).is_err());
}

#[test]
fn generated_map_has_one_direction_record_for_every_dispatched_position() {
    let active = (0..40).collect::<BTreeSet<_>>();
    let source = rectangular_navigation_fixture();
    let navigation = build_sparse_name_navigation(&source, &active).unwrap();

    assert_eq!(navigation.bytes.len(), NAME_NAVIGATION_POSITION_COUNT * 4);
}

fn rectangular_navigation_fixture() -> Vec<u8> {
    const COLUMN_COUNT: usize = 10;
    const ROW_COUNT: usize = 9;

    let mut navigation = vec![0; NAME_NAVIGATION_MAP_BYTES];
    for position in 0..NAME_CANDIDATE_POSITION_COUNT {
        let row = position / COLUMN_COUNT;
        let column = position % COLUMN_COUNT;
        let action = FIRST_ACTION + u8::try_from(row.min(6)).unwrap();
        navigation[position * 4] =
            u8::try_from(((row + ROW_COUNT - 1) % ROW_COUNT) * COLUMN_COUNT + column).unwrap();
        navigation[position * 4 + 1] =
            u8::try_from(((row + 1) % ROW_COUNT) * COLUMN_COUNT + column).unwrap();
        navigation[position * 4 + 2] = if column == 0 {
            action
        } else {
            u8::try_from(position - 1).unwrap()
        };
        navigation[position * 4 + 3] = if column + 1 == COLUMN_COUNT {
            action
        } else {
            u8::try_from(position + 1).unwrap()
        };
    }
    for action_index in 0..7 {
        let position = NAME_CANDIDATE_POSITION_COUNT + action_index;
        navigation[position * 4] = FIRST_ACTION + u8::try_from((action_index + 6) % 7).unwrap();
        navigation[position * 4 + 1] = FIRST_ACTION + u8::try_from((action_index + 1) % 7).unwrap();
        navigation[position * 4 + 2] =
            u8::try_from(action_index * COLUMN_COUNT + COLUMN_COUNT - 1).unwrap();
        navigation[position * 4 + 3] = u8::try_from(action_index * COLUMN_COUNT).unwrap();
    }
    navigation
}
