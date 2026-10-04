//! Projection status, semantic, physical, protected, and occurrence-ledger links.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};

use crate::mode_descendant_graphics::source::ModeDescendantSourceRecord;
use crate::pipeline::sha256_bytes;

use super::super::model::{
    PhysicalRegionId, PracticalResultAssets, PracticalResultDevelopmentStatus,
    PracticalResultEntry, PracticalResultSourceUsageStatus, PracticalResultStrategy,
    SourceReferenceId,
};
use super::super::projection_model::*;
use super::super::source_ownership::{
    PhysicalSourceCellKey, PracticalResultSourceOwnership, SemanticSourceReference,
};
use super::consumers::projection_parts;
use super::descriptor::DescriptorView;
use super::source_evidence::{parse_hex, source_for_path, validate_declared_overlay_base};

#[derive(Debug, PartialEq, Eq)]
pub(super) struct SemanticProjectionSignature {
    residency_id: String,
    clut_binding_id: String,
    ordered_physical_cells: Vec<PhysicalSourceCellKey>,
}

pub(super) struct ProjectionLinkAuthorities<'a> {
    entries: BTreeMap<&'a str, &'a PracticalResultEntry>,
    physical_ids: BTreeSet<&'a str>,
    protected_ids: BTreeSet<&'a str>,
    references: BTreeMap<&'a str, &'a SemanticSourceReference>,
}

impl<'a> ProjectionLinkAuthorities<'a> {
    pub(super) fn from_assets(
        assets: &'a PracticalResultAssets,
        ownership: &'a PracticalResultSourceOwnership,
    ) -> Self {
        Self {
            entries: assets
                .entries
                .iter()
                .map(|entry| (entry.id.as_str(), entry))
                .collect(),
            physical_ids: assets
                .physical_region_catalog
                .regions
                .iter()
                .map(|region| region.region_id.as_str())
                .collect(),
            protected_ids: assets
                .protected_content
                .regions
                .iter()
                .map(|region| region.id.as_str())
                .collect(),
            references: ownership
                .semantic_references
                .iter()
                .map(|reference| (reference.reference_id.as_str(), reference))
                .collect(),
        }
    }

    pub(super) fn contains_protected(&self, id: &str) -> bool {
        self.protected_ids.contains(id)
    }
}

pub(super) fn validate_action_consumer_occurrence_ledger<'a>(
    assets: &'a PracticalResultAssets,
    sources: &[ModeDescendantSourceRecord],
    descriptors: &BTreeMap<&str, DescriptorView<'_>>,
    links: &ProjectionLinkAuthorities<'_>,
) -> Result<BTreeMap<&'a str, &'a PracticalResultActionConsumerOccurrence>> {
    ensure!(
        !assets
            .action_consumer_occurrence_catalog
            .occurrences
            .is_empty(),
        "practical-result action consumer occurrence catalog is empty"
    );
    let mut occurrences = BTreeMap::new();
    let mut descriptor_ids = BTreeSet::new();
    let mut consumer_ids = BTreeSet::new();
    for occurrence in &assets.action_consumer_occurrence_catalog.occurrences {
        ensure!(
            !occurrence.occurrence_id.is_empty()
                && !occurrence.consumer_id.is_empty()
                && !occurrence.descriptor_id.is_empty()
                && occurrences
                    .insert(occurrence.occurrence_id.as_str(), occurrence)
                    .is_none()
                && descriptor_ids.insert(occurrence.descriptor_id.as_str())
                && consumer_ids.insert(occurrence.consumer_id.as_str()),
            "duplicate or empty practical-result action consumer occurrence identity"
        );
        let overlay = source_for_path(sources, &occurrence.overlay_path)?;
        ensure!(
            overlay.decoded.len() == occurrence.overlay_size
                && sha256_bytes(&overlay.decoded) == occurrence.overlay_sha256,
            "action consumer occurrence {} overlay identity changed",
            occurrence.occurrence_id
        );
        validate_declared_overlay_base(
            &occurrence.overlay_path,
            &occurrence.runtime_base,
            "action consumer occurrence",
        )?;
        let descriptor = descriptors
            .get(occurrence.descriptor_id.as_str())
            .with_context(|| {
                format!(
                    "action consumer occurrence {} names unknown descriptor {}",
                    occurrence.occurrence_id, occurrence.descriptor_id
                )
            })?;
        ensure!(
            occurrence.consumer_id.as_str() == descriptor.consumer_id.as_str()
                && occurrence.overlay_path == descriptor.overlay_path
                && occurrence.overlay_sha256 == descriptor.overlay_sha256
                && occurrence.runtime_base == descriptor.runtime_base
                && occurrence.pointer_table_offset == descriptor.pointer_table_offset
                && occurrence.pointer_table_runtime_address
                    == descriptor.pointer_table_runtime_address
                && occurrence.pointer_table_size == descriptor.pointer_table_size
                && occurrence.pointer_table_sha256 == descriptor.pointer_table_sha256
                && occurrence.aliases == descriptor.aliases
                && occurrence.descriptor_offset == descriptor.descriptor_offset
                && occurrence.descriptor_runtime_address == descriptor.descriptor_runtime_address
                && occurrence.descriptor_capacity == descriptor.descriptor_capacity
                && occurrence.descriptor_sha256 == descriptor.descriptor_sha256,
            "action consumer occurrence {} differs from its binary-validated descriptor declaration",
            occurrence.occurrence_id
        );
        let has_reason = occurrence
            .unresolved_reason
            .as_deref()
            .is_some_and(|reason| !reason.trim().is_empty());
        ensure!(
            occurrence.unresolved_reason.is_none() || has_reason,
            "action consumer occurrence {} has an empty unresolved reason",
            occurrence.occurrence_id
        );
        match &occurrence.semantic_entry_id {
            Some(entry_id) => {
                let entry = links.entries.get(entry_id.as_str()).with_context(|| {
                    format!(
                        "action consumer occurrence {} names unknown semantic entry {}",
                        occurrence.occurrence_id, entry_id
                    )
                })?;
                ensure!(
                    entry.strategy == PracticalResultStrategy::GlyphSequence
                        && entry.development_status == PracticalResultDevelopmentStatus::Authored
                        && matches!(
                            (occurrence.evidence_status, occurrence.semantic_status),
                            (
                                PracticalResultProjectionEvidenceStatus::ConfirmedAndRuntimeObserved,
                                PracticalResultProjectionSemanticStatus::RuntimeObserved
                            ) | (
                                PracticalResultProjectionEvidenceStatus::Confirmed,
                                PracticalResultProjectionSemanticStatus::StaticConfirmed
                            )
                        )
                        && !has_reason,
                    "semantic action consumer occurrence {} has an inconsistent evidence tuple",
                    occurrence.occurrence_id
                );
            }
            None => ensure!(
                occurrence.evidence_status
                    == PracticalResultProjectionEvidenceStatus::DescriptorOnly
                    && occurrence.semantic_status
                        == PracticalResultProjectionSemanticStatus::Unresolved
                    && has_reason,
                "unresolved action consumer occurrence {} has an inconsistent evidence tuple",
                occurrence.occurrence_id
            ),
        }
    }
    for occurrence in occurrences.values().filter(|occurrence| {
        occurrence.evidence_status == PracticalResultProjectionEvidenceStatus::Confirmed
            && occurrence.semantic_status
                == PracticalResultProjectionSemanticStatus::StaticConfirmed
    }) {
        ensure!(
            occurrences.values().any(|observed| {
                observed.evidence_status
                    == PracticalResultProjectionEvidenceStatus::ConfirmedAndRuntimeObserved
                    && observed.semantic_status
                        == PracticalResultProjectionSemanticStatus::RuntimeObserved
                    && observed.semantic_entry_id == occurrence.semantic_entry_id
                    && observed.aliases == occurrence.aliases
                    && observed.descriptor_capacity == occurrence.descriptor_capacity
                    && observed.descriptor_sha256 == occurrence.descriptor_sha256
            }),
            "static semantic action occurrence {} has no byte-identical runtime-observed counterpart",
            occurrence.occurrence_id
        );
    }
    ensure!(
        descriptor_ids == descriptors.keys().copied().collect::<BTreeSet<_>>(),
        "action consumer occurrence ledger and descriptor declarations are not an exact bijection"
    );
    Ok(occurrences)
}

pub(super) fn validate_action_consumer_occurrence_projection(
    mechanism: &PracticalResultConsumerMechanism,
    occurrence: &PracticalResultActionConsumerOccurrence,
) -> Result<()> {
    let PracticalResultConsumerMechanism::CatalogDescriptorProjection {
        occurrence_id,
        source,
        binding,
        descriptor_evidence,
        semantic_entry_id,
        ..
    } = mechanism
    else {
        anyhow::bail!("non-action projection appeared in the action occurrence population")
    };
    ensure!(
        occurrence_id.as_str() == occurrence.occurrence_id.as_str()
            && source.consumer_id.as_str() == occurrence.consumer_id.as_str()
            && source.overlay_path == occurrence.overlay_path
            && source.overlay_size == occurrence.overlay_size
            && source.overlay_sha256 == occurrence.overlay_sha256
            && source.runtime_base == occurrence.runtime_base
            && descriptor_evidence.descriptor_id.as_str() == occurrence.descriptor_id.as_str()
            && descriptor_evidence.pointer_table_offset == occurrence.pointer_table_offset
            && descriptor_evidence.selector_indices == occurrence.aliases
            && descriptor_evidence.descriptor_offset == occurrence.descriptor_offset
            && descriptor_evidence.descriptor_runtime_address
                == occurrence.descriptor_runtime_address
            && descriptor_evidence.descriptor_capacity == occurrence.descriptor_capacity
            && descriptor_evidence.descriptor_sha256 == occurrence.descriptor_sha256
            && semantic_entry_id
                .as_ref()
                .map(PracticalResultSemanticEntryId::as_str)
                == occurrence
                    .semantic_entry_id
                    .as_ref()
                    .map(PracticalResultSemanticEntryId::as_str)
            && binding.evidence_status == occurrence.evidence_status
            && binding.semantic_status == occurrence.semantic_status
            && binding.unresolved_reason.as_deref() == occurrence.unresolved_reason.as_deref(),
        "catalog-descriptor projection {} differs from action consumer occurrence {}",
        source.id,
        occurrence.occurrence_id
    );
    Ok(())
}

pub(super) fn validate_projection_status(
    mechanism: &PracticalResultConsumerMechanism,
    clut: &PracticalResultClutBinding,
) -> Result<()> {
    let (source, binding) = projection_parts(mechanism)?;
    let has_reason = binding
        .unresolved_reason
        .as_deref()
        .is_some_and(|reason| !reason.trim().is_empty());
    ensure!(
        binding.unresolved_reason.is_none() || has_reason,
        "projection {} has an empty unresolved reason",
        source.id
    );
    let clut_is_resolved = matches!(
        clut.evidence_status(),
        PracticalResultClutBindingEvidenceStatus::StaticConfirmed
            | PracticalResultClutBindingEvidenceStatus::StaticConfirmedExternalProducerFamily
    );
    let valid = match mechanism {
        PracticalResultConsumerMechanism::CatalogDescriptorProjection {
            semantic_entry_id, ..
        } => match semantic_entry_id {
            Some(_) => {
                matches!(
                    (binding.evidence_status, binding.semantic_status),
                    (
                        PracticalResultProjectionEvidenceStatus::ConfirmedAndRuntimeObserved,
                        PracticalResultProjectionSemanticStatus::RuntimeObserved
                    ) | (
                        PracticalResultProjectionEvidenceStatus::Confirmed,
                        PracticalResultProjectionSemanticStatus::StaticConfirmed
                    )
                ) && !has_reason
                    && clut_is_resolved
            }
            None => {
                binding.evidence_status == PracticalResultProjectionEvidenceStatus::DescriptorOnly
                    && binding.semantic_status
                        == PracticalResultProjectionSemanticStatus::Unresolved
                    && has_reason
                    && clut_is_resolved
            }
        },
        PracticalResultConsumerMechanism::CatalogDigitDescriptorProjection { .. }
        | PracticalResultConsumerMechanism::DirectSpriteSelectorProjection { .. } => {
            binding.evidence_status == PracticalResultProjectionEvidenceStatus::Confirmed
                && binding.semantic_status
                    == PracticalResultProjectionSemanticStatus::StaticConfirmed
                && !has_reason
                && clut_is_resolved
        }
        PracticalResultConsumerMechanism::DirectNumericSpriteSitesProjection { .. }
        | PracticalResultConsumerMechanism::GCatalogDescriptorSelectorProjection { .. }
        | PracticalResultConsumerMechanism::GoCatalogDescriptorSelectorProjection { .. }
        | PracticalResultConsumerMechanism::NumericLookupRendererProjection { .. } => {
            binding.evidence_status == PracticalResultProjectionEvidenceStatus::Confirmed
                && binding.semantic_status
                    == PracticalResultProjectionSemanticStatus::PartiallyResolved
                && has_reason
                && clut_is_resolved
        }
        PracticalResultConsumerMechanism::StaticMatrixTileStreamProjection { .. }
        | PracticalResultConsumerMechanism::StaticSelectorTileStreamProjection { .. } => {
            binding.evidence_status
                == PracticalResultProjectionEvidenceStatus::ConsumerPathConfirmed
                && binding.semantic_status == PracticalResultProjectionSemanticStatus::Unresolved
                && has_reason
                && clut_is_resolved
        }
        PracticalResultConsumerMechanism::GDynamicConfigTileStreamProjection { .. }
        | PracticalResultConsumerMechanism::GoDynamicConfigTileStreamProjection { .. } => {
            binding.evidence_status
                == PracticalResultProjectionEvidenceStatus::DormantOutsideCompleteHeaderInterfaceConfirmed
                && binding.semantic_status == PracticalResultProjectionSemanticStatus::Unresolved
                && has_reason
                && clut_is_resolved
        }
        PracticalResultConsumerMechanism::CatalogDescriptorRenderer { .. } => false,
    };
    ensure!(
        valid && binding.source_read_reachability_evidence.is_some(),
        "projection {} mechanism, evidence, semantic, reason, semantic-presence, and CLUT tuple conflicts",
        source.id
    );
    Ok(())
}

pub(super) fn validate_descriptor_semantic_cells(
    projection_id: &str,
    descriptor: &DescriptorView<'_>,
    residency: &PracticalResultVramResidency,
    ordered_reference_ids: &[SourceReferenceId],
    physical_region_ids: &[PhysicalRegionId],
    links: &ProjectionLinkAuthorities<'_>,
) -> Result<()> {
    let descriptor_cells = normalized_descriptor_cells(descriptor, residency)?.with_context(|| {
        format!(
            "semantic projection {projection_id} descriptor fragments are not aligned 20x20 residency cells"
        )
    })?;
    ensure!(
        descriptor_cells.len() == ordered_reference_ids.len()
            && descriptor_cells.len() == physical_region_ids.len(),
        "semantic projection {projection_id} descriptor/source-reference cell denominator changed"
    );
    for ((descriptor_cell, reference_id), physical_region_id) in descriptor_cells
        .iter()
        .zip(ordered_reference_ids)
        .zip(physical_region_ids)
    {
        let reference = links
            .references
            .get(reference_id.as_str())
            .with_context(|| {
                format!(
                    "semantic projection {projection_id} names unknown reference {reference_id}"
                )
            })?;
        ensure!(
            reference.physical_region_id.as_str() == physical_region_id.as_str()
                && &reference.physical_cell == descriptor_cell,
            "semantic projection {projection_id} descriptor cells differ from its ordered physical references"
        );
    }
    Ok(())
}

pub(super) fn normalized_descriptor_cells(
    descriptor: &DescriptorView<'_>,
    residency: &PracticalResultVramResidency,
) -> Result<Option<Vec<PhysicalSourceCellKey>>> {
    const CELL_SIZE: usize = 20;
    const TEXTURE_PAGE_PIXEL_WIDTH: usize = 256;

    let residency_page_x = usize::from(residency.texture_page)
        .checked_mul(TEXTURE_PAGE_PIXEL_WIDTH)
        .context("residency texture-page coordinate overflow")?;
    let tim_offset = parse_hex(&residency.tim_offset, "semantic residency TIM offset")?;
    let mut cells = Vec::new();
    for fragment in descriptor.fragments {
        if fragment.texture_bank != residency.texture_bank
            || fragment.width == 0
            || fragment.height == 0
            || fragment.width % CELL_SIZE != 0
            || fragment.height % CELL_SIZE != 0
        {
            return Ok(None);
        }
        let fragment_page_x = usize::from(fragment.texture_page)
            .checked_mul(TEXTURE_PAGE_PIXEL_WIDTH)
            .context("descriptor texture-page coordinate overflow")?;
        let fragment_global_x = fragment_page_x
            .checked_add(fragment.source_u)
            .context("descriptor fragment X coordinate overflow")?;
        let Some(local_x) = fragment_global_x.checked_sub(residency_page_x) else {
            return Ok(None);
        };
        if local_x % CELL_SIZE != 0 || fragment.source_v % CELL_SIZE != 0 {
            return Ok(None);
        }
        let local_x_end = local_x
            .checked_add(fragment.width)
            .context("descriptor fragment width overflow")?;
        let local_y_end = fragment
            .source_v
            .checked_add(fragment.height)
            .context("descriptor fragment height overflow")?;
        if local_x_end > residency.pixel_width || local_y_end > residency.pixel_height {
            return Ok(None);
        }
        for y in (fragment.source_v..local_y_end).step_by(CELL_SIZE) {
            for x in (local_x..local_x_end).step_by(CELL_SIZE) {
                cells.push(PhysicalSourceCellKey {
                    source_path: residency.source_path.clone(),
                    tim_offset,
                    bpp: residency.bpp,
                    x,
                    y,
                    width: CELL_SIZE,
                    height: CELL_SIZE,
                });
            }
        }
    }
    Ok((!cells.is_empty()).then_some(cells))
}

pub(super) fn validate_repeated_semantic_projection(
    mechanism: &PracticalResultConsumerMechanism,
    links: &ProjectionLinkAuthorities<'_>,
    signatures: &mut BTreeMap<String, SemanticProjectionSignature>,
) -> Result<()> {
    let PracticalResultConsumerMechanism::CatalogDescriptorProjection {
        source,
        binding,
        semantic_entry_id: Some(entry_id),
        ordered_source_reference_ids: Some(ordered_reference_ids),
        ..
    } = mechanism
    else {
        return Ok(());
    };
    let ordered_physical_cells = ordered_reference_ids
        .iter()
        .map(|reference_id| {
            let reference = links
                .references
                .get(reference_id.as_str())
                .with_context(|| {
                    format!(
                        "projection {} names unknown semantic reference {}",
                        source.id, reference_id
                    )
                })?;
            ensure!(
                reference.entry_id == entry_id.as_str(),
                "projection {} semantic reference belongs to another entry",
                source.id
            );
            Ok(reference.physical_cell.clone())
        })
        .collect::<Result<Vec<_>>>()?;
    let signature = SemanticProjectionSignature {
        residency_id: binding.residency_id.as_str().to_string(),
        clut_binding_id: binding.clut_binding_id.as_str().to_string(),
        ordered_physical_cells,
    };
    if let Some(existing) = signatures.get(entry_id.as_str()) {
        ensure!(
            existing == &signature,
            "repeated semantic projection {entry_id} changes residency, CLUT, or ordered physical cells"
        );
    } else {
        signatures.insert(entry_id.as_str().to_string(), signature);
    }
    Ok(())
}

pub(super) fn projection_has_nested_unresolved(
    mechanism: &PracticalResultConsumerMechanism,
) -> bool {
    projection_parts(mechanism).is_ok_and(|(_, binding)| {
        binding.unresolved_reason.is_some()
            || matches!(
                binding.semantic_status,
                PracticalResultProjectionSemanticStatus::PartiallyResolved
                    | PracticalResultProjectionSemanticStatus::Unresolved
            )
    })
}

pub(super) fn validate_semantic_links(
    projection_id: &str,
    semantic_status: PracticalResultProjectionSemanticStatus,
    semantic_entry_id: Option<&str>,
    ordered_reference_ids: Option<&[SourceReferenceId]>,
    physical_region_ids: &[PhysicalRegionId],
    links: &ProjectionLinkAuthorities<'_>,
) -> Result<Option<String>> {
    let Some(entry_id) = semantic_entry_id else {
        ensure!(
            ordered_reference_ids.is_none_or(<[_]>::is_empty),
            "projection {projection_id} has references without semantic authority"
        );
        validate_physical_ids(physical_region_ids, links)?;
        return Ok(None);
    };
    let entry = links
        .entries
        .get(entry_id)
        .with_context(|| format!("projection {projection_id} names unknown entry {entry_id}"))?;
    ensure!(
        entry.strategy == PracticalResultStrategy::GlyphSequence
            && entry.development_status == PracticalResultDevelopmentStatus::Authored
            && entry.unresolved_source_references.is_empty(),
        "projection {projection_id} semantic authority is not a fully resolved authored glyph sequence"
    );
    let ordered_reference_ids = ordered_reference_ids
        .context("semantic-bound projection has no ordered source references")?;
    ensure!(
        !ordered_reference_ids.is_empty()
            && ordered_reference_ids.len() == entry.source_references.len()
            && ordered_reference_ids.len() == physical_region_ids.len(),
        "projection {projection_id} semantic/physical denominator changed"
    );
    let mut references_by_sequence = vec![None; entry.source_references.len()];
    for source_reference in &entry.source_references {
        let sequence_index = source_reference.sequence_index.with_context(|| {
            format!("projection {projection_id} source reference has no sequence index")
        })?;
        ensure!(
            sequence_index < references_by_sequence.len()
                && references_by_sequence[sequence_index]
                    .replace(source_reference)
                    .is_none(),
            "projection {projection_id} source-reference sequence is duplicated or out of range"
        );
    }
    for (index, ((reference_id, physical_id), source_reference)) in ordered_reference_ids
        .iter()
        .zip(physical_region_ids)
        .zip(references_by_sequence)
        .enumerate()
    {
        let source_reference = source_reference
            .with_context(|| format!("projection {projection_id} source sequence has a gap"))?;
        let reference = links
            .references
            .get(reference_id.as_str())
            .with_context(|| {
                format!("projection {projection_id} names unknown source reference {reference_id}")
            })?;
        ensure!(
            source_reference.reference_id == *reference_id
                && source_reference.physical_region_id == *physical_id
                && reference.entry_id == entry_id
                && reference.sequence_index == Some(index)
                && reference.physical_region_id.as_str() == physical_id.as_str()
                && links.physical_ids.contains(physical_id.as_str()),
            "projection {projection_id} semantic order or physical link changed"
        );
        if semantic_status == PracticalResultProjectionSemanticStatus::RuntimeObserved {
            ensure!(
                source_reference.source_usage_status
                    == PracticalResultSourceUsageStatus::RuntimeObserved,
                "runtime-observed projection {projection_id} has a source reference without runtime-observed usage"
            );
        }
    }
    Ok(Some(entry_id.to_string()))
}

pub(super) fn validate_physical_ids(
    ids: &[PhysicalRegionId],
    links: &ProjectionLinkAuthorities<'_>,
) -> Result<()> {
    ensure!(
        ids.iter()
            .all(|id| links.physical_ids.contains(id.as_str())),
        "projection names an unknown physical region"
    );
    Ok(())
}

pub(super) fn validate_protected_ids(
    ids: &[PracticalResultProtectedRegionId],
    links: &ProjectionLinkAuthorities<'_>,
) -> Result<()> {
    ensure!(
        ids.iter()
            .all(|id| !id.is_empty() && links.protected_ids.contains(id.as_str())),
        "projection names an unknown protected region"
    );
    Ok(())
}
