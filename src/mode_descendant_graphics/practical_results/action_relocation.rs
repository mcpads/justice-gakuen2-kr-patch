//! Relocates action text whose original glyph cells conflict with other rows.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};

use crate::decoded_record_write_plan::DecodedDataClaim;
use crate::font::{HorizontalTextAlignment, IndexedTextRasterizer};
use crate::mode_descendant_graphics::model::ModeDescendantFontSources;
use crate::mode_descendant_graphics::practical_exam::{
    PracticalExamConsumer, PracticalExamSecondaryTextureBank,
    external_secondary_descriptors_avoid_source_cell, parse_practical_exam_secondary_descriptor,
};
use crate::mode_descendant_graphics::source::ModeDescendantSourceRecord;
use crate::pipeline::{difference_ranges, sha256_bytes};
use crate::tim::{
    cells_overlap, read_4bpp_palette_words_in_prefix, read_indexed_cell_in_prefix,
    write_indexed_cell_in_prefix_with_report,
};
use crate::write_scope::changed_ranges_are_within;

use super::action_cells::{RenderedActionCells, action_palette_roles};
use super::assets::parse_hex_offset;
use super::model::{
    PracticalResultActionCellTargetCatalog, PracticalResultEntry,
    PracticalResultPhysicalRegionCatalog, PracticalResultProtectedContent,
    PracticalResultSourceReadFootprintBuild,
};
use super::projection_model::PracticalResultActionConsumerOccurrenceCatalog;

pub(super) struct RelocatedActionBuild {
    pub(super) rendered: RenderedActionCells,
    pub(super) basics_consumer_overlay: Vec<u8>,
    pub(super) exam_1999_consumer_overlay: Vec<u8>,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn relocate_action_entries(
    entries: &[PracticalResultEntry],
    target_catalog: &PracticalResultActionCellTargetCatalog,
    occurrence_catalog: &PracticalResultActionConsumerOccurrenceCatalog,
    physical_catalog: &PracticalResultPhysicalRegionCatalog,
    protected_content: &PracticalResultProtectedContent,
    footprints: &[PracticalResultSourceReadFootprintBuild],
    fonts: &ModeDescendantFontSources,
    sources: &[ModeDescendantSourceRecord],
    texture_source: &ModeDescendantSourceRecord,
    texture_base: &[u8],
    basics_consumer_base: &[u8],
    exam_1999_consumer_base: &[u8],
) -> Result<RelocatedActionBuild> {
    let entries_by_id = entries
        .iter()
        .map(|entry| (entry.id.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    let occurrences_by_id = occurrence_catalog
        .occurrences
        .iter()
        .map(|occurrence| (occurrence.occurrence_id.as_str(), occurrence))
        .collect::<BTreeMap<_, _>>();
    let source_overlays = sources
        .iter()
        .map(|source| (source.path, source))
        .collect::<BTreeMap<_, _>>();
    let rasterizer = IndexedTextRasterizer::load(&fonts.practical_result_action.path)?;
    let palette = read_4bpp_palette_words_in_prefix(
        &texture_source.decoded,
        0,
        target_catalog
            .targets
            .first()
            .context("practical-result action target catalog is empty")?
            .palette_index,
    )?;
    let source_action_cells = entries
        .iter()
        .filter(|entry| {
            entry.font_role == super::super::model::ModeDescendantFontRole::PracticalResultAction
        })
        .flat_map(|entry| &entry.source_references)
        .filter_map(|reference| {
            physical_catalog.regions.iter().find(|region| {
                region.region_id == reference.physical_region_id
                    && region.source_path == texture_source.path
                    && region.tim_offset == "0x00000"
                    && region.bpp == 4
            })
        })
        .map(|region| read_indexed_cell_in_prefix(&texture_source.decoded, 0, region.cell))
        .collect::<Result<Vec<_>>>()?;
    let (clear_index, first_ink_index, last_ink_index) =
        action_palette_roles(&source_action_cells, &palette)?;

    let mut patched_texture = texture_base.to_vec();
    let mut basics_overlay = basics_consumer_base.to_vec();
    let mut exam_overlay = exam_1999_consumer_base.to_vec();
    let mut claims = Vec::new();
    let mut completed_entry_ids = BTreeSet::new();
    let mut changed_bytes_by_entry = BTreeMap::new();
    let mut rendered_references_by_entry = BTreeMap::new();
    let mut texture_allowed_ranges = Vec::new();
    let mut rewrite_count = 0usize;

    for target in &target_catalog.targets {
        let entry = entries_by_id
            .get(target.semantic_entry_id.as_str())
            .with_context(|| format!("missing action entry {}", target.semantic_entry_id))?;
        let korean_text = entry
            .korean_text
            .as_deref()
            .context("relocated action has no Korean text")?;
        validate_target_is_unread(
            target.target_record_path.as_str(),
            parse_hex_offset(&target.target_tim_offset)?,
            target.target_bpp,
            target.target_cell,
            physical_catalog,
            protected_content,
            footprints,
        )?;
        ensure!(
            target.target_record_path == texture_source.path
                && target.target_tim_offset == "0x00000"
                && target.target_bpp == 4
                && read_indexed_cell_in_prefix(&texture_source.decoded, 0, target.target_cell)?
                    .iter()
                    .all(|pixel| *pixel == clear_index)
                && read_indexed_cell_in_prefix(&patched_texture, 0, target.target_cell)?
                    == read_indexed_cell_in_prefix(&texture_source.decoded, 0, target.target_cell)?,
            "relocated action target {} is not an untouched transparent allocation",
            target.target_id
        );
        let raster = rasterizer.rasterize_shifted_with_coverage_ramp(
            korean_text,
            target.target_cell.width,
            target.target_cell.height,
            target.font_px,
            0.0,
            target.vertical_shift_px,
            clear_index,
            first_ink_index,
            last_ink_index,
            HorizontalTextAlignment::Center,
        )?;
        let write = write_indexed_cell_in_prefix_with_report(
            &mut patched_texture,
            0,
            target.target_cell,
            &raster.pixels,
        )?;
        ensure!(
            write.changed_byte_count > 0,
            "relocated action changed no pixels"
        );
        claims.extend(DecodedDataClaim::from_effective_ranges(
            &format!("mode-descendant:practical-result:{}:relocated", entry.id),
            &format!("render relocated practical-result action {}", entry.id),
            &texture_source.decoded,
            &patched_texture,
            write.allowed_ranges.iter().copied(),
        )?);
        texture_allowed_ranges.extend(write.allowed_ranges);
        ensure!(
            completed_entry_ids.insert(entry.id.clone()),
            "duplicate relocated action"
        );
        changed_bytes_by_entry.insert(entry.id.clone(), write.changed_byte_count);
        rendered_references_by_entry.insert(entry.id.clone(), entry.source_references.len());

        for occurrence_id in &target.consumer_occurrence_ids {
            let occurrence = occurrences_by_id
                .get(occurrence_id.as_str())
                .with_context(|| format!("missing action occurrence {occurrence_id}"))?;
            let source_overlay = source_overlays
                .get(occurrence.overlay_path.as_str())
                .with_context(|| format!("missing source overlay {}", occurrence.overlay_path))?;
            let consumer = consumer_for_overlay(&occurrence.overlay_path)?;
            ensure!(
                external_secondary_descriptors_avoid_source_cell(
                    &source_overlay.decoded,
                    consumer,
                    target.target_cell,
                )?,
                "action allocation {} has an existing secondary-descriptor reader",
                target.target_id
            );
            let descriptor_offset = parse_hex_offset(&occurrence.descriptor_offset)?;
            let parsed = parse_practical_exam_secondary_descriptor(
                &source_overlay.decoded,
                consumer,
                descriptor_offset,
                &occurrence.aliases,
            )?;
            ensure!(
                parsed.capacity == occurrence.descriptor_capacity
                    && sha256_bytes(
                        &source_overlay.decoded
                            [descriptor_offset..descriptor_offset + occurrence.descriptor_capacity]
                    ) == occurrence.descriptor_sha256,
                "action occurrence {occurrence_id} descriptor preimage changed"
            );
            let template = parsed
                .fragments
                .first()
                .context("action descriptor has no source fragment")?;
            ensure!(
                parsed.fragments.iter().all(|fragment| {
                    fragment.texture_page == template.texture_page
                        && fragment.texture_bank == template.texture_bank
                        && fragment.clut_x_index == template.clut_x_index
                        && fragment.clut_y_offset == template.clut_y_offset
                }) && template.texture_bank == PracticalExamSecondaryTextureBank::External,
                "action occurrence {occurrence_id} mixes source bindings"
            );
            let encoded = encode_single_fragment_descriptor(
                occurrence.descriptor_capacity,
                template.texture_page,
                template.clut_x_index,
                template.clut_y_offset,
                target.target_cell,
            )?;
            let overlay = match consumer {
                PracticalExamConsumer::BasicsReview => &mut basics_overlay,
                PracticalExamConsumer::Exam1999 => &mut exam_overlay,
            };
            ensure!(
                overlay[descriptor_offset..descriptor_offset + occurrence.descriptor_capacity]
                    == source_overlay.decoded
                        [descriptor_offset..descriptor_offset + occurrence.descriptor_capacity],
                "an earlier overlay compositor changed action occurrence {occurrence_id}"
            );
            overlay[descriptor_offset..descriptor_offset + occurrence.descriptor_capacity]
                .copy_from_slice(&encoded);
            ensure!(
                overlay[descriptor_offset] == 1
                    && overlay[descriptor_offset + 5] == u8::try_from(target.target_cell.x)?,
                "action occurrence {occurrence_id} descriptor readback failed"
            );
            rewrite_count += 1;
        }
    }

    ensure!(
        !texture_allowed_ranges.is_empty()
            && changed_ranges_are_within(
                &difference_ranges(texture_base, &patched_texture),
                &texture_allowed_ranges,
            ),
        "relocated action writes escaped their allocated cells"
    );
    Ok(RelocatedActionBuild {
        rendered: RenderedActionCells {
            decoded: patched_texture,
            decoded_write_claims: claims,
            completed_entry_ids,
            changed_bytes_by_entry,
            rendered_references_by_entry,
            completed_cell_write_count: target_catalog.targets.len(),
            shared_suffix_cell_count: 0,
            cell_writes_are_unique_and_non_overlapping: true,
            changes_confined_to_owned_cells: true,
            consumer_projection_rewrite_count: rewrite_count,
        },
        basics_consumer_overlay: basics_overlay,
        exam_1999_consumer_overlay: exam_overlay,
    })
}

pub(super) fn validate_target_is_unread(
    record_path: &str,
    tim_offset: usize,
    bpp: u8,
    target: crate::tim::Cell,
    physical_catalog: &PracticalResultPhysicalRegionCatalog,
    protected_content: &PracticalResultProtectedContent,
    footprints: &[PracticalResultSourceReadFootprintBuild],
) -> Result<()> {
    let physical_overlap = physical_catalog.regions.iter().find(|region| {
        region.source_path == record_path
            && parse_hex_offset(&region.tim_offset).ok() == Some(tim_offset)
            && region.bpp == bpp
            && cells_overlap(region.cell, target)
    });
    ensure!(
        physical_overlap.is_none(),
        "target allocation overlaps declared physical region {}",
        physical_overlap
            .map(|region| region.region_id.to_string())
            .unwrap_or_default()
    );
    let protected_overlap = protected_content.regions.iter().find(|region| {
        region.source_path == record_path
            && parse_hex_offset(&region.tim_offset).ok() == Some(tim_offset)
            && region.bpp == bpp
            && cells_overlap(region.cell, target)
    });
    ensure!(
        protected_overlap.is_none(),
        "target allocation overlaps protected region {}",
        protected_overlap
            .map(|region| region.id.to_string())
            .unwrap_or_default()
    );
    let footprint_overlap = footprints.iter().find(|footprint| {
        footprint.source_atlas_domain_id == "siken2_tim0_indexed_4bpp"
            && footprint
                .source_read_rectangles
                .iter()
                .any(|cell| cells_overlap(*cell, target))
    });
    ensure!(
        footprint_overlap.is_none(),
        "target allocation overlaps derived source reader {}",
        footprint_overlap
            .map(|footprint| footprint.projection_id.as_str())
            .unwrap_or_default()
    );
    Ok(())
}

fn consumer_for_overlay(path: &str) -> Result<PracticalExamConsumer> {
    match path {
        "DAT1/SIKEN.BIN" => Ok(PracticalExamConsumer::BasicsReview),
        "DAT1/SIKEN2.BIN" => Ok(PracticalExamConsumer::Exam1999),
        _ => anyhow::bail!("unsupported relocated-action overlay {path}"),
    }
}

fn encode_single_fragment_descriptor(
    capacity: usize,
    texture_page: u8,
    clut_x_index: u8,
    clut_y_offset: u8,
    cell: crate::tim::Cell,
) -> Result<Vec<u8>> {
    ensure!(
        capacity >= 12,
        "action descriptor has no single-fragment capacity"
    );
    let mut bytes = vec![0; capacity];
    bytes[..9].copy_from_slice(&[
        1,
        texture_page,
        1,
        clut_x_index,
        clut_y_offset,
        u8::try_from(cell.x)?,
        u8::try_from(cell.y)?,
        u8::try_from(cell.width)?,
        u8::try_from(cell.height)?,
    ]);
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::encode_single_fragment_descriptor;
    use crate::tim::Cell;

    #[test]
    fn relocated_action_descriptor_uses_one_external_fragment() {
        let encoded = encode_single_fragment_descriptor(
            20,
            0x0e,
            1,
            3,
            Cell {
                x: 160,
                y: 208,
                width: 32,
                height: 16,
            },
        )
        .unwrap();
        assert_eq!(&encoded[..9], &[1, 0x0e, 1, 1, 3, 160, 208, 32, 16]);
        assert!(encoded[9..].iter().all(|byte| *byte == 0));
    }
}
