use psx_r3000a::{Instruction, Register, encode};

use std::collections::BTreeSet;

use super::super::catalog::{
    DiscRecordLoaderBoundedIndexProfile, DiscRecordLoaderForwarderProfile,
    DiscRecordLoaderOwnerCallProfile, DiscRecordLoaderOwnerProfile, DiscRecordLoaderProfiles,
    SourceCatalog, disc_record_load_call_candidates,
    disc_record_load_call_candidates_with_profiles,
};
use super::{
    analyze_loaded_image, declared_sink_audit, declared_source_region_audits,
    validate_consumer_profiles,
};
use crate::consumer_analysis::profiles::{
    MAIN_DISC_RECORD_LOADER_ADDRESS, StaticConsumerEdgeProfile, StaticConsumerSinkDisposition,
    StaticConsumerSinkProfile, StaticConsumerSourceRegionProfile,
};
use crate::source_disc::{LoadedImage, LoadedImageEntrypoint};

const RUNTIME_BASE: u32 = 0x800a_2000;

#[test]
fn loaded_image_analysis_uses_only_entrypoint_reachable_instructions() {
    let data = encode_instructions(&[
        Instruction::Lui {
            rt: Register::S0,
            immediate: 0x800a,
        },
        Instruction::Addiu {
            rt: Register::S0,
            rs: Register::S0,
            immediate: 0x2040,
        },
        Instruction::Jr { rs: Register::RA },
        Instruction::Sll {
            rd: Register::ZERO,
            rt: Register::ZERO,
            shift: 0,
        },
        Instruction::Lui {
            rt: Register::S1,
            immediate: 0x800a,
        },
        Instruction::Addiu {
            rt: Register::S1,
            rs: Register::S1,
            immediate: 0x2060,
        },
    ]);
    let image = LoadedImage {
        path: "DAT1/SYNTH.BIN".to_string(),
        data,
        runtime_base: Some(RUNTIME_BASE),
        entrypoints: vec![entrypoint("primary", 0, RUNTIME_BASE)],
    };

    let report = analyze_loaded_image(
        &image,
        64,
        64,
        &SourceCatalog::fixture(60, "DAT2/SYNTH.BIZ"),
    )
    .unwrap();

    assert_eq!(report.analysis_status, "analyzed");
    assert_eq!(report.reachable_instruction_count, 4);
    assert_eq!(report.value_flow_seed_count, 1);
    assert_eq!(report.executable_ranges.len(), 1);
    assert_eq!(report.executable_ranges[0].end_offset, "0x10");
}

#[test]
fn loaded_image_analysis_unions_explicit_entrypoint_roots() {
    let data = encode_instructions(&[
        Instruction::Jr { rs: Register::RA },
        Instruction::nop(),
        Instruction::Lui {
            rt: Register::S0,
            immediate: 0x800a,
        },
        Instruction::Addiu {
            rt: Register::S0,
            rs: Register::S0,
            immediate: 0x2040,
        },
        Instruction::Jr { rs: Register::RA },
        Instruction::nop(),
    ]);
    let image = LoadedImage {
        path: "DAT1/SYNTH.BIN".to_string(),
        data,
        runtime_base: Some(RUNTIME_BASE),
        entrypoints: vec![
            entrypoint("primary", 0, RUNTIME_BASE),
            entrypoint("callback", 4, RUNTIME_BASE + 8),
        ],
    };

    let report = analyze_loaded_image(
        &image,
        64,
        64,
        &SourceCatalog::fixture(60, "DAT2/SYNTH.BIZ"),
    )
    .unwrap();

    assert_eq!(report.declared_entrypoints.len(), 2);
    assert_eq!(report.reachable_instruction_count, 6);
    assert_eq!(report.value_flow_seed_count, 1);
}

#[test]
fn entrypoint_unresolved_image_keeps_full_image_loader_candidates() {
    let image = LoadedImage {
        path: "DAT1/SYNTH.BIN".to_string(),
        data: encode_instructions(&[
            Instruction::Jal {
                target: MAIN_DISC_RECORD_LOADER_ADDRESS,
            },
            Instruction::Addiu {
                rt: Register::A1,
                rs: Register::ZERO,
                immediate: 60,
            },
        ]),
        runtime_base: Some(RUNTIME_BASE),
        entrypoints: Vec::new(),
    };

    let report = analyze_loaded_image(
        &image,
        64,
        64,
        &SourceCatalog::fixture(60, "DAT2/SYNTH.BIZ"),
    )
    .unwrap();

    assert_eq!(report.analysis_status, "entrypoint_unresolved");
    assert_eq!(report.disc_record_load_call_candidates.len(), 1);
    assert!(!report.disc_record_load_call_candidates[0].entrypoint_reachable);
    assert_eq!(
        report.disc_record_load_call_candidates[0].catalog_record_paths,
        ["DAT2/SYNTH.BIZ"]
    );
}

#[test]
fn consumer_profile_validation_accepts_relationships_with_distinct_members() {
    let regions = [source_region(&[2, 7])];
    let sinks = [sink("renderer")];
    let edges = [edge("asset_to_renderer", &[2, 7], "renderer")];

    validate_consumer_profiles(&regions, &sinks, &edges).unwrap();
}

#[test]
fn source_region_census_keeps_unbound_assets_visible() {
    let profiles = [
        source_region(&[]),
        StaticConsumerSourceRegionProfile {
            source_record_path: "DAT2/UNBOUND.BIZ",
            source_region_id: "unbound_region",
            source_member_indices: &[],
            source_catalog_index: 61,
            unbound_consumer_reason: Some("consumer_route_not_yet_bound"),
        },
    ];
    let edges = [crate::consumer_analysis::model::StaticConsumerEdgeAudit {
        id: "asset_to_renderer".to_string(),
        source_record_path: "DAT2/SYNTH.BIZ".to_string(),
        source_region_id: "synthetic_region".to_string(),
        source_member_indices: vec![],
        relationship: "renders".to_string(),
        sink_id: "renderer".to_string(),
        consumer_image_path: "DAT1/SYNTH.BIN".to_string(),
        consumer_runtime_addresses: vec!["0x800a2000".to_string()],
        sink_entrypoint_reachable: true,
        evidence: "synthetic".to_string(),
        runtime_confirmation_required: true,
    }];

    let audits = declared_source_region_audits(&profiles, &edges, &[]);

    assert_eq!(audits.len(), 2);
    assert_eq!(
        audits[0].consumer_binding_status,
        "consumer_bound_all_sinks_entrypoint_reachable"
    );
    assert_eq!(audits[1].consumer_binding_status, "consumer_unbound");
    assert_eq!(audits[1].evidence, "consumer_route_not_yet_bound");
}

#[test]
fn dormant_candidates_remain_visible_without_blocking_an_active_source_edge() {
    let profiles = [source_region(&[])];
    let active_edge = crate::consumer_analysis::model::StaticConsumerEdgeAudit {
        id: "active_asset_to_renderer".to_string(),
        source_record_path: "DAT2/SYNTH.BIZ".to_string(),
        source_region_id: "synthetic_region".to_string(),
        source_member_indices: vec![],
        relationship: "renders".to_string(),
        sink_id: "active_renderer".to_string(),
        consumer_image_path: "DAT1/SYNTH.BIN".to_string(),
        consumer_runtime_addresses: vec!["0x800a2000".to_string()],
        sink_entrypoint_reachable: true,
        evidence: "synthetic active edge".to_string(),
        runtime_confirmation_required: true,
    };
    let dormant_edge = crate::consumer_analysis::model::StaticConsumerEdgeAudit {
        id: "asset_to_dormant_candidate".to_string(),
        sink_id: "dormant_candidate".to_string(),
        sink_entrypoint_reachable: false,
        evidence: "synthetic dormant candidate".to_string(),
        ..active_edge.clone()
    };

    let audits = declared_source_region_audits(&profiles, &[active_edge], &[dormant_edge]);
    assert_eq!(
        audits[0].consumer_binding_status,
        "consumer_bound_all_sinks_entrypoint_reachable"
    );
    assert_eq!(audits[0].declared_edge_ids, ["active_asset_to_renderer"]);
    assert_eq!(
        audits[0].dormant_candidate_edge_ids,
        ["asset_to_dormant_candidate"]
    );
}

#[test]
fn disc_load_candidates_separate_catalog_resolution_from_entrypoint_reachability() {
    let literal_data = encode_instructions(&[
        Instruction::Jal {
            target: MAIN_DISC_RECORD_LOADER_ADDRESS,
        },
        Instruction::Addiu {
            rt: Register::A1,
            rs: Register::ZERO,
            immediate: 61,
        },
    ]);
    let literal_image = LoadedImage {
        path: "DAT1/SYNTH.BIN".to_string(),
        data: literal_data,
        runtime_base: Some(RUNTIME_BASE),
        entrypoints: vec![],
    };
    let profile = StaticConsumerSourceRegionProfile {
        source_record_path: "DAT2/TARGET.BIZ",
        source_region_id: "target_region",
        source_member_indices: &[],
        source_catalog_index: 61,
        unbound_consumer_reason: None,
    };
    let catalog = SourceCatalog::fixture(61, "DAT2/TARGET.BIZ");

    let catalog_only = disc_record_load_call_candidates(
        &literal_image,
        &BTreeSet::from([0, 4]),
        &[],
        &catalog,
        &[],
    )
    .unwrap();
    assert_eq!(catalog_only[0].catalog_record_paths, ["DAT2/TARGET.BIZ"]);
    assert!(catalog_only[0].declared_source_region_paths.is_empty());

    let literal = disc_record_load_call_candidates(
        &literal_image,
        &BTreeSet::from([0, 4]),
        &[],
        &catalog,
        &[profile],
    )
    .unwrap();

    assert_eq!(literal.len(), 1);
    assert_eq!(literal[0].catalog_index_candidates, [61]);
    assert_eq!(literal[0].catalog_index_resolution, "delay_slot_literal");
    assert_eq!(
        literal[0].catalog_index_argument_producer,
        "delay_slot_literal"
    );
    assert_eq!(literal[0].catalog_record_paths, ["DAT2/TARGET.BIZ"]);
    assert_eq!(literal[0].declared_source_region_paths, ["DAT2/TARGET.BIZ"]);
    assert!(literal[0].entrypoint_reachable);

    let unresolved_data = encode_instructions(&[
        Instruction::Jal {
            target: MAIN_DISC_RECORD_LOADER_ADDRESS,
        },
        Instruction::Addu {
            rd: Register::A1,
            rs: Register::S0,
            rt: Register::S1,
        },
    ]);
    let unresolved_image = LoadedImage {
        data: unresolved_data,
        ..literal_image
    };
    let unresolved = disc_record_load_call_candidates(
        &unresolved_image,
        &BTreeSet::from([0, 4]),
        &[],
        &catalog,
        &[profile],
    )
    .unwrap();

    assert_eq!(unresolved.len(), 1);
    assert!(unresolved[0].catalog_index_candidates.is_empty());
    assert_eq!(unresolved[0].catalog_index_resolution, "unresolved");
    assert_eq!(
        unresolved[0].catalog_index_argument_producer,
        "delay_slot_computation"
    );
    assert!(unresolved[0].entrypoint_reachable);

    let unreachable = disc_record_load_call_candidates(
        &unresolved_image,
        &BTreeSet::new(),
        &[],
        &catalog,
        &[profile],
    )
    .unwrap();
    assert_eq!(unreachable.len(), 1);
    assert!(!unreachable[0].entrypoint_reachable);
}

#[test]
fn disc_load_candidate_resolves_an_immediately_preceding_literal() {
    let data = encode_instructions(&[
        Instruction::Addiu {
            rt: Register::A1,
            rs: Register::ZERO,
            immediate: 61,
        },
        Instruction::Jal {
            target: MAIN_DISC_RECORD_LOADER_ADDRESS,
        },
        Instruction::nop(),
    ]);
    let image = LoadedImage {
        path: "SYNTH.BIN".to_string(),
        data,
        runtime_base: Some(RUNTIME_BASE),
        entrypoints: vec![],
    };
    let catalog = SourceCatalog::fixture(61, "DAT2/TARGET.BIZ");

    let candidates =
        disc_record_load_call_candidates(&image, &BTreeSet::new(), &[], &catalog, &[]).unwrap();

    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].catalog_index_candidates, [61]);
    assert_eq!(candidates[0].catalog_index_resolution, "preceding_literal");
    assert_eq!(
        candidates[0].catalog_index_argument_producer,
        "preceding_literal"
    );
    assert_eq!(candidates[0].catalog_record_paths, ["DAT2/TARGET.BIZ"]);

    assert!(!candidates[0].entrypoint_reachable);
}

#[test]
fn disc_load_candidate_exposes_an_unresolved_preceding_memory_load() {
    let data = encode_instructions(&[
        Instruction::Lhu {
            rt: Register::A1,
            base: Register::S0,
            offset: 4,
        },
        Instruction::Jal {
            target: MAIN_DISC_RECORD_LOADER_ADDRESS,
        },
        Instruction::nop(),
    ]);
    let image = LoadedImage {
        path: "SYNTH.BIN".to_string(),
        data,
        runtime_base: Some(RUNTIME_BASE),
        entrypoints: vec![],
    };
    let catalog = SourceCatalog::fixture(61, "DAT2/TARGET.BIZ");

    let candidates =
        disc_record_load_call_candidates(&image, &BTreeSet::new(), &[], &catalog, &[]).unwrap();

    assert_eq!(candidates.len(), 1);
    assert!(candidates[0].catalog_index_candidates.is_empty());
    assert_eq!(candidates[0].catalog_index_resolution, "unresolved");
    assert_eq!(
        candidates[0].catalog_index_argument_producer,
        "preceding_memory_load"
    );
}

#[test]
fn disc_load_candidate_reports_the_affine_address_of_a_table_argument() {
    let data = encode_instructions(&[
        Instruction::Addiu {
            rt: Register::S0,
            rs: Register::A0,
            immediate: 3,
        },
        Instruction::Sll {
            rd: Register::S0,
            rt: Register::S0,
            shift: 2,
        },
        Instruction::Jal {
            target: RUNTIME_BASE + 0x100,
        },
        Instruction::nop(),
        Instruction::Lui {
            rt: Register::AT,
            immediate: 0x8009,
        },
        Instruction::Addu {
            rd: Register::AT,
            rs: Register::AT,
            rt: Register::S0,
        },
        Instruction::Lhu {
            rt: Register::A1,
            base: Register::AT,
            offset: -0x100,
        },
        Instruction::Jal {
            target: MAIN_DISC_RECORD_LOADER_ADDRESS,
        },
        Instruction::nop(),
    ]);
    let image = LoadedImage {
        path: "SYNTH.BIN".to_string(),
        data,
        runtime_base: Some(RUNTIME_BASE),
        entrypoints: vec![],
    };
    let catalog = SourceCatalog::fixture(61, "DAT2/TARGET.BIZ");

    let candidates =
        disc_record_load_call_candidates(&image, &BTreeSet::new(), &[], &catalog, &[]).unwrap();

    let memory_load = candidates[0].catalog_index_memory_load.as_ref().unwrap();
    assert_eq!(memory_load.load_kind, "unsigned_halfword");
    assert_eq!(memory_load.address_resolution, "affine");
    assert_eq!(
        memory_load.effective_address_when_selector_zero.as_deref(),
        Some("0x8008ff0c")
    );
    assert_eq!(memory_load.selector_register.as_deref(), Some("a0"));
    assert_eq!(memory_load.selector_stride_bytes, Some(4));
    assert_eq!(
        memory_load.selector_origin_kind.as_deref(),
        Some("local_scan_boundary")
    );
}

#[test]
fn disc_load_candidate_retains_a_memory_loaded_selector_origin() {
    let data = encode_instructions(&[
        Instruction::Lbu {
            rt: Register::T0,
            base: Register::A0,
            offset: 7,
        },
        Instruction::Sll {
            rd: Register::T0,
            rt: Register::T0,
            shift: 3,
        },
        Instruction::Lui {
            rt: Register::AT,
            immediate: 0x8009,
        },
        Instruction::Addu {
            rd: Register::AT,
            rs: Register::AT,
            rt: Register::T0,
        },
        Instruction::Lhu {
            rt: Register::A1,
            base: Register::AT,
            offset: -0x400,
        },
        Instruction::Jal {
            target: MAIN_DISC_RECORD_LOADER_ADDRESS,
        },
        Instruction::nop(),
    ]);
    let image = LoadedImage {
        path: "SYNTH.BIN".to_string(),
        data,
        runtime_base: Some(RUNTIME_BASE),
        entrypoints: vec![],
    };
    let catalog = SourceCatalog::fixture(61, "DAT2/TARGET.BIZ");

    let candidates =
        disc_record_load_call_candidates(&image, &BTreeSet::new(), &[], &catalog, &[]).unwrap();

    let memory_load = candidates[0].catalog_index_memory_load.as_ref().unwrap();
    assert_eq!(
        memory_load.effective_address_when_selector_zero.as_deref(),
        Some("0x8008fc00")
    );
    assert_eq!(memory_load.selector_register.as_deref(), Some("t0"));
    assert_eq!(memory_load.selector_stride_bytes, Some(8));
    assert_eq!(
        memory_load.selector_origin_kind.as_deref(),
        Some("unsigned_byte_load")
    );
    assert_eq!(
        memory_load.selector_origin_runtime_address.as_deref(),
        Some("0x800a2000")
    );
}

#[test]
fn disc_load_candidate_resolves_a_literal_forwarded_through_a_validated_wrapper() {
    let wrapper_offset = 4 * 4;
    let loader_call_offset = wrapper_offset + 4;
    let data = encode_instructions(&[
        Instruction::Jal {
            target: RUNTIME_BASE + wrapper_offset as u32,
        },
        Instruction::Addiu {
            rt: Register::A0,
            rs: Register::ZERO,
            immediate: 61,
        },
        Instruction::Jr { rs: Register::RA },
        Instruction::nop(),
        Instruction::Addu {
            rd: Register::S0,
            rs: Register::A0,
            rt: Register::ZERO,
        },
        Instruction::Jal {
            target: MAIN_DISC_RECORD_LOADER_ADDRESS,
        },
        Instruction::Andi {
            rt: Register::A1,
            rs: Register::S0,
            immediate: u16::MAX,
        },
        Instruction::Jr { rs: Register::RA },
        Instruction::nop(),
    ]);
    let image = LoadedImage {
        path: "SLPS_021.20".to_string(),
        data,
        runtime_base: Some(RUNTIME_BASE),
        entrypoints: vec![],
    };
    let catalog = SourceCatalog::fixture(61, "DAT2/TARGET.BIZ");
    let forwarder = DiscRecordLoaderForwarderProfile {
        image_path: "SLPS_021.20",
        entrypoint_runtime_address: RUNTIME_BASE + wrapper_offset as u32,
        source_argument_register: Register::A0,
        saved_argument_register: Register::S0,
        save_runtime_address: RUNTIME_BASE + wrapper_offset as u32,
        loader_call_runtime_address: RUNTIME_BASE + loader_call_offset as u32,
        forward_runtime_address: RUNTIME_BASE + loader_call_offset as u32 + 4,
        forwarded_argument_register: Register::A1,
        mask: u16::MAX,
    };

    let candidates = disc_record_load_call_candidates_with_profiles(
        &image,
        &BTreeSet::new(),
        &[],
        &catalog,
        &[],
        DiscRecordLoaderProfiles {
            forwarders: &[forwarder],
            bounded_indices: &[],
            owners: &[],
        },
    )
    .unwrap();

    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].catalog_index_candidates, [61]);
    assert_eq!(
        candidates[0].catalog_index_resolution,
        "forwarded_call_argument"
    );
    assert_eq!(
        candidates[0].catalog_index_argument_producer,
        "validated_wrapper_call_argument"
    );
    assert_eq!(candidates[0].catalog_record_paths, ["DAT2/TARGET.BIZ"]);
    assert_eq!(
        candidates[0].forwarding_call_runtime_addresses,
        ["0x800a2000"]
    );

    let changed_forwarder = DiscRecordLoaderForwarderProfile {
        saved_argument_register: Register::S1,
        ..forwarder
    };
    let error = disc_record_load_call_candidates_with_profiles(
        &image,
        &BTreeSet::new(),
        &[],
        &catalog,
        &[],
        DiscRecordLoaderProfiles {
            forwarders: &[changed_forwarder],
            bounded_indices: &[],
            owners: &[],
        },
    )
    .expect_err("changed wrapper grammar must fail closed");
    assert!(
        error
            .to_string()
            .contains("disc-loader forwarder argument save changed")
    );
}

#[test]
fn disc_load_candidate_accepts_only_a_validated_bounded_index_set() {
    const VALIDATED_INSTRUCTIONS: &[(u32, Instruction)] = &[(
        RUNTIME_BASE + 8,
        Instruction::Sltiu {
            rt: Register::V0,
            rs: Register::A0,
            immediate: 2,
        },
    )];
    const CATALOG_INDICES: &[u16] = &[61, 62];
    let data = encode_instructions(&[
        Instruction::Jal {
            target: MAIN_DISC_RECORD_LOADER_ADDRESS,
        },
        Instruction::nop(),
        VALIDATED_INSTRUCTIONS[0].1.clone(),
    ]);
    let image = LoadedImage {
        path: "SLPS_021.20".to_string(),
        data,
        runtime_base: Some(RUNTIME_BASE),
        entrypoints: vec![],
    };
    let catalog = SourceCatalog::fixture(61, "DAT2/TARGET.BIZ");
    let bounded = DiscRecordLoaderBoundedIndexProfile {
        image_path: "SLPS_021.20",
        call_runtime_address: RUNTIME_BASE,
        validated_instructions: VALIDATED_INSTRUCTIONS,
        catalog_indices: CATALOG_INDICES,
        resolution: "validated_bounded_range",
        argument_producer: "bounded_fixture",
    };

    let candidates = disc_record_load_call_candidates_with_profiles(
        &image,
        &BTreeSet::new(),
        &[],
        &catalog,
        &[],
        DiscRecordLoaderProfiles {
            forwarders: &[],
            bounded_indices: &[bounded],
            owners: &[],
        },
    )
    .unwrap();

    assert_eq!(candidates[0].catalog_index_candidates, [61, 62]);
    assert_eq!(
        candidates[0].catalog_index_resolution,
        "validated_bounded_range"
    );
    assert_eq!(
        candidates[0].catalog_index_argument_producer,
        "bounded_fixture"
    );
    assert_eq!(candidates[0].catalog_record_paths, ["DAT2/TARGET.BIZ"]);

    const CHANGED_INSTRUCTIONS: &[(u32, Instruction)] = &[(
        RUNTIME_BASE + 8,
        Instruction::Sltiu {
            rt: Register::V0,
            rs: Register::A0,
            immediate: 3,
        },
    )];
    let changed = DiscRecordLoaderBoundedIndexProfile {
        validated_instructions: CHANGED_INSTRUCTIONS,
        ..bounded
    };
    let error = disc_record_load_call_candidates_with_profiles(
        &image,
        &BTreeSet::new(),
        &[],
        &catalog,
        &[],
        DiscRecordLoaderProfiles {
            forwarders: &[],
            bounded_indices: &[changed],
            owners: &[],
        },
    )
    .expect_err("changed bounded-index grammar must fail closed");
    assert!(
        error
            .to_string()
            .contains("bounded catalog-index instruction changed")
    );
}

#[test]
fn disc_load_candidate_retains_an_owner_with_a_complete_direct_caller_census() {
    const FUNCTION_ENTRY: u32 = RUNTIME_BASE + 16;
    const LOADER_CALL: u32 = RUNTIME_BASE + 20;
    const DESTINATION: u32 = 0x8010_2000;
    const LOADER_CALLS: &[DiscRecordLoaderOwnerCallProfile] = &[DiscRecordLoaderOwnerCallProfile {
        call_runtime_address: LOADER_CALL,
        destination_runtime_address: DESTINATION,
    }];
    const DIRECT_CALLERS: &[u32] = &[RUNTIME_BASE];
    let data = encode_instructions(&[
        Instruction::Jal {
            target: FUNCTION_ENTRY,
        },
        Instruction::nop(),
        Instruction::Jr { rs: Register::RA },
        Instruction::nop(),
        Instruction::Lui {
            rt: Register::A0,
            immediate: 0x8010,
        },
        Instruction::Jal {
            target: MAIN_DISC_RECORD_LOADER_ADDRESS,
        },
        Instruction::Ori {
            rt: Register::A0,
            rs: Register::A0,
            immediate: 0x2000,
        },
    ]);
    let image = LoadedImage {
        path: "SLPS_021.20".to_string(),
        data,
        runtime_base: Some(RUNTIME_BASE),
        entrypoints: vec![],
    };
    let catalog = SourceCatalog::fixture(61, "DAT2/TARGET.BIZ");
    let owner = DiscRecordLoaderOwnerProfile {
        image_path: "SLPS_021.20",
        owner_id: "four_slot_resource_setup",
        function_entry_runtime_address: FUNCTION_ENTRY,
        loader_calls: LOADER_CALLS,
        direct_caller_runtime_addresses: DIRECT_CALLERS,
    };

    let candidates = disc_record_load_call_candidates_with_profiles(
        &image,
        &BTreeSet::new(),
        &[],
        &catalog,
        &[],
        DiscRecordLoaderProfiles {
            forwarders: &[],
            bounded_indices: &[],
            owners: &[owner],
        },
    )
    .unwrap();

    assert!(candidates[0].catalog_index_candidates.is_empty());
    assert_eq!(
        candidates[0].static_owner_id.as_deref(),
        Some("four_slot_resource_setup")
    );
    assert_eq!(
        candidates[0]
            .static_owner_function_runtime_address
            .as_deref(),
        Some("0x800a2010")
    );
    assert_eq!(
        candidates[0]
            .static_owner_destination_runtime_address
            .as_deref(),
        Some("0x80102000")
    );
    assert_eq!(
        candidates[0].static_owner_direct_caller_runtime_addresses,
        ["0x800a2000"]
    );

    let changed = DiscRecordLoaderOwnerProfile {
        direct_caller_runtime_addresses: &[],
        ..owner
    };
    let error = disc_record_load_call_candidates_with_profiles(
        &image,
        &BTreeSet::new(),
        &[],
        &catalog,
        &[],
        DiscRecordLoaderProfiles {
            forwarders: &[],
            bounded_indices: &[],
            owners: &[changed],
        },
    )
    .expect_err("changed owner caller census must fail closed");
    assert!(
        error
            .to_string()
            .contains("direct caller census changed for disc-loader owner")
    );

    const CHANGED_LOADER_CALLS: &[DiscRecordLoaderOwnerCallProfile] =
        &[DiscRecordLoaderOwnerCallProfile {
            call_runtime_address: LOADER_CALL,
            destination_runtime_address: DESTINATION + 4,
        }];
    let changed = DiscRecordLoaderOwnerProfile {
        loader_calls: CHANGED_LOADER_CALLS,
        ..owner
    };
    let error = disc_record_load_call_candidates_with_profiles(
        &image,
        &BTreeSet::new(),
        &[],
        &catalog,
        &[],
        DiscRecordLoaderProfiles {
            forwarders: &[],
            bounded_indices: &[],
            owners: &[changed],
        },
    )
    .expect_err("changed owner destination must fail closed");
    assert!(error.to_string().contains("destination changed"));
}

#[test]
fn grouped_sink_requires_every_declared_site_to_be_reachable() {
    let profile = StaticConsumerSinkProfile {
        additional_runtime_addresses: &[RUNTIME_BASE + 8],
        ..sink("renderer_group")
    };
    let image = LoadedImage {
        path: "DAT1/SYNTH.BIN".to_string(),
        data: vec![0; 12],
        runtime_base: Some(RUNTIME_BASE),
        entrypoints: vec![entrypoint("primary", 0, RUNTIME_BASE)],
    };

    let partial = declared_sink_audit(&profile, &image, Some(&BTreeSet::from([0])));
    assert!(!partial.entrypoint_reachable);
    assert!(partial.sites[0].entrypoint_reachable);
    assert!(!partial.sites[1].entrypoint_reachable);

    let complete = declared_sink_audit(&profile, &image, Some(&BTreeSet::from([0, 8])));
    assert!(complete.entrypoint_reachable);
    assert!(complete.sites.iter().all(|site| site.entrypoint_reachable));
}

#[test]
fn consumer_profile_validation_rejects_ambiguous_ids_and_references() {
    let duplicate_sinks = [sink("renderer"), sink("renderer")];
    let duplicate_edges = [
        edge("asset_to_renderer", &[], "renderer"),
        edge("asset_to_renderer", &[], "renderer"),
    ];
    let missing_sink = [edge("asset_to_missing", &[], "missing")];
    let repeated_member = [edge("asset_to_renderer", &[2, 2], "renderer")];
    let repeated_site = [StaticConsumerSinkProfile {
        additional_runtime_addresses: &[RUNTIME_BASE],
        ..sink("renderer")
    }];

    assert!(
        validate_consumer_profiles(&[source_region(&[])], &duplicate_sinks, &[])
            .unwrap_err()
            .to_string()
            .contains("duplicate static consumer sink ID")
    );
    assert!(
        validate_consumer_profiles(&[source_region(&[])], &[sink("renderer")], &duplicate_edges,)
            .unwrap_err()
            .to_string()
            .contains("duplicate static consumer edge ID")
    );
    assert!(
        validate_consumer_profiles(&[source_region(&[])], &[sink("renderer")], &missing_sink)
            .unwrap_err()
            .to_string()
            .contains("references unknown sink")
    );
    assert!(
        validate_consumer_profiles(
            &[source_region(&[2, 2])],
            &[sink("renderer")],
            &repeated_member,
        )
        .unwrap_err()
        .to_string()
        .contains("repeats a source member index")
    );
    assert!(
        validate_consumer_profiles(&[source_region(&[])], &repeated_site, &[])
            .unwrap_err()
            .to_string()
            .contains("repeats a runtime address")
    );
}

#[test]
fn consumer_profile_validation_rejects_edge_without_source_region() {
    let error = validate_consumer_profiles(
        &[],
        &[sink("renderer")],
        &[edge("asset_to_renderer", &[], "renderer")],
    )
    .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("references unknown source region")
    );
}

const fn sink(id: &'static str) -> StaticConsumerSinkProfile {
    StaticConsumerSinkProfile {
        id,
        kind: "renderer",
        image_path: "DAT1/SYNTH.BIN",
        runtime_address: RUNTIME_BASE,
        additional_runtime_addresses: &[],
        disposition: StaticConsumerSinkDisposition::ActiveConsumer,
    }
}

const fn entrypoint(
    role: &'static str,
    source_reference_offset: usize,
    runtime_address: u32,
) -> LoadedImageEntrypoint {
    LoadedImageEntrypoint {
        role,
        source_reference_kind: "fixture",
        source_reference_offset,
        runtime_address,
    }
}

const fn edge(
    id: &'static str,
    source_member_indices: &'static [u8],
    sink_id: &'static str,
) -> StaticConsumerEdgeProfile {
    StaticConsumerEdgeProfile {
        id,
        source_record_path: "DAT2/SYNTH.BIZ",
        source_region_id: "synthetic_region",
        source_member_indices,
        relationship: "renders",
        sink_id,
    }
}

const fn source_region(source_member_indices: &'static [u8]) -> StaticConsumerSourceRegionProfile {
    StaticConsumerSourceRegionProfile {
        source_record_path: "DAT2/SYNTH.BIZ",
        source_region_id: "synthetic_region",
        source_member_indices,
        source_catalog_index: 60,
        unbound_consumer_reason: None,
    }
}

fn encode_instructions(instructions: &[Instruction]) -> Vec<u8> {
    instructions
        .iter()
        .enumerate()
        .flat_map(|(index, instruction)| {
            encode(instruction, RUNTIME_BASE + (index * 4) as u32)
                .unwrap()
                .to_le_bytes()
        })
        .collect()
}
