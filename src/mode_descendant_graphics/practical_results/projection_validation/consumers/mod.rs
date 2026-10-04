//! Exhaustive dispatch across practical-result consumer mechanisms.

mod catalog;
mod source_read_reachability;
mod source_tile_projection;
mod sprite;
mod tile_renderer;
mod tile_stream;

use std::collections::BTreeMap;

use anyhow::{Context, Result, ensure};

use crate::mode_descendant_graphics::source::ModeDescendantSourceRecord;

use super::super::projection_model::*;
use super::super::source_atlas_domain_model::{
    PracticalResultSourceAtlasDomain, SourceAtlasDomainId,
};
use super::clut::ValidatedClut;
use super::descriptor::{DescriptorView, SECONDARY_DESCRIPTOR_CLUT_BASE_Y};
use super::footprint::{
    SourceAtlasRectangle, SourceAtlasTileGeometry, SourceProjectionFootprint,
    SourceReadFootprintDerivation, derive_declared_rectangle_source_footprint,
    derive_dynamic_source_footprint, derive_static_source_footprint,
};
use super::geometry::{validate_digit_geometry, validate_projection_geometry};
use super::projection_links::{
    ProjectionLinkAuthorities, validate_descriptor_semantic_cells, validate_physical_ids,
    validate_protected_ids, validate_semantic_links,
};
use super::source_evidence::{ValidationCounts, address_and_span};

use catalog::{
    validate_catalog_renderer, validate_digit_descriptor_table, validate_g_selector,
    validate_go_selector,
};
use source_read_reachability::validate_source_read_reachability;
use source_tile_projection::ValidatedTileProjectionGeometry;
use sprite::{validate_direct_numeric_site, validate_direct_renderer};
use tile_renderer::{
    validate_g_dynamic_renderer, validate_go_dynamic_renderer, validate_static_renderer,
};
use tile_stream::{
    validate_dynamic_config_graph, validate_matrix_streams, validate_selector_streams,
};

pub(super) struct ValidatedProjectionMechanism {
    pub(super) matched_authored_entry_id: Option<String>,
    pub(super) source_footprint:
        Option<SourceProjectionFootprint<PracticalResultConsumerProjectionId, SourceAtlasDomainId>>,
}

pub(super) struct ProjectionValidationContext<'a, 'assets> {
    pub(super) overlay: &'a ModeDescendantSourceRecord,
    pub(super) base: u32,
    pub(super) source_atlas: &'a PracticalResultSourceAtlasDomain,
    pub(super) residency: &'a PracticalResultVramResidency,
    pub(super) clut: &'a ValidatedClut<'assets>,
    pub(super) descriptors: &'a BTreeMap<&'assets str, DescriptorView<'assets>>,
    pub(super) links: &'a ProjectionLinkAuthorities<'assets>,
}

pub(super) fn validate_projection_mechanism(
    mechanism: &PracticalResultConsumerMechanism,
    context: ProjectionValidationContext<'_, '_>,
    counts: &mut ValidationCounts,
) -> Result<ValidatedProjectionMechanism> {
    let ProjectionValidationContext {
        overlay,
        base,
        source_atlas,
        residency,
        clut,
        descriptors,
        links,
    } = context;
    let (projection_source, projection_binding) = projection_parts(mechanism)?;
    let mut matched_authored_entry_id = None;
    let source_footprint;
    validate_projection_geometry(mechanism, source_atlas)?;
    match mechanism {
        PracticalResultConsumerMechanism::CatalogDescriptorProjection {
            source,
            binding,
            descriptor_evidence,
            semantic_entry_id,
            ordered_source_reference_ids,
            physical_region_ids,
            ..
        } => {
            let descriptor = descriptors
                .get(descriptor_evidence.descriptor_id.as_str())
                .with_context(|| {
                    format!(
                        "projection {} names unknown descriptor {}",
                        source.id, descriptor_evidence.descriptor_id
                    )
                })?;
            ensure!(
                source.consumer_id.as_str() == descriptor.consumer_id.as_str()
                    && source.overlay_path == descriptor.overlay_path
                    && source.runtime_base == descriptor.runtime_base
                    && descriptor_evidence.pointer_table_offset == descriptor.pointer_table_offset
                    && descriptor_evidence.selector_indices == descriptor.aliases
                    && descriptor_evidence.descriptor_offset == descriptor.descriptor_offset
                    && descriptor_evidence.descriptor_runtime_address
                        == descriptor.descriptor_runtime_address
                    && descriptor_evidence.descriptor_capacity == descriptor.descriptor_capacity
                    && descriptor_evidence.descriptor_sha256 == descriptor.descriptor_sha256,
                "projection {} differs from its declared descriptor identity",
                source.id
            );
            validate_catalog_descriptor_physical_binding(
                source.id.as_str(),
                descriptor,
                residency,
                clut,
            )?;
            address_and_span(
                &overlay.decoded,
                base,
                &descriptor_evidence.descriptor_offset,
                &descriptor_evidence.descriptor_runtime_address,
                descriptor_evidence.descriptor_capacity,
                &descriptor_evidence.descriptor_sha256,
                "projection descriptor",
                counts,
            )?;
            ensure!(
                semantic_entry_id
                    .as_ref()
                    .is_none_or(|entry_id| !entry_id.is_empty()),
                "projection {} has an empty semantic entry ID",
                source.id
            );
            if semantic_entry_id.is_some() {
                validate_descriptor_semantic_cells(
                    source.id.as_str(),
                    descriptor,
                    residency,
                    ordered_source_reference_ids
                        .as_deref()
                        .context("semantic descriptor projection has no ordered references")?,
                    physical_region_ids,
                    links,
                )?;
            }
            matched_authored_entry_id = validate_semantic_links(
                source.id.as_str(),
                binding.semantic_status,
                semantic_entry_id
                    .as_ref()
                    .map(PracticalResultSemanticEntryId::as_str),
                ordered_source_reference_ids.as_deref(),
                physical_region_ids,
                links,
            )?;
            let consumer_reachability = validate_adopted_source_read_reachability(
                &overlay.decoded,
                base,
                &[descriptor.renderer_runtime_address],
                projection_binding,
                counts,
            )?;
            source_footprint = Some(derive_declared_rectangle_source_footprint(
                projection_source.id.clone(),
                projection_binding.source_atlas_domain_id.clone(),
                descriptor
                    .fragments
                    .iter()
                    .map(|fragment| SourceAtlasRectangle {
                        x: fragment.source_u,
                        y: fragment.source_v,
                        width: fragment.width,
                        height: fragment.height,
                    }),
                SourceReadFootprintDerivation::CatalogDescriptorFragments,
                source_atlas.pixel_width,
                source_atlas.pixel_height,
                consumer_reachability,
            )?);
        }
        PracticalResultConsumerMechanism::CatalogDigitDescriptorProjection {
            renderer_evidence,
            descriptor_table_evidence,
            protected_region_ids,
            ..
        } => {
            validate_catalog_renderer(&overlay.decoded, base, renderer_evidence, counts)?;
            validate_digit_descriptor_table(
                &overlay.decoded,
                base,
                renderer_evidence,
                descriptor_table_evidence,
                counts,
            )?;
            validate_protected_ids(protected_region_ids, links)?;
            let consumer_reachability = validate_adopted_source_read_reachability(
                &overlay.decoded,
                base,
                &[renderer_evidence.renderer_runtime_address.as_str()],
                projection_binding,
                counts,
            )?;
            source_footprint = Some(derive_declared_rectangle_source_footprint(
                projection_source.id.clone(),
                projection_binding.source_atlas_domain_id.clone(),
                descriptor_table_evidence
                    .raw_value_entries
                    .iter()
                    .map(|entry| source_rectangle(&entry.source_rect)),
                SourceReadFootprintDerivation::CatalogDigitDescriptorTable,
                source_atlas.pixel_width,
                source_atlas.pixel_height,
                consumer_reachability,
            )?);
        }
        PracticalResultConsumerMechanism::DirectNumericSpriteSitesProjection {
            sites,
            physical_region_ids,
            protected_region_ids,
            ..
        } => {
            ensure!(!sites.is_empty(), "direct-numeric projection has no sites");
            for site in sites {
                validate_direct_numeric_site(&overlay.decoded, base, site, counts)?;
            }
            validate_physical_ids(physical_region_ids, links)?;
            validate_protected_ids(protected_region_ids, links)?;
            let consumer_addresses = sites
                .iter()
                .map(direct_numeric_consumer_address)
                .collect::<Vec<_>>();
            let consumer_reachability = validate_adopted_source_read_reachability(
                &overlay.decoded,
                base,
                &consumer_addresses,
                projection_binding,
                counts,
            )?;
            source_footprint = Some(derive_declared_rectangle_source_footprint(
                projection_source.id.clone(),
                projection_binding.source_atlas_domain_id.clone(),
                direct_numeric_source_rectangles(sites),
                SourceReadFootprintDerivation::DirectNumericSpriteSites,
                source_atlas.pixel_width,
                source_atlas.pixel_height,
                consumer_reachability,
            )?);
        }
        PracticalResultConsumerMechanism::DirectSpriteSelectorProjection {
            renderer_evidence,
            selector_evidence,
            protected_region_ids,
            ..
        } => {
            validate_direct_renderer(&overlay.decoded, base, renderer_evidence, counts)?;
            address_and_span(
                &overlay.decoded,
                base,
                &selector_evidence.uv_table_offset,
                &selector_evidence.uv_table_runtime_address,
                selector_evidence.uv_table_size,
                &selector_evidence.uv_table_sha256,
                "selector UV table",
                counts,
            )?;
            validate_protected_ids(protected_region_ids, links)?;
            for entry in &selector_evidence.raw_selector_entries {
                ensure!(
                    links.contains_protected(entry.protected_region_id.as_str())
                        && protected_region_ids
                            .iter()
                            .any(|id| id.as_str() == entry.protected_region_id.as_str()),
                    "direct selector names an unbound protected region"
                );
            }
            let consumer_reachability = validate_adopted_source_read_reachability(
                &overlay.decoded,
                base,
                &[renderer_evidence.function_runtime_address.as_str()],
                projection_binding,
                counts,
            )?;
            source_footprint = Some(derive_declared_rectangle_source_footprint(
                projection_source.id.clone(),
                projection_binding.source_atlas_domain_id.clone(),
                selector_evidence
                    .raw_selector_entries
                    .iter()
                    .map(|entry| source_rectangle(&entry.source_rect)),
                SourceReadFootprintDerivation::DirectSpriteSelectorTable,
                source_atlas.pixel_width,
                source_atlas.pixel_height,
                consumer_reachability,
            )?);
        }
        PracticalResultConsumerMechanism::GCatalogDescriptorSelectorProjection {
            renderer_evidence,
            selector_evidence,
            protected_region_ids,
            ..
        } => {
            validate_catalog_renderer(&overlay.decoded, base, renderer_evidence, counts)?;
            validate_g_selector(
                &overlay.decoded,
                base,
                renderer_evidence,
                selector_evidence,
                counts,
            )?;
            validate_protected_ids(protected_region_ids, links)?;
            let consumer_reachability = validate_adopted_source_read_reachability(
                &overlay.decoded,
                base,
                &[renderer_evidence.renderer_runtime_address.as_str()],
                projection_binding,
                counts,
            )?;
            source_footprint = Some(derive_declared_rectangle_source_footprint(
                projection_source.id.clone(),
                projection_binding.source_atlas_domain_id.clone(),
                selector_evidence
                    .raw_selector_entries
                    .iter()
                    .map(|entry| source_rectangle(&entry.source_rect)),
                SourceReadFootprintDerivation::CatalogDescriptorSelectorTable,
                source_atlas.pixel_width,
                source_atlas.pixel_height,
                consumer_reachability,
            )?);
        }
        PracticalResultConsumerMechanism::GoCatalogDescriptorSelectorProjection {
            renderer_evidence,
            selector_evidence,
            protected_region_ids,
            ..
        } => {
            validate_catalog_renderer(&overlay.decoded, base, renderer_evidence, counts)?;
            validate_go_selector(
                &overlay.decoded,
                base,
                renderer_evidence,
                selector_evidence,
                counts,
            )?;
            validate_protected_ids(protected_region_ids, links)?;
            let consumer_reachability = validate_adopted_source_read_reachability(
                &overlay.decoded,
                base,
                &[renderer_evidence.renderer_runtime_address.as_str()],
                projection_binding,
                counts,
            )?;
            source_footprint = Some(derive_declared_rectangle_source_footprint(
                projection_source.id.clone(),
                projection_binding.source_atlas_domain_id.clone(),
                selector_evidence
                    .raw_selector_entries
                    .iter()
                    .map(|entry| source_rectangle(&entry.source_rect)),
                SourceReadFootprintDerivation::CatalogDescriptorSelectorTable,
                source_atlas.pixel_width,
                source_atlas.pixel_height,
                consumer_reachability,
            )?);
        }
        PracticalResultConsumerMechanism::NumericLookupRendererProjection {
            renderer_evidence,
            lookup_table_evidence,
            protected_region_ids,
            ..
        } => {
            validate_direct_renderer(&overlay.decoded, base, renderer_evidence, counts)?;
            address_and_span(
                &overlay.decoded,
                base,
                &lookup_table_evidence.lookup_table_offset,
                &lookup_table_evidence.lookup_table_runtime_address,
                lookup_table_evidence.lookup_table_size,
                &lookup_table_evidence.lookup_table_sha256,
                "numeric lookup table",
                counts,
            )?;
            validate_digit_geometry(&lookup_table_evidence.source_geometry)?;
            validate_protected_ids(protected_region_ids, links)?;
            let consumer_reachability = validate_adopted_source_read_reachability(
                &overlay.decoded,
                base,
                &[renderer_evidence.function_runtime_address.as_str()],
                projection_binding,
                counts,
            )?;
            source_footprint = Some(derive_declared_rectangle_source_footprint(
                projection_source.id.clone(),
                projection_binding.source_atlas_domain_id.clone(),
                digit_geometry_rectangles(&lookup_table_evidence.source_geometry),
                SourceReadFootprintDerivation::NumericLookupTable,
                source_atlas.pixel_width,
                source_atlas.pixel_height,
                consumer_reachability,
            )?);
        }
        PracticalResultConsumerMechanism::StaticMatrixTileStreamProjection {
            renderer_evidence,
            stream_table_evidence,
            ..
        } => {
            ensure!(
                matches!(
                    projection_binding
                        .source_read_reachability_evidence
                        .as_ref(),
                    Some(
                        PracticalResultSourceReadReachabilityEvidence::LoadedImageHeaderCallback { .. }
                    )
                ),
                "static tile-stream projection must use header-callback reachability evidence"
            );
            let projection_geometry = validate_static_renderer(
                &overlay.decoded,
                base,
                renderer_evidence,
                residency,
                counts,
            )?;
            let streams =
                validate_matrix_streams(&overlay.decoded, base, stream_table_evidence, counts)?;
            let consumer_reachability = validate_source_read_reachability(
                &overlay.decoded,
                base,
                &[renderer_evidence.exported_callback_runtime_address.as_str()],
                projection_binding
                    .source_read_reachability_evidence
                    .as_ref()
                    .context("static tile-stream projection has no reachability evidence")?,
                counts,
            )?;
            source_footprint = Some(derive_static_source_footprint(
                projection_source.id.clone(),
                projection_binding.source_atlas_domain_id.clone(),
                &streams,
                source_tile_geometry(source_atlas, projection_geometry)?,
                consumer_reachability,
            )?);
        }
        PracticalResultConsumerMechanism::StaticSelectorTileStreamProjection {
            renderer_evidence,
            stream_table_evidence,
            ..
        } => {
            ensure!(
                matches!(
                    projection_binding
                        .source_read_reachability_evidence
                        .as_ref(),
                    Some(
                        PracticalResultSourceReadReachabilityEvidence::LoadedImageHeaderCallback { .. }
                    )
                ),
                "static tile-stream projection must use header-callback reachability evidence"
            );
            let projection_geometry = validate_static_renderer(
                &overlay.decoded,
                base,
                renderer_evidence,
                residency,
                counts,
            )?;
            let streams =
                validate_selector_streams(&overlay.decoded, base, stream_table_evidence, counts)?;
            let consumer_reachability = validate_source_read_reachability(
                &overlay.decoded,
                base,
                &[renderer_evidence.exported_callback_runtime_address.as_str()],
                projection_binding
                    .source_read_reachability_evidence
                    .as_ref()
                    .context("static tile-stream projection has no reachability evidence")?,
                counts,
            )?;
            source_footprint = Some(derive_static_source_footprint(
                projection_source.id.clone(),
                projection_binding.source_atlas_domain_id.clone(),
                &streams,
                source_tile_geometry(source_atlas, projection_geometry)?,
                consumer_reachability,
            )?);
        }
        PracticalResultConsumerMechanism::GDynamicConfigTileStreamProjection {
            renderer_evidence,
            config_graph_evidence,
            ..
        } => {
            ensure!(
                matches!(
                    projection_binding.source_read_reachability_evidence.as_ref(),
                    Some(
                        PracticalResultSourceReadReachabilityEvidence::DormantOutsideCompleteLoadedImageHeaderInterface {
                            ..
                        }
                    )
                ),
                "dynamic tile-stream candidate must use complete-header dormancy evidence"
            );
            let projection_geometry = validate_g_dynamic_renderer(
                &overlay.decoded,
                base,
                renderer_evidence,
                residency,
                counts,
            )?;
            let graph = validate_dynamic_config_graph(
                &overlay.decoded,
                base,
                config_graph_evidence,
                counts,
            )?;
            let consumer_reachability = validate_source_read_reachability(
                &overlay.decoded,
                base,
                &[renderer_evidence.function_runtime_address.as_str()],
                projection_binding
                    .source_read_reachability_evidence
                    .as_ref()
                    .context("dynamic tile-stream projection has no reachability evidence")?,
                counts,
            )?;
            source_footprint = Some(derive_dynamic_source_footprint(
                projection_source.id.clone(),
                projection_binding.source_atlas_domain_id.clone(),
                &graph,
                source_tile_geometry(source_atlas, projection_geometry)?,
                consumer_reachability,
            )?);
        }
        PracticalResultConsumerMechanism::GoDynamicConfigTileStreamProjection {
            renderer_evidence,
            config_graph_evidence,
            ..
        } => {
            ensure!(
                matches!(
                    projection_binding.source_read_reachability_evidence.as_ref(),
                    Some(
                        PracticalResultSourceReadReachabilityEvidence::DormantOutsideCompleteLoadedImageHeaderInterface {
                            ..
                        }
                    )
                ),
                "dynamic tile-stream candidate must use complete-header dormancy evidence"
            );
            let projection_geometry = validate_go_dynamic_renderer(
                &overlay.decoded,
                base,
                renderer_evidence,
                residency,
                counts,
            )?;
            let graph = validate_dynamic_config_graph(
                &overlay.decoded,
                base,
                config_graph_evidence,
                counts,
            )?;
            let consumer_reachability = validate_source_read_reachability(
                &overlay.decoded,
                base,
                &[renderer_evidence.function_runtime_address.as_str()],
                projection_binding
                    .source_read_reachability_evidence
                    .as_ref()
                    .context("dynamic tile-stream projection has no reachability evidence")?,
                counts,
            )?;
            source_footprint = Some(derive_dynamic_source_footprint(
                projection_source.id.clone(),
                projection_binding.source_atlas_domain_id.clone(),
                &graph,
                source_tile_geometry(source_atlas, projection_geometry)?,
                consumer_reachability,
            )?);
        }
        PracticalResultConsumerMechanism::CatalogDescriptorRenderer { .. } => {
            anyhow::bail!("descriptor renderer appeared in the projection population")
        }
    }
    Ok(ValidatedProjectionMechanism {
        matched_authored_entry_id,
        source_footprint,
    })
}

pub(super) fn derive_alternative_source_footprint(
    mechanism: &PracticalResultConsumerMechanism,
    binding: &PracticalResultAlternativeSourceBinding,
    source_atlas: &PracticalResultSourceAtlasDomain,
    residency: &PracticalResultVramResidency,
    clut: &ValidatedClut<'_>,
    descriptors: &BTreeMap<&str, DescriptorView<'_>>,
    base_footprint: &SourceProjectionFootprint<
        PracticalResultConsumerProjectionId,
        SourceAtlasDomainId,
    >,
) -> Result<SourceProjectionFootprint<PracticalResultConsumerProjectionId, SourceAtlasDomainId>> {
    ensure!(
        base_footprint.tile_geometry.is_none()
            && base_footprint.decoded_tile_ids.is_none()
            && base_footprint.selector_referenced_tile_ids.is_none()
            && base_footprint.structural_stream_source_offsets.is_none()
            && base_footprint
                .selector_referenced_stream_source_offsets
                .is_none()
            && base_footprint.dynamic_stream_state_headers.is_none()
            && base_footprint.selector_referenced_rectangles.is_none(),
        "alternative source binding {} is not a declared-rectangle projection",
        binding.source_projection_id
    );
    if let PracticalResultConsumerMechanism::CatalogDescriptorProjection {
        descriptor_evidence,
        ..
    } = mechanism
    {
        let descriptor = descriptors
            .get(descriptor_evidence.descriptor_id.as_str())
            .context("alternative source binding names an unknown descriptor")?;
        validate_catalog_descriptor_physical_binding(
            binding.source_projection_id.as_str(),
            descriptor,
            residency,
            clut,
        )?;
    }
    derive_declared_rectangle_source_footprint(
        binding.source_projection_id.clone(),
        binding.source_atlas_domain_id.clone(),
        base_footprint.source_read_rectangles.iter().copied(),
        base_footprint.derivation,
        source_atlas.pixel_width,
        source_atlas.pixel_height,
        base_footprint.consumer_reachability,
    )
    .map_err(Into::into)
}

fn validate_catalog_descriptor_physical_binding(
    projection_id: &str,
    descriptor: &DescriptorView<'_>,
    residency: &PracticalResultVramResidency,
    clut: &ValidatedClut<'_>,
) -> Result<()> {
    ensure!(
        descriptor.fragments.iter().all(|fragment| {
            fragment.texture_page == residency.texture_page
                && fragment.texture_bank == residency.texture_bank
                && fragment
                    .source_u
                    .checked_add(fragment.width)
                    .is_some_and(|end| end <= residency.pixel_width)
                && fragment
                    .source_v
                    .checked_add(fragment.height)
                    .is_some_and(|end| end <= residency.pixel_height)
        }),
        "projection {projection_id} descriptor geometry differs from its residency"
    );
    ensure!(
        descriptor.fragments.iter().all(|fragment| {
            fragment.clut_x_index.checked_mul(16) == Some(clut.binding.clut_vram_x)
                && SECONDARY_DESCRIPTOR_CLUT_BASE_Y.checked_add(fragment.clut_y_offset)
                    == Some(clut.binding.clut_vram_y)
        }),
        "projection {projection_id} descriptor CLUT differs from its binding"
    );
    Ok(())
}

fn validate_adopted_source_read_reachability(
    overlay: &[u8],
    base: u32,
    expected_consumer_addresses: &[&str],
    binding: &PracticalResultProjectionBinding,
    counts: &mut ValidationCounts,
) -> Result<super::footprint::SourceConsumerReachability> {
    ensure!(
        matches!(
            binding.source_read_reachability_evidence.as_ref(),
            Some(
                PracticalResultSourceReadReachabilityEvidence::AdoptedDeclaredEntrypointClosure { .. }
            )
        ),
        "declared-rectangle projection must use adopted entrypoint-closure evidence"
    );
    validate_source_read_reachability(
        overlay,
        base,
        expected_consumer_addresses,
        binding
            .source_read_reachability_evidence
            .as_ref()
            .context("declared-rectangle projection has no reachability evidence")?,
        counts,
    )
}

fn source_rectangle(rectangle: &PracticalResultSourceRect) -> SourceAtlasRectangle {
    SourceAtlasRectangle {
        x: rectangle.u,
        y: rectangle.v,
        width: rectangle.width,
        height: rectangle.height,
    }
}

fn digit_geometry_rectangles(
    geometry: &PracticalResultDigitSourceGeometry,
) -> Vec<SourceAtlasRectangle> {
    geometry
        .u_by_raw_value
        .iter()
        .map(|&x| SourceAtlasRectangle {
            x,
            y: geometry.v,
            width: geometry.width,
            height: geometry.height,
        })
        .collect()
}

fn direct_numeric_consumer_address(site: &PracticalResultDirectNumericSpriteSite) -> &str {
    match site {
        PracticalResultDirectNumericSpriteSite::Geometry(site) => {
            &site.texture_page_setup_runtime_address
        }
        PracticalResultDirectNumericSpriteSite::Lookup(site) => {
            &site.texture_page_setup_runtime_address
        }
    }
}

fn direct_numeric_source_rectangles(
    sites: &[PracticalResultDirectNumericSpriteSite],
) -> Vec<SourceAtlasRectangle> {
    sites
        .iter()
        .flat_map(|site| match site {
            PracticalResultDirectNumericSpriteSite::Geometry(site) => site
                .source_rects
                .iter()
                .map(source_rectangle)
                .collect::<Vec<_>>(),
            PracticalResultDirectNumericSpriteSite::Lookup(site) => {
                digit_geometry_rectangles(&site.source_geometry)
            }
        })
        .collect()
}

fn source_tile_geometry(
    source_atlas: &PracticalResultSourceAtlasDomain,
    projection: ValidatedTileProjectionGeometry,
) -> Result<SourceAtlasTileGeometry> {
    ensure!(
        projection.column_count.checked_mul(projection.tile_width)
            == Some(source_atlas.pixel_width)
            && source_atlas
                .pixel_height
                .is_multiple_of(projection.tile_height),
        "typed source-tile projection does not partition source-atlas domain {}",
        source_atlas.id
    );
    Ok(SourceAtlasTileGeometry {
        atlas_width: source_atlas.pixel_width,
        atlas_height: source_atlas.pixel_height,
        tile_width: projection.tile_width,
        tile_height: projection.tile_height,
    })
}

pub(super) fn projection_parts(
    mechanism: &PracticalResultConsumerMechanism,
) -> Result<(
    &PracticalResultProjectionSource,
    &PracticalResultProjectionBinding,
)> {
    match mechanism {
        PracticalResultConsumerMechanism::CatalogDescriptorProjection {
            source, binding, ..
        }
        | PracticalResultConsumerMechanism::CatalogDigitDescriptorProjection {
            source,
            binding,
            ..
        }
        | PracticalResultConsumerMechanism::DirectNumericSpriteSitesProjection {
            source,
            binding,
            ..
        }
        | PracticalResultConsumerMechanism::DirectSpriteSelectorProjection {
            source,
            binding,
            ..
        }
        | PracticalResultConsumerMechanism::GCatalogDescriptorSelectorProjection {
            source,
            binding,
            ..
        }
        | PracticalResultConsumerMechanism::GDynamicConfigTileStreamProjection {
            source,
            binding,
            ..
        }
        | PracticalResultConsumerMechanism::GoCatalogDescriptorSelectorProjection {
            source,
            binding,
            ..
        }
        | PracticalResultConsumerMechanism::GoDynamicConfigTileStreamProjection {
            source,
            binding,
            ..
        }
        | PracticalResultConsumerMechanism::NumericLookupRendererProjection {
            source,
            binding,
            ..
        }
        | PracticalResultConsumerMechanism::StaticMatrixTileStreamProjection {
            source,
            binding,
            ..
        }
        | PracticalResultConsumerMechanism::StaticSelectorTileStreamProjection {
            source,
            binding,
            ..
        } => Ok((source, binding)),
        PracticalResultConsumerMechanism::CatalogDescriptorRenderer { .. } => {
            anyhow::bail!("descriptor renderer appeared in the projection population")
        }
    }
}
