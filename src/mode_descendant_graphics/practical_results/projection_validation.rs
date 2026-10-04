//! Bounded source validation for adopted practical-result projection catalogs.
//!
//! This facade coordinates independently validated source, residency, palette,
//! descriptor, consumer, and semantic-link authorities. It intentionally does
//! not rediscover consumers or run broad disassembly.

#[path = "projection_validation/clut.rs"]
mod clut;
#[path = "projection_validation/consumers/mod.rs"]
mod consumers;
#[path = "projection_validation/descriptor.rs"]
mod descriptor;
#[path = "projection_validation/footprint/mod.rs"]
mod footprint;
#[path = "projection_validation/geometry.rs"]
mod geometry;
#[path = "projection_validation/opaque_pointer_runs.rs"]
mod opaque_pointer_runs;
#[path = "projection_validation/projection_links.rs"]
mod projection_links;
#[path = "projection_validation/residency.rs"]
mod residency;
#[path = "projection_validation/source_atlas_domain.rs"]
mod source_atlas_domain;
#[path = "projection_validation/source_evidence.rs"]
mod source_evidence;
#[path = "projection_validation/source_footprint_report.rs"]
mod source_footprint_report;
#[path = "projection_validation/source_read_assessment.rs"]
mod source_read_assessment;

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};

use crate::mode_descendant_graphics::source::ModeDescendantSourceRecord;
use crate::pipeline::sha256_bytes;

use super::assets::{
    validate_siken20_indexed_result_consumer_path, validate_siken20_indexed_result_texture_lifetime,
};
use super::model::{
    PracticalResultAssets, PracticalResultDevelopmentStatus,
    PracticalResultSourceAtlasRewriteGateBuild, PracticalResultSourceReadFootprintBuild,
    PracticalResultStrategy,
};
use super::projection_model::*;
use super::source_ownership::PracticalResultSourceOwnership;

use clut::validate_cluts;
use consumers::{
    ProjectionValidationContext, derive_alternative_source_footprint, projection_parts,
    validate_projection_mechanism,
};
use descriptor::validate_descriptors;
use opaque_pointer_runs::validate_opaque_pointer_runs;
use projection_links::{
    ProjectionLinkAuthorities, projection_has_nested_unresolved,
    validate_action_consumer_occurrence_ledger, validate_action_consumer_occurrence_projection,
    validate_projection_status, validate_repeated_semantic_projection,
};
use residency::validate_residencies;
use source_atlas_domain::{
    validate_source_atlas_alternative_projection_link, validate_source_atlas_clut_links,
    validate_source_atlas_descriptor_links, validate_source_atlas_domains,
    validate_source_atlas_projection_link, validate_source_atlas_residency_links,
};
use source_evidence::{ValidationCounts, source_for_path, validate_declared_overlay_base};
use source_footprint_report::build_source_read_footprint_reports;
use source_read_assessment::{
    SourceAtlasConsumerProjectionDenominator, assess_source_projection_reads,
};

#[derive(Debug)]
pub(in crate::mode_descendant_graphics) struct PracticalResultProjectionValidation {
    pub(super) source_atlas_domain_count: usize,
    pub(super) domain_bound_physical_region_count: usize,
    pub(super) physical_region_without_declared_source_atlas_domain_count: usize,
    pub(super) domain_bound_protected_region_count: usize,
    pub(super) domain_bound_source_glyph_bank_count: usize,
    pub(super) domain_bound_vram_residency_count: usize,
    pub(super) domain_bound_clut_binding_count: usize,
    pub(super) external_clut_producer_family_count: usize,
    pub(super) external_clut_palette_family_sha256s: Vec<String>,
    pub(super) unresolved_external_clut_binding_count: usize,
    pub(super) domain_bound_consumer_descriptor_count: usize,
    pub(super) domain_bound_consumer_projection_count: usize,
    pub(super) alternative_consumer_source_binding_count: usize,
    pub(super) assessed_source_projection_read_count: usize,
    pub(super) unassessed_source_projection_read_count: usize,
    pub(super) declared_read_set_complete_source_projection_count: usize,
    pub(super) consumer_reachability_closed_source_projection_count: usize,
    pub(super) consumer_reachability_unassessed_source_projection_count: usize,
    pub(super) consumer_reachability_unresolved_source_projection_count: usize,
    pub(super) consumer_reachability_dormant_source_projection_count: usize,
    pub(super) source_projection_read_assessment_coverage_complete: bool,
    pub(super) source_atlas_in_place_rewrite_gate_open_domain_count: usize,
    pub(super) source_atlas_in_place_rewrite_gate_blocked_domain_count: usize,
    pub(super) source_atlas_in_place_rewrite_gates: Vec<PracticalResultSourceAtlasRewriteGateBuild>,
    pub(super) source_atlas_evidence_joins_complete: bool,
    pub(super) indexed_result_selected_member_count: usize,
    pub(super) indexed_result_known_post_upload_descriptor_aliases: Vec<u16>,
    pub(super) indexed_result_direct_descriptor_render_call_offsets: Vec<String>,
    pub(super) indexed_result_known_post_upload_draw_path_validated: bool,
    pub(super) indexed_result_delegated_overlay_callback_index: usize,
    pub(super) indexed_result_delegated_overlay_dispatch_offset: String,
    pub(super) indexed_result_tim_upload_call_offsets: Vec<String>,
    pub(super) indexed_result_validated_result_title_member_count: usize,
    pub(super) indexed_result_texture_lifetime_validated: bool,
    pub(super) declared_opaque_pointer_run_count: usize,
    pub(super) source_atlas_excluded_opaque_pointer_run_count: usize,
    pub(super) unresolved_opaque_pointer_run_count: usize,
    pub(super) vram_residency_count: usize,
    pub(super) resolved_clut_binding_count: usize,
    pub(super) consumer_descriptor_count: usize,
    pub(super) consumer_projection_count: usize,
    pub(super) declared_action_consumer_occurrence_count: usize,
    pub(super) validated_action_consumer_occurrence_count: usize,
    pub(super) action_consumer_occurrence_coverage_complete: bool,
    pub(super) semantic_bound_consumer_projection_count: usize,
    pub(super) matched_authored_glyph_sequence_unit_count: usize,
    pub(super) authored_glyph_sequence_projection_coverage_complete: bool,
    pub(super) residency_bound_consumer_projection_count: usize,
    pub(super) resolved_clut_bound_consumer_projection_count: usize,
    pub(super) unresolved_external_clut_projection_count: usize,
    pub(super) hashed_span_count: usize,
    pub(super) runtime_address_count: usize,
    pub(super) pointer_alias_count: usize,
    pub(super) source_read_footprints: Vec<PracticalResultSourceReadFootprintBuild>,
    pub(super) complete: bool,
}

pub(super) fn validate_practical_result_projections(
    assets: &PracticalResultAssets,
    ownership: &PracticalResultSourceOwnership,
    sources: &[ModeDescendantSourceRecord],
) -> Result<PracticalResultProjectionValidation> {
    ensure!(
        matches!(
            assets.vram_residency_catalog.kind,
            PracticalResultVramResidencyCatalogKind::JusticeGakuen2PracticalResultVramResidencyCatalog
        ) && matches!(
            assets.clut_binding_catalog.kind,
            PracticalResultClutBindingCatalogKind::JusticeGakuen2PracticalResultClutBindingCatalog
        ) && matches!(
            assets.action_consumer_occurrence_catalog.kind,
            PracticalResultActionConsumerOccurrenceCatalogKind::JusticeGakuen2PracticalResultActionConsumerOccurrenceCatalog
        ) && matches!(
            assets.action_consumer_occurrence_catalog.scope,
            PracticalResultActionConsumerOccurrenceCatalogScope::CatalogDescriptorActionConsumersOnly
        ) && matches!(
            assets.consumer_projection_catalog.kind,
            PracticalResultConsumerProjectionCatalogKind::JusticeGakuen2PracticalResultConsumerProjectionCatalog
        ),
        "unsupported practical-result projection catalog kind"
    );
    let source_atlas_domains = validate_source_atlas_domains(assets, sources)?;
    let indexed_result_consumer = validate_siken20_indexed_result_consumer_path(
        &source_for_path(sources, "DAT1/SIKEN2.BIN")?.decoded,
        "SIKEN20 indexed-result producer and known post-upload draw path",
    )?;
    let indexed_result_texture_lifetime = validate_siken20_indexed_result_texture_lifetime(
        &source_for_path(sources, "DAT2/SIKEN20.BIZ")?.decoded,
        &source_for_path(sources, "DAT2/SIKENKK.BIZ")?.decoded,
    )?;
    ensure!(
        indexed_result_texture_lifetime.validated_siken20_member_count
            == indexed_result_consumer.selected_member_count,
        "SIKEN20 selected-member and texture-lifetime denominators differ"
    );
    let residencies = validate_residencies(assets, sources)?;
    validate_source_atlas_residency_links(&source_atlas_domains, &residencies)?;
    let cluts = validate_cluts(assets, &residencies, sources)?;
    validate_source_atlas_clut_links(&source_atlas_domains, &cluts)?;
    let mut counts = ValidationCounts::default();
    let opaque_pointer_runs = validate_opaque_pointer_runs(assets, sources, &mut counts)?;
    let descriptors = validate_descriptors(assets, sources, &mut counts)?;
    validate_source_atlas_descriptor_links(&source_atlas_domains, &residencies, &descriptors)?;

    let link_authorities = ProjectionLinkAuthorities::from_assets(assets, ownership);
    let action_consumer_occurrences = validate_action_consumer_occurrence_ledger(
        assets,
        sources,
        &descriptors,
        &link_authorities,
    )?;

    let mut projection_ids = BTreeSet::new();
    let mut source_projection_ids = BTreeSet::new();
    let mut projection_domains = BTreeMap::new();
    let mut projected_action_consumer_occurrence_ids = BTreeSet::new();
    let mut semantic_bound_projection_count = 0usize;
    let mut matched_authored_entry_ids = BTreeSet::new();
    let mut unresolved_external_clut_projection_count = 0usize;
    let mut residency_bound_consumer_projection_count = 0usize;
    let mut resolved_clut_bound_consumer_projection_count = 0usize;
    let mut semantic_projection_signatures = BTreeMap::new();
    let mut source_projection_read_footprints = Vec::new();
    let mut alternative_consumer_source_binding_count = 0usize;
    let mut siken20_bound_logical_projection_ids = BTreeMap::<&str, BTreeSet<&str>>::new();
    let mut all_projection_semantics_complete = true;
    let mut has_nested_unresolved = residencies
        .values()
        .any(|residency| !residency.unresolved_reason.trim().is_empty())
        || cluts.values().any(|clut| {
            matches!(
                clut.binding.evidence_status(),
                PracticalResultClutBindingEvidenceStatus::Unresolved
            ) || clut.binding.unresolved_reason().is_some()
        });

    ensure!(
        !assets.consumer_projection_catalog.projections.is_empty(),
        "practical-result consumer projection catalog is empty"
    );
    for mechanism in &assets.consumer_projection_catalog.projections {
        let (source, binding) = projection_parts(mechanism)?;
        ensure!(
            !source.id.is_empty()
                && !source.consumer_id.is_empty()
                && projection_ids.insert(source.id.as_str())
                && source_projection_ids.insert(source.id.as_str()),
            "duplicate or empty practical-result projection identity"
        );
        let overlay = source_for_path(sources, &source.overlay_path)?;
        ensure!(
            overlay.decoded.len() == source.overlay_size
                && sha256_bytes(&overlay.decoded) == source.overlay_sha256,
            "practical-result projection {} overlay identity changed",
            source.id
        );
        let runtime_base = validate_declared_overlay_base(
            &source.overlay_path,
            &source.runtime_base,
            "projection",
        )?;
        let source_atlas = source_atlas_domains
            .by_id
            .get(binding.source_atlas_domain_id.as_str())
            .with_context(|| {
                format!(
                    "projection {} names unknown source-atlas domain {}",
                    source.id, binding.source_atlas_domain_id
                )
            })?;
        let residency = residencies
            .get(binding.residency_id.as_str())
            .with_context(|| {
                format!(
                    "projection {} names unknown residency {}",
                    source.id, binding.residency_id
                )
            })?;
        residency_bound_consumer_projection_count += 1;
        let clut = cluts
            .get(binding.clut_binding_id.as_str())
            .with_context(|| {
                format!(
                    "projection {} names unknown CLUT binding {}",
                    source.id, binding.clut_binding_id
                )
            })?;
        validate_source_atlas_projection_link(
            mechanism,
            binding,
            residency,
            clut,
            &descriptors,
            &source_atlas_domains,
        )?;
        ensure!(
            projection_domains
                .insert(source.id.as_str(), binding.source_atlas_domain_id.as_str())
                .is_none(),
            "duplicate practical-result projection domain identity"
        );
        if matches!(
            clut.binding.evidence_status(),
            PracticalResultClutBindingEvidenceStatus::Unresolved
        ) {
            unresolved_external_clut_projection_count += 1;
        } else {
            resolved_clut_bound_consumer_projection_count += 1;
        }
        validate_projection_status(mechanism, clut.binding)?;
        if let PracticalResultConsumerMechanism::CatalogDescriptorProjection {
            occurrence_id, ..
        } = mechanism
        {
            let occurrence = action_consumer_occurrences
                .get(occurrence_id.as_str())
                .with_context(|| {
                    format!(
                        "catalog-descriptor projection {} names unknown action consumer occurrence {}",
                        source.id, occurrence_id
                    )
                })?;
            ensure!(
                projected_action_consumer_occurrence_ids.insert(occurrence_id.as_str()),
                "duplicate action consumer occurrence projection {occurrence_id}"
            );
            validate_action_consumer_occurrence_projection(mechanism, occurrence)?;
        }
        all_projection_semantics_complete &= matches!(
            binding.semantic_status,
            PracticalResultProjectionSemanticStatus::RuntimeObserved
                | PracticalResultProjectionSemanticStatus::StaticConfirmed
        );
        has_nested_unresolved |= projection_has_nested_unresolved(mechanism);

        let validated_mechanism = validate_projection_mechanism(
            mechanism,
            ProjectionValidationContext {
                overlay,
                base: runtime_base,
                source_atlas,
                residency,
                clut,
                descriptors: &descriptors,
                links: &link_authorities,
            },
            &mut counts,
        )?;
        if let Some(entry_id) = validated_mechanism.matched_authored_entry_id {
            semantic_bound_projection_count += 1;
            matched_authored_entry_ids.insert(entry_id);
        }
        if let Some(source_footprint) = validated_mechanism.source_footprint {
            let alternative_source_bindings = alternative_source_bindings(mechanism);
            if !alternative_source_bindings.is_empty() {
                let mut alternative_domain_ids = BTreeSet::new();
                for alternative in alternative_source_bindings {
                    ensure!(
                        !alternative.source_projection_id.is_empty()
                            && source_projection_ids
                                .insert(alternative.source_projection_id.as_str()),
                        "duplicate or empty alternative source-projection identity"
                    );
                    ensure!(
                        alternative_domain_ids.insert(alternative.source_atlas_domain_id.as_str()),
                        "projection {} repeats alternative source-atlas domain {}",
                        source.id,
                        alternative.source_atlas_domain_id
                    );
                    validate_alternative_source_activation(
                        mechanism,
                        alternative,
                        &indexed_result_consumer.known_post_upload_descriptor_aliases,
                        indexed_result_consumer.delegated_result_callback_index,
                        overlay,
                    )?;
                    siken20_bound_logical_projection_ids
                        .entry(alternative.source_atlas_domain_id.as_str())
                        .or_default()
                        .insert(source.id.as_str());
                    alternative_consumer_source_binding_count += 1;
                    let alternative_source_atlas = source_atlas_domains
                        .by_id
                        .get(alternative.source_atlas_domain_id.as_str())
                        .with_context(|| {
                            format!(
                                "alternative source projection {} names unknown source-atlas domain {}",
                                alternative.source_projection_id,
                                alternative.source_atlas_domain_id
                            )
                        })?;
                    let alternative_residency = residencies
                        .get(alternative.residency_id.as_str())
                        .with_context(|| {
                            format!(
                                "alternative source projection {} names unknown residency {}",
                                alternative.source_projection_id, alternative.residency_id
                            )
                        })?;
                    residency_bound_consumer_projection_count += 1;
                    let alternative_clut = cluts
                        .get(alternative.clut_binding_id.as_str())
                        .with_context(|| {
                            format!(
                                "alternative source projection {} names unknown CLUT binding {}",
                                alternative.source_projection_id, alternative.clut_binding_id
                            )
                        })?;
                    validate_source_atlas_alternative_projection_link(
                        mechanism,
                        alternative,
                        (residency, clut),
                        (alternative_residency, alternative_clut),
                        &descriptors,
                        &source_atlas_domains,
                    )?;
                    ensure!(
                        projection_domains
                            .insert(
                                alternative.source_projection_id.as_str(),
                                alternative.source_atlas_domain_id.as_str(),
                            )
                            .is_none(),
                        "duplicate alternative source-projection domain identity"
                    );
                    if matches!(
                        alternative_clut.binding.evidence_status(),
                        PracticalResultClutBindingEvidenceStatus::Unresolved
                    ) {
                        unresolved_external_clut_projection_count += 1;
                    } else {
                        resolved_clut_bound_consumer_projection_count += 1;
                    }
                    source_projection_read_footprints.push(derive_alternative_source_footprint(
                        mechanism,
                        alternative,
                        alternative_source_atlas,
                        alternative_residency,
                        alternative_clut,
                        &descriptors,
                        &source_footprint,
                    )?);
                }
            }
            source_projection_read_footprints.push(source_footprint);
        }
        validate_repeated_semantic_projection(
            mechanism,
            &link_authorities,
            &mut semantic_projection_signatures,
        )?;
    }

    validate_siken20_alternative_source_binding_denominator(&siken20_bound_logical_projection_ids)?;

    ensure!(
        projected_action_consumer_occurrence_ids
            == action_consumer_occurrences
                .keys()
                .copied()
                .collect::<BTreeSet<_>>(),
        "action consumer occurrence ledger and catalog-descriptor projections are not an exact bijection"
    );
    ensure!(
        source_projection_read_footprints.len() == source_projection_ids.len()
            && projection_domains.len() == source_projection_ids.len(),
        "source projection bindings and derived source-read footprints are not an exact bijection"
    );
    let source_atlas_denominators = source_atlas_domains
        .by_id
        .iter()
        .map(|(domain_id, domain)| {
            (
                *domain_id,
                SourceAtlasConsumerProjectionDenominator {
                    status: domain.consumer_projection_denominator_status,
                    incomplete_reason: domain
                        .consumer_projection_denominator_incomplete_reason
                        .as_deref(),
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    let source_read_assessment = assess_source_projection_reads(
        &source_projection_read_footprints,
        &projection_domains,
        &source_atlas_denominators,
        opaque_pointer_runs.unresolved_count,
    )?;
    let source_read_footprints =
        build_source_read_footprint_reports(&source_projection_read_footprints, assets)?;
    let action_consumer_occurrence_coverage_complete =
        projected_action_consumer_occurrence_ids.len() == action_consumer_occurrences.len();

    let authored_glyph_sequence_entry_count = assets
        .entries
        .iter()
        .filter(|entry| {
            entry.strategy == PracticalResultStrategy::GlyphSequence
                && entry.development_status == PracticalResultDevelopmentStatus::Authored
        })
        .count();
    ensure!(
        matched_authored_entry_ids.len() <= authored_glyph_sequence_entry_count,
        "practical-result projection matched more authored glyph units than exist"
    );

    let external_clut_palette_family_sha256s = cluts
        .values()
        .filter_map(|clut| match &clut.binding.evidence {
            PracticalResultClutBindingEvidence::StaticConfirmedExternalProducerFamily {
                palette_family_sha256,
                ..
            } => Some(palette_family_sha256.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    let authored_glyph_sequence_projection_coverage_complete =
        matched_authored_entry_ids.len() == authored_glyph_sequence_entry_count;
    let complete = authored_glyph_sequence_projection_coverage_complete
        && action_consumer_occurrence_coverage_complete
        && residency_bound_consumer_projection_count == source_projection_ids.len()
        && resolved_clut_bound_consumer_projection_count == source_projection_ids.len()
        && all_projection_semantics_complete
        && !has_nested_unresolved;
    Ok(PracticalResultProjectionValidation {
        source_atlas_domain_count: source_atlas_domains.by_id.len(),
        domain_bound_physical_region_count: source_atlas_domains.domain_bound_physical_region_count,
        physical_region_without_declared_source_atlas_domain_count: assets
            .physical_region_catalog
            .regions
            .len()
            - source_atlas_domains.domain_bound_physical_region_count,
        domain_bound_protected_region_count: assets.protected_content.regions.len(),
        domain_bound_source_glyph_bank_count: source_atlas_domains
            .domain_bound_source_glyph_bank_count,
        domain_bound_vram_residency_count: residencies.len(),
        domain_bound_clut_binding_count: cluts
            .values()
            .filter(|clut| clut.binding.source_atlas_domain_id().is_some())
            .count(),
        external_clut_producer_family_count: external_clut_palette_family_sha256s.len(),
        external_clut_palette_family_sha256s,
        unresolved_external_clut_binding_count: cluts
            .values()
            .filter(|clut| {
                matches!(
                    clut.binding.evidence_status(),
                    PracticalResultClutBindingEvidenceStatus::Unresolved
                )
            })
            .count(),
        domain_bound_consumer_descriptor_count: descriptors.len(),
        domain_bound_consumer_projection_count: projection_domains.len(),
        alternative_consumer_source_binding_count,
        assessed_source_projection_read_count: source_read_assessment.assessed_projection_count,
        unassessed_source_projection_read_count: source_read_assessment.unassessed_projection_count,
        declared_read_set_complete_source_projection_count: source_read_assessment
            .declared_read_set_complete_projection_count,
        consumer_reachability_closed_source_projection_count: source_read_assessment
            .consumer_reachability_closed_projection_count,
        consumer_reachability_unassessed_source_projection_count: source_read_assessment
            .consumer_reachability_unassessed_projection_count,
        consumer_reachability_unresolved_source_projection_count: source_read_assessment
            .consumer_reachability_unresolved_projection_count,
        consumer_reachability_dormant_source_projection_count: source_read_assessment
            .consumer_reachability_dormant_projection_count,
        source_projection_read_assessment_coverage_complete: source_read_assessment
            .assessment_coverage_complete,
        source_atlas_in_place_rewrite_gate_open_domain_count: source_read_assessment
            .in_place_rewrite_gate_open_domain_count,
        source_atlas_in_place_rewrite_gate_blocked_domain_count: source_read_assessment
            .in_place_rewrite_gate_blocked_domain_count,
        source_atlas_in_place_rewrite_gates: source_read_assessment.in_place_rewrite_gates,
        source_atlas_evidence_joins_complete: true,
        indexed_result_selected_member_count: indexed_result_consumer.selected_member_count,
        indexed_result_known_post_upload_descriptor_aliases: indexed_result_consumer
            .known_post_upload_descriptor_aliases
            .to_vec(),
        indexed_result_direct_descriptor_render_call_offsets: indexed_result_consumer
            .direct_descriptor_render_call_offsets
            .into_iter()
            .map(|offset| format!("0x{offset:04x}"))
            .collect(),
        indexed_result_known_post_upload_draw_path_validated: true,
        indexed_result_delegated_overlay_callback_index: indexed_result_consumer
            .delegated_result_callback_index,
        indexed_result_delegated_overlay_dispatch_offset: format!(
            "0x{:04x}",
            indexed_result_consumer.delegated_result_dispatch_offset
        ),
        indexed_result_tim_upload_call_offsets: indexed_result_texture_lifetime
            .tim_upload_call_offsets
            .into_iter()
            .map(|offset| format!("0x{offset:04x}"))
            .collect(),
        indexed_result_validated_result_title_member_count: indexed_result_texture_lifetime
            .validated_result_title_member_count,
        indexed_result_texture_lifetime_validated: true,
        declared_opaque_pointer_run_count: opaque_pointer_runs.declared_count,
        source_atlas_excluded_opaque_pointer_run_count: opaque_pointer_runs.excluded_count,
        unresolved_opaque_pointer_run_count: opaque_pointer_runs.unresolved_count,
        vram_residency_count: residencies.len(),
        resolved_clut_binding_count: cluts
            .values()
            .filter(|binding| {
                matches!(
                    binding.binding.evidence_status(),
                    PracticalResultClutBindingEvidenceStatus::StaticConfirmed
                        | PracticalResultClutBindingEvidenceStatus::StaticConfirmedExternalProducerFamily
                )
            })
            .count(),
        consumer_descriptor_count: descriptors.len(),
        consumer_projection_count: projection_ids.len(),
        declared_action_consumer_occurrence_count: action_consumer_occurrences.len(),
        validated_action_consumer_occurrence_count: projected_action_consumer_occurrence_ids.len(),
        action_consumer_occurrence_coverage_complete,
        semantic_bound_consumer_projection_count: semantic_bound_projection_count,
        matched_authored_glyph_sequence_unit_count: matched_authored_entry_ids.len(),
        authored_glyph_sequence_projection_coverage_complete,
        residency_bound_consumer_projection_count,
        resolved_clut_bound_consumer_projection_count,
        unresolved_external_clut_projection_count,
        hashed_span_count: counts.hashed_spans,
        runtime_address_count: counts.runtime_addresses,
        pointer_alias_count: counts.pointer_aliases,
        source_read_footprints,
        complete,
    })
}

fn alternative_source_bindings(
    mechanism: &PracticalResultConsumerMechanism,
) -> &[PracticalResultAlternativeSourceBinding] {
    match mechanism {
        PracticalResultConsumerMechanism::CatalogDescriptorProjection {
            alternative_source_bindings,
            ..
        }
        | PracticalResultConsumerMechanism::CatalogDigitDescriptorProjection {
            alternative_source_bindings,
            ..
        }
        | PracticalResultConsumerMechanism::DirectNumericSpriteSitesProjection {
            alternative_source_bindings,
            ..
        }
        | PracticalResultConsumerMechanism::DirectSpriteSelectorProjection {
            alternative_source_bindings,
            ..
        }
        | PracticalResultConsumerMechanism::GCatalogDescriptorSelectorProjection {
            alternative_source_bindings,
            ..
        } => alternative_source_bindings,
        _ => &[],
    }
}

fn validate_siken20_alternative_source_binding_denominator(
    bound_logical_projection_ids: &BTreeMap<&str, BTreeSet<&str>>,
) -> Result<()> {
    let expected = [
        "siken2_aliases_46_56_projection",
        "siken2_aliases_47_57_projection",
        "siken2_alias_55_projection",
        "sikeng21_rating_selector_projection",
        "sikeng21_outcome_badge_projection",
        "sikeng21_digit_descriptor_projection",
        "sikeng21_direct_numeric_projection",
        "sikeng22_rating_selector_projection",
        "sikeng22_outcome_badge_projection",
        "sikeng22_digit_descriptor_projection",
        "sikeng22_direct_numeric_projection",
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    let expected_domains = [
        "siken20_member00_tim0_indexed_4bpp",
        "siken20_member01_tim0_indexed_4bpp",
    ];
    ensure!(
        bound_logical_projection_ids.len() == expected_domains.len(),
        "SIKEN20 alternative source bindings do not cover exactly the two indexed result members"
    );
    for domain_id in expected_domains {
        ensure!(
            bound_logical_projection_ids.get(domain_id) == Some(&expected),
            "SIKEN20 source-atlas domain {domain_id} has an incomplete or unexpected post-upload consumer denominator"
        );
    }
    Ok(())
}

fn validate_alternative_source_activation(
    mechanism: &PracticalResultConsumerMechanism,
    alternative: &PracticalResultAlternativeSourceBinding,
    known_post_upload_descriptor_aliases: &[u16],
    delegated_result_callback_index: usize,
    overlay: &ModeDescendantSourceRecord,
) -> Result<()> {
    let (source, binding) = projection_parts(mechanism)?;
    ensure!(
        binding.source_atlas_domain_id.as_str() == "siken2_tim0_indexed_4bpp"
            && matches!(
                alternative.source_atlas_domain_id.as_str(),
                "siken20_member00_tim0_indexed_4bpp" | "siken20_member01_tim0_indexed_4bpp"
            ),
        "alternative source projection {} names a domain outside the two validated SIKEN20 indexed members",
        alternative.source_projection_id
    );
    match &alternative.activation_evidence {
        PracticalResultAlternativeSourceActivationEvidence::Siken20IndexedResultPostUploadDescriptorAlias {
            descriptor_alias,
        } => {
            let PracticalResultConsumerMechanism::CatalogDescriptorProjection {
                descriptor_evidence,
                ..
            } = mechanism
            else {
                anyhow::bail!(
                    "alternative source projection {} uses descriptor activation for a non-descriptor consumer",
                    alternative.source_projection_id
                );
            };
            ensure!(
                source.overlay_path == "DAT1/SIKEN2.BIN",
                "alternative source projection {} descriptor activation is outside SIKEN2",
                alternative.source_projection_id
            );
            let descriptor_alias = u16::try_from(*descriptor_alias).with_context(|| {
                format!(
                    "alternative source projection {} descriptor alias is outside the indexed-result selector width",
                    alternative.source_projection_id
                )
            })?;
            ensure!(
                known_post_upload_descriptor_aliases.contains(&descriptor_alias),
                "alternative source projection {} uses descriptor alias {} outside the validated SIKEN20 post-upload draw path",
                alternative.source_projection_id,
                descriptor_alias
            );
            ensure!(
                descriptor_evidence
                    .selector_indices
                    .contains(&usize::from(descriptor_alias)),
                "alternative source projection {} activation alias {} does not select its logical descriptor",
                alternative.source_projection_id,
                descriptor_alias
            );
        }
        PracticalResultAlternativeSourceActivationEvidence::Siken20IndexedResultPostUploadDelegatedCallback {
            callback_index,
            route_state_byte_equals_zero,
        } => {
            ensure!(
                matches!(
                    mechanism,
                    PracticalResultConsumerMechanism::CatalogDigitDescriptorProjection { .. }
                        | PracticalResultConsumerMechanism::DirectNumericSpriteSitesProjection { .. }
                        | PracticalResultConsumerMechanism::DirectSpriteSelectorProjection { .. }
                        | PracticalResultConsumerMechanism::GCatalogDescriptorSelectorProjection { .. }
                ),
                "alternative source projection {} uses delegated activation for an unsupported consumer",
                alternative.source_projection_id
            );
            ensure!(
                *callback_index == delegated_result_callback_index,
                "alternative source projection {} uses an unvalidated delegated callback index",
                alternative.source_projection_id
            );
            let (expected_path, expected_base, expected_callback, expected_zero_route) =
                match source.id.as_str() {
                    "sikeng21_rating_selector_projection"
                    | "sikeng21_outcome_badge_projection"
                    | "sikeng21_digit_descriptor_projection"
                    | "sikeng21_direct_numeric_projection" => {
                        ("DAT1/SIKENG21.BIN", "0x80152000", 0x8015_3e9c, false)
                    }
                    "sikeng22_rating_selector_projection"
                    | "sikeng22_outcome_badge_projection"
                    | "sikeng22_digit_descriptor_projection"
                    | "sikeng22_direct_numeric_projection" => {
                        ("DAT1/SIKENG22.BIN", "0x8017a000", 0x8017_be9c, true)
                    }
                    _ => anyhow::bail!(
                        "alternative source projection {} is outside the finite delegated result-consumer set",
                        alternative.source_projection_id
                    ),
                };
            ensure!(
                source.overlay_path == expected_path
                    && source.runtime_base == expected_base
                    && *route_state_byte_equals_zero == expected_zero_route
                    && overlay.path == expected_path,
                "alternative source projection {} delegated overlay route changed",
                alternative.source_projection_id
            );
            let callback_pointer_offset = callback_index
                .checked_mul(4)
                .context("delegated callback pointer offset overflow")?;
            let callback_pointer = u32::from_le_bytes(
                overlay
                    .decoded
                    .get(callback_pointer_offset..callback_pointer_offset + 4)
                    .context("delegated result callback pointer is truncated")?
                    .try_into()
                    .expect("four-byte callback pointer"),
            );
            ensure!(
                callback_pointer == expected_callback,
                "alternative source projection {} delegated result callback changed",
                alternative.source_projection_id
            );
        }
    }
    Ok(())
}
