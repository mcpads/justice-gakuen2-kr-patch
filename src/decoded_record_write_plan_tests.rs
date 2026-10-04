use expected_write::WriteIntent;

use super::{CandidateRecordWrite, CandidateWriteClaim, DecodedDataClaim, DecodedRecordWritePlan};
use crate::pipeline::sha256_bytes;

fn claim(id: &str, range: std::ops::Range<usize>) -> CandidateWriteClaim {
    CandidateWriteClaim {
        id: id.to_string(),
        purpose: format!("apply {id}"),
        range,
        intent: WriteIntent::Data,
    }
}

fn compose_in_order(reverse: bool) -> anyhow::Result<Vec<u8>> {
    let source = [0x10, 0x20, 0x30, 0x40, 0x50, 0x60];
    let mut left = source;
    left[1] = 0xa1;
    let mut right = source;
    right[4] = 0xb4;
    let source_sha256 = sha256_bytes(&source);
    let candidates = [
        CandidateRecordWrite {
            owner: "left feature",
            source_sha256: &source_sha256,
            candidate: &left,
            claims: vec![claim("left", 1..2)],
        },
        CandidateRecordWrite {
            owner: "right feature",
            source_sha256: &source_sha256,
            candidate: &right,
            claims: vec![claim("right", 4..5)],
        },
    ];
    let mut plan = DecodedRecordWritePlan::new("fixture", &source, &source_sha256)?;
    if reverse {
        for candidate in candidates.into_iter().rev() {
            plan.register_candidate(candidate)?;
        }
    } else {
        for candidate in candidates {
            plan.register_candidate(candidate)?;
        }
    }
    plan.apply(None)
}

#[test]
fn independent_candidates_compose_without_registration_order() {
    let expected = vec![0x10, 0xa1, 0x30, 0x40, 0xb4, 0x60];
    assert_eq!(compose_in_order(false).unwrap(), expected);
    assert_eq!(compose_in_order(true).unwrap(), expected);
}

#[test]
fn effective_claims_merge_owned_ranges_and_drop_unchanged_ones() {
    let source = [0u8; 8];
    let mut candidate = source;
    candidate[1] = 1;
    let claims = DecodedDataClaim::from_effective_ranges(
        "fixture",
        "change the fixture",
        &source,
        &candidate,
        [[0, 1], [1, 2], [5, 7]],
    )
    .unwrap();

    assert_eq!(claims.len(), 1);
    assert_eq!(claims[0].range, 0..2);
}

#[test]
fn candidate_diff_outside_its_claim_is_rejected() {
    let source = [0u8; 8];
    let mut candidate = source;
    candidate[2] = 1;
    candidate[6] = 1;
    let source_sha256 = sha256_bytes(&source);
    let mut plan = DecodedRecordWritePlan::new("fixture", &source, &source_sha256).unwrap();

    assert!(
        plan.register_candidate(CandidateRecordWrite {
            owner: "partial claimant",
            source_sha256: &source_sha256,
            candidate: &candidate,
            claims: vec![claim("claimed", 2..3)],
        })
        .is_err()
    );
}

#[test]
fn candidate_from_a_different_source_identity_is_rejected() {
    let source = [0u8; 8];
    let mut candidate = source;
    candidate[3] = 1;
    let source_sha256 = sha256_bytes(&source);
    let other_source_sha256 = sha256_bytes(&[1u8; 8]);
    let mut plan = DecodedRecordWritePlan::new("fixture", &source, &source_sha256).unwrap();

    assert!(
        plan.register_candidate(CandidateRecordWrite {
            owner: "foreign-source feature",
            source_sha256: &other_source_sha256,
            candidate: &candidate,
            claims: vec![claim("foreign-source", 3..4)],
        })
        .is_err()
    );
    assert_eq!(plan.apply(None).unwrap(), source);
}

#[test]
fn failed_multi_claim_registration_leaves_the_prior_plan_unchanged() {
    let source = [0u8; 8];
    let mut first = source;
    first[2] = 1;
    let mut second = source;
    second[2] = 2;
    second[6] = 2;
    let source_sha256 = sha256_bytes(&source);
    let mut plan = DecodedRecordWritePlan::new("fixture", &source, &source_sha256).unwrap();
    plan.register_candidate(CandidateRecordWrite {
        owner: "first feature",
        source_sha256: &source_sha256,
        candidate: &first,
        claims: vec![claim("first", 2..3)],
    })
    .unwrap();
    assert!(
        plan.register_candidate(CandidateRecordWrite {
            owner: "second feature",
            source_sha256: &source_sha256,
            candidate: &second,
            claims: vec![
                claim("second-disjoint", 6..7),
                claim("second-overlap", 2..3)
            ],
        })
        .is_err()
    );
    assert_eq!(plan.apply(None).unwrap(), first);
}

#[test]
fn duplicate_claim_identity_is_rejected_before_composition() {
    let source = [0u8; 8];
    let mut candidate = source;
    candidate[1] = 1;
    candidate[5] = 1;
    let source_sha256 = sha256_bytes(&source);
    let mut plan = DecodedRecordWritePlan::new("fixture", &source, &source_sha256).unwrap();

    assert!(
        plan.register_candidate(CandidateRecordWrite {
            owner: "duplicate identity feature",
            source_sha256: &source_sha256,
            candidate: &candidate,
            claims: vec![claim("duplicate", 1..2), claim("duplicate", 5..6)],
        })
        .is_err()
    );
    assert_eq!(plan.apply(None).unwrap(), source);
}
