//! Exact joins from one JP source TIM to its residency and consumer evidence.
//!
//! These joins describe source occupancy only. They never create Korean atlas
//! slots, destinations, or write authority.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};

use crate::embedded_tim::detect_embedded_tim_images;
use crate::mode_descendant_graphics::source::ModeDescendantSourceRecord;
use crate::pipeline::sha256_bytes;

use super::super::model::{PhysicalRegionId, PracticalResultAssets};
use super::super::projection_model::*;
use super::super::source_atlas_domain_model::{
    PracticalResultSourceAtlasDomain, SourceAtlasDomainId,
};
use super::clut::ValidatedClut;
use super::descriptor::DescriptorView;
use super::geometry::validate_projection_geometry;
use super::source_evidence::{checked_end, parse_hex, source_for_path};

pub(super) struct ValidatedSourceAtlasDomains<'a> {
    pub(super) by_id: BTreeMap<&'a str, &'a PracticalResultSourceAtlasDomain>,
    physical_domain_by_id: BTreeMap<&'a str, &'a SourceAtlasDomainId>,
    protected_domain_by_id: BTreeMap<&'a str, &'a SourceAtlasDomainId>,
    pub(super) domain_bound_physical_region_count: usize,
    pub(super) domain_bound_source_glyph_bank_count: usize,
}

pub(super) fn validate_source_atlas_domains<'a>(
    assets: &'a PracticalResultAssets,
    sources: &[ModeDescendantSourceRecord],
) -> Result<ValidatedSourceAtlasDomains<'a>> {
    let mut by_id = BTreeMap::new();
    for domain in &assets.source_atlas_domain_catalog.domains {
        ensure!(
            by_id.insert(domain.id.as_str(), domain).is_none(),
            "duplicate source-atlas domain {}",
            domain.id
        );
        let source = source_for_path(sources, &domain.source_path)?;
        let tim_offset = parse_hex(&domain.tim_offset, "source-atlas TIM offset")?;
        let tim_end = checked_end(tim_offset, domain.source_tim_size, "source-atlas TIM")?;
        let tim_bytes = source
            .decoded
            .get(tim_offset..tim_end)
            .with_context(|| format!("source-atlas domain {} TIM is truncated", domain.id))?;
        let tim = detect_embedded_tim_images(&source.decoded)
            .into_iter()
            .find(|tim| tim.offset == tim_offset)
            .with_context(|| {
                format!(
                    "source-atlas domain {} does not begin at a detected TIM",
                    domain.id
                )
            })?;
        ensure!(
            sha256_bytes(tim_bytes) == domain.source_tim_sha256
                && tim.total_size == domain.source_tim_size
                && tim.bits_per_pixel == domain.bpp
                && tim.pixel_width == domain.pixel_width
                && tim.pixel_height == domain.pixel_height,
            "source-atlas domain {} exact TIM identity changed",
            domain.id
        );
    }
    let domain_bound_physical_region_count = assets
        .physical_region_catalog
        .regions
        .iter()
        .filter(|region| region.source_atlas_domain_id.is_some())
        .count();
    let domain_bound_source_glyph_bank_count = assets.source_glyph_catalog.banks.len();
    let physical_domain_by_id = assets
        .physical_region_catalog
        .regions
        .iter()
        .filter_map(|region| {
            region
                .source_atlas_domain_id
                .as_ref()
                .map(|domain_id| (region.region_id.as_str(), domain_id))
        })
        .collect();
    let protected_domain_by_id = assets
        .protected_content
        .regions
        .iter()
        .map(|region| (region.id.as_str(), &region.source_atlas_domain_id))
        .collect();
    Ok(ValidatedSourceAtlasDomains {
        by_id,
        physical_domain_by_id,
        protected_domain_by_id,
        domain_bound_physical_region_count,
        domain_bound_source_glyph_bank_count,
    })
}

pub(super) fn validate_source_atlas_residency_links(
    domains: &ValidatedSourceAtlasDomains<'_>,
    residencies: &BTreeMap<&str, &PracticalResultVramResidency>,
) -> Result<()> {
    for residency in residencies.values() {
        let domain = domains
            .by_id
            .get(residency.source_atlas_domain_id.as_str())
            .with_context(|| {
                format!(
                    "residency {} names unknown source-atlas domain {}",
                    residency.id, residency.source_atlas_domain_id
                )
            })?;
        ensure!(
            residency.source_path == domain.source_path
                && residency.tim_offset == domain.tim_offset
                && residency.bpp == domain.bpp
                && residency.source_tim_size == domain.source_tim_size
                && residency.source_tim_sha256 == domain.source_tim_sha256
                && residency.pixel_width == domain.pixel_width
                && residency.pixel_height == domain.pixel_height,
            "residency {} crosses its source-atlas domain",
            residency.id
        );
    }
    Ok(())
}

pub(super) fn validate_source_atlas_clut_links(
    domains: &ValidatedSourceAtlasDomains<'_>,
    cluts: &BTreeMap<&str, ValidatedClut<'_>>,
) -> Result<()> {
    for clut in cluts.values() {
        match &clut.binding.evidence {
            PracticalResultClutBindingEvidence::StaticConfirmed {
                source_atlas_domain_id: domain_id,
                source_path,
                tim_offset,
                bpp,
                ..
            } => {
                let domain = domains.by_id.get(domain_id.as_str()).with_context(|| {
                    format!(
                        "CLUT {} names unknown source-atlas domain {}",
                        clut.binding.id, domain_id
                    )
                })?;
                ensure!(
                    source_path == &domain.source_path
                        && tim_offset == &domain.tim_offset
                        && *bpp == domain.bpp,
                    "CLUT {} crosses its source-atlas domain",
                    clut.binding.id
                );
            }
            PracticalResultClutBindingEvidence::StaticConfirmedExternalProducerFamily {
                ..
            }
            | PracticalResultClutBindingEvidence::Unresolved { .. } => {}
        }
    }
    Ok(())
}

pub(super) fn validate_source_atlas_descriptor_links(
    domains: &ValidatedSourceAtlasDomains<'_>,
    residencies: &BTreeMap<&str, &PracticalResultVramResidency>,
    descriptors: &BTreeMap<&str, DescriptorView<'_>>,
) -> Result<()> {
    for descriptor in descriptors.values() {
        ensure!(
            !descriptor.source_atlas_domain_ids.is_empty()
                && descriptor
                    .source_atlas_domain_ids
                    .iter()
                    .map(SourceAtlasDomainId::as_str)
                    .collect::<BTreeSet<_>>()
                    .len()
                    == descriptor.source_atlas_domain_ids.len(),
            "descriptor {} has empty or duplicate source-atlas domains",
            descriptor.descriptor_id
        );
        for domain_id in descriptor.source_atlas_domain_ids {
            let domain = domains.by_id.get(domain_id.as_str()).with_context(|| {
                format!(
                    "descriptor {} names unknown source-atlas domain {}",
                    descriptor.descriptor_id, domain_id
                )
            })?;
            ensure!(
                descriptor.fragments.iter().all(|fragment| {
                    fragment
                        .source_u
                        .checked_add(fragment.width)
                        .is_some_and(|end| end <= domain.pixel_width)
                        && fragment
                            .source_v
                            .checked_add(fragment.height)
                            .is_some_and(|end| end <= domain.pixel_height)
                        && residencies.values().any(|residency| {
                            residency.source_atlas_domain_id == domain.id
                                && fragment.texture_page == residency.texture_page
                                && fragment.texture_bank == residency.texture_bank
                        })
                }),
                "descriptor {} geometry or page crosses source-atlas domain {}",
                descriptor.descriptor_id,
                domain.id
            );
        }
    }
    Ok(())
}

pub(super) fn validate_source_atlas_projection_link(
    mechanism: &PracticalResultConsumerMechanism,
    binding: &PracticalResultProjectionBinding,
    residency: &PracticalResultVramResidency,
    clut: &ValidatedClut<'_>,
    descriptors: &BTreeMap<&str, DescriptorView<'_>>,
    domains: &ValidatedSourceAtlasDomains<'_>,
) -> Result<()> {
    let domain = validate_source_atlas_physical_binding(
        &binding.source_atlas_domain_id,
        residency,
        clut,
        domains,
    )?;

    let (physical_ids, protected_ids) = projection_source_region_ids(mechanism);
    for physical_id in physical_ids {
        ensure!(
            domains
                .physical_domain_by_id
                .get(physical_id.as_str())
                .copied()
                == Some(&domain.id),
            "projection physical region {physical_id} crosses or lacks its source-atlas domain"
        );
    }
    for protected_id in protected_ids {
        ensure!(
            domains
                .protected_domain_by_id
                .get(protected_id.as_str())
                .copied()
                == Some(&domain.id),
            "projection protected region {protected_id} crosses its source-atlas domain"
        );
    }
    if let PracticalResultConsumerMechanism::CatalogDescriptorProjection {
        descriptor_evidence,
        ..
    } = mechanism
    {
        let descriptor = descriptors
            .get(descriptor_evidence.descriptor_id.as_str())
            .context("projection names an unknown descriptor during source-atlas join")?;
        ensure!(
            descriptor.source_atlas_domain_ids.contains(&domain.id),
            "projection and descriptor cross source-atlas domains"
        );
    }
    Ok(())
}

pub(super) fn validate_source_atlas_alternative_projection_link(
    mechanism: &PracticalResultConsumerMechanism,
    binding: &PracticalResultAlternativeSourceBinding,
    base_physical_binding: (&PracticalResultVramResidency, &ValidatedClut<'_>),
    alternative_physical_binding: (&PracticalResultVramResidency, &ValidatedClut<'_>),
    descriptors: &BTreeMap<&str, DescriptorView<'_>>,
    domains: &ValidatedSourceAtlasDomains<'_>,
) -> Result<()> {
    let (base_residency, base_clut) = base_physical_binding;
    let (residency, clut) = alternative_physical_binding;
    let domain = validate_source_atlas_physical_binding(
        &binding.source_atlas_domain_id,
        residency,
        clut,
        domains,
    )?;
    ensure!(
        residency.bpp == base_residency.bpp
            && residency.image_vram_word_x == base_residency.image_vram_word_x
            && residency.image_vram_y == base_residency.image_vram_y
            && residency.pixel_width == base_residency.pixel_width
            && residency.pixel_height == base_residency.pixel_height
            && residency.texture_page == base_residency.texture_page
            && residency.texture_bank == base_residency.texture_bank,
        "alternative projection {} residency differs from its logical projection",
        binding.source_projection_id
    );
    ensure!(
        clut.binding.clut_vram_x == base_clut.binding.clut_vram_x
            && clut.binding.clut_vram_y == base_clut.binding.clut_vram_y,
        "alternative projection {} CLUT coordinates differ from its logical projection",
        binding.source_projection_id
    );
    validate_projection_geometry(mechanism, domain)?;
    if let PracticalResultConsumerMechanism::CatalogDescriptorProjection {
        descriptor_evidence,
        ..
    } = mechanism
    {
        let descriptor = descriptors
            .get(descriptor_evidence.descriptor_id.as_str())
            .context("alternative projection names an unknown descriptor")?;
        ensure!(
            descriptor.source_atlas_domain_ids.contains(&domain.id),
            "alternative projection and descriptor cross source-atlas domains"
        );
    }
    Ok(())
}

fn validate_source_atlas_physical_binding<'a>(
    domain_id: &SourceAtlasDomainId,
    residency: &PracticalResultVramResidency,
    clut: &ValidatedClut<'_>,
    domains: &'a ValidatedSourceAtlasDomains<'_>,
) -> Result<&'a PracticalResultSourceAtlasDomain> {
    let domain = domains
        .by_id
        .get(domain_id.as_str())
        .with_context(|| format!("projection names unknown source-atlas domain {domain_id}"))?;
    ensure!(
        residency.source_atlas_domain_id == domain.id,
        "projection and residency cross source-atlas domains"
    );
    if let Some(clut_domain_id) = clut.binding.source_atlas_domain_id() {
        ensure!(
            clut_domain_id == &domain.id,
            "projection and resolved CLUT cross source-atlas domains"
        );
    }
    Ok(domain)
}

fn projection_source_region_ids(
    mechanism: &PracticalResultConsumerMechanism,
) -> (&[PhysicalRegionId], &[PracticalResultProtectedRegionId]) {
    match mechanism {
        PracticalResultConsumerMechanism::CatalogDescriptorProjection {
            physical_region_ids,
            ..
        } => (physical_region_ids, &[]),
        PracticalResultConsumerMechanism::CatalogDigitDescriptorProjection {
            protected_region_ids,
            ..
        }
        | PracticalResultConsumerMechanism::DirectSpriteSelectorProjection {
            protected_region_ids,
            ..
        }
        | PracticalResultConsumerMechanism::GCatalogDescriptorSelectorProjection {
            protected_region_ids,
            ..
        }
        | PracticalResultConsumerMechanism::GoCatalogDescriptorSelectorProjection {
            protected_region_ids,
            ..
        }
        | PracticalResultConsumerMechanism::NumericLookupRendererProjection {
            protected_region_ids,
            ..
        } => (&[], protected_region_ids),
        PracticalResultConsumerMechanism::DirectNumericSpriteSitesProjection {
            physical_region_ids,
            protected_region_ids,
            ..
        } => (physical_region_ids, protected_region_ids),
        PracticalResultConsumerMechanism::GDynamicConfigTileStreamProjection { .. }
        | PracticalResultConsumerMechanism::GoDynamicConfigTileStreamProjection { .. }
        | PracticalResultConsumerMechanism::StaticMatrixTileStreamProjection { .. }
        | PracticalResultConsumerMechanism::StaticSelectorTileStreamProjection { .. }
        | PracticalResultConsumerMechanism::CatalogDescriptorRenderer { .. } => (&[], &[]),
    }
}
