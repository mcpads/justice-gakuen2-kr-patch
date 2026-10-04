//! Declared route occurrences that the production pipeline has not bound yet.

use std::collections::BTreeSet;

use crate::character_select_graphics::model::{
    CharacterSelectConsumerReferenceKind, CharacterSelectProducerBindingStatus,
    CharacterSelectRouteOccurrence, CharacterSelectRouteOwner, CharacterSelectTextureSurface,
};
use crate::character_select_graphics::resource_loads::resource_load_byte_offsets;

use super::{consumer_location_status, consumer_target, producer_target};

struct PendingOccurrenceSpec {
    occurrence_id: &'static str,
    producer_family: &'static str,
    producer_evidence: PendingProducerEvidence,
    owner: CharacterSelectRouteOwner,
    source_ui_id: &'static str,
    producer_records: &'static [(&'static str, Option<usize>)],
    surface: Option<CharacterSelectTextureSurface>,
    physical_region_id: &'static str,
    consumer_records: &'static [&'static str],
    runtime_state: &'static str,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PendingProducerEvidence {
    Known,
    Candidate,
}

const SHARED_ATLAS_OFFSET: usize = 0x17800;
const BATTLE_READY_SELP: &[(&str, Option<usize>)] = &[
    ("DAT2/SELP3.BIZ", Some(SHARED_ATLAS_OFFSET)),
    ("DAT2/SELP4.BIZ", Some(SHARED_ATLAS_OFFSET)),
    ("DAT2/SELP5.BIZ", Some(SHARED_ATLAS_OFFSET)),
];

const PENDING_OCCURRENCES: &[PendingOccurrenceSpec] = &[
    spec(
        "plsel3-battle-ready-label",
        "selp3_ready_label",
        PendingProducerEvidence::Known,
        CharacterSelectRouteOwner::CharacterSelectGraphics,
        "battle_ready_label",
        &[("DAT2/SELP3.BIZ", Some(SHARED_ATLAS_OFFSET))],
        consumer(
            Some(CharacterSelectTextureSurface::BattleReadyAtlas),
            "ready-label-source-region",
            &["DAT1/PLSEL3.BIN"],
            "battle-ready",
        ),
    ),
    spec(
        "plsel4-battle-ready-label",
        "selp4_ready_label",
        PendingProducerEvidence::Known,
        CharacterSelectRouteOwner::CharacterSelectGraphics,
        "battle_ready_label",
        &[("DAT2/SELP4.BIZ", Some(SHARED_ATLAS_OFFSET))],
        consumer(
            Some(CharacterSelectTextureSurface::BattleReadyAtlas),
            "ready-label-source-region",
            &["DAT1/PLSEL4.BIN"],
            "battle-ready",
        ),
    ),
    spec(
        "plsel5-battle-ready-label",
        "selp5_ready_label",
        PendingProducerEvidence::Known,
        CharacterSelectRouteOwner::CharacterSelectGraphics,
        "battle_ready_label",
        &[("DAT2/SELP5.BIZ", Some(SHARED_ATLAS_OFFSET))],
        consumer(
            Some(CharacterSelectTextureSurface::BattleReadyAtlas),
            "ready-label-source-region",
            &["DAT1/PLSEL5.BIN"],
            "battle-ready",
        ),
    ),
    spec(
        "selp-shared-ready-controller-instruction",
        "selp_shared_ready_screen",
        PendingProducerEvidence::Known,
        CharacterSelectRouteOwner::CharacterSelectGraphics,
        "battle_ready_controller_instruction",
        BATTLE_READY_SELP,
        consumer(
            Some(CharacterSelectTextureSurface::BattleReadyAtlas),
            "ready-controller-instruction-source-region",
            &[],
            "battle-ready",
        ),
    ),
    spec(
        "selp3-league-win-label",
        "selp3_league_standings",
        PendingProducerEvidence::Known,
        CharacterSelectRouteOwner::CharacterSelectGraphics,
        "league_win_label",
        &[("DAT2/SELP3.BIZ", Some(SHARED_ATLAS_OFFSET))],
        consumer(
            Some(CharacterSelectTextureSurface::LeagueStandingsAtlas),
            "league-win-label-source-region",
            &["DAT1/PLSEL3.BIN"],
            "league-standings",
        ),
    ),
    spec(
        "selp3-league-loss-label",
        "selp3_league_standings",
        PendingProducerEvidence::Known,
        CharacterSelectRouteOwner::CharacterSelectGraphics,
        "league_loss_label",
        &[("DAT2/SELP3.BIZ", Some(SHARED_ATLAS_OFFSET))],
        consumer(
            Some(CharacterSelectTextureSurface::LeagueStandingsAtlas),
            "league-loss-label-source-region",
            &["DAT1/PLSEL3.BIN"],
            "league-standings",
        ),
    ),
    spec(
        "plsel1-practical-1999-exam",
        "selp1_practical_selection",
        PendingProducerEvidence::Known,
        CharacterSelectRouteOwner::CharacterSelectGraphics,
        "practical_1999_exam",
        &[("DAT2/SELP1.BIZ", Some(SHARED_ATLAS_OFFSET))],
        consumer(
            Some(CharacterSelectTextureSurface::PracticalSelectionAtlas),
            "practical-1999-exam-source-region",
            &["DAT1/PLSEL1.BIN"],
            "top-level-state-3/substate-2/practical-selection",
        ),
    ),
    spec(
        "plsel1-practical-basics-review",
        "selp1_practical_selection",
        PendingProducerEvidence::Known,
        CharacterSelectRouteOwner::CharacterSelectGraphics,
        "practical_basics_review",
        &[("DAT2/SELP1.BIZ", Some(SHARED_ATLAS_OFFSET))],
        consumer(
            Some(CharacterSelectTextureSurface::PracticalSelectionAtlas),
            "practical-basics-review-source-region",
            &["DAT1/PLSEL1.BIN"],
            "top-level-state-3/substate-2/practical-selection",
        ),
    ),
    spec(
        "selp1-solo-challenge-prompt",
        "over_solo_state_prompts",
        PendingProducerEvidence::Known,
        CharacterSelectRouteOwner::CharacterSelectGraphics,
        "solo_challenge_prompt",
        &[("DAT2/OVER.TIZ", Some(0))],
        consumer(
            Some(CharacterSelectTextureSurface::SoloStatePromptAtlas),
            "solo-challenge-source-region",
            &["SLPS_021.20"],
            "solo-select/prompt-state-6/challenge",
        ),
    ),
    spec(
        "selp1-solo-press-start-prompt",
        "over_solo_state_prompts",
        PendingProducerEvidence::Known,
        CharacterSelectRouteOwner::CharacterSelectGraphics,
        "solo_press_start_prompt",
        &[("DAT2/OVER.TIZ", Some(0))],
        consumer(
            Some(CharacterSelectTextureSurface::SoloStatePromptAtlas),
            "solo-press-start-source-region",
            &["SLPS_021.20"],
            "solo-select/prompt-state-5-or-1/press-start",
        ),
    ),
    spec(
        "selp1-solo-wait-prompt",
        "over_solo_state_prompts",
        PendingProducerEvidence::Known,
        CharacterSelectRouteOwner::CharacterSelectGraphics,
        "solo_wait_prompt",
        &[("DAT2/OVER.TIZ", Some(0))],
        consumer(
            Some(CharacterSelectTextureSurface::SoloStatePromptAtlas),
            "solo-wait-source-region",
            &["SLPS_021.20"],
            "solo-select/prompt-state-8/wait",
        ),
    ),
    spec(
        "selp1-versus-handicap-label",
        "selp1_versus_handicap",
        PendingProducerEvidence::Known,
        CharacterSelectRouteOwner::CharacterSelectGraphics,
        "versus_handicap_label",
        &[("DAT2/SELP1.BIZ", Some(SHARED_ATLAS_OFFSET))],
        consumer(
            Some(CharacterSelectTextureSurface::VersusHandicapAtlas),
            "versus-handicap-source-region",
            &["DAT1/PLSEL1.BIN"],
            "versus-handicap",
        ),
    ),
    spec(
        "plsel4-completed-bracket-champion-label",
        "selp4_completed_tournament_bracket",
        PendingProducerEvidence::Known,
        CharacterSelectRouteOwner::CharacterSelectGraphics,
        "tournament_champion_label",
        &[("DAT2/SELP4.BIZ", Some(SHARED_ATLAS_OFFSET))],
        consumer(
            Some(CharacterSelectTextureSurface::SharedFixedStripAtlas),
            "completed-bracket-champion-source-cells",
            &["DAT1/PLSEL4.BIN"],
            "top-level-state-1/inner-state-8/completed-bracket",
        ),
    ),
];

const fn spec(
    occurrence_id: &'static str,
    producer_family: &'static str,
    producer_evidence: PendingProducerEvidence,
    owner: CharacterSelectRouteOwner,
    source_ui_id: &'static str,
    producer_records: &'static [(&'static str, Option<usize>)],
    consumer: PendingConsumerSpec,
) -> PendingOccurrenceSpec {
    PendingOccurrenceSpec {
        occurrence_id,
        producer_family,
        producer_evidence,
        owner,
        source_ui_id,
        producer_records,
        surface: consumer.surface,
        physical_region_id: consumer.physical_region_id,
        consumer_records: consumer.records,
        runtime_state: consumer.runtime_state,
    }
}

#[derive(Clone, Copy)]
struct PendingConsumerSpec {
    surface: Option<CharacterSelectTextureSurface>,
    physical_region_id: &'static str,
    records: &'static [&'static str],
    runtime_state: &'static str,
}

const fn consumer(
    surface: Option<CharacterSelectTextureSurface>,
    physical_region_id: &'static str,
    records: &'static [&'static str],
    runtime_state: &'static str,
) -> PendingConsumerSpec {
    PendingConsumerSpec {
        surface,
        physical_region_id,
        records,
        runtime_state,
    }
}

pub(super) fn declared_pending_occurrences(
    translated_source_ui_ids: &BTreeSet<&str>,
) -> Vec<CharacterSelectRouteOccurrence> {
    PENDING_OCCURRENCES
        .iter()
        .filter(|spec| translated_source_ui_ids.contains(spec.source_ui_id))
        .map(|spec| {
            let consumer_targets = spec
                .consumer_records
                .iter()
                .map(|record| {
                    let source_record = spec
                        .producer_records
                        .first()
                        .expect("a pending consumer has a declared producer")
                        .0;
                    let (reference_kind, byte_offsets) =
                        pending_consumer_reference(source_record, record);
                    consumer_target(
                        source_record,
                        *record,
                        reference_kind,
                        byte_offsets.iter().copied(),
                        [spec.runtime_state],
                    )
                })
                .collect::<Vec<_>>();
            let consumer_location_status = consumer_location_status(&consumer_targets);
            CharacterSelectRouteOccurrence {
                occurrence_id: spec.occurrence_id.to_string(),
                producer_family: spec.producer_family.to_string(),
                producer_binding_status: CharacterSelectProducerBindingStatus::Pending,
                consumer_location_status,
                readiness: super::pending_readiness(
                    spec.producer_evidence == PendingProducerEvidence::Candidate,
                    consumer_location_status,
                ),
                owner: spec.owner,
                source_ui_ids: vec![spec.source_ui_id.to_string()],
                producer_targets: spec
                    .producer_records
                    .iter()
                    .map(|(record, tim_offset)| {
                        producer_target(
                            *record,
                            *tim_offset,
                            spec.surface,
                            [spec.physical_region_id],
                        )
                    })
                    .collect(),
                consumer_targets,
            }
        })
        .collect()
}

fn pending_consumer_reference(
    source_record: &str,
    consumer_record: &str,
) -> (CharacterSelectConsumerReferenceKind, &'static [usize]) {
    resource_load_byte_offsets(source_record, consumer_record).map_or(
        (CharacterSelectConsumerReferenceKind::RecordIdentity, &[]),
        |byte_offsets| {
            (
                CharacterSelectConsumerReferenceKind::ResourceLoad,
                byte_offsets,
            )
        },
    )
}
