use std::collections::{BTreeMap, BTreeSet};

use super::{SourceAtlasConsumerProjectionDenominator, assess_source_projection_reads};
use crate::mode_descendant_graphics::practical_results::model::PracticalResultSourceAtlasRewriteGateBlocker;
use crate::mode_descendant_graphics::practical_results::projection_model::PracticalResultConsumerProjectionId;
use crate::mode_descendant_graphics::practical_results::projection_validation::footprint::{
    SourceAtlasTileGeometry, SourceConsumerReachability, SourceProjectionFootprint,
    SourceProjectionReadPopulation, SourceReadFootprintDerivation, SourceReadSetAssessment,
};
use crate::mode_descendant_graphics::practical_results::source_atlas_domain_model::PracticalResultSourceAtlasConsumerProjectionDenominatorStatus;
use crate::mode_descendant_graphics::practical_results::source_atlas_domain_model::SourceAtlasDomainId;

fn complete_denominator() -> SourceAtlasConsumerProjectionDenominator<'static> {
    SourceAtlasConsumerProjectionDenominator {
        status: PracticalResultSourceAtlasConsumerProjectionDenominatorStatus::Complete,
        incomplete_reason: None,
    }
}

fn footprint(
    projection_id: &str,
    source_read_set: SourceReadSetAssessment,
    consumer_reachability: SourceConsumerReachability,
) -> SourceProjectionFootprint<PracticalResultConsumerProjectionId, SourceAtlasDomainId> {
    SourceProjectionFootprint {
        projection_id: serde_json::from_str(&format!("\"{projection_id}\"")).unwrap(),
        source_atlas_domain_id: serde_json::from_str("\"source_domain\"").unwrap(),
        tile_geometry: Some(SourceAtlasTileGeometry {
            atlas_width: 256,
            atlas_height: 256,
            tile_width: 32,
            tile_height: 16,
        }),
        derivation: SourceReadFootprintDerivation::DynamicConfigGraph,
        read_population: SourceProjectionReadPopulation::DynamicRootArrayTargets {
            pointer_target_count: 1,
            unique_pointer_target_count: 1,
            selector_referenced_unique_pointer_target_count: 1,
        },
        decoded_tile_ids: Some(BTreeSet::from([0])),
        selector_referenced_tile_ids: Some(BTreeSet::from([0])),
        structural_stream_source_offsets: Some(BTreeSet::from([0x20])),
        selector_referenced_stream_source_offsets: Some(BTreeSet::from([0x20])),
        dynamic_stream_state_headers: Some(Vec::new()),
        source_read_rectangles: Vec::new(),
        selector_referenced_rectangles: Some(Vec::new()),
        source_read_set_assessment: source_read_set,
        consumer_reachability,
    }
}

fn assess(
    footprints: &[SourceProjectionFootprint<
        PracticalResultConsumerProjectionId,
        SourceAtlasDomainId,
    >],
    unscoped_blocker_count: usize,
) -> super::SourceProjectionReadAssessment {
    let projection_domains = footprints
        .iter()
        .map(|footprint| (footprint.projection_id.as_str(), "source_domain"))
        .collect::<BTreeMap<_, _>>();
    assess_source_projection_reads(
        footprints,
        &projection_domains,
        &BTreeMap::from([("source_domain", complete_denominator())]),
        unscoped_blocker_count,
    )
    .unwrap()
}

#[test]
fn reachability_report_does_not_merge_unassessed_with_unresolved() {
    let footprints = vec![
        footprint(
            "static_projection",
            SourceReadSetAssessment::InputSetUnassessed,
            SourceConsumerReachability::Unassessed,
        ),
        footprint(
            "dynamic_projection",
            SourceReadSetAssessment::DeclaredReadSetComplete,
            SourceConsumerReachability::RuntimeRootSelectionUnresolved,
        ),
    ];
    let assessment = assess(&footprints, 0);

    assert_eq!(assessment.assessed_projection_count, 2);
    assert_eq!(assessment.unassessed_projection_count, 0);
    assert_eq!(assessment.declared_read_set_complete_projection_count, 1);
    assert_eq!(
        assessment.consumer_reachability_unassessed_projection_count,
        1
    );
    assert_eq!(
        assessment.consumer_reachability_unresolved_projection_count,
        1
    );
    assert_eq!(assessment.consumer_reachability_dormant_projection_count, 0);
    assert!(!assessment.assessment_coverage_complete);
    assert_eq!(assessment.in_place_rewrite_gate_open_domain_count, 0);
    assert_eq!(assessment.in_place_rewrite_gate_blocked_domain_count, 1);
    assert_eq!(
        assessment.in_place_rewrite_gates[0].blockers,
        vec![
            PracticalResultSourceAtlasRewriteGateBlocker::IncompleteSourceReadSet,
            PracticalResultSourceAtlasRewriteGateBlocker::ConsumerReachabilityUnassessed,
            PracticalResultSourceAtlasRewriteGateBlocker::ConsumerReachabilityUnresolved,
        ]
    );
}

#[test]
fn rewrite_gate_report_names_domains_without_bound_consumers() {
    let footprints = vec![footprint(
        "projection",
        SourceReadSetAssessment::DeclaredReadSetComplete,
        SourceConsumerReachability::Closed,
    )];
    let projection_domains = BTreeMap::from([("projection", "source_domain")]);
    let source_atlas_denominators = BTreeMap::from([
        ("source_domain", complete_denominator()),
        ("unbound_domain", complete_denominator()),
    ]);

    let assessment = assess_source_projection_reads(
        &footprints,
        &projection_domains,
        &source_atlas_denominators,
        0,
    )
    .unwrap();

    assert_eq!(assessment.in_place_rewrite_gate_open_domain_count, 1);
    assert_eq!(assessment.in_place_rewrite_gate_blocked_domain_count, 1);
    assert_eq!(
        assessment
            .in_place_rewrite_gates
            .iter()
            .map(|gate| (
                gate.source_atlas_domain_id.as_str(),
                gate.gate_open,
                gate.blockers.as_slice(),
            ))
            .collect::<Vec<_>>(),
        vec![
            ("source_domain", true, [].as_slice()),
            (
                "unbound_domain",
                false,
                [PracticalResultSourceAtlasRewriteGateBlocker::NoBoundConsumerProjection]
                    .as_slice(),
            ),
        ]
    );
}

#[test]
fn incomplete_consumer_denominator_blocks_a_complete_partial_projection() {
    let footprints = vec![footprint(
        "known_projection",
        SourceReadSetAssessment::DeclaredReadSetComplete,
        SourceConsumerReachability::Closed,
    )];
    let projection_domains = BTreeMap::from([("known_projection", "source_domain")]);
    let source_atlas_denominators = BTreeMap::from([(
        "source_domain",
        SourceAtlasConsumerProjectionDenominator {
            status: PracticalResultSourceAtlasConsumerProjectionDenominatorStatus::Incomplete,
            incomplete_reason: Some("more consumers remain"),
        },
    )]);

    let assessment = assess_source_projection_reads(
        &footprints,
        &projection_domains,
        &source_atlas_denominators,
        0,
    )
    .unwrap();
    let gate = &assessment.in_place_rewrite_gates[0];

    assert!(!gate.gate_open);
    assert_eq!(gate.rewrite_eligible_projection_count, 1);
    assert_eq!(
        gate.blockers,
        vec![PracticalResultSourceAtlasRewriteGateBlocker::ConsumerProjectionDenominatorIncomplete]
    );
    assert_eq!(
        gate.consumer_projection_denominator_incomplete_reason
            .as_deref(),
        Some("more consumers remain")
    );
}

#[test]
fn in_place_rewrite_gate_requires_both_read_axes_and_no_unscoped_blocker() {
    for (graph, reachability, unscoped_blockers, expected_open) in [
        (
            SourceReadSetAssessment::InputSetUnassessed,
            SourceConsumerReachability::Closed,
            0,
            0,
        ),
        (
            SourceReadSetAssessment::DeclaredReadSetComplete,
            SourceConsumerReachability::Unassessed,
            0,
            0,
        ),
        (
            SourceReadSetAssessment::DeclaredReadSetComplete,
            SourceConsumerReachability::RuntimeRootSelectionUnresolved,
            0,
            0,
        ),
        (
            SourceReadSetAssessment::DeclaredReadSetComplete,
            SourceConsumerReachability::Closed,
            1,
            0,
        ),
        (
            SourceReadSetAssessment::DeclaredReadSetComplete,
            SourceConsumerReachability::Closed,
            0,
            1,
        ),
        (
            SourceReadSetAssessment::DeclaredReadSetComplete,
            SourceConsumerReachability::DormantOutsideDeclaredEntrypoints,
            0,
            1,
        ),
    ] {
        let assessment = assess(
            &[footprint("projection", graph, reachability)],
            unscoped_blockers,
        );
        assert_eq!(
            assessment.in_place_rewrite_gate_open_domain_count, expected_open,
            "unexpected gate result for {graph:?}, {reachability:?}, blockers={unscoped_blockers}"
        );
        assert_eq!(
            assessment.assessment_coverage_complete,
            matches!(
                (graph, reachability),
                (
                    SourceReadSetAssessment::DeclaredReadSetComplete,
                    SourceConsumerReachability::Closed
                        | SourceConsumerReachability::DormantOutsideDeclaredEntrypoints
                )
            )
        );
    }
}
