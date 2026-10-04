//! Derives JP source-read closure without creating Korean write authority.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Result, ensure};

use super::super::model::{
    PracticalResultSourceAtlasRewriteGateBlocker, PracticalResultSourceAtlasRewriteGateBuild,
};
use super::super::projection_model::PracticalResultConsumerProjectionId;
use super::super::source_atlas_domain_model::{
    PracticalResultSourceAtlasConsumerProjectionDenominatorStatus, SourceAtlasDomainId,
};
use super::footprint::{
    SourceConsumerReachability, SourceProjectionFootprint, SourceReadSetAssessment,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SourceProjectionReadAssessment {
    pub(super) assessed_projection_count: usize,
    pub(super) unassessed_projection_count: usize,
    pub(super) declared_read_set_complete_projection_count: usize,
    pub(super) consumer_reachability_closed_projection_count: usize,
    pub(super) consumer_reachability_unassessed_projection_count: usize,
    pub(super) consumer_reachability_unresolved_projection_count: usize,
    pub(super) consumer_reachability_dormant_projection_count: usize,
    pub(super) assessment_coverage_complete: bool,
    pub(super) in_place_rewrite_gate_open_domain_count: usize,
    pub(super) in_place_rewrite_gate_blocked_domain_count: usize,
    pub(super) in_place_rewrite_gates: Vec<PracticalResultSourceAtlasRewriteGateBuild>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct SourceAtlasConsumerProjectionDenominator<'a> {
    pub(super) status: PracticalResultSourceAtlasConsumerProjectionDenominatorStatus,
    pub(super) incomplete_reason: Option<&'a str>,
}

pub(super) fn assess_source_projection_reads(
    footprints: &[SourceProjectionFootprint<
        PracticalResultConsumerProjectionId,
        SourceAtlasDomainId,
    >],
    projection_domains: &BTreeMap<&str, &str>,
    source_atlas_denominators: &BTreeMap<&str, SourceAtlasConsumerProjectionDenominator<'_>>,
    unscoped_source_read_blocker_count: usize,
) -> Result<SourceProjectionReadAssessment> {
    ensure!(
        !source_atlas_denominators.is_empty(),
        "source-read assessment has no source-atlas domains"
    );
    ensure!(
        projection_domains
            .values()
            .all(|domain_id| source_atlas_denominators.contains_key(domain_id)),
        "source-read assessment contains an unknown source-atlas domain"
    );

    let mut footprints_by_projection = BTreeMap::new();
    for footprint in footprints {
        let projection_id = footprint.projection_id.as_str();
        let source_atlas_domain_id = footprint.source_atlas_domain_id.as_str();
        ensure!(
            projection_domains.get(projection_id).copied() == Some(source_atlas_domain_id),
            "source-read footprint {projection_id} is absent or crosses domains"
        );
        ensure!(
            footprints_by_projection
                .insert(projection_id, footprint)
                .is_none(),
            "source-read footprint {projection_id} is duplicated"
        );
    }

    let assessed_projection_count = footprints_by_projection.len();
    let unassessed_projection_count = projection_domains
        .len()
        .checked_sub(assessed_projection_count)
        .expect("footprints are a validated projection subset");
    let declared_read_set_complete_projection_count = footprints
        .iter()
        .filter(|footprint| {
            matches!(
                footprint.source_read_set_assessment,
                SourceReadSetAssessment::DeclaredReadSetComplete
            )
        })
        .count();
    let consumer_reachability_closed_projection_count = footprints
        .iter()
        .filter(|footprint| consumer_reachability_is_closed(footprint.consumer_reachability))
        .count();
    let consumer_reachability_unassessed_projection_count = footprints
        .iter()
        .filter(|footprint| {
            matches!(
                footprint.consumer_reachability,
                SourceConsumerReachability::Unassessed
            )
        })
        .count();
    let consumer_reachability_unresolved_projection_count = footprints
        .iter()
        .filter(|footprint| {
            matches!(
                footprint.consumer_reachability,
                SourceConsumerReachability::RuntimeRootSelectionUnresolved
            )
        })
        .count();
    let consumer_reachability_dormant_projection_count = footprints
        .iter()
        .filter(|footprint| {
            matches!(
                footprint.consumer_reachability,
                SourceConsumerReachability::DormantOutsideDeclaredEntrypoints
            )
        })
        .count();
    ensure!(
        consumer_reachability_closed_projection_count
            + consumer_reachability_unassessed_projection_count
            + consumer_reachability_unresolved_projection_count
            + consumer_reachability_dormant_projection_count
            == assessed_projection_count,
        "source consumer-reachability classifications do not partition assessed projections"
    );
    let assessment_coverage_complete = unassessed_projection_count == 0
        && declared_read_set_complete_projection_count == assessed_projection_count
        && consumer_reachability_unassessed_projection_count == 0
        && consumer_reachability_unresolved_projection_count == 0;

    let in_place_rewrite_gates = source_atlas_denominators
        .iter()
        .map(|(domain_id, denominator)| {
            let bound_projection_ids = projection_domains
                .iter()
                .filter(|(_, projection_domain_id)| *projection_domain_id == domain_id)
                .map(|(projection_id, _)| *projection_id)
                .collect::<Vec<_>>();
            let mut blockers = BTreeSet::new();
            if bound_projection_ids.is_empty() {
                blockers.insert(
                    PracticalResultSourceAtlasRewriteGateBlocker::NoBoundConsumerProjection,
                );
            }
            if matches!(
                denominator.status,
                PracticalResultSourceAtlasConsumerProjectionDenominatorStatus::Incomplete
            ) {
                blockers.insert(
                    PracticalResultSourceAtlasRewriteGateBlocker::ConsumerProjectionDenominatorIncomplete,
                );
            }
            if unscoped_source_read_blocker_count != 0 {
                blockers.insert(
                    PracticalResultSourceAtlasRewriteGateBlocker::UnscopedSourceReadBlocker,
                );
            }

            let mut assessed_source_projection_read_count = 0usize;
            let mut declared_read_set_complete_projection_count = 0usize;
            let mut rewrite_eligible_projection_count = 0usize;
            for projection_id in &bound_projection_ids {
                let Some(footprint) = footprints_by_projection.get(projection_id) else {
                    blockers.insert(
                        PracticalResultSourceAtlasRewriteGateBlocker::UnassessedSourceProjectionRead,
                    );
                    continue;
                };
                assessed_source_projection_read_count += 1;

                let read_set_complete = matches!(
                    footprint.source_read_set_assessment,
                    SourceReadSetAssessment::DeclaredReadSetComplete
                );
                if read_set_complete {
                    declared_read_set_complete_projection_count += 1;
                } else {
                    blockers.insert(
                        PracticalResultSourceAtlasRewriteGateBlocker::IncompleteSourceReadSet,
                    );
                }

                match footprint.consumer_reachability {
                    SourceConsumerReachability::Unassessed => {
                        blockers.insert(
                            PracticalResultSourceAtlasRewriteGateBlocker::ConsumerReachabilityUnassessed,
                        );
                    }
                    SourceConsumerReachability::RuntimeRootSelectionUnresolved => {
                        blockers.insert(
                            PracticalResultSourceAtlasRewriteGateBlocker::ConsumerReachabilityUnresolved,
                        );
                    }
                    SourceConsumerReachability::Closed
                    | SourceConsumerReachability::DormantOutsideDeclaredEntrypoints => {}
                }

                if read_set_complete
                    && consumer_reachability_allows_rewrite(footprint.consumer_reachability)
                {
                    rewrite_eligible_projection_count += 1;
                }
            }

            PracticalResultSourceAtlasRewriteGateBuild {
                source_atlas_domain_id: (*domain_id).to_string(),
                consumer_projection_denominator_status: denominator.status,
                consumer_projection_denominator_incomplete_reason: denominator
                    .incomplete_reason
                    .map(str::to_owned),
                bound_consumer_projection_count: bound_projection_ids.len(),
                assessed_source_projection_read_count,
                declared_read_set_complete_projection_count,
                rewrite_eligible_projection_count,
                unscoped_source_read_blocker_count,
                gate_open: blockers.is_empty(),
                blockers: blockers.into_iter().collect(),
            }
        })
        .collect::<Vec<_>>();
    let in_place_rewrite_gate_open_domain_count = in_place_rewrite_gates
        .iter()
        .filter(|gate| gate.gate_open)
        .count();
    let in_place_rewrite_gate_blocked_domain_count = source_atlas_denominators
        .len()
        .checked_sub(in_place_rewrite_gate_open_domain_count)
        .expect("open rewrite gates are a source-atlas-domain subset");

    Ok(SourceProjectionReadAssessment {
        assessed_projection_count,
        unassessed_projection_count,
        declared_read_set_complete_projection_count,
        consumer_reachability_closed_projection_count,
        consumer_reachability_unassessed_projection_count,
        consumer_reachability_unresolved_projection_count,
        consumer_reachability_dormant_projection_count,
        assessment_coverage_complete,
        in_place_rewrite_gate_open_domain_count,
        in_place_rewrite_gate_blocked_domain_count,
        in_place_rewrite_gates,
    })
}

fn consumer_reachability_is_closed(reachability: SourceConsumerReachability) -> bool {
    match reachability {
        SourceConsumerReachability::Unassessed
        | SourceConsumerReachability::RuntimeRootSelectionUnresolved
        | SourceConsumerReachability::DormantOutsideDeclaredEntrypoints => false,
        SourceConsumerReachability::Closed => true,
    }
}

fn consumer_reachability_allows_rewrite(reachability: SourceConsumerReachability) -> bool {
    matches!(
        reachability,
        SourceConsumerReachability::Closed
            | SourceConsumerReachability::DormantOutsideDeclaredEntrypoints
    )
}

#[cfg(test)]
#[path = "source_read_assessment_tests.rs"]
mod tests;
