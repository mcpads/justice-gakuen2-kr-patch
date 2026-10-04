use std::{collections::BTreeSet, path::Path};

use anyhow::{Context, Result, ensure};

use super::catalog::{
    SourceCatalog, disc_record_load_call_candidates, validate_source_catalog_bindings,
};
use super::catalog_selector_domain::resolve_declared_selector_table_loads;
use super::model::{
    DeclaredEntrypointAudit, ExecutableRangeAudit, LoadedImageConsumerAudit,
    StaticConsumerAuditConfig, StaticConsumerAuditReport, StaticConsumerEdgeAudit,
    StaticConsumerSinkAudit, StaticConsumerSinkSiteAudit, StaticConsumerSourceRegionAudit,
    StaticDiscRecordLoadAudit, StaticProfiledStateAccessAudit,
};
use super::pointer_run_references::{audit_declared_pointer_runs, validate_pointer_run_profiles};
use super::profiles::{
    STATIC_CONSUMER_EDGE_PROFILES, STATIC_CONSUMER_SINK_PROFILES,
    STATIC_CONSUMER_SOURCE_REGION_PROFILES, STATIC_POINTER_RUN_NON_ADDRESS_FLOW_PROFILES,
    STATIC_POINTER_RUN_PROFILES, StaticConsumerEdgeProfile, StaticConsumerSinkDisposition,
    StaticConsumerSinkProfile, StaticConsumerSourceRegionProfile,
};
use super::program_analysis::analyze_loaded_program;
use super::renderer_argument_boundary::resolve_renderer_argument_boundaries;
use super::renderer_call_census::audit_declared_renderer_calls;
use super::selector_state::{
    loader_selector_state_accesses, loader_selector_upstream_state_accesses,
};
use crate::pipeline::sha256_bytes;
use crate::psx_static_analysis::ExecutableDomain;
use crate::psx_static_analysis::reachable_control_flow::UnresolvedRegisterTransferKind;
use crate::psx_static_analysis::value_flow::DerivedAddressScan;
use crate::source_disc::{
    LoadedImage, SupportedSourceDisc, load_dat1_images, load_main_executable_image,
};

pub fn audit_static_consumers(
    config: &StaticConsumerAuditConfig,
) -> Result<StaticConsumerAuditReport> {
    ensure!(
        config.value_flow_state_budget > 0,
        "value-flow state budget must be positive"
    );
    ensure!(
        config.pointer_run_value_flow_state_budget >= config.value_flow_state_budget,
        "pointer-run value-flow state budget must be at least the general budget"
    );
    validate_consumer_profiles(
        STATIC_CONSUMER_SOURCE_REGION_PROFILES,
        STATIC_CONSUMER_SINK_PROFILES,
        STATIC_CONSUMER_EDGE_PROFILES,
    )?;
    validate_pointer_run_profiles(
        STATIC_POINTER_RUN_PROFILES,
        STATIC_POINTER_RUN_NON_ADDRESS_FLOW_PROFILES,
    )?;
    let source = SupportedSourceDisc::open(&config.cue)?;
    let source_catalog = SourceCatalog::read(&source)?;
    validate_source_catalog_bindings(
        &source,
        &source_catalog,
        STATIC_CONSUMER_SOURCE_REGION_PROFILES,
    )?;
    let mut images = load_dat1_images(source.image_path())?;
    images.push(load_main_executable_image(source.image_path())?);
    images.sort_by(|left, right| left.path.cmp(&right.path));

    let mut image_reports = images
        .iter()
        .map(|image| {
            analyze_loaded_image(
                image,
                config.value_flow_state_budget,
                config.pointer_run_value_flow_state_budget,
                &source_catalog,
            )
        })
        .collect::<Result<Vec<_>>>()?;
    ensure!(
        sum(&image_reports, |image| image.declared_pointer_runs.len())
            == STATIC_POINTER_RUN_PROFILES.len(),
        "not every static pointer-run profile matched a loaded image"
    );
    ensure!(
        sum(&image_reports, |image| image
            .declared_renderer_call_censuses
            .len())
            == 1,
        "the KANRI renderer-call census did not match exactly one loaded image"
    );
    resolve_declared_selector_table_loads(
        &images,
        &mut image_reports,
        &source_catalog,
        STATIC_CONSUMER_SOURCE_REGION_PROFILES,
    )?;
    resolve_renderer_argument_boundaries(&images, &mut image_reports)?;
    let declared_consumer_edges = declared_edge_audits(
        &image_reports,
        StaticConsumerSinkDisposition::ActiveConsumer,
    )?;
    let declared_dormant_candidate_edges = declared_edge_audits(
        &image_reports,
        StaticConsumerSinkDisposition::DormantCandidate,
    )?;
    ensure!(
        declared_dormant_candidate_edges
            .iter()
            .all(|edge| !edge.sink_entrypoint_reachable),
        "a dormant consumer candidate became reachable from a declared entrypoint"
    );
    let declared_source_regions = declared_source_region_audits(
        STATIC_CONSUMER_SOURCE_REGION_PROFILES,
        &declared_consumer_edges,
        &declared_dormant_candidate_edges,
    );
    let reachable_declared_active_consumer_sink_count = image_reports
        .iter()
        .flat_map(|image| &image.declared_semantic_sinks)
        .filter(|sink| sink.entrypoint_reachable)
        .count();
    let declared_active_consumer_sink_count =
        sum(&image_reports, |image| image.declared_semantic_sinks.len());
    let report = StaticConsumerAuditReport {
        kind: "Justice Gakuen 2 static consumer structure audit".to_string(),
        source_bin_sha256: source.source_bin_sha256().to_string(),
        loaded_image_count: image_reports.len(),
        analyzed_image_count: count_status(&image_reports, "analyzed"),
        unresolved_entrypoint_image_count: count_status(
            &image_reports,
            "entrypoint_unresolved",
        ),
        image_without_runtime_base_count: count_status(&image_reports, "no_runtime_base"),
        reachable_instruction_count: sum(&image_reports, |image| {
            image.reachable_instruction_count
        }),
        value_flow_seed_count: sum(&image_reports, |image| image.value_flow_seed_count),
        value_flow_instruction_state_count: sum(&image_reports, |image| {
            image.value_flow_instruction_state_count
        }),
        value_flow_budget_exhausted_seed_count: sum(&image_reports, |image| {
            image.value_flow_budget_exhausted_seed_count
        }),
        unresolved_indirect_jump_count: sum(&image_reports, |image| {
            image.unresolved_indirect_jump_count
        }),
        unresolved_indirect_call_count: sum(&image_reports, |image| {
            image.unresolved_indirect_call_count
        }),
        declared_indirect_jump_profile_count: sum(&image_reports, |image| {
            image.declared_indirect_jumps.len()
        }),
        declared_indirect_jump_table_entry_count: image_reports
            .iter()
            .flat_map(|image| &image.declared_indirect_jumps)
            .map(|jump| jump.table_entry_count)
            .sum(),
        bounded_jump_table_count: sum(&image_reports, |image| image.bounded_jump_table_count),
        bounded_jump_table_entry_count: sum(&image_reports, |image| {
            image.bounded_jump_table_entry_count
        }),
        declared_pointer_run_count: sum(&image_reports, |image| {
            image.declared_pointer_runs.len()
        }),
        pointer_run_decoded_pointer_count: image_reports
            .iter()
            .flat_map(|image| &image.declared_pointer_runs)
            .map(|run| run.decoded_targets.len())
            .sum(),
        pointer_run_raw_address_reference_count: image_reports
            .iter()
            .flat_map(|image| &image.declared_pointer_runs)
            .map(|run| run.raw_address_references.len())
            .sum(),
        pointer_run_external_raw_address_reference_count: image_reports
            .iter()
            .flat_map(|image| &image.declared_pointer_runs)
            .map(|run| run.external_raw_address_reference_count)
            .sum(),
        pointer_run_reachable_derived_reference_count: image_reports
            .iter()
            .flat_map(|image| &image.declared_pointer_runs)
            .map(|run| run.reachable_derived_references.len())
            .sum(),
        pointer_run_reachable_pointer_table_reference_count: image_reports
            .iter()
            .flat_map(|image| &image.declared_pointer_runs)
            .map(|run| run.reachable_pointer_table_reference_count)
            .sum(),
        pointer_run_reachable_target_arena_reference_count: image_reports
            .iter()
            .flat_map(|image| &image.declared_pointer_runs)
            .map(|run| run.reachable_target_arena_reference_count)
            .sum(),
        pointer_run_reachable_derived_instruction_site_count: image_reports
            .iter()
            .flat_map(|image| &image.declared_pointer_runs)
            .flat_map(|run| &run.reachable_derived_references)
            .map(|reference| {
                (
                    reference.seed_instruction_runtime_address.as_str(),
                    reference.instruction_runtime_address.as_str(),
                )
            })
            .collect::<BTreeSet<_>>()
            .len(),
        pointer_run_profiled_non_address_exhausted_seed_count: image_reports
            .iter()
            .flat_map(|image| &image.declared_pointer_runs)
            .map(|run| run.reachable_analysis_profiled_non_address_exhausted_seed_count)
            .sum(),
        pointer_run_unprofiled_exhausted_seed_count: image_reports
            .iter()
            .flat_map(|image| &image.declared_pointer_runs)
            .map(|run| run.reachable_analysis_unprofiled_exhausted_seed_count)
            .sum(),
        pointer_run_value_flow_state_budget: config.pointer_run_value_flow_state_budget,
        declared_source_region_count: declared_source_regions.len(),
        consumer_bound_declared_source_region_count: declared_source_regions
            .iter()
            .filter(|region| region.consumer_binding_status != "consumer_unbound")
            .count(),
        consumer_unbound_declared_source_region_count: declared_source_regions
            .iter()
            .filter(|region| region.consumer_binding_status == "consumer_unbound")
            .count(),
        fully_entrypoint_reachable_consumer_bound_source_region_count: declared_source_regions
            .iter()
            .filter(|region| {
                region.consumer_binding_status == "consumer_bound_all_sinks_entrypoint_reachable"
            })
            .count(),
        source_region_scope_complete: false,
        direct_disc_record_loader_call_candidate_count: sum(&image_reports, |image| {
            image.disc_record_load_call_candidates.len()
        }),
        entrypoint_reachable_disc_record_loader_call_candidate_count: image_reports
            .iter()
            .flat_map(|image| &image.disc_record_load_call_candidates)
            .filter(|load| load.entrypoint_reachable)
            .count(),
        entrypoint_unreachable_disc_record_loader_call_candidate_count: image_reports
            .iter()
            .flat_map(|image| &image.disc_record_load_call_candidates)
            .filter(|load| !load.entrypoint_reachable)
            .count(),
        catalog_index_resolved_disc_record_loader_call_candidate_count: image_reports
            .iter()
            .flat_map(|image| &image.disc_record_load_call_candidates)
            .filter(|load| !load.catalog_index_candidates.is_empty())
            .count(),
        catalog_index_unresolved_disc_record_loader_call_candidate_count: image_reports
            .iter()
            .flat_map(|image| &image.disc_record_load_call_candidates)
            .filter(|load| load.catalog_index_candidates.is_empty())
            .count(),
        profiled_selector_table_load_count: image_reports
            .iter()
            .flat_map(|image| &image.disc_record_load_call_candidates)
            .filter(|load| {
                load.catalog_index_memory_load
                    .as_ref()
                    .is_some_and(|memory| {
                        memory.declared_selector_domain.is_some()
                            || memory.custom_record_selector.is_some()
                    })
            })
            .count(),
        profiled_custom_record_table_load_count: image_reports
            .iter()
            .flat_map(|image| &image.disc_record_load_call_candidates)
            .filter(|load| {
                load.catalog_index_memory_load
                    .as_ref()
                    .is_some_and(|memory| memory.custom_record_selector.is_some())
            })
            .count(),
        finite_profiled_selector_table_load_count: image_reports
            .iter()
            .flat_map(|image| &image.disc_record_load_call_candidates)
            .filter(|load| {
                load.catalog_index_memory_load
                    .as_ref()
                    .is_some_and(|memory| {
                        memory.declared_selector_domain.as_ref().is_some_and(|domain| {
                            domain.value_resolution == "finite_declared_writer_domain"
                        }) || memory.custom_record_selector.as_ref().is_some_and(|selector| {
                            selector.selector_value_resolution == "finite_declared_writer_domain"
                        })
                    })
            })
            .count(),
        dynamic_profiled_selector_table_load_count: image_reports
            .iter()
            .flat_map(|image| &image.disc_record_load_call_candidates)
            .filter(|load| {
                load.catalog_index_memory_load
                    .as_ref()
                    .is_some_and(|memory| {
                        memory.declared_selector_domain.as_ref().is_some_and(|domain| {
                            domain.value_resolution == "partial_dynamic_declared_writer_domain"
                        }) || memory.custom_record_selector.as_ref().is_some_and(|selector| {
                            selector.selector_value_resolution
                                == "partial_dynamic_declared_writer_domain"
                        })
                    })
            })
            .count(),
        source_record_resolved_disc_record_loader_call_candidate_count: image_reports
            .iter()
            .flat_map(|image| &image.disc_record_load_call_candidates)
            .filter(|load| !load.catalog_record_paths.is_empty())
            .count(),
        source_record_unresolved_disc_record_loader_call_candidate_count: image_reports
            .iter()
            .flat_map(|image| &image.disc_record_load_call_candidates)
            .filter(|load| load.catalog_record_paths.is_empty())
            .count(),
        catalog_or_static_owner_resolved_disc_record_loader_call_candidate_count: image_reports
            .iter()
            .flat_map(|image| &image.disc_record_load_call_candidates)
            .filter(|load| {
                !load.catalog_record_paths.is_empty() || load.static_owner_id.is_some()
            })
            .count(),
        catalog_and_static_owner_unresolved_disc_record_loader_call_candidate_count: image_reports
            .iter()
            .flat_map(|image| &image.disc_record_load_call_candidates)
            .filter(|load| {
                load.catalog_record_paths.is_empty() && load.static_owner_id.is_none()
            })
            .count(),
        disc_record_loader_selector_state_access_candidate_count: image_reports
            .iter()
            .map(|image| image.disc_record_loader_selector_state_accesses.len())
            .sum(),
        disc_record_loader_selector_state_unique_memory_access_site_count:
            selector_state_unique_access_site_count(&image_reports, None),
        disc_record_loader_selector_state_unique_read_site_count:
            selector_state_unique_access_site_count(&image_reports, Some("read")),
        disc_record_loader_selector_state_unique_write_site_count:
            selector_state_unique_access_site_count(&image_reports, Some("write")),
        disc_record_loader_selector_state_classified_write_site_count:
            selector_state_writer_site_count(&image_reports, Some("classified")),
        disc_record_loader_selector_state_exact_value_write_site_count:
            selector_state_writer_site_count(&image_reports, Some("exact_candidates")),
        disc_record_loader_selector_state_bounded_value_write_site_count:
            selector_state_writer_site_count(&image_reports, Some("bounded_range")),
        disc_record_loader_selector_state_runtime_source_write_site_count:
            selector_state_writer_site_count(&image_reports, Some("runtime_source")),
        disc_record_loader_selector_state_unclassified_write_site_count:
            selector_state_writer_site_count(&image_reports, None),
        disc_record_loader_selector_upstream_state_access_candidate_count: image_reports
            .iter()
            .map(|image| image.disc_record_loader_selector_upstream_state_accesses.len())
            .sum(),
        disc_record_loader_selector_upstream_state_unique_memory_access_site_count:
            selector_upstream_state_unique_access_site_count(&image_reports, None),
        disc_record_loader_selector_upstream_state_unique_read_site_count:
            selector_upstream_state_unique_access_site_count(&image_reports, Some("read")),
        disc_record_loader_selector_upstream_state_unique_write_site_count:
            selector_upstream_state_unique_access_site_count(&image_reports, Some("write")),
        disc_record_loader_selector_upstream_state_classified_write_site_count:
            selector_upstream_state_writer_site_count(&image_reports, Some("classified")),
        disc_record_loader_selector_upstream_state_exact_value_write_site_count:
            selector_upstream_state_writer_site_count(&image_reports, Some("exact_candidates")),
        disc_record_loader_selector_upstream_state_bounded_value_write_site_count:
            selector_upstream_state_writer_site_count(&image_reports, Some("bounded_range")),
        disc_record_loader_selector_upstream_state_runtime_source_write_site_count:
            selector_upstream_state_writer_site_count(&image_reports, Some("runtime_source")),
        disc_record_loader_selector_upstream_state_unclassified_write_site_count:
            selector_upstream_state_writer_site_count(&image_reports, None),
        declared_active_consumer_sink_count,
        reachable_declared_active_consumer_sink_count,
        unreachable_declared_active_consumer_sink_count: declared_active_consumer_sink_count
            - reachable_declared_active_consumer_sink_count,
        declared_dormant_candidate_sink_count: sum(&image_reports, |image| {
            image.declared_dormant_candidate_sinks.len()
        }),
        reachable_declared_dormant_candidate_sink_count: image_reports
            .iter()
            .flat_map(|image| &image.declared_dormant_candidate_sinks)
            .filter(|sink| sink.entrypoint_reachable)
            .count(),
        unreachable_declared_dormant_candidate_sink_count: image_reports
            .iter()
            .flat_map(|image| &image.declared_dormant_candidate_sinks)
            .filter(|sink| !sink.entrypoint_reachable)
            .count(),
        declared_consumer_edge_count: declared_consumer_edges.len(),
        entrypoint_reachable_declared_consumer_edge_count: declared_consumer_edges
            .iter()
            .filter(|edge| edge.sink_entrypoint_reachable)
            .count(),
        entrypoint_unresolved_declared_consumer_edge_count: declared_consumer_edges
            .iter()
            .filter(|edge| !edge.sink_entrypoint_reachable)
            .count(),
        declared_dormant_candidate_edge_count: declared_dormant_candidate_edges.len(),
        declared_consumer_reachability_complete: declared_active_consumer_sink_count
            == reachable_declared_active_consumer_sink_count
            && declared_consumer_edges
                .iter()
                .all(|edge| edge.sink_entrypoint_reachable),
        semantic_sink_classification_complete: false,
        declared_renderer_call_census_count: sum(&image_reports, |image| {
            image.declared_renderer_call_censuses.len()
        }),
        direct_renderer_call_site_count: image_reports
            .iter()
            .flat_map(|image| &image.declared_renderer_call_censuses)
            .map(|census| census.direct_call_site_count)
            .sum(),
        entrypoint_reachable_direct_renderer_call_site_count: image_reports
            .iter()
            .flat_map(|image| &image.declared_renderer_call_censuses)
            .map(|census| census.entrypoint_reachable_call_site_count)
            .sum(),
        classified_direct_renderer_call_site_count: image_reports
            .iter()
            .flat_map(|image| &image.declared_renderer_call_censuses)
            .map(|census| census.classified_call_site_count)
            .sum(),
        finite_source_domain_direct_renderer_call_site_count: image_reports
            .iter()
            .flat_map(|image| &image.declared_renderer_call_censuses)
            .map(|census| census.finite_source_domain_call_site_count)
            .sum(),
        dynamic_source_domain_direct_renderer_call_site_count: image_reports
            .iter()
            .flat_map(|image| &image.declared_renderer_call_censuses)
            .map(|census| census.dynamic_source_domain_call_site_count)
            .sum(),
        closed_dynamic_source_boundary_direct_renderer_call_site_count: image_reports
            .iter()
            .flat_map(|image| &image.declared_renderer_call_censuses)
            .flat_map(|census| &census.call_sites)
            .filter(|site| site.dynamic_source_boundary.is_some())
            .count(),
        renderer_call_site_classification_complete: image_reports
            .iter()
            .flat_map(|image| &image.declared_renderer_call_censuses)
            .all(|census| census.call_site_classification_complete),
        renderer_source_domain_analysis_complete: image_reports
            .iter()
            .flat_map(|image| &image.declared_renderer_call_censuses)
            .all(|census| census.source_domain_complete),
        product_build_input: false,
        images: image_reports,
        declared_source_regions,
        declared_consumer_edges,
        declared_dormant_candidate_edges,
        limitations: vec![
            "The report closes executable reachability and value flow from each explicitly declared entrypoint; it does not classify every call, memory access, packet constructor, renderer, or graphics upload as a semantic consumer.".to_string(),
            "A loaded image contributes its primary entrypoint plus only independently source-bound external dispatch roots. Code outside the complete declared interface is retained as a dormant candidate, not counted as an active consumer edge, until another source-bound dispatch is proven.".to_string(),
            "Entrypoint reachability is a conservative executable-domain approximation. A fallthrough into embedded data remains possible until a target profile explicitly separates that span.".to_string(),
            "An unresolved indirect transfer remains an explicit graph edge gap. Increasing the value-flow budget is a diagnostic and cannot by itself establish closure.".to_string(),
            "A declared indirect-jump profile is admitted only after validating its exact source instructions, loop bound, cursor update, table entries, and targets. It extends the executable closure without pretending that the generic bounded-jump recognizer derived a selector relationship it cannot prove.".to_string(),
            "The declared source-region population is a reviewed census of the currently admitted analysis families, not a complete inventory of every source-disc graphics region.".to_string(),
            "The declared pointer-run census is investigation-only. It separately reports decoded table targets, aligned full-word values in the loaded image, and addresses derived by the declared-entrypoint value-flow analysis. Raw address-shaped words and an absence of reachable derived references do not establish a consumer, dormant status, or write authority. An exhausted seed leaves the flow open unless an explicit image-specific profile validates its exact non-address scalar load and frontier; raw and unprofiled exhaustion counts remain visible.".to_string(),
            "A direct JAL to the profiled disc loader is listed as a candidate even outside the current entrypoint closure; its entrypoint-reachable flag is independent from catalog-index resolution. Full-image candidates can include instruction-shaped data. Catalog resolution covers local literals, values recovered by shared address flow, literal callers of explicitly validated forwarding wrappers, explicitly validated finite index sets, and finite selector-table domains assembled from classified writers. A static owner is admitted only with an exact loader-call, destination-buffer, and complete direct-caller profile. Selector-table values are read from the exact source image and are never inferred from neighboring catalog entries.".to_string(),
            "The disc-loader selector-state census includes only memory accesses recovered by the declared-entrypoint value-flow closure. Seventeen explicitly profiled descending fills and three bounded configuration-copy loops are restricted to exact finite footprints only after validating literal setup, preserved registers, loop bounds, backedges and delay slots, and the complete reachable direct-entry set. One instruction can bind several explicit state ranges or several possible addresses, so profile-bound candidate count and unique instruction-site counts are separate. All observed reachable writes are classified, but runtime-source classifications do not invent upstream values and the population is not complete while roots, indirect entries, and value-flow budgets remain open.".to_string(),
            "The selector upstream-state census is separate from the final selector-state denominator. Its explicit configuration, counter-limit, counter, table-group, and ordinary-selector source ranges have their own read/write counts. Reused bounded-copy evidence constrains source loads and destinations, while explicit PLSEL1, PLSEL3, PLSEL4, and main-executable loop profiles remove only value-flow spill outside their validated finite footprints. Every observed reachable upstream writer has typed value-source evidence, but runtime-source classifications intentionally stop where the source value remains dynamic.".to_string(),
            "The profiled selector-table domain is the union of classified writers visible in the declared-entrypoint analysis, not a route-order proof or a whole-program producer census. Identity-preserving runtime sources are propagated byte-for-byte into separately classified upstream state; transformed and dynamically addressed sources stop explicitly. The eight custom-record calls have separate explicit function profiles covering their record-index state, forty-byte record addressing, conditional or fixed record fields, selector transforms, table loads, and loader calls. Their runtime record-byte values remain unresolved rather than being inferred from address shape. A table call is promoted only when its complete declared selector domain is finite, and missing external roots, indirect entries, or exhausted flows can still widen the real runtime domain.".to_string(),
            "The declared renderer-call census scans every aligned source word for direct JAL instructions to the profiled renderer and requires exact equality with its literal call-site profile. This closes the direct-call denominator only. Indirect calls remain outside that denominator, and a classified call site does not by itself close a runtime-selected argument domain or prove execution on a captured route.".to_string(),
            "The report is investigation evidence under work/. Product builders consume reviewed explicit specifications and do not read this output.".to_string(),
        ],
    };
    write_report(&config.output, &report)?;
    Ok(report)
}

fn validate_consumer_profiles(
    source_regions: &[StaticConsumerSourceRegionProfile],
    sinks: &[StaticConsumerSinkProfile],
    edges: &[StaticConsumerEdgeProfile],
) -> Result<()> {
    let mut source_region_keys = BTreeSet::new();
    for region in source_regions {
        ensure!(
            source_region_keys.insert((region.source_record_path, region.source_region_id)),
            "duplicate static consumer source region {} {}",
            region.source_record_path,
            region.source_region_id
        );
        ensure!(
            region
                .source_member_indices
                .iter()
                .copied()
                .collect::<BTreeSet<_>>()
                .len()
                == region.source_member_indices.len(),
            "static consumer source region {} {} repeats a source member index",
            region.source_record_path,
            region.source_region_id
        );
    }
    let mut sink_ids = BTreeSet::new();
    for sink in sinks {
        ensure!(
            sink_ids.insert(sink.id),
            "duplicate static consumer sink ID {}",
            sink.id
        );
        ensure!(
            std::iter::once(sink.runtime_address)
                .chain(sink.additional_runtime_addresses.iter().copied())
                .collect::<BTreeSet<_>>()
                .len()
                == sink.additional_runtime_addresses.len() + 1,
            "static consumer sink {} repeats a runtime address",
            sink.id
        );
    }
    let mut edge_ids = BTreeSet::new();
    for edge in edges {
        ensure!(
            edge_ids.insert(edge.id),
            "duplicate static consumer edge ID {}",
            edge.id
        );
        ensure!(
            sink_ids.contains(edge.sink_id),
            "static consumer edge {} references unknown sink {}",
            edge.id,
            edge.sink_id
        );
        let source_region = source_regions
            .iter()
            .find(|region| {
                region.source_record_path == edge.source_record_path
                    && region.source_region_id == edge.source_region_id
            })
            .with_context(|| {
                format!(
                    "static consumer edge {} references unknown source region {} {}",
                    edge.id, edge.source_record_path, edge.source_region_id
                )
            })?;
        ensure!(
            source_region.source_member_indices == edge.source_member_indices,
            "static consumer edge {} source member indices disagree with source region",
            edge.id
        );
        ensure!(
            edge.source_member_indices
                .iter()
                .copied()
                .collect::<BTreeSet<_>>()
                .len()
                == edge.source_member_indices.len(),
            "static consumer edge {} repeats a source member index",
            edge.id
        );
    }
    Ok(())
}

fn declared_source_region_audits(
    profiles: &[StaticConsumerSourceRegionProfile],
    edges: &[StaticConsumerEdgeAudit],
    dormant_candidate_edges: &[StaticConsumerEdgeAudit],
) -> Vec<StaticConsumerSourceRegionAudit> {
    profiles
        .iter()
        .map(|profile| {
            let matching_edges = edges
                .iter()
                .filter(|edge| {
                    edge.source_record_path == profile.source_record_path
                        && edge.source_region_id == profile.source_region_id
                })
                .collect::<Vec<_>>();
            let reachable_edge_count = matching_edges
                .iter()
                .filter(|edge| edge.sink_entrypoint_reachable)
                .count();
            let matching_dormant_candidate_edges = dormant_candidate_edges
                .iter()
                .filter(|edge| {
                    edge.source_record_path == profile.source_record_path
                        && edge.source_region_id == profile.source_region_id
                })
                .collect::<Vec<_>>();
            let (consumer_binding_status, evidence) = if matching_edges.is_empty() {
                (
                    "consumer_unbound",
                    profile
                        .unbound_consumer_reason
                        .unwrap_or("consumer_edge_not_yet_declared"),
                )
            } else if reachable_edge_count == matching_edges.len() {
                (
                    "consumer_bound_all_sinks_entrypoint_reachable",
                    "source_region_declared_and_all_consumer_sinks_entrypoint_reachable",
                )
            } else if reachable_edge_count == 0 {
                (
                    "consumer_bound_no_sink_entrypoint_reachable",
                    "source_region_declared_but_no_consumer_sink_entrypoint_reachable",
                )
            } else {
                (
                    "consumer_bound_some_sinks_entrypoint_unresolved",
                    "source_region_declared_but_some_consumer_sinks_are_not_entrypoint_reachable",
                )
            };
            StaticConsumerSourceRegionAudit {
                source_record_path: profile.source_record_path.to_string(),
                source_region_id: profile.source_region_id.to_string(),
                source_member_indices: profile.source_member_indices.to_vec(),
                source_catalog_index: profile.source_catalog_index,
                consumer_binding_status: consumer_binding_status.to_string(),
                declared_edge_ids: matching_edges.iter().map(|edge| edge.id.clone()).collect(),
                dormant_candidate_edge_ids: matching_dormant_candidate_edges
                    .iter()
                    .map(|edge| edge.id.clone())
                    .collect(),
                evidence: evidence.to_string(),
            }
        })
        .collect()
}

fn declared_edge_audits(
    images: &[LoadedImageConsumerAudit],
    disposition: StaticConsumerSinkDisposition,
) -> Result<Vec<StaticConsumerEdgeAudit>> {
    STATIC_CONSUMER_EDGE_PROFILES
        .iter()
        .filter(|profile| {
            STATIC_CONSUMER_SINK_PROFILES
                .iter()
                .find(|sink| sink.id == profile.sink_id)
                .is_some_and(|sink| sink.disposition == disposition)
        })
        .map(|profile| {
            let sink = images
                .iter()
                .flat_map(|image| {
                    let sinks = match disposition {
                        StaticConsumerSinkDisposition::ActiveConsumer => {
                            &image.declared_semantic_sinks
                        }
                        StaticConsumerSinkDisposition::DormantCandidate => {
                            &image.declared_dormant_candidate_sinks
                        }
                    };
                    sinks.iter().map(move |sink| (image.path.as_str(), sink))
                })
                .find(|(_, sink)| sink.id == profile.sink_id)
                .with_context(|| {
                    format!(
                        "declared consumer edge {} has no sink {}",
                        profile.id, profile.sink_id
                    )
                })?;
            Ok(StaticConsumerEdgeAudit {
                id: profile.id.to_string(),
                source_record_path: profile.source_record_path.to_string(),
                source_region_id: profile.source_region_id.to_string(),
                source_member_indices: profile.source_member_indices.to_vec(),
                relationship: profile.relationship.to_string(),
                sink_id: profile.sink_id.to_string(),
                consumer_image_path: sink.0.to_string(),
                consumer_runtime_addresses: sink
                    .1
                    .sites
                    .iter()
                    .map(|site| site.runtime_address.clone())
                    .collect(),
                sink_entrypoint_reachable: sink.1.entrypoint_reachable,
                evidence: match (disposition, sink.1.entrypoint_reachable) {
                    (StaticConsumerSinkDisposition::ActiveConsumer, true) => {
                        "profile_declared_and_sink_entrypoint_reachable"
                    }
                    (StaticConsumerSinkDisposition::ActiveConsumer, false) => {
                        "profile_declared_but_sink_not_entrypoint_reachable"
                    }
                    (StaticConsumerSinkDisposition::DormantCandidate, true) => {
                        "dormant_candidate_unexpectedly_entrypoint_reachable"
                    }
                    (StaticConsumerSinkDisposition::DormantCandidate, false) => {
                        "dormant_candidate_outside_declared_entrypoint_closure"
                    }
                }
                .to_string(),
                runtime_confirmation_required: true,
            })
        })
        .collect()
}

fn analyze_loaded_image(
    image: &LoadedImage,
    value_flow_state_budget: usize,
    pointer_run_value_flow_state_budget: usize,
    source_catalog: &SourceCatalog,
) -> Result<LoadedImageConsumerAudit> {
    let loaded_bytes_sha256 = sha256_bytes(&image.data);
    let runtime_base = image.runtime_base.map(hex_address);
    let declared_entrypoints = image
        .entrypoints
        .iter()
        .map(|entrypoint| DeclaredEntrypointAudit {
            role: entrypoint.role.to_string(),
            source_reference_kind: entrypoint.source_reference_kind.to_string(),
            source_reference_offset: hex_offset(entrypoint.source_reference_offset),
            runtime_address: hex_address(entrypoint.runtime_address),
        })
        .collect();
    let Some(analysis) = analyze_loaded_program(image, value_flow_state_budget)? else {
        let loader_candidates = disc_record_load_call_candidates(
            image,
            &BTreeSet::new(),
            &[],
            source_catalog,
            STATIC_CONSUMER_SOURCE_REGION_PROFILES,
        )?;
        let pointer_runs = audit_declared_pointer_runs(
            image,
            None,
            pointer_run_value_flow_state_budget,
            0,
            STATIC_POINTER_RUN_PROFILES,
            STATIC_POINTER_RUN_NON_ADDRESS_FLOW_PROFILES,
        )?;
        let renderer_call_censuses = audit_declared_renderer_calls(image, None)?;
        return Ok(empty_image_report(
            image,
            loaded_bytes_sha256,
            declared_entrypoints,
            loader_candidates,
            pointer_runs,
            renderer_call_censuses,
            if image.runtime_base.is_none() {
                "no_runtime_base"
            } else {
                "entrypoint_unresolved"
            },
        ));
    };
    let loader_candidates = disc_record_load_call_candidates(
        image,
        &analysis.closure.scan.instruction_offsets,
        &analysis.value_flow.resolved_direct_call_arguments,
        source_catalog,
        STATIC_CONSUMER_SOURCE_REGION_PROFILES,
    )?;
    let selector_state_accesses = loader_selector_state_accesses(
        image,
        &analysis.value_flow,
        &analysis.closure.scan.instruction_offsets,
    )?;
    let selector_upstream_state_accesses = loader_selector_upstream_state_accesses(
        image,
        &analysis.value_flow,
        &analysis.closure.scan.instruction_offsets,
    )?;
    let pointer_run_analysis = if STATIC_POINTER_RUN_PROFILES
        .iter()
        .any(|profile| profile.image_path == image.path)
    {
        analyze_loaded_program(image, pointer_run_value_flow_state_budget)?
    } else {
        None
    };
    let pointer_run_analysis = pointer_run_analysis.as_ref().unwrap_or(&analysis);
    let pointer_run_unresolved_indirect_jump_count = pointer_run_analysis
        .closure
        .scan
        .unresolved_indirect_transfers
        .values()
        .filter(|transfer| transfer.kind == UnresolvedRegisterTransferKind::Jump)
        .count();
    let pointer_runs = audit_declared_pointer_runs(
        image,
        Some(&pointer_run_analysis.value_flow),
        pointer_run_value_flow_state_budget,
        pointer_run_unresolved_indirect_jump_count,
        STATIC_POINTER_RUN_PROFILES,
        STATIC_POINTER_RUN_NON_ADDRESS_FLOW_PROFILES,
    )?;
    let renderer_call_censuses =
        audit_declared_renderer_calls(image, Some(&analysis.closure.scan.instruction_offsets))?;
    Ok(completed_image_report(
        image,
        loaded_bytes_sha256,
        runtime_base,
        declared_entrypoints,
        &analysis.executable_domain,
        analysis.refinement_pass_count,
        &analysis.closure,
        &analysis.value_flow,
        loader_candidates,
        selector_state_accesses,
        selector_upstream_state_accesses,
        pointer_runs,
        renderer_call_censuses,
        analysis.declared_indirect_jumps.clone(),
    ))
}

#[allow(clippy::too_many_arguments)]
fn completed_image_report(
    image: &LoadedImage,
    loaded_bytes_sha256: String,
    runtime_base: Option<String>,
    declared_entrypoints: Vec<DeclaredEntrypointAudit>,
    executable_domain: &ExecutableDomain,
    refinement_pass_count: usize,
    closure: &crate::psx_static_analysis::reachable_control_flow::ReachableInstructionClosure,
    value_flow: &DerivedAddressScan,
    loader_candidates: Vec<StaticDiscRecordLoadAudit>,
    selector_state_accesses: Vec<StaticProfiledStateAccessAudit>,
    selector_upstream_state_accesses: Vec<StaticProfiledStateAccessAudit>,
    pointer_runs: Vec<super::model::StaticPointerRunAudit>,
    renderer_call_censuses: Vec<super::model::StaticRendererCallCensusAudit>,
    declared_indirect_jumps: Vec<super::model::StaticDeclaredIndirectJumpAudit>,
) -> LoadedImageConsumerAudit {
    let unresolved_indirect_jump_count = closure
        .scan
        .unresolved_indirect_transfers
        .values()
        .filter(|transfer| transfer.kind == UnresolvedRegisterTransferKind::Jump)
        .count();
    let unresolved_indirect_call_count = closure
        .scan
        .unresolved_indirect_transfers
        .values()
        .filter(|transfer| transfer.kind == UnresolvedRegisterTransferKind::LinkedCall)
        .count();
    LoadedImageConsumerAudit {
        path: image.path.clone(),
        loaded_bytes_sha256,
        loaded_byte_count: image.data.len(),
        runtime_base,
        declared_entrypoints,
        analysis_status: "analyzed".to_string(),
        executable_domain_source: Some("declared-entrypoint reachable closure".to_string()),
        executable_ranges: executable_domain
            .instruction_ranges()
            .iter()
            .map(|range| ExecutableRangeAudit {
                start_offset: hex_offset(range.start),
                end_offset: hex_offset(range.end),
            })
            .collect(),
        reachability_refinement_pass_count: refinement_pass_count,
        reachable_instruction_count: closure.scan.instruction_offsets.len(),
        reachable_lui_instruction_count: closure.scan.lui_instruction_offsets.len(),
        decode_failure_count: closure.scan.decode_failure_offsets.len(),
        decode_failure_offsets: closure
            .scan
            .decode_failure_offsets
            .iter()
            .copied()
            .map(hex_offset)
            .collect(),
        resolved_register_transfer_count: value_flow.resolved_register_transfers.len(),
        resolved_direct_call_argument_count: value_flow.resolved_direct_call_arguments.len(),
        value_flow_seed_count: value_flow.seed_count,
        value_flow_instruction_state_count: value_flow.instruction_state_count,
        value_flow_budget_exhausted_seed_count: value_flow.budget_exhausted_seed_count,
        value_flow_budget_exhausted_seed_offsets: value_flow
            .budget_exhausted_seed_offsets
            .iter()
            .copied()
            .map(hex_offset)
            .collect(),
        unresolved_indirect_jump_count,
        unresolved_indirect_jump_offsets: unresolved_transfer_offsets(
            closure,
            UnresolvedRegisterTransferKind::Jump,
        ),
        unresolved_indirect_call_count,
        unresolved_indirect_call_offsets: unresolved_transfer_offsets(
            closure,
            UnresolvedRegisterTransferKind::LinkedCall,
        ),
        declared_indirect_jumps,
        bounded_jump_table_count: closure.bounded_jump_tables.len(),
        bounded_jump_table_entry_count: closure
            .bounded_jump_tables
            .values()
            .map(|table| table.targets.len())
            .sum(),
        outside_loaded_image_transfer_target_count: closure
            .scan
            .outside_image_transfer_targets
            .len(),
        outside_loaded_image_transfer_targets: closure
            .scan
            .outside_image_transfer_targets
            .iter()
            .copied()
            .map(hex_address)
            .collect(),
        disc_record_load_call_candidates: loader_candidates,
        disc_record_loader_selector_state_accesses: selector_state_accesses,
        disc_record_loader_selector_upstream_state_accesses: selector_upstream_state_accesses,
        declared_pointer_runs: pointer_runs,
        declared_renderer_call_censuses: renderer_call_censuses,
        declared_semantic_sinks: declared_sink_audits(
            image,
            Some(&closure.scan.instruction_offsets),
            StaticConsumerSinkDisposition::ActiveConsumer,
        ),
        declared_dormant_candidate_sinks: declared_sink_audits(
            image,
            Some(&closure.scan.instruction_offsets),
            StaticConsumerSinkDisposition::DormantCandidate,
        ),
    }
}

fn empty_image_report(
    image: &LoadedImage,
    loaded_bytes_sha256: String,
    declared_entrypoints: Vec<DeclaredEntrypointAudit>,
    loader_candidates: Vec<StaticDiscRecordLoadAudit>,
    pointer_runs: Vec<super::model::StaticPointerRunAudit>,
    renderer_call_censuses: Vec<super::model::StaticRendererCallCensusAudit>,
    analysis_status: &str,
) -> LoadedImageConsumerAudit {
    LoadedImageConsumerAudit {
        path: image.path.clone(),
        loaded_bytes_sha256,
        loaded_byte_count: image.data.len(),
        runtime_base: image.runtime_base.map(hex_address),
        declared_entrypoints,
        analysis_status: analysis_status.to_string(),
        executable_domain_source: None,
        executable_ranges: Vec::new(),
        reachability_refinement_pass_count: 0,
        reachable_instruction_count: 0,
        reachable_lui_instruction_count: 0,
        decode_failure_count: 0,
        decode_failure_offsets: Vec::new(),
        resolved_register_transfer_count: 0,
        resolved_direct_call_argument_count: 0,
        value_flow_seed_count: 0,
        value_flow_instruction_state_count: 0,
        value_flow_budget_exhausted_seed_count: 0,
        value_flow_budget_exhausted_seed_offsets: Vec::new(),
        unresolved_indirect_jump_count: 0,
        unresolved_indirect_jump_offsets: Vec::new(),
        unresolved_indirect_call_count: 0,
        unresolved_indirect_call_offsets: Vec::new(),
        declared_indirect_jumps: Vec::new(),
        bounded_jump_table_count: 0,
        bounded_jump_table_entry_count: 0,
        outside_loaded_image_transfer_target_count: 0,
        outside_loaded_image_transfer_targets: Vec::new(),
        disc_record_load_call_candidates: loader_candidates,
        disc_record_loader_selector_state_accesses: Vec::new(),
        disc_record_loader_selector_upstream_state_accesses: Vec::new(),
        declared_pointer_runs: pointer_runs,
        declared_renderer_call_censuses: renderer_call_censuses,
        declared_semantic_sinks: declared_sink_audits(
            image,
            None,
            StaticConsumerSinkDisposition::ActiveConsumer,
        ),
        declared_dormant_candidate_sinks: declared_sink_audits(
            image,
            None,
            StaticConsumerSinkDisposition::DormantCandidate,
        ),
    }
}

fn declared_sink_audits(
    image: &LoadedImage,
    reachable_instruction_offsets: Option<&std::collections::BTreeSet<usize>>,
    disposition: StaticConsumerSinkDisposition,
) -> Vec<StaticConsumerSinkAudit> {
    STATIC_CONSUMER_SINK_PROFILES
        .iter()
        .filter(|profile| profile.image_path == image.path && profile.disposition == disposition)
        .map(|profile| declared_sink_audit(profile, image, reachable_instruction_offsets))
        .collect()
}

fn declared_sink_audit(
    profile: &StaticConsumerSinkProfile,
    image: &LoadedImage,
    reachable_instruction_offsets: Option<&BTreeSet<usize>>,
) -> StaticConsumerSinkAudit {
    let sites = std::iter::once(profile.runtime_address)
        .chain(profile.additional_runtime_addresses.iter().copied())
        .map(|runtime_address| {
            let instruction_offset = image.runtime_base.and_then(|base| {
                runtime_address
                    .checked_sub(base)
                    .and_then(|offset| usize::try_from(offset).ok())
            });
            let entrypoint_reachable = instruction_offset.is_some_and(|offset| {
                reachable_instruction_offsets.is_some_and(|reachable| reachable.contains(&offset))
            });
            StaticConsumerSinkSiteAudit {
                runtime_address: hex_address(runtime_address),
                instruction_offset: instruction_offset.map(hex_offset),
                entrypoint_reachable,
            }
        })
        .collect::<Vec<_>>();
    let entrypoint_reachable = sites.iter().all(|site| site.entrypoint_reachable);
    StaticConsumerSinkAudit {
        id: profile.id.to_string(),
        kind: profile.kind.to_string(),
        sites,
        entrypoint_reachable,
        evidence: if entrypoint_reachable {
            "all_profile_declared_sites_entrypoint_reachable"
        } else {
            "one_or_more_profile_declared_sites_not_entrypoint_reachable"
        }
        .to_string(),
    }
}

fn unresolved_transfer_offsets(
    closure: &crate::psx_static_analysis::reachable_control_flow::ReachableInstructionClosure,
    kind: UnresolvedRegisterTransferKind,
) -> Vec<String> {
    closure
        .scan
        .unresolved_indirect_transfers
        .iter()
        .filter_map(|(&offset, transfer)| (transfer.kind == kind).then_some(hex_offset(offset)))
        .collect()
}

fn count_status(images: &[LoadedImageConsumerAudit], status: &str) -> usize {
    images
        .iter()
        .filter(|image| image.analysis_status == status)
        .count()
}

fn sum(
    images: &[LoadedImageConsumerAudit],
    value: impl Fn(&LoadedImageConsumerAudit) -> usize,
) -> usize {
    images.iter().map(value).sum()
}

fn selector_state_unique_access_site_count(
    images: &[LoadedImageConsumerAudit],
    operation: Option<&str>,
) -> usize {
    images
        .iter()
        .flat_map(|image| {
            image
                .disc_record_loader_selector_state_accesses
                .iter()
                .filter(move |access| operation.is_none_or(|value| access.operation == value))
                .map(|access| {
                    (
                        image.path.as_str(),
                        access.instruction_runtime_address.as_str(),
                        access.operation.as_str(),
                        access.width_bytes,
                    )
                })
        })
        .collect::<BTreeSet<_>>()
        .len()
}

fn selector_upstream_state_unique_access_site_count(
    images: &[LoadedImageConsumerAudit],
    operation: Option<&str>,
) -> usize {
    images
        .iter()
        .flat_map(|image| {
            image
                .disc_record_loader_selector_upstream_state_accesses
                .iter()
                .filter(move |access| operation.is_none_or(|value| access.operation == value))
                .map(|access| {
                    (
                        image.path.as_str(),
                        access.instruction_runtime_address.as_str(),
                        access.operation.as_str(),
                        access.width_bytes,
                    )
                })
        })
        .collect::<BTreeSet<_>>()
        .len()
}

fn selector_upstream_state_writer_site_count(
    images: &[LoadedImageConsumerAudit],
    classification: Option<&str>,
) -> usize {
    images
        .iter()
        .flat_map(|image| {
            image
                .disc_record_loader_selector_upstream_state_accesses
                .iter()
                .filter(|access| access.operation == "write")
                .filter(move |access| match classification {
                    Some("classified") => access.writer_evidence.is_some(),
                    Some(value_resolution) => access
                        .writer_evidence
                        .as_ref()
                        .is_some_and(|evidence| evidence.value_resolution == value_resolution),
                    None => access.writer_evidence.is_none(),
                })
                .map(move |access| (image.path.as_str(), access.instruction_offset.as_str()))
        })
        .collect::<BTreeSet<_>>()
        .len()
}

fn selector_state_writer_site_count(
    images: &[LoadedImageConsumerAudit],
    classification: Option<&str>,
) -> usize {
    images
        .iter()
        .flat_map(|image| {
            image
                .disc_record_loader_selector_state_accesses
                .iter()
                .filter(|access| access.operation == "write")
                .filter(move |access| match classification {
                    Some("classified") => access.writer_evidence.is_some(),
                    Some(value_resolution) => access
                        .writer_evidence
                        .as_ref()
                        .is_some_and(|evidence| evidence.value_resolution == value_resolution),
                    None => access.writer_evidence.is_none(),
                })
                .map(move |access| (image.path.as_str(), access.instruction_offset.as_str()))
        })
        .collect::<BTreeSet<_>>()
        .len()
}

fn write_report(path: &Path, report: &StaticConsumerAuditReport) -> Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let mut bytes = serde_json::to_vec_pretty(report)?;
    bytes.push(b'\n');
    std::fs::write(path, bytes).with_context(|| format!("failed to write {}", path.display()))
}

fn hex_address(value: u32) -> String {
    format!("0x{value:08x}")
}

fn hex_offset(value: usize) -> String {
    format!("0x{value:x}")
}

#[cfg(test)]
#[path = "audit_tests.rs"]
mod tests;
