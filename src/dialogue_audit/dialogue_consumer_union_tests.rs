use std::collections::{BTreeMap, BTreeSet};

use super::dialogue_consumer_union::{
    DialogueConsumerUnionCounts, consumer_union_counts, summarize_consumer_asset_coverage,
};
use super::dialogue_consumer_union_model::DialogueConsumerAssetCoverage;

#[test]
fn consumer_union_deduplicates_semantics_and_physical_coordinates_independently() {
    let counts = consumer_union_counts(
        &set(["primary-only", "shared"]),
        &set(["selector-only", "shared"]),
        &set(["primary-coordinate", "shared-coordinate"]),
        &set(["selector-coordinate", "shared-coordinate"]),
    );

    assert_eq!(
        counts,
        DialogueConsumerUnionCounts {
            primary_semantic_group_count: 2,
            selector_semantic_group_count: 2,
            overlapping_semantic_group_count: 1,
            consumer_union_semantic_group_count: 3,
            primary_coordinate_count: 2,
            selector_coordinate_count: 2,
            overlapping_coordinate_count: 1,
            consumer_union_coordinate_count: 3,
        }
    );
}

#[test]
fn asset_coverage_preserves_source_order_and_reports_the_unowned_remainder() {
    let stored = BTreeMap::from([
        ("DAT2/MGA.BIZ".to_string(), set(["a-1", "a-2", "a-3"])),
        ("DAT2/MGB.BIZ".to_string(), set(["b-1", "b-2"])),
    ]);

    let coverage =
        summarize_consumer_asset_coverage(&stored, &set(["a-1", "b-1"]), &set(["a-2"])).unwrap();

    assert_eq!(
        coverage,
        vec![
            DialogueConsumerAssetCoverage {
                source_path: "DAT2/MGA.BIZ".to_string(),
                stored_coordinate_count: 3,
                primary_coordinate_count: 1,
                selector_coordinate_count: 1,
                consumer_union_coordinate_count: 2,
                stored_coordinate_outside_consumer_union_count: 1,
            },
            DialogueConsumerAssetCoverage {
                source_path: "DAT2/MGB.BIZ".to_string(),
                stored_coordinate_count: 2,
                primary_coordinate_count: 1,
                selector_coordinate_count: 0,
                consumer_union_coordinate_count: 1,
                stored_coordinate_outside_consumer_union_count: 1,
            },
        ]
    );
}

fn set<const N: usize>(values: [&str; N]) -> BTreeSet<String> {
    values.into_iter().map(str::to_string).collect()
}
