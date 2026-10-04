//! Full producer-to-consumer occurrence census for translated character-select UI.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Result, ensure};

use super::model::{
    CharacterSelectConsumerLocationStatus, CharacterSelectConsumerReferenceKind,
    CharacterSelectProducerBindingStatus, CharacterSelectRouteCensus,
    CharacterSelectRouteConsumerTarget, CharacterSelectRouteOccurrence, CharacterSelectRouteOwner,
    CharacterSelectRouteProducerTarget, CharacterSelectRouteReadiness,
    CharacterSelectTextureSurface,
};

#[path = "route_census/pending.rs"]
mod pending;

pub(super) fn bound_occurrence(
    occurrence_id: impl Into<String>,
    producer_family: impl Into<String>,
    source_ui_ids: impl IntoIterator<Item = impl Into<String>>,
    producer_targets: Vec<CharacterSelectRouteProducerTarget>,
    consumer_targets: Vec<CharacterSelectRouteConsumerTarget>,
) -> CharacterSelectRouteOccurrence {
    let consumer_location_status = consumer_location_status(&consumer_targets);
    CharacterSelectRouteOccurrence {
        occurrence_id: occurrence_id.into(),
        producer_family: producer_family.into(),
        producer_binding_status: CharacterSelectProducerBindingStatus::Bound,
        consumer_location_status,
        readiness: match consumer_location_status {
            CharacterSelectConsumerLocationStatus::ExactByteOffsets => {
                CharacterSelectRouteReadiness::ProducerAndConsumerKnown
            }
            CharacterSelectConsumerLocationStatus::RecordOnly
            | CharacterSelectConsumerLocationStatus::Unresolved => {
                CharacterSelectRouteReadiness::ProducerKnownConsumerPending
            }
        },
        owner: CharacterSelectRouteOwner::CharacterSelectGraphics,
        source_ui_ids: source_ui_ids.into_iter().map(Into::into).collect(),
        producer_targets,
        consumer_targets,
    }
}

pub(super) fn pending_occurrence(
    occurrence_id: impl Into<String>,
    producer_family: impl Into<String>,
    source_ui_ids: impl IntoIterator<Item = impl Into<String>>,
    producer_targets: Vec<CharacterSelectRouteProducerTarget>,
    consumer_targets: Vec<CharacterSelectRouteConsumerTarget>,
) -> CharacterSelectRouteOccurrence {
    let consumer_location_status = consumer_location_status(&consumer_targets);
    CharacterSelectRouteOccurrence {
        occurrence_id: occurrence_id.into(),
        producer_family: producer_family.into(),
        producer_binding_status: CharacterSelectProducerBindingStatus::Pending,
        consumer_location_status,
        readiness: pending_readiness(false, consumer_location_status),
        owner: CharacterSelectRouteOwner::CharacterSelectGraphics,
        source_ui_ids: source_ui_ids.into_iter().map(Into::into).collect(),
        producer_targets,
        consumer_targets,
    }
}

fn pending_readiness(
    producer_candidate: bool,
    consumer_location_status: CharacterSelectConsumerLocationStatus,
) -> CharacterSelectRouteReadiness {
    if producer_candidate {
        CharacterSelectRouteReadiness::ProducerCandidateConsumerPending
    } else if consumer_location_status == CharacterSelectConsumerLocationStatus::ExactByteOffsets {
        CharacterSelectRouteReadiness::ProducerAndConsumerKnown
    } else {
        CharacterSelectRouteReadiness::ProducerKnownConsumerPending
    }
}

pub(super) fn consumer_location_status(
    consumer_targets: &[CharacterSelectRouteConsumerTarget],
) -> CharacterSelectConsumerLocationStatus {
    if consumer_targets.is_empty() {
        CharacterSelectConsumerLocationStatus::Unresolved
    } else if consumer_targets.iter().all(|target| {
        target.reference_kind != CharacterSelectConsumerReferenceKind::RecordIdentity
            && !target.byte_offsets.is_empty()
    }) {
        CharacterSelectConsumerLocationStatus::ExactByteOffsets
    } else {
        CharacterSelectConsumerLocationStatus::RecordOnly
    }
}

pub(super) fn producer_target(
    source_record: impl Into<String>,
    tim_offset: Option<usize>,
    surface: Option<CharacterSelectTextureSurface>,
    physical_region_ids: impl IntoIterator<Item = impl Into<String>>,
) -> CharacterSelectRouteProducerTarget {
    CharacterSelectRouteProducerTarget {
        source_record: source_record.into(),
        tim_offset,
        surface,
        physical_region_ids: physical_region_ids.into_iter().map(Into::into).collect(),
    }
}

pub(super) fn consumer_target(
    source_record: impl Into<String>,
    consumer_record: impl Into<String>,
    reference_kind: CharacterSelectConsumerReferenceKind,
    byte_offsets: impl IntoIterator<Item = usize>,
    runtime_states: impl IntoIterator<Item = impl Into<String>>,
) -> CharacterSelectRouteConsumerTarget {
    CharacterSelectRouteConsumerTarget {
        source_record: source_record.into(),
        consumer_record: consumer_record.into(),
        reference_kind,
        byte_offsets: byte_offsets.into_iter().collect(),
        runtime_states: runtime_states.into_iter().map(Into::into).collect(),
    }
}

pub(super) fn census_routes(
    translated_source_ui_ids: &BTreeSet<&str>,
    emitted_occurrences: Vec<CharacterSelectRouteOccurrence>,
) -> Result<CharacterSelectRouteCensus> {
    let mut occurrences = pending::declared_pending_occurrences(translated_source_ui_ids)
        .into_iter()
        .map(|occurrence| (occurrence.occurrence_id.clone(), occurrence))
        .collect::<BTreeMap<_, _>>();
    let mut emitted_ids = BTreeSet::new();
    for occurrence in emitted_occurrences {
        ensure!(
            emitted_ids.insert(occurrence.occurrence_id.clone()),
            "duplicate emitted character-select route occurrence {}",
            occurrence.occurrence_id
        );
        if let Some(declared) = occurrences.get(&occurrence.occurrence_id) {
            ensure!(
                declared.source_ui_ids == occurrence.source_ui_ids,
                "character-select route occurrence {} changed its logical source set",
                occurrence.occurrence_id
            );
        }
        occurrences.insert(occurrence.occurrence_id.clone(), occurrence);
    }

    validate_occurrences(translated_source_ui_ids, occurrences.values())?;
    let represented = occurrences
        .values()
        .flat_map(|occurrence| occurrence.source_ui_ids.iter().map(String::as_str))
        .collect::<BTreeSet<_>>();
    let missing_source_ui_ids = translated_source_ui_ids
        .difference(&represented)
        .map(|source_ui_id| (*source_ui_id).to_string())
        .collect::<Vec<_>>();
    for source_ui_id in missing_source_ui_ids {
        let occurrence_id = format!("unclassified:{source_ui_id}");
        occurrences.insert(
            occurrence_id.clone(),
            CharacterSelectRouteOccurrence {
                occurrence_id,
                producer_family: "unclassified".to_string(),
                producer_binding_status: CharacterSelectProducerBindingStatus::Pending,
                consumer_location_status: CharacterSelectConsumerLocationStatus::Unresolved,
                readiness: CharacterSelectRouteReadiness::ProducerCandidateConsumerPending,
                owner: CharacterSelectRouteOwner::Unassigned,
                source_ui_ids: vec![source_ui_id],
                producer_targets: Vec::new(),
                consumer_targets: Vec::new(),
            },
        );
    }

    let occurrences = occurrences.into_values().collect::<Vec<_>>();
    validate_occurrences(translated_source_ui_ids, occurrences.iter())?;
    let mut route_completion_by_source = translated_source_ui_ids
        .iter()
        .map(|source_ui_id| (*source_ui_id, Vec::new()))
        .collect::<BTreeMap<_, _>>();
    for occurrence in &occurrences {
        for source_ui_id in &occurrence.source_ui_ids {
            route_completion_by_source
                .get_mut(source_ui_id.as_str())
                .expect("route source validated against translation inventory")
                .push(route_is_complete(occurrence));
        }
    }
    let fully_routed_source_ui_count = route_completion_by_source
        .values()
        .filter(|statuses| !statuses.is_empty() && statuses.iter().all(|status| *status))
        .count();
    let partially_routed_source_ui_ids = route_completion_by_source
        .iter()
        .filter(|(_, statuses)| statuses.contains(&true) && statuses.contains(&false))
        .map(|(source_ui_id, _)| (*source_ui_id).to_string())
        .collect::<Vec<_>>();
    let unresolved_only_source_ui_ids = route_completion_by_source
        .iter()
        .filter(|(_, statuses)| !statuses.is_empty() && statuses.iter().all(|status| !*status))
        .map(|(source_ui_id, _)| (*source_ui_id).to_string())
        .collect::<Vec<_>>();
    let source_ui_ids_with_multiple_occurrences = route_completion_by_source
        .iter()
        .filter(|(_, statuses)| statuses.len() > 1)
        .map(|(source_ui_id, _)| (*source_ui_id).to_string())
        .collect::<Vec<_>>();
    let producer_pending = occurrences
        .iter()
        .filter(|occurrence| {
            occurrence.producer_binding_status == CharacterSelectProducerBindingStatus::Pending
        })
        .collect::<Vec<_>>();
    let pending_count = |readiness| {
        producer_pending
            .iter()
            .filter(|occurrence| occurrence.readiness == readiness)
            .count()
    };
    let unclassified_pending_occurrence_ids = producer_pending
        .iter()
        .filter(|occurrence| occurrence.producer_family == "unclassified")
        .map(|occurrence| occurrence.occurrence_id.clone())
        .collect::<Vec<_>>();
    let consumer_location_count = |status| {
        occurrences
            .iter()
            .filter(|occurrence| occurrence.consumer_location_status == status)
            .count()
    };
    let fully_routed_occurrence_count = occurrences
        .iter()
        .filter(|occurrence| route_is_complete(occurrence))
        .count();

    Ok(CharacterSelectRouteCensus {
        translated_source_ui_count: translated_source_ui_ids.len(),
        route_occurrence_count: occurrences.len(),
        producer_bound_occurrence_count: occurrences.len() - producer_pending.len(),
        producer_pending_occurrence_count: producer_pending.len(),
        exact_consumer_location_occurrence_count: consumer_location_count(
            CharacterSelectConsumerLocationStatus::ExactByteOffsets,
        ),
        record_only_consumer_location_occurrence_count: consumer_location_count(
            CharacterSelectConsumerLocationStatus::RecordOnly,
        ),
        unresolved_consumer_location_occurrence_count: consumer_location_count(
            CharacterSelectConsumerLocationStatus::Unresolved,
        ),
        fully_routed_occurrence_count,
        unresolved_route_occurrence_count: occurrences.len() - fully_routed_occurrence_count,
        fully_routed_source_ui_count,
        partially_routed_source_ui_ids,
        unresolved_only_source_ui_ids,
        source_ui_ids_with_multiple_occurrences,
        producer_and_consumer_known_pending_occurrence_count: pending_count(
            CharacterSelectRouteReadiness::ProducerAndConsumerKnown,
        ),
        producer_known_consumer_pending_occurrence_count: pending_count(
            CharacterSelectRouteReadiness::ProducerKnownConsumerPending,
        ),
        producer_candidate_consumer_pending_occurrence_count: pending_count(
            CharacterSelectRouteReadiness::ProducerCandidateConsumerPending,
        ),
        unclassified_pending_occurrence_ids,
        occurrences,
    })
}

pub(super) fn route_is_complete(occurrence: &CharacterSelectRouteOccurrence) -> bool {
    occurrence.producer_binding_status == CharacterSelectProducerBindingStatus::Bound
        && occurrence.consumer_location_status
            == CharacterSelectConsumerLocationStatus::ExactByteOffsets
}

fn validate_occurrences<'a>(
    translated_source_ui_ids: &BTreeSet<&str>,
    occurrences: impl IntoIterator<Item = &'a CharacterSelectRouteOccurrence>,
) -> Result<()> {
    for occurrence in occurrences {
        ensure!(
            !occurrence.occurrence_id.is_empty(),
            "character-select route occurrence has an empty identity"
        );
        ensure!(
            !occurrence.source_ui_ids.is_empty(),
            "character-select route occurrence {} has no logical source",
            occurrence.occurrence_id
        );
        ensure!(
            occurrence
                .source_ui_ids
                .iter()
                .all(|source_ui_id| translated_source_ui_ids.contains(source_ui_id.as_str())),
            "character-select route occurrence {} references a non-translated logical source",
            occurrence.occurrence_id
        );
        ensure!(
            occurrence.consumer_location_status
                == consumer_location_status(&occurrence.consumer_targets),
            "character-select route occurrence {} has inconsistent consumer location evidence",
            occurrence.occurrence_id
        );
        ensure!(
            occurrence.consumer_targets.iter().all(|target| {
                (target.reference_kind == CharacterSelectConsumerReferenceKind::RecordIdentity)
                    == target.byte_offsets.is_empty()
            }),
            "character-select route occurrence {} has inconsistent consumer reference kinds",
            occurrence.occurrence_id
        );
        let producer_source_records = occurrence
            .producer_targets
            .iter()
            .map(|target| target.source_record.as_str())
            .collect::<BTreeSet<_>>();
        let consumer_source_records = occurrence
            .consumer_targets
            .iter()
            .map(|target| target.source_record.as_str())
            .collect::<BTreeSet<_>>();
        ensure!(
            consumer_source_records.is_subset(&producer_source_records),
            "character-select route occurrence {} consumes a source outside its producer set",
            occurrence.occurrence_id
        );
        if occurrence.consumer_location_status
            == CharacterSelectConsumerLocationStatus::ExactByteOffsets
        {
            ensure!(
                consumer_source_records == producer_source_records,
                "character-select route occurrence {} lacks an exact consumer for every producer source",
                occurrence.occurrence_id
            );
        }
        let readiness_matches_evidence = match occurrence.readiness {
            CharacterSelectRouteReadiness::ProducerAndConsumerKnown => {
                occurrence.consumer_location_status
                    == CharacterSelectConsumerLocationStatus::ExactByteOffsets
            }
            CharacterSelectRouteReadiness::ProducerKnownConsumerPending => {
                occurrence.consumer_location_status
                    != CharacterSelectConsumerLocationStatus::ExactByteOffsets
            }
            CharacterSelectRouteReadiness::ProducerCandidateConsumerPending => {
                occurrence.producer_binding_status == CharacterSelectProducerBindingStatus::Pending
            }
        };
        ensure!(
            readiness_matches_evidence,
            "character-select route occurrence {} readiness disagrees with route evidence",
            occurrence.occurrence_id
        );
        if occurrence.producer_binding_status == CharacterSelectProducerBindingStatus::Bound {
            ensure!(
                !occurrence.producer_targets.is_empty() && !occurrence.consumer_targets.is_empty(),
                "producer-bound character-select route occurrence {} lacks a producer or consumer target",
                occurrence.occurrence_id
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_second_pending_occurrence_prevents_a_logical_source_from_looking_complete() {
        let translated = BTreeSet::from(["shared"]);
        let bound = bound_occurrence(
            "bound-copy",
            "fixture",
            ["shared"],
            vec![producer_target(
                "texture.bin",
                Some(0),
                None,
                ["bound-region"],
            )],
            vec![consumer_target(
                "texture.bin",
                "overlay.bin",
                CharacterSelectConsumerReferenceKind::PrimitiveSetup,
                [4],
                ["first-screen"],
            )],
        );
        let pending = pending_occurrence(
            "pending-copy",
            "fixture",
            ["shared"],
            vec![producer_target(
                "texture.bin",
                Some(32),
                None,
                ["pending-region"],
            )],
            vec![consumer_target(
                "texture.bin",
                "overlay.bin",
                CharacterSelectConsumerReferenceKind::PrimitiveSetup,
                [8],
                ["second-screen"],
            )],
        );

        assert_eq!(
            pending.readiness,
            CharacterSelectRouteReadiness::ProducerAndConsumerKnown
        );

        let census = census_routes(&translated, vec![bound, pending]).unwrap();

        assert_eq!(census.route_occurrence_count, 2);
        assert_eq!(census.producer_bound_occurrence_count, 1);
        assert_eq!(census.producer_pending_occurrence_count, 1);
        assert_eq!(census.fully_routed_occurrence_count, 1);
        assert_eq!(census.unresolved_route_occurrence_count, 1);
        assert_eq!(census.partially_routed_source_ui_ids, ["shared"]);
        assert_eq!(census.source_ui_ids_with_multiple_occurrences, ["shared"]);
    }

    #[test]
    fn a_record_name_without_byte_offsets_is_not_a_complete_route() {
        let translated = BTreeSet::from(["shared"]);
        let record_only = bound_occurrence(
            "record-only-copy",
            "fixture",
            ["shared"],
            vec![producer_target(
                "texture.bin",
                Some(0),
                None,
                ["bound-region"],
            )],
            vec![consumer_target(
                "texture.bin",
                "overlay.bin",
                CharacterSelectConsumerReferenceKind::RecordIdentity,
                [],
                ["screen"],
            )],
        );

        let census = census_routes(&translated, vec![record_only]).unwrap();

        assert_eq!(census.producer_bound_occurrence_count, 1);
        assert_eq!(census.record_only_consumer_location_occurrence_count, 1);
        assert_eq!(census.fully_routed_occurrence_count, 0);
        assert_eq!(census.unresolved_route_occurrence_count, 1);
        assert_eq!(census.unresolved_only_source_ui_ids, ["shared"]);
    }
}
