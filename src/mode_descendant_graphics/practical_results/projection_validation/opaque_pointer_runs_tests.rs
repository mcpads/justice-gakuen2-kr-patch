use super::validate_adopted_consumer_exclusion;
use crate::mode_descendant_graphics::practical_results::opaque_pointer_run_model::{
    PracticalResultOpaquePointerRunConsumerExclusionConclusion,
    PracticalResultOpaquePointerRunConsumerExclusionEvidence,
    PracticalResultOpaquePointerRunEntrypointEvidence,
};
use crate::pipeline::{BASELINE_BIN_SHA256, sha256_bytes};

#[test]
fn adopted_exclusion_requires_zero_references_and_source_bound_entrypoints() {
    let mut overlay = vec![0; 0x20];
    overlay[0..4].copy_from_slice(&0x800a_2010_u32.to_le_bytes());
    let overlay_sha256 = sha256_bytes(&overlay);
    let mut evidence = evidence(&overlay_sha256);

    validate_adopted_consumer_exclusion(
        "synthetic_pointer_run",
        2,
        &overlay_sha256,
        &evidence,
        &overlay,
    )
    .unwrap();

    // The locator is historical metadata; adoption is bound by the hashes and facts.
    evidence.analysis_report_path = "/restored/evidence/static-consumers.json".into();
    validate_adopted_consumer_exclusion(
        "synthetic_pointer_run",
        2,
        &overlay_sha256,
        &evidence,
        &overlay,
    )
    .unwrap();

    evidence.reachable_target_arena_reference_count = 1;
    let reference_error = validate_adopted_consumer_exclusion(
        "synthetic_pointer_run",
        2,
        &overlay_sha256,
        &evidence,
        &overlay,
    )
    .expect_err("a reachable arena reference must invalidate exclusion");
    assert!(
        reference_error
            .to_string()
            .contains("adopted exclusion evidence is incomplete")
    );

    evidence.reachable_target_arena_reference_count = 0;
    evidence.declared_entrypoints[0].runtime_address = "0x800a2014".to_string();
    let entrypoint_error = validate_adopted_consumer_exclusion(
        "synthetic_pointer_run",
        2,
        &overlay_sha256,
        &evidence,
        &overlay,
    )
    .expect_err("an entrypoint detached from its source word must invalidate exclusion");
    assert!(
        entrypoint_error
            .to_string()
            .contains("adopted entrypoint changed")
    );
}

fn evidence(overlay_sha256: &str) -> PracticalResultOpaquePointerRunConsumerExclusionEvidence {
    PracticalResultOpaquePointerRunConsumerExclusionEvidence {
        analysis_report_path: "work/static-analysis/static-consumers.json".to_string(),
        analysis_report_sha256: "a".repeat(64),
        source_bin_sha256: BASELINE_BIN_SHA256.to_string(),
        loaded_image_sha256: overlay_sha256.to_string(),
        declared_entrypoints: vec![PracticalResultOpaquePointerRunEntrypointEvidence {
            source_reference_offset: "0x0".to_string(),
            runtime_address: "0x800a2010".to_string(),
        }],
        value_flow_state_budget: 262_144,
        decoded_pointer_count: 2,
        raw_address_reference_count: 2,
        external_raw_address_reference_count: 0,
        reachable_derived_reference_count: 0,
        reachable_pointer_table_reference_count: 0,
        reachable_target_arena_reference_count: 0,
        profiled_non_address_exhausted_seed_ids: vec![
            "synthetic_sprite_coordinate_record_loop".to_string(),
        ],
        unprofiled_exhausted_seed_count: 0,
        unresolved_indirect_jump_count: 0,
        analysis_product_build_input: false,
        conclusion: PracticalResultOpaquePointerRunConsumerExclusionConclusion::NoReachablePointerRunReferenceInReviewedDeclaredEntrypointFlow,
    }
}
