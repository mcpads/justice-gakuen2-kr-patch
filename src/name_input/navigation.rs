use std::collections::BTreeSet;

use anyhow::{Result, ensure};

pub const NAME_CANDIDATE_POSITION_COUNT: usize = 90;
pub const NAME_NAVIGATION_POSITION_COUNT: usize = 97;
const DIRECTION_COUNT: usize = 4;
pub const NAME_NAVIGATION_MAP_BYTES: usize = NAME_NAVIGATION_POSITION_COUNT * DIRECTION_COUNT;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NameNavigationMap {
    pub bytes: Vec<u8>,
    pub redirected_edge_count: usize,
    pub cycle_fallback_edge_count: usize,
}

pub fn build_sparse_name_navigation(
    source_navigation: &[u8],
    active_candidate_positions: &BTreeSet<u8>,
) -> Result<NameNavigationMap> {
    ensure!(
        source_navigation.len() == NAME_NAVIGATION_MAP_BYTES,
        "source name navigation map size changed"
    );
    ensure!(
        source_navigation
            .iter()
            .all(|target| usize::from(*target) < NAME_NAVIGATION_POSITION_COUNT),
        "source name navigation target is outside the dispatch table"
    );
    ensure!(
        !active_candidate_positions.is_empty(),
        "name navigation page must retain at least one active candidate"
    );
    ensure!(
        active_candidate_positions
            .iter()
            .all(|position| usize::from(*position) < NAME_CANDIDATE_POSITION_COUNT),
        "name navigation active candidate is outside the 90-cell grid"
    );

    let fallback = *active_candidate_positions
        .first()
        .expect("nonempty active set has a first cell");
    let mut bytes = source_navigation.to_vec();
    let mut redirected_edge_count = 0usize;
    let mut cycle_fallback_edge_count = 0usize;

    for source_position in 0..NAME_NAVIGATION_POSITION_COUNT {
        for direction in 0..DIRECTION_COUNT {
            let edge = source_position * DIRECTION_COUNT + direction;
            let original_target = source_navigation[edge];
            let mut target = original_target;
            let mut visited = BTreeSet::new();
            while usize::from(target) < NAME_CANDIDATE_POSITION_COUNT
                && !active_candidate_positions.contains(&target)
            {
                if !visited.insert(target) {
                    target = fallback;
                    cycle_fallback_edge_count += 1;
                    break;
                }
                target = source_navigation[usize::from(target) * DIRECTION_COUNT + direction];
            }
            if target != original_target {
                bytes[edge] = target;
                redirected_edge_count += 1;
            }
        }
    }

    ensure!(
        map_reaches_active_or_action(&bytes, active_candidate_positions),
        "name navigation rewrite still reaches an inactive candidate"
    );
    Ok(NameNavigationMap {
        bytes,
        redirected_edge_count,
        cycle_fallback_edge_count,
    })
}

fn map_reaches_active_or_action(navigation: &[u8], active_candidate_cells: &BTreeSet<u8>) -> bool {
    navigation.iter().all(|target| {
        usize::from(*target) >= NAME_CANDIDATE_POSITION_COUNT
            || active_candidate_cells.contains(target)
    })
}
