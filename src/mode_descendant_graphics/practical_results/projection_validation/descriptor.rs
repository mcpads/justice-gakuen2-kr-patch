//! Central practical-exam descriptor parsing and catalog binding.

use std::collections::BTreeMap;

use anyhow::{Context, Result, ensure};

use crate::mode_descendant_graphics::practical_exam::{
    PracticalExamConsumer, PracticalExamSecondaryTextureBank,
    parse_practical_exam_secondary_descriptor,
};
use crate::mode_descendant_graphics::source::ModeDescendantSourceRecord;
use crate::pipeline::sha256_bytes;

use super::super::model::PracticalResultAssets;
use super::super::projection_model::*;
use super::super::source_atlas_domain_model::SourceAtlasDomainId;
use super::source_evidence::{
    ValidationCounts, address_and_span, ensure_strictly_increasing, source_for_path,
    validate_declared_overlay_base, validate_pointer_aliases,
};

pub(super) const SECONDARY_DESCRIPTOR_CLUT_BASE_Y: usize = 480;

pub(super) struct DescriptorView<'a> {
    pub(super) descriptor_id: &'a PracticalResultConsumerDescriptorId,
    pub(super) source_atlas_domain_ids: &'a [SourceAtlasDomainId],
    pub(super) consumer_id: &'a PracticalResultConsumerId,
    pub(super) overlay_path: &'a str,
    pub(super) overlay_sha256: &'a str,
    pub(super) runtime_base: &'a str,
    pub(super) renderer_offset: &'a str,
    pub(super) renderer_runtime_address: &'a str,
    pub(super) renderer_size: usize,
    pub(super) renderer_sha256: &'a str,
    pub(super) pointer_table_offset: &'a str,
    pub(super) pointer_table_runtime_address: &'a str,
    pub(super) pointer_table_size: usize,
    pub(super) pointer_table_sha256: &'a str,
    pub(super) aliases: &'a [usize],
    pub(super) descriptor_offset: &'a str,
    pub(super) descriptor_runtime_address: &'a str,
    pub(super) descriptor_capacity: usize,
    pub(super) descriptor_sha256: &'a str,
    pub(super) fragments: &'a [PracticalResultCatalogDescriptorFragment],
    pub(super) evidence_status: &'a PracticalResultProjectionEvidenceStatus,
}

pub(super) fn validate_descriptors<'a>(
    assets: &'a PracticalResultAssets,
    sources: &[ModeDescendantSourceRecord],
    counts: &mut ValidationCounts,
) -> Result<BTreeMap<&'a str, DescriptorView<'a>>> {
    let mut by_id = BTreeMap::new();
    for mechanism in &assets.consumer_projection_catalog.descriptors {
        let descriptor = descriptor_view(mechanism)?;
        ensure!(
            !descriptor.descriptor_id.is_empty()
                && !descriptor.consumer_id.is_empty()
                && by_id
                    .insert(descriptor.descriptor_id.as_str(), descriptor)
                    .is_none(),
            "duplicate or empty practical-result descriptor identity"
        );
    }
    ensure!(
        !by_id.is_empty(),
        "practical-result descriptor catalog is empty"
    );
    for descriptor in by_id.values() {
        let overlay = source_for_path(sources, descriptor.overlay_path)?;
        ensure!(
            sha256_bytes(&overlay.decoded) == descriptor.overlay_sha256,
            "descriptor {} overlay identity changed",
            descriptor.descriptor_id
        );
        ensure!(
            matches!(
                descriptor.evidence_status,
                PracticalResultProjectionEvidenceStatus::Confirmed
            ),
            "descriptor {} has an unsupported evidence status",
            descriptor.descriptor_id
        );
        let base = validate_declared_overlay_base(
            descriptor.overlay_path,
            descriptor.runtime_base,
            "descriptor",
        )?;
        address_and_span(
            &overlay.decoded,
            base,
            descriptor.renderer_offset,
            descriptor.renderer_runtime_address,
            descriptor.renderer_size,
            descriptor.renderer_sha256,
            "descriptor renderer",
            counts,
        )?;
        let pointer_table_offset = address_and_span(
            &overlay.decoded,
            base,
            descriptor.pointer_table_offset,
            descriptor.pointer_table_runtime_address,
            descriptor.pointer_table_size,
            descriptor.pointer_table_sha256,
            "descriptor pointer table",
            counts,
        )?;
        let descriptor_offset = address_and_span(
            &overlay.decoded,
            base,
            descriptor.descriptor_offset,
            descriptor.descriptor_runtime_address,
            descriptor.descriptor_capacity,
            descriptor.descriptor_sha256,
            "action descriptor",
            counts,
        )?;
        let parsed = parse_practical_exam_secondary_descriptor(
            &overlay.decoded,
            practical_exam_consumer(descriptor.overlay_path)?,
            descriptor_offset,
            descriptor.aliases,
        )?;
        ensure!(
            parsed.offset == descriptor_offset
                && parsed.capacity == descriptor.descriptor_capacity
                && parsed.source_sha256 == descriptor.descriptor_sha256
                && parsed.descriptor_index_aliases == descriptor.aliases
                && parsed.fragments.len() == descriptor.fragments.len()
                && parsed
                    .fragments
                    .iter()
                    .zip(descriptor.fragments)
                    .all(|(parsed, declared)| {
                        let texture_bank = match parsed.texture_bank {
                            PracticalExamSecondaryTextureBank::SharedProducer => 0,
                            PracticalExamSecondaryTextureBank::External => 1,
                        };
                        texture_bank == declared.texture_bank
                            && parsed.texture_page == declared.texture_page
                            && usize::from(parsed.clut_x_index) == declared.clut_x_index
                            && usize::from(parsed.clut_y_offset) == declared.clut_y_offset
                            && usize::from(parsed.u) == declared.source_u
                            && usize::from(parsed.v) == declared.source_v
                            && usize::from(parsed.width) == declared.width
                            && usize::from(parsed.height) == declared.height
                    }),
            "descriptor {} differs from the central practical-exam parser",
            descriptor.descriptor_id
        );
        ensure_strictly_increasing(descriptor.aliases, "descriptor aliases")?;
        validate_pointer_aliases(
            &overlay.decoded,
            pointer_table_offset,
            descriptor.aliases,
            base.checked_add(u32::try_from(descriptor_offset)?)
                .context("descriptor runtime pointer overflow")?,
            descriptor.descriptor_id.as_str(),
        )?;
        counts.pointer_aliases += descriptor.aliases.len();
        ensure!(
            !descriptor.fragments.is_empty()
                && descriptor.fragments.iter().all(|fragment| {
                    fragment.width > 0
                        && fragment.height > 0
                        && fragment.source_u + fragment.width <= 256
                        && fragment.source_v + fragment.height <= 256
                }),
            "descriptor {} has invalid fragment geometry",
            descriptor.descriptor_id
        );
    }
    Ok(by_id)
}

pub(super) fn descriptor_view(
    mechanism: &PracticalResultConsumerMechanism,
) -> Result<DescriptorView<'_>> {
    match mechanism {
        PracticalResultConsumerMechanism::CatalogDescriptorRenderer {
            descriptor_id,
            source_atlas_domain_ids,
            consumer_id,
            overlay_path,
            overlay_sha256,
            runtime_base,
            renderer_offset,
            renderer_runtime_address,
            renderer_size,
            renderer_sha256,
            pointer_table_offset,
            pointer_table_runtime_address,
            pointer_table_size,
            pointer_table_sha256,
            aliases,
            descriptor_offset,
            descriptor_runtime_address,
            descriptor_capacity,
            descriptor_sha256,
            fragments,
            evidence_status,
        } => Ok(DescriptorView {
            descriptor_id,
            source_atlas_domain_ids,
            consumer_id,
            overlay_path,
            overlay_sha256,
            runtime_base,
            renderer_offset,
            renderer_runtime_address,
            renderer_size: *renderer_size,
            renderer_sha256,
            pointer_table_offset,
            pointer_table_runtime_address,
            pointer_table_size: *pointer_table_size,
            pointer_table_sha256,
            aliases,
            descriptor_offset,
            descriptor_runtime_address,
            descriptor_capacity: *descriptor_capacity,
            descriptor_sha256,
            fragments,
            evidence_status,
        }),
        _ => anyhow::bail!("projection mechanism appeared in the descriptor population"),
    }
}

pub(super) fn practical_exam_consumer(path: &str) -> Result<PracticalExamConsumer> {
    match path {
        "DAT1/SIKEN.BIN" => Ok(PracticalExamConsumer::BasicsReview),
        "DAT1/SIKEN2.BIN" => Ok(PracticalExamConsumer::Exam1999),
        _ => anyhow::bail!("descriptor source {path} has no practical-exam consumer authority"),
    }
}
