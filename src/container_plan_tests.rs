use super::{ContainerMemberSpec, ContainerPlan, MemberContribution};
use crate::pipeline::sha256_bytes;

fn member(
    id: &str,
    writable_range: std::ops::Range<usize>,
    slot_range: std::ops::Range<usize>,
    source: &[u8],
) -> ContainerMemberSpec {
    ContainerMemberSpec {
        id: id.to_string(),
        expected_source_sha256: sha256_bytes(&source[writable_range.clone()]),
        writable_range,
        slot_range,
        slot_start_alignment: 8,
    }
}

fn fixture_plan(source: &[u8]) -> ContainerPlan<'_> {
    let specs = vec![
        member("first", 8..12, 8..16, source),
        member("second", 16..20, 16..24, source),
        member("unchanged", 24..28, 24..32, source),
    ];
    ContainerPlan::new(
        "fixture.bin",
        "fixture compositor",
        "emit one fixture record",
        source,
        &sha256_bytes(source),
        specs,
    )
    .unwrap()
}

#[test]
fn seals_member_diffs_once_and_preserves_container_structure() {
    let source = (0u8..32).collect::<Vec<_>>();
    let mut first = source[8..12].to_vec();
    first[0] = 0xa0;
    first[3] = 0xa3;
    let mut second = source[16..20].to_vec();
    second[1] = 0xb1;
    let mut plan = fixture_plan(&source);
    let first_source_sha256 = sha256_bytes(&source[8..12]);
    plan.register_member(MemberContribution {
        member_id: "first",
        owner: "first producer",
        purpose: "replace first member",
        consumed_source_sha256: &first_source_sha256,
        candidate: &first,
        lineage_id: "first stored result",
    })
    .unwrap();
    let second_source_sha256 = sha256_bytes(&source[16..20]);
    plan.register_member(MemberContribution {
        member_id: "second",
        owner: "second producer",
        purpose: "replace second member",
        consumed_source_sha256: &second_source_sha256,
        candidate: &second,
        lineage_id: "second stored result",
    })
    .unwrap();

    let output = plan.seal().unwrap().unwrap();
    assert_eq!(&output.data[8..12], first);
    assert_eq!(&output.data[16..20], second);
    assert_eq!(&output.data[..8], &source[..8]);
    assert_eq!(&output.data[12..16], &source[12..16]);
    assert_eq!(&output.data[20..], &source[20..]);
    assert_eq!(output.report.changed_byte_count, 3);
    let unchanged = &output.report.members[2];
    assert_eq!(unchanged.owner, None);
    assert_eq!(unchanged.lineage_id, None);
    assert_eq!(unchanged.source_sha256, unchanged.candidate_sha256);
    assert_eq!(unchanged.changed_byte_count, 0);
}

#[test]
fn rejects_invalid_member_geometry_or_source_identity_before_registration() {
    let source = vec![0u8; 32];
    let source_sha256 = sha256_bytes(&source);
    let overlapping_slots = vec![
        member("left", 8..12, 8..20, &source),
        member("right", 16..20, 16..24, &source),
    ];
    assert!(
        ContainerPlan::new(
            "fixture.bin",
            "compositor",
            "compose fixture",
            &source,
            &source_sha256,
            overlapping_slots,
        )
        .is_err()
    );

    let mut wrong_hash = member("member", 8..12, 8..16, &source);
    wrong_hash.expected_source_sha256 = "00".repeat(32);
    assert!(
        ContainerPlan::new(
            "fixture.bin",
            "compositor",
            "compose fixture",
            &source,
            &source_sha256,
            vec![wrong_hash],
        )
        .is_err()
    );
}

#[test]
fn rejects_wrong_source_extent_noop_and_duplicate_member_contributions() {
    let source = (0u8..32).collect::<Vec<_>>();
    let source_member_sha256 = sha256_bytes(&source[8..12]);

    let mut wrong_source_plan = fixture_plan(&source);
    let wrong_source_sha256 = "00".repeat(32);
    assert!(
        wrong_source_plan
            .register_member(MemberContribution {
                member_id: "first",
                owner: "producer",
                purpose: "replace first member",
                consumed_source_sha256: &wrong_source_sha256,
                candidate: &[1, 2, 3, 4],
                lineage_id: "stored result",
            })
            .is_err()
    );

    let mut wrong_extent_plan = fixture_plan(&source);
    assert!(
        wrong_extent_plan
            .register_member(MemberContribution {
                member_id: "first",
                owner: "producer",
                purpose: "replace first member",
                consumed_source_sha256: &source_member_sha256,
                candidate: &[1, 2, 3],
                lineage_id: "stored result",
            })
            .is_err()
    );

    let mut noop_plan = fixture_plan(&source);
    assert!(
        noop_plan
            .register_member(MemberContribution {
                member_id: "first",
                owner: "producer",
                purpose: "replace first member",
                consumed_source_sha256: &source_member_sha256,
                candidate: &source[8..12],
                lineage_id: "stored result",
            })
            .is_err()
    );

    let mut duplicate_plan = fixture_plan(&source);
    let first_candidate = [1, 2, 3, 4];
    duplicate_plan
        .register_member(MemberContribution {
            member_id: "first",
            owner: "producer",
            purpose: "replace first member",
            consumed_source_sha256: &source_member_sha256,
            candidate: &first_candidate,
            lineage_id: "stored result",
        })
        .unwrap();
    assert!(
        duplicate_plan
            .register_member(MemberContribution {
                member_id: "first",
                owner: "other producer",
                purpose: "replace first member again",
                consumed_source_sha256: &source_member_sha256,
                candidate: &[4, 3, 2, 1],
                lineage_id: "other stored result",
            })
            .is_err()
    );
}

#[test]
fn emits_no_outer_record_when_no_member_contributes_a_change() {
    let source = (0u8..32).collect::<Vec<_>>();
    assert!(fixture_plan(&source).seal().unwrap().is_none());
}
