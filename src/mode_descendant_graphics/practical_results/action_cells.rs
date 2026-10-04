//! Replaces only the runtime-bound action cells in the resident result atlas.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};

use crate::decoded_record_write_plan::DecodedDataClaim;
use crate::font::{HorizontalTextAlignment, IndexedTextRasterizer};
use crate::mode_descendant_graphics::model::ModeDescendantFontSources;
use crate::mode_descendant_graphics::source::ModeDescendantSourceRecord;
use crate::pipeline::{difference_ranges, sha256_bytes};
use crate::tim::{
    read_4bpp_palette_words_in_prefix, read_indexed_cell_in_prefix,
    write_indexed_cell_in_prefix_with_report,
};
use crate::write_scope::changed_ranges_are_within;

use super::assets::parse_hex_offset;
use super::model::{
    PracticalResultEntry, PracticalResultPhysicalRegion, PracticalResultPhysicalRegionCatalog,
};

const ACTION_PALETTE_INDEX: usize = 1;

pub(super) struct RenderedActionCells {
    pub(super) decoded: Vec<u8>,
    pub(super) decoded_write_claims: Vec<DecodedDataClaim>,
    pub(super) completed_entry_ids: BTreeSet<String>,
    pub(super) changed_bytes_by_entry: BTreeMap<String, usize>,
    pub(super) rendered_references_by_entry: BTreeMap<String, usize>,
    pub(super) completed_cell_write_count: usize,
    pub(super) shared_suffix_cell_count: usize,
    pub(super) cell_writes_are_unique_and_non_overlapping: bool,
    pub(super) changes_confined_to_owned_cells: bool,
    pub(super) consumer_projection_rewrite_count: usize,
}

struct BoundAction<'a> {
    entry: &'a PracticalResultEntry,
    regions: Vec<&'a PracticalResultPhysicalRegion>,
    korean_chars: Vec<char>,
}

pub(super) fn render_action_cell_entries(
    entries: &[&PracticalResultEntry],
    physical_catalog: &PracticalResultPhysicalRegionCatalog,
    fonts: &ModeDescendantFontSources,
    source: &ModeDescendantSourceRecord,
    base_decoded: &[u8],
) -> Result<RenderedActionCells> {
    ensure!(
        !entries.is_empty() && source.decoded.len() == base_decoded.len(),
        "practical-result action input is empty or changed record extent"
    );
    let regions_by_id = physical_catalog
        .regions
        .iter()
        .map(|region| (region.region_id.as_str(), region))
        .collect::<BTreeMap<_, _>>();
    let actions = entries
        .iter()
        .map(|entry| bind_action(entry, &regions_by_id, source, base_decoded))
        .collect::<Result<Vec<_>>>()?;
    let shared_suffix_cell_count = common_region_suffix_len(&actions)?;
    let shared_text = common_text_suffix(&actions)?;
    ensure!(
        shared_suffix_cell_count > 0 && shared_text.chars().count() == shared_suffix_cell_count,
        "shared practical-result action cells do not match the shared Korean suffix"
    );

    let source_pixels = actions
        .iter()
        .flat_map(|action| action.regions.iter())
        .map(|region| read_source_cell(source, region))
        .collect::<Result<Vec<_>>>()?;
    let palette = read_4bpp_palette_words_in_prefix(&source.decoded, 0, ACTION_PALETTE_INDEX)?;
    let (clear_index, first_ink_index, last_ink_index) =
        action_palette_roles(&source_pixels, &palette)?;
    let rasterizer = IndexedTextRasterizer::load(&fonts.practical_result_action.path)?;
    let mut patched = base_decoded.to_vec();
    let mut claims = Vec::new();
    let mut changed_bytes_by_entry = BTreeMap::new();
    let mut rendered_references_by_entry = BTreeMap::new();
    let mut completed_entry_ids = BTreeSet::new();
    let mut allowed_ranges = Vec::new();
    let mut completed_cell_write_count = 0usize;

    for action in &actions {
        let prefix_cell_count = action.regions.len() - shared_suffix_cell_count;
        let prefix_text = action.korean_chars
            [..action.korean_chars.len() - shared_suffix_cell_count]
            .iter()
            .collect::<String>();
        ensure!(
            prefix_cell_count > 0 && !prefix_text.trim().is_empty(),
            "practical-result action {} has no unique prefix",
            action.entry.id
        );
        let write = render_segment(
            &mut patched,
            &source.decoded,
            &action.regions[..prefix_cell_count],
            &prefix_text,
            HorizontalTextAlignment::Right,
            &rasterizer,
            fonts.practical_result_action.font_px,
            fonts.practical_result_action.vertical_shift_px,
            clear_index,
            first_ink_index,
            last_ink_index,
        )?;
        claims.extend(DecodedDataClaim::from_effective_ranges(
            &format!(
                "mode-descendant:practical-result:{}:prefix",
                action.entry.id
            ),
            &format!("render practical-result action prefix {}", action.entry.id),
            &source.decoded,
            &patched,
            write.allowed_ranges.iter().copied(),
        )?);
        allowed_ranges.extend(write.allowed_ranges);
        changed_bytes_by_entry.insert(action.entry.id.clone(), write.changed_byte_count);
        rendered_references_by_entry.insert(
            action.entry.id.clone(),
            action.entry.source_references.len(),
        );
        ensure!(
            completed_entry_ids.insert(action.entry.id.clone()),
            "duplicate practical-result action {}",
            action.entry.id
        );
        completed_cell_write_count += prefix_cell_count;
    }

    let shared_regions = &actions[0].regions[actions[0].regions.len() - shared_suffix_cell_count..];
    ensure!(
        actions.iter().all(|action| {
            action.regions[action.regions.len() - shared_suffix_cell_count..]
                .iter()
                .map(|region| region.region_id.as_str())
                .eq(shared_regions
                    .iter()
                    .map(|region| region.region_id.as_str()))
        }),
        "practical-result action suffix cells diverged after binding"
    );
    let shared_write = render_segment(
        &mut patched,
        &source.decoded,
        shared_regions,
        &shared_text,
        HorizontalTextAlignment::Left,
        &rasterizer,
        fonts.practical_result_action.font_px,
        fonts.practical_result_action.vertical_shift_px,
        clear_index,
        first_ink_index,
        last_ink_index,
    )?;
    claims.extend(DecodedDataClaim::from_effective_ranges(
        "mode-descendant:practical-result:shared-action-suffix",
        "render shared practical-result action suffix",
        &source.decoded,
        &patched,
        shared_write.allowed_ranges.iter().copied(),
    )?);
    allowed_ranges.extend(shared_write.allowed_ranges);
    for action in &actions {
        *changed_bytes_by_entry
            .get_mut(&action.entry.id)
            .context("completed practical-result action lost its byte count")? +=
            shared_write.changed_byte_count;
    }
    completed_cell_write_count += shared_suffix_cell_count;

    ensure!(
        completed_cell_write_count > shared_suffix_cell_count
            && !claims.is_empty()
            && changed_ranges_are_within(
                &difference_ranges(base_decoded, &patched),
                &allowed_ranges
            ),
        "practical-result action writes escaped their runtime-bound cells"
    );
    Ok(RenderedActionCells {
        decoded: patched,
        decoded_write_claims: claims,
        completed_entry_ids,
        changed_bytes_by_entry,
        rendered_references_by_entry,
        completed_cell_write_count,
        shared_suffix_cell_count,
        cell_writes_are_unique_and_non_overlapping: true,
        changes_confined_to_owned_cells: true,
        consumer_projection_rewrite_count: 0,
    })
}

/// Replaces a finite, runtime-observed label whose source glyph cells are
/// already consumed in sequence. Unlike the action rows, these labels do not
/// share storage or a semantic suffix with another entry.
pub(super) fn render_independent_label_sequences(
    entries: &[&PracticalResultEntry],
    physical_catalog: &PracticalResultPhysicalRegionCatalog,
    fonts: &ModeDescendantFontSources,
    source: &ModeDescendantSourceRecord,
    base_decoded: &[u8],
) -> Result<RenderedActionCells> {
    ensure!(
        !entries.is_empty()
            && entries.iter().all(|entry| {
                entry.strategy == super::model::PracticalResultStrategy::GlyphSequence
                    && entry.font_role
                        == super::super::model::ModeDescendantFontRole::PracticalResultLabel
                    && entry.development_status
                        == super::model::PracticalResultDevelopmentStatus::Authored
                    && entry.unresolved_source_references.is_empty()
                    && entry.source_references.iter().all(|reference| {
                        reference.source_usage_status
                            == super::model::PracticalResultSourceUsageStatus::RuntimeObserved
                    })
            }),
        "independent practical-result label set is empty or not runtime-bound"
    );
    let regions_by_id = physical_catalog
        .regions
        .iter()
        .map(|region| (region.region_id.as_str(), region))
        .collect::<BTreeMap<_, _>>();
    let labels = entries
        .iter()
        .map(|entry| bind_action(entry, &regions_by_id, source, base_decoded))
        .collect::<Result<Vec<_>>>()?;
    ensure!(
        labels.iter().all(|label| {
            label.korean_chars.len() == label.regions.len()
                && label.entry.source_text.chars().count() == label.regions.len()
        }),
        "independent practical-result labels must preserve their one-cell-per-glyph arity"
    );

    let source_pixels = labels
        .iter()
        .flat_map(|label| label.regions.iter())
        .map(|region| read_source_cell(source, region))
        .collect::<Result<Vec<_>>>()?;
    let palette =
        read_4bpp_palette_words_in_prefix(source.decoded.as_slice(), 0, ACTION_PALETTE_INDEX)?;
    let (clear_index, first_ink_index, last_ink_index) =
        action_palette_roles(&source_pixels, &palette)?;
    let rasterizer = IndexedTextRasterizer::load(&fonts.practical_result_label.path)?;
    let mut patched = base_decoded.to_vec();
    let mut claims = Vec::new();
    let mut completed_entry_ids = BTreeSet::new();
    let mut changed_bytes_by_entry = BTreeMap::new();
    let mut rendered_references_by_entry = BTreeMap::new();
    let mut allowed_ranges = Vec::new();
    let mut completed_cell_write_count = 0usize;

    for label in labels {
        let korean_text = label.korean_chars.iter().collect::<String>();
        let write = render_segment(
            &mut patched,
            &source.decoded,
            &label.regions,
            &korean_text,
            HorizontalTextAlignment::Left,
            &rasterizer,
            fonts.practical_result_label.font_px,
            fonts.practical_result_label.vertical_shift_px,
            clear_index,
            first_ink_index,
            last_ink_index,
        )?;
        claims.extend(DecodedDataClaim::from_effective_ranges(
            &format!(
                "mode-descendant:practical-result:{}:observed-label",
                label.entry.id
            ),
            &format!(
                "render runtime-observed practical-result label {}",
                label.entry.id
            ),
            &source.decoded,
            &patched,
            write.allowed_ranges.iter().copied(),
        )?);
        allowed_ranges.extend(write.allowed_ranges);
        changed_bytes_by_entry.insert(label.entry.id.clone(), write.changed_byte_count);
        rendered_references_by_entry
            .insert(label.entry.id.clone(), label.entry.source_references.len());
        ensure!(
            completed_entry_ids.insert(label.entry.id.clone()),
            "duplicate independent practical-result label {}",
            label.entry.id
        );
        completed_cell_write_count += label.regions.len();
    }

    ensure!(
        completed_cell_write_count > 0
            && !claims.is_empty()
            && changed_ranges_are_within(
                &difference_ranges(base_decoded, &patched),
                &allowed_ranges,
            ),
        "independent practical-result label writes escaped their runtime-bound cells"
    );
    Ok(RenderedActionCells {
        decoded: patched,
        decoded_write_claims: claims,
        completed_entry_ids,
        changed_bytes_by_entry,
        rendered_references_by_entry,
        completed_cell_write_count,
        shared_suffix_cell_count: 0,
        cell_writes_are_unique_and_non_overlapping: true,
        changes_confined_to_owned_cells: true,
        consumer_projection_rewrite_count: 0,
    })
}

pub(super) fn merge_rendered_action_cells(
    mut in_place: RenderedActionCells,
    relocated: RenderedActionCells,
) -> Result<RenderedActionCells> {
    ensure!(
        in_place.decoded.len() == relocated.decoded.len()
            && in_place
                .completed_entry_ids
                .is_disjoint(&relocated.completed_entry_ids),
        "practical-result action compositors overlap semantic entries"
    );
    in_place.decoded = relocated.decoded;
    in_place
        .decoded_write_claims
        .extend(relocated.decoded_write_claims);
    in_place
        .completed_entry_ids
        .extend(relocated.completed_entry_ids);
    for (entry_id, changed_bytes) in relocated.changed_bytes_by_entry {
        ensure!(
            in_place
                .changed_bytes_by_entry
                .insert(entry_id, changed_bytes)
                .is_none(),
            "practical-result action byte report overlaps an entry"
        );
    }
    for (entry_id, reference_count) in relocated.rendered_references_by_entry {
        ensure!(
            in_place
                .rendered_references_by_entry
                .insert(entry_id, reference_count)
                .is_none(),
            "practical-result action reference report overlaps an entry"
        );
    }
    in_place.completed_cell_write_count += relocated.completed_cell_write_count;
    in_place.shared_suffix_cell_count += relocated.shared_suffix_cell_count;
    in_place.cell_writes_are_unique_and_non_overlapping &=
        relocated.cell_writes_are_unique_and_non_overlapping;
    in_place.changes_confined_to_owned_cells &= relocated.changes_confined_to_owned_cells;
    in_place.consumer_projection_rewrite_count += relocated.consumer_projection_rewrite_count;
    Ok(in_place)
}

fn bind_action<'a>(
    entry: &'a PracticalResultEntry,
    regions_by_id: &BTreeMap<&str, &'a PracticalResultPhysicalRegion>,
    source: &ModeDescendantSourceRecord,
    base_decoded: &[u8],
) -> Result<BoundAction<'a>> {
    let korean_text = entry
        .korean_text
        .as_deref()
        .context("authored practical-result action lost Korean text")?;
    let mut references = entry.source_references.iter().collect::<Vec<_>>();
    references.sort_by_key(|reference| reference.sequence_index);
    ensure!(
        references
            .iter()
            .enumerate()
            .all(|(index, reference)| { reference.sequence_index == Some(index) }),
        "practical-result action {} source sequence is not contiguous",
        entry.id
    );
    let regions = references
        .iter()
        .map(|reference| {
            regions_by_id
                .get(reference.physical_region_id.as_str())
                .copied()
                .with_context(|| {
                    format!(
                        "practical-result action {} lost region {}",
                        entry.id, reference.physical_region_id
                    )
                })
        })
        .collect::<Result<Vec<_>>>()?;
    ensure!(
        !regions.is_empty()
            && regions.iter().all(|region| {
                region.source_path == source.path
                    && region.bpp == 4
                    && region.cell.width == 20
                    && region.cell.height == 20
                    && region.tim_offset == "0x00000"
            }),
        "practical-result action {} left the resident 20px result cells",
        entry.id
    );
    let mut source_sequence = Vec::new();
    for region in &regions {
        let pixels = read_source_cell(source, region)?;
        ensure!(
            sha256_bytes(&pixels) == region.source_indexed_sha256,
            "practical-result action region {} preimage changed",
            region.region_id
        );
        ensure!(
            read_indexed_cell_in_prefix(base_decoded, 0, region.cell)? == pixels,
            "practical-result action region {} was modified by an earlier compositor",
            region.region_id
        );
        source_sequence.extend(pixels);
    }
    ensure!(
        entry
            .source_cell_sequence_indexed_sha256
            .as_deref()
            .is_some_and(|expected| expected == sha256_bytes(&source_sequence)),
        "practical-result action {} source sequence changed",
        entry.id
    );
    Ok(BoundAction {
        entry,
        regions,
        korean_chars: korean_text.chars().collect(),
    })
}

fn read_source_cell(
    source: &ModeDescendantSourceRecord,
    region: &PracticalResultPhysicalRegion,
) -> Result<Vec<u8>> {
    read_indexed_cell_in_prefix(
        &source.decoded,
        parse_hex_offset(&region.tim_offset)?,
        region.cell,
    )
}

#[allow(clippy::too_many_arguments)]
fn render_segment(
    patched: &mut [u8],
    immutable_source: &[u8],
    regions: &[&PracticalResultPhysicalRegion],
    text: &str,
    alignment: HorizontalTextAlignment,
    rasterizer: &IndexedTextRasterizer,
    font_px: f32,
    vertical_shift_px: i32,
    clear_index: u8,
    first_ink_index: u8,
    last_ink_index: u8,
) -> Result<SegmentWrite> {
    let height = regions
        .first()
        .context("practical-result action segment has no cells")?
        .cell
        .height;
    ensure!(
        regions.iter().all(|region| region.cell.height == height),
        "practical-result action segment mixes cell heights"
    );
    let width = regions
        .iter()
        .map(|region| region.cell.width)
        .sum::<usize>();
    let raster = rasterizer.rasterize_shifted_with_coverage_ramp(
        text,
        width,
        height,
        font_px,
        0.0,
        vertical_shift_px,
        clear_index,
        first_ink_index,
        last_ink_index,
        alignment,
    )?;
    let split = split_virtual_pixels(&raster.pixels, width, height, regions)?;
    let mut allowed_ranges = Vec::new();
    let mut changed_byte_count = 0usize;
    for (region, pixels) in regions.iter().zip(split) {
        ensure!(
            read_indexed_cell_in_prefix(immutable_source, 0, region.cell)? != pixels,
            "practical-result action cell {} changed no indexed pixels",
            region.region_id
        );
        let write = write_indexed_cell_in_prefix_with_report(patched, 0, region.cell, &pixels)?;
        ensure!(
            write.changed_byte_count > 0,
            "practical-result action cell {} changed no bytes",
            region.region_id
        );
        allowed_ranges.extend(write.allowed_ranges);
        changed_byte_count += write.changed_byte_count;
    }
    Ok(SegmentWrite {
        allowed_ranges,
        changed_byte_count,
    })
}

struct SegmentWrite {
    allowed_ranges: Vec<[usize; 2]>,
    changed_byte_count: usize,
}

fn split_virtual_pixels(
    pixels: &[u8],
    width: usize,
    height: usize,
    regions: &[&PracticalResultPhysicalRegion],
) -> Result<Vec<Vec<u8>>> {
    ensure!(
        pixels.len() == width * height
            && regions
                .iter()
                .map(|region| region.cell.width)
                .sum::<usize>()
                == width,
        "practical-result virtual action raster has invalid geometry"
    );
    let mut output = regions
        .iter()
        .map(|region| Vec::with_capacity(region.cell.width * height))
        .collect::<Vec<_>>();
    for y in 0..height {
        let mut x = 0usize;
        for (cell_pixels, region) in output.iter_mut().zip(regions) {
            cell_pixels
                .extend_from_slice(&pixels[y * width + x..y * width + x + region.cell.width]);
            x += region.cell.width;
        }
    }
    Ok(output)
}

fn common_region_suffix_len(actions: &[BoundAction<'_>]) -> Result<usize> {
    let shortest = actions
        .iter()
        .map(|action| action.regions.len())
        .min()
        .context("practical-result action set is empty")?;
    Ok((1..=shortest)
        .take_while(|distance| {
            let first = actions[0].regions[actions[0].regions.len() - distance]
                .region_id
                .as_str();
            actions.iter().all(|action| {
                action.regions[action.regions.len() - distance]
                    .region_id
                    .as_str()
                    == first
            })
        })
        .count())
}

fn common_text_suffix(actions: &[BoundAction<'_>]) -> Result<String> {
    let shortest = actions
        .iter()
        .map(|action| action.korean_chars.len())
        .min()
        .context("practical-result action set is empty")?;
    let suffix_len = (1..=shortest)
        .take_while(|distance| {
            let first = actions[0].korean_chars[actions[0].korean_chars.len() - distance];
            actions
                .iter()
                .all(|action| action.korean_chars[action.korean_chars.len() - distance] == first)
        })
        .count();
    ensure!(
        suffix_len > 0,
        "practical-result actions share no Korean suffix"
    );
    Ok(
        actions[0].korean_chars[actions[0].korean_chars.len() - suffix_len..]
            .iter()
            .collect(),
    )
}

pub(super) fn action_palette_roles(
    source_cells: &[Vec<u8>],
    palette: &[u16; 16],
) -> Result<(u8, u8, u8)> {
    let mut population = [0usize; 16];
    for pixel in source_cells.iter().flatten() {
        population[usize::from(*pixel)] += 1;
    }
    let clear_index = palette
        .iter()
        .enumerate()
        .filter(|(_, word)| (**word & 0x7fff) == 0)
        .max_by_key(|(index, _)| population[*index])
        .map(|(index, _)| index as u8)
        .context("practical-result action palette has no transparent index")?;
    let mut ink_indices = population
        .iter()
        .enumerate()
        .filter(|(index, count)| {
            **count > 0 && *index != usize::from(clear_index) && (palette[*index] & 0x7fff) != 0
        })
        .map(|(index, _)| index as u8);
    let first_ink_index = ink_indices
        .next()
        .context("practical-result action cells have no ink index")?;
    let last_ink_index = ink_indices.next_back().unwrap_or(first_ink_index);
    ensure!(
        clear_index < first_ink_index && first_ink_index <= last_ink_index,
        "practical-result action palette ramp is not ordered"
    );
    Ok((clear_index, first_ink_index, last_ink_index))
}

#[cfg(test)]
mod tests {
    use super::action_palette_roles;

    #[test]
    fn action_palette_roles_follow_transparency_and_used_ink() {
        let mut palette = [0u16; 16];
        palette[3] = 0x1234;
        palette[4] = 0x2345;
        let roles = action_palette_roles(&[vec![0, 0, 3, 4, 4]], &palette).unwrap();
        assert_eq!(roles, (0, 3, 4));
    }
}
