use anyhow::Result;

use crate::decoded_record_write_plan::DecodedDataClaim;

use super::PairedDecodedRecordPlan;

#[test]
fn disjoint_writers_compose_independently_of_registration_order() -> Result<()> {
    let forward = composed_fixture(false)?;
    let reverse = composed_fixture(true)?;

    assert_eq!(forward, reverse);
    assert_eq!(forward.0, vec![5, 1, 0, 2]);
    assert_eq!(forward.1, vec![3, 0, 4, 0]);
    Ok(())
}

#[test]
fn overlapping_candidates_are_rejected_when_the_pair_is_composed() {
    let mut plan = fixture_with_fixed_candidate();
    plan.register_writer(
        "overlapping writer",
        |first| {
            first[1] = 2;
            Ok(vec![[1, 2]])
        },
        |second| {
            second[0] = 3;
            Ok(vec![[0, 1]])
        },
    )
    .unwrap();

    let error = plan.compose().unwrap_err();
    assert!(error.to_string().contains("claims overlap"));
}

#[test]
fn failed_second_candidate_does_not_partially_register_the_writer() {
    let mut plan = fixture_with_fixed_candidate();
    let error = plan
        .register_writer(
            "invalid writer",
            |first| {
                first[3] = 2;
                Ok(vec![[3, 4]])
            },
            |second| {
                second[0] = 3;
                Ok(vec![[1, 2]])
            },
        )
        .unwrap_err();
    assert!(error.to_string().contains("outside its claimed ranges"));

    plan.register_writer(
        "valid writer",
        |first| {
            first[3] = 2;
            Ok(vec![[3, 4]])
        },
        |second| {
            second[0] = 3;
            Ok(vec![[0, 1]])
        },
    )
    .unwrap();
    let records = plan.compose().unwrap();
    assert_eq!(records.first, vec![0, 1, 0, 2]);
    assert_eq!(records.second, vec![3, 0, 0, 0]);
}

fn composed_fixture(reverse: bool) -> Result<(Vec<u8>, Vec<u8>)> {
    let mut plan = fixture_with_fixed_candidate();
    let register_first = |plan: &mut PairedDecodedRecordPlan| {
        plan.register_writer(
            "first writer",
            |first| {
                first[3] = 2;
                Ok(vec![[3, 4]])
            },
            |second| {
                second[0] = 3;
                Ok(vec![[0, 1]])
            },
        )
    };
    let register_second = |plan: &mut PairedDecodedRecordPlan| {
        plan.register_writer(
            "second writer",
            |first| {
                first[0] = 5;
                Ok(vec![[0, 1]])
            },
            |second| {
                second[2] = 4;
                Ok(vec![[2, 3]])
            },
        )
    };
    if reverse {
        register_second(&mut plan)?;
        register_first(&mut plan)?;
    } else {
        register_first(&mut plan)?;
        register_second(&mut plan)?;
    }
    let records = plan.compose()?;
    Ok((records.first, records.second))
}

fn fixture_with_fixed_candidate() -> PairedDecodedRecordPlan {
    let source = vec![0; 4];
    let mut fixed = source.clone();
    fixed[1] = 1;
    let mut plan = PairedDecodedRecordPlan::new("first", source, "second", vec![0; 4]).unwrap();
    plan.register_first_candidate(
        "fixed UI",
        fixed,
        vec![DecodedDataClaim {
            id: "fixed-ui".to_string(),
            purpose: "render fixed UI".to_string(),
            range: 1..2,
        }],
    )
    .unwrap();
    plan
}

#[test]
fn overlay_only_candidate_composes_without_claiming_texture_bytes() {
    let mut plan = fixture_with_fixed_candidate();
    plan.register_second_candidate(
        "geometry",
        vec![0, 7, 0, 0],
        DecodedDataClaim::from_ranges("geometry", "sample texture", vec![[1, 2]]),
    )
    .unwrap();
    plan.register_writer(
        "text",
        |first| {
            first[3] = 2;
            Ok(vec![[3, 4]])
        },
        |second| {
            second[0] = 3;
            Ok(vec![[0, 1]])
        },
    )
    .unwrap();
    let result = plan.compose().unwrap();
    assert_eq!(result.first, vec![0, 1, 0, 2]);
    assert_eq!(result.second, vec![3, 7, 0, 0]);
}

#[test]
fn overlay_only_candidate_still_rejects_overlapping_paired_writes() {
    let mut plan = fixture_with_fixed_candidate();
    plan.register_second_candidate(
        "geometry",
        vec![7, 0, 0, 0],
        DecodedDataClaim::from_ranges("geometry", "sample texture", vec![[0, 1]]),
    )
    .unwrap();
    plan.register_writer(
        "text",
        |first| {
            first[3] = 2;
            Ok(vec![[3, 4]])
        },
        |second| {
            second[0] = 3;
            Ok(vec![[0, 1]])
        },
    )
    .unwrap();
    assert!(
        plan.compose()
            .unwrap_err()
            .to_string()
            .contains("claims overlap")
    );
}
