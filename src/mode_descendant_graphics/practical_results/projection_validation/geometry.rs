//! Source-atlas geometry and selector denominator validation.

use std::collections::BTreeSet;

use anyhow::{Result, ensure};

use super::super::projection_model::*;
use super::super::source_atlas_domain_model::PracticalResultSourceAtlasDomain;

pub(super) fn validate_projection_geometry(
    mechanism: &PracticalResultConsumerMechanism,
    source_atlas: &PracticalResultSourceAtlasDomain,
) -> Result<()> {
    match mechanism {
        PracticalResultConsumerMechanism::CatalogDigitDescriptorProjection {
            descriptor_table_evidence,
            ..
        } => {
            validate_catalog_selector_entries(
                descriptor_table_evidence
                    .raw_value_entries
                    .iter()
                    .map(|entry| (entry.raw_value, entry.descriptor_index, &entry.source_rect)),
                source_atlas,
                "digit descriptor",
            )?;
        }
        PracticalResultConsumerMechanism::DirectNumericSpriteSitesProjection { sites, .. } => {
            for site in sites {
                match site {
                    PracticalResultDirectNumericSpriteSite::Geometry(site) => {
                        for rect in &site.source_rects {
                            validate_source_rect(rect, source_atlas, "direct numeric geometry")?;
                        }
                    }
                    PracticalResultDirectNumericSpriteSite::Lookup(site) => {
                        validate_digit_geometry_bounds(
                            &site.source_geometry,
                            source_atlas,
                            "direct numeric lookup",
                        )?;
                    }
                }
            }
        }
        PracticalResultConsumerMechanism::DirectSpriteSelectorProjection {
            selector_evidence,
            ..
        } => validate_direct_selector_denominator(selector_evidence, source_atlas)?,
        PracticalResultConsumerMechanism::GCatalogDescriptorSelectorProjection {
            selector_evidence,
            ..
        } => validate_catalog_selector_entries(
            selector_evidence.raw_selector_entries.iter().map(|entry| {
                (
                    entry.raw_selector_value,
                    entry.descriptor_index,
                    &entry.source_rect,
                )
            }),
            source_atlas,
            "G catalog selector",
        )?,
        PracticalResultConsumerMechanism::GoCatalogDescriptorSelectorProjection {
            selector_evidence,
            ..
        } => validate_catalog_selector_entries(
            selector_evidence.raw_selector_entries.iter().map(|entry| {
                (
                    entry.raw_selector_value,
                    entry.descriptor_index,
                    &entry.source_rect,
                )
            }),
            source_atlas,
            "GO catalog selector",
        )?,
        PracticalResultConsumerMechanism::NumericLookupRendererProjection {
            lookup_table_evidence,
            ..
        } => validate_digit_geometry_bounds(
            &lookup_table_evidence.source_geometry,
            source_atlas,
            "numeric lookup renderer",
        )?,
        _ => {}
    }
    Ok(())
}

pub(super) fn validate_catalog_selector_entries<'a>(
    entries: impl IntoIterator<Item = (usize, usize, &'a PracticalResultSourceRect)>,
    source_atlas: &PracticalResultSourceAtlasDomain,
    role: &str,
) -> Result<()> {
    let entries = entries.into_iter().collect::<Vec<_>>();
    let mut descriptor_indices = BTreeSet::new();
    ensure!(
        !entries.is_empty()
            && entries
                .iter()
                .enumerate()
                .all(|(expected, (raw, descriptor, _))| {
                    *raw == expected && descriptor_indices.insert(*descriptor)
                }),
        "{role} raw values or descriptor-index denominator changed"
    );
    for (_, _, rect) in entries {
        validate_source_rect(rect, source_atlas, role)?;
    }
    Ok(())
}

pub(super) fn validate_direct_selector_denominator(
    evidence: &PracticalResultDirectSpriteSelectorEvidence,
    source_atlas: &PracticalResultSourceAtlasDomain,
) -> Result<()> {
    ensure!(
        !evidence.raw_selector_entries.is_empty(),
        "direct selector has no entries"
    );
    let mut raw_indices = Vec::new();
    let mut status_values = Vec::new();
    let mut protected_ids = BTreeSet::new();
    for entry in &evidence.raw_selector_entries {
        ensure!(
            !entry.raw_table_indices.is_empty()
                && entry.raw_table_indices.len() == entry.status_code_values.len()
                && entry
                    .raw_table_indices
                    .windows(2)
                    .all(|pair| pair[0] < pair[1])
                && entry
                    .status_code_values
                    .windows(2)
                    .all(|pair| pair[0] < pair[1])
                && !entry.protected_region_id.is_empty()
                && protected_ids.insert(entry.protected_region_id.as_str()),
            "direct selector entry has an invalid index/status/protected denominator"
        );
        raw_indices.extend_from_slice(&entry.raw_table_indices);
        status_values.extend_from_slice(&entry.status_code_values);
        validate_source_rect(&entry.source_rect, source_atlas, "direct selector")?;
    }
    ensure!(
        raw_indices
            .iter()
            .enumerate()
            .all(|(expected, found)| *found == expected)
            && status_values
                .iter()
                .enumerate()
                .all(|(index, found)| *found == index + 1),
        "direct selector raw-index/status order or population changed"
    );
    Ok(())
}

pub(super) fn validate_source_rect(
    rect: &PracticalResultSourceRect,
    source_atlas: &PracticalResultSourceAtlasDomain,
    role: &str,
) -> Result<()> {
    ensure!(
        rect.width > 0
            && rect.height > 0
            && rect
                .u
                .checked_add(rect.width)
                .is_some_and(|end| end <= source_atlas.pixel_width)
            && rect
                .v
                .checked_add(rect.height)
                .is_some_and(|end| end <= source_atlas.pixel_height),
        "{role} source rectangle escapes source-atlas domain {}",
        source_atlas.id
    );
    Ok(())
}

pub(super) fn validate_digit_geometry_bounds(
    geometry: &PracticalResultDigitSourceGeometry,
    source_atlas: &PracticalResultSourceAtlasDomain,
    role: &str,
) -> Result<()> {
    validate_digit_geometry(geometry)?;
    ensure!(
        geometry
            .raw_values
            .iter()
            .enumerate()
            .all(|(expected, found)| *found == expected)
            && geometry
                .u_by_raw_value
                .windows(2)
                .all(|pair| pair[0] < pair[1])
            && geometry.u_by_raw_value.iter().all(|u| {
                u.checked_add(geometry.width)
                    .is_some_and(|end| end <= source_atlas.pixel_width)
            })
            && geometry
                .v
                .checked_add(geometry.height)
                .is_some_and(|end| end <= source_atlas.pixel_height),
        "{role} digit raw order or source-atlas bounds changed"
    );
    Ok(())
}

pub(super) fn validate_digit_geometry(geometry: &PracticalResultDigitSourceGeometry) -> Result<()> {
    ensure!(
        !geometry.raw_values.is_empty()
            && geometry.raw_values.len() == geometry.u_by_raw_value.len()
            && geometry.width > 0
            && geometry.height > 0,
        "digit source-geometry denominator changed"
    );
    Ok(())
}
