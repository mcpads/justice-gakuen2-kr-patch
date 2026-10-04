//! Renders runtime-bound result stamps without inheriting authority across variants.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::f64::consts::PI;

use anyhow::{Context, Result, ensure};

use crate::decoded_record_write_plan::DecodedDataClaim;
use crate::font::{HorizontalTextAlignment, IndexedTextRasterizers};
use crate::mode_descendant_graphics::gorin_heading::dilate_mask;
use crate::mode_descendant_graphics::model::ModeDescendantFontSources;
use crate::mode_descendant_graphics::source::ModeDescendantSourceRecord;
use crate::pipeline::sha256_bytes;
use crate::tim::{
    Cell, cells_overlap, read_4bpp_palette_words_in_prefix, read_indexed_cell_in_prefix,
    write_indexed_cell_in_prefix_with_report,
};

use super::assets::parse_hex_offset;
use super::model::{
    PracticalResultDevelopmentStatus, PracticalResultEntry,
    PracticalResultJudgmentStampTargetCatalog, PracticalResultJudgmentStampTargetId,
    PracticalResultStrategy,
};

const RASTER_WIDTH: usize = 256;
const CHARACTER_HEIGHT: usize = 256;
const CLEAR_MARKER: u8 = 0;
const FILL_MARKER: u8 = 1;
const OUTER_DILATION_STEPS: usize = 2;
const INNER_EROSION_STEPS: usize = 5;
const SOURCE_FOOTPRINT_INSET: f64 = 0.97;

pub(super) struct JudgmentStampPlan {
    pub(super) deferred_entry_ids: BTreeSet<String>,
    pub(super) validated_target_count: usize,
    pub(super) layout_candidate_count: usize,
    pub(super) expected_write_count: usize,
    pub(super) write_contract_complete: bool,
}

pub(super) struct RenderedJudgmentStamps {
    pub(super) decoded: Vec<u8>,
    pub(super) decoded_write_claims: Vec<DecodedDataClaim>,
    pub(super) completed_target_ids: BTreeSet<PracticalResultJudgmentStampTargetId>,
    pub(super) changed_bytes_by_entry: BTreeMap<String, usize>,
    pub(super) rendered_references_by_entry: BTreeMap<String, usize>,
    pub(super) cells_are_unique_and_non_overlapping: bool,
    pub(super) changes_confined_to_owned_cells: bool,
}

pub(super) struct JudgmentStampCompletion {
    pub(super) completed_target_ids: BTreeSet<PracticalResultJudgmentStampTargetId>,
    pub(super) changed_bytes_by_entry: BTreeMap<String, usize>,
    pub(super) rendered_references_by_entry: BTreeMap<String, usize>,
    pub(super) cells_are_unique_and_non_overlapping: bool,
    pub(super) changes_confined_to_owned_cells: bool,
}

pub(super) fn merge_rendered_judgment_stamps(
    rendered_records: &[&RenderedJudgmentStamps],
) -> Result<JudgmentStampCompletion> {
    ensure!(
        !rendered_records.is_empty(),
        "judgment-stamp completion has no rendered records"
    );
    let mut completed_target_ids = BTreeSet::new();
    let mut changed_bytes_by_entry = BTreeMap::<String, usize>::new();
    let mut rendered_references_by_entry = BTreeMap::<String, usize>::new();
    for rendered in rendered_records {
        for target_id in &rendered.completed_target_ids {
            ensure!(
                completed_target_ids.insert(target_id.clone()),
                "judgment-stamp target {target_id} completed in two records"
            );
        }
        for (entry_id, changed_byte_count) in &rendered.changed_bytes_by_entry {
            *changed_bytes_by_entry.entry(entry_id.clone()).or_default() += changed_byte_count;
        }
        for (entry_id, rendered_reference_count) in &rendered.rendered_references_by_entry {
            *rendered_references_by_entry
                .entry(entry_id.clone())
                .or_default() += rendered_reference_count;
        }
    }
    Ok(JudgmentStampCompletion {
        completed_target_ids,
        changed_bytes_by_entry,
        rendered_references_by_entry,
        cells_are_unique_and_non_overlapping: rendered_records
            .iter()
            .all(|rendered| rendered.cells_are_unique_and_non_overlapping),
        changes_confined_to_owned_cells: rendered_records
            .iter()
            .all(|rendered| rendered.changes_confined_to_owned_cells),
    })
}

pub(super) fn render_judgment_stamp_entries(
    entries: &[PracticalResultEntry],
    catalog: &PracticalResultJudgmentStampTargetCatalog,
    fonts: &ModeDescendantFontSources,
    source: &ModeDescendantSourceRecord,
    base_decoded: &[u8],
) -> Result<RenderedJudgmentStamps> {
    ensure!(
        source.decoded.len() == base_decoded.len(),
        "judgment-stamp base changed record extent"
    );
    let entries_by_id = entries
        .iter()
        .map(|entry| (entry.id.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    let targets = catalog
        .targets
        .iter()
        .filter(|target| target.target_record_path == source.path)
        .collect::<Vec<_>>();
    ensure!(
        !targets.is_empty(),
        "judgment-stamp source {} has no authorized target",
        source.path
    );

    let mut rasterizers = IndexedTextRasterizers::default();
    let mut patched = base_decoded.to_vec();
    let mut occupied = Vec::<(usize, Cell)>::new();
    let mut decoded_write_claims = Vec::new();
    let mut completed_target_ids = BTreeSet::new();
    let mut changed_bytes_by_entry = BTreeMap::<String, usize>::new();
    let mut rendered_references_by_entry = BTreeMap::<String, usize>::new();
    for target in targets {
        ensure!(
            target.target_bpp == 4 && target.target_cell.x == 0 && target.target_cell.y == 0,
            "judgment-stamp target {} is not a whole 4-bpp TIM",
            target.target_id
        );
        let entry = entries_by_id
            .get(target.semantic_entry_id.as_str())
            .with_context(|| {
                format!("judgment-stamp target {} lost its entry", target.target_id)
            })?;
        ensure!(
            entry.strategy == PracticalResultStrategy::JudgmentStamp
                && entry.development_status == PracticalResultDevelopmentStatus::Authored,
            "judgment-stamp target {} names an unauthored stamp",
            target.target_id
        );
        let korean_text = entry
            .korean_text
            .as_deref()
            .context("authored judgment stamp lost Korean text")?;
        let tim_offset = parse_hex_offset(&target.target_tim_offset)?;
        ensure!(
            occupied.iter().all(|(offset, cell)| {
                *offset != tim_offset || !cells_overlap(*cell, target.target_cell)
            }),
            "judgment-stamp target {} overlaps another owned cell",
            target.target_id
        );
        occupied.push((tim_offset, target.target_cell));
        let source_pixels =
            read_indexed_cell_in_prefix(&source.decoded, tim_offset, target.target_cell)?;
        let actual_preimage = sha256_bytes(&source_pixels);
        ensure!(
            actual_preimage == target.expected_preimage_indexed_sha256,
            "judgment-stamp target {} preimage changed: expected {}, got {}",
            target.target_id,
            target.expected_preimage_indexed_sha256,
            actual_preimage
        );
        let palette = read_4bpp_palette_words_in_prefix(&source.decoded, tim_offset, 0)?;
        let rasterizer = rasterizers.for_font(&fonts.practical_result_judgment_stamp.path)?;
        let characters = korean_text.chars().collect::<Vec<_>>();
        ensure!(!characters.is_empty(), "judgment stamp has no characters");
        let raster_height = CHARACTER_HEIGHT * characters.len();
        let mut raster = vec![CLEAR_MARKER; RASTER_WIDTH * raster_height];
        for (index, character) in characters.iter().enumerate() {
            let top = index * CHARACTER_HEIGHT;
            let bottom = top + CHARACTER_HEIGHT;
            let glyph = rasterizer.rasterize_shifted(
                &character.to_string(),
                RASTER_WIDTH,
                bottom - top,
                fonts.practical_result_judgment_stamp.font_px,
                0.0,
                fonts.practical_result_judgment_stamp.vertical_shift_px,
                CLEAR_MARKER,
                None,
                FILL_MARKER,
                HorizontalTextAlignment::Center,
            )?;
            raster[top * RASTER_WIDTH..bottom * RASTER_WIDTH].copy_from_slice(&glyph.pixels);
        }
        let composed = compose_judgment_stamp(
            &source_pixels,
            target.target_cell,
            &palette,
            &raster,
            RASTER_WIDTH,
            raster_height,
        )?;
        let write = write_indexed_cell_in_prefix_with_report(
            &mut patched,
            tim_offset,
            target.target_cell,
            &composed,
        )?;
        ensure!(
            write.changed_byte_count > 0,
            "judgment-stamp target {} changed no bytes",
            target.target_id
        );
        decoded_write_claims.extend(DecodedDataClaim::from_effective_ranges(
            &format!("mode-descendant:practical-result:{}", target.target_id),
            &format!("render practical-result judgment stamp {}", entry.id),
            &source.decoded,
            &patched,
            write.allowed_ranges,
        )?);
        ensure!(
            completed_target_ids.insert(target.target_id.clone()),
            "judgment-stamp target {} rendered twice",
            target.target_id
        );
        *changed_bytes_by_entry.entry(entry.id.clone()).or_default() += write.changed_byte_count;
        *rendered_references_by_entry
            .entry(entry.id.clone())
            .or_default() += 1;
    }

    Ok(RenderedJudgmentStamps {
        decoded: patched,
        decoded_write_claims,
        completed_target_ids,
        changed_bytes_by_entry,
        rendered_references_by_entry,
        cells_are_unique_and_non_overlapping: true,
        changes_confined_to_owned_cells: true,
    })
}

fn compose_judgment_stamp(
    source_pixels: &[u8],
    target: Cell,
    palette: &[u16; 16],
    raster: &[u8],
    raster_width: usize,
    raster_height: usize,
) -> Result<Vec<u8>> {
    ensure!(
        source_pixels.len() == target.width * target.height
            && raster.len() == raster_width * raster_height,
        "judgment-stamp geometry changed"
    );
    let clear_index = derive_clear_index(source_pixels, palette)?;
    let source_ink = significant_ink_mask(source_pixels, clear_index, target.width, target.height)?;
    let ink_index = derive_ink_index(source_pixels, &source_ink, clear_index)?;
    let source_bounds = mask_bounds(&source_ink, target.width, target.height)
        .context("judgment-stamp source has no significant ink")?;
    // The source axis describes a vertical stack, not a horizontal word.
    // Subtract its upright axis so each Korean character keeps its orientation.
    let angle = principal_axis_angle(&source_ink, target.width)? - PI / 2.0;

    let fill = raster
        .iter()
        .map(|pixel| *pixel == FILL_MARKER)
        .collect::<Vec<_>>();
    ensure!(
        fill.iter().any(|pixel| *pixel),
        "judgment-stamp raster has no ink"
    );
    let mut outer = fill.clone();
    for _ in 0..OUTER_DILATION_STEPS {
        outer = dilate_mask(&outer, raster_width, raster_height);
    }
    let mut inner = fill;
    for _ in 0..INNER_EROSION_STEPS {
        inner = erode_mask(&inner, raster_width, raster_height);
    }
    let ring = outer
        .iter()
        .zip(&inner)
        .map(|(outer, inner)| *outer && !*inner)
        .collect::<Vec<_>>();
    ensure!(
        ring.iter().any(|pixel| *pixel),
        "judgment-stamp outline is empty"
    );
    let ring_bounds = mask_bounds(&ring, raster_width, raster_height)
        .context("judgment-stamp outline lost its bounds")?;

    let (sin, cos) = angle.sin_cos();
    let ring_center_x = (ring_bounds[0] + ring_bounds[2] - 1) as f64 / 2.0;
    let ring_center_y = (ring_bounds[1] + ring_bounds[3] - 1) as f64 / 2.0;
    let corners = [
        (ring_bounds[0] as f64, ring_bounds[1] as f64),
        (ring_bounds[2] as f64, ring_bounds[1] as f64),
        (ring_bounds[0] as f64, ring_bounds[3] as f64),
        (ring_bounds[2] as f64, ring_bounds[3] as f64),
    ];
    let mut rotated_min_x = f64::INFINITY;
    let mut rotated_max_x = f64::NEG_INFINITY;
    let mut rotated_min_y = f64::INFINITY;
    let mut rotated_max_y = f64::NEG_INFINITY;
    for (x, y) in corners {
        let dx = x - ring_center_x;
        let dy = y - ring_center_y;
        let rotated_x = dx * cos - dy * sin;
        let rotated_y = dx * sin + dy * cos;
        rotated_min_x = rotated_min_x.min(rotated_x);
        rotated_max_x = rotated_max_x.max(rotated_x);
        rotated_min_y = rotated_min_y.min(rotated_y);
        rotated_max_y = rotated_max_y.max(rotated_y);
    }
    let source_width = (source_bounds[2] - source_bounds[0]) as f64;
    let source_height = (source_bounds[3] - source_bounds[1]) as f64;
    let scale = (source_width / (rotated_max_x - rotated_min_x))
        .min(source_height / (rotated_max_y - rotated_min_y))
        * SOURCE_FOOTPRINT_INSET;
    ensure!(
        scale.is_finite() && scale > 0.0,
        "invalid judgment-stamp scale"
    );

    let target_center_x = (source_bounds[0] + source_bounds[2] - 1) as f64 / 2.0;
    let target_center_y = (source_bounds[1] + source_bounds[3] - 1) as f64 / 2.0;
    let mut output = vec![clear_index; source_pixels.len()];
    for y in 0..target.height {
        for x in 0..target.width {
            let dx = (x as f64 - target_center_x) / scale;
            let dy = (y as f64 - target_center_y) / scale;
            let source_x = ring_center_x + dx * cos + dy * sin;
            let source_y = ring_center_y - dx * sin + dy * cos;
            let local_x = source_x.round() as isize;
            let local_y = source_y.round() as isize;
            if local_x >= 0
                && local_y >= 0
                && (local_x as usize) < raster_width
                && (local_y as usize) < raster_height
                && ring[local_y as usize * raster_width + local_x as usize]
            {
                output[y * target.width + x] = ink_index;
            }
        }
    }
    ensure!(
        output.contains(&ink_index),
        "judgment-stamp transform emitted no ink"
    );
    Ok(output)
}

fn derive_clear_index(source_pixels: &[u8], palette: &[u16; 16]) -> Result<u8> {
    let mut population = [0usize; 16];
    for pixel in source_pixels {
        population[usize::from(*pixel)] += 1;
    }
    palette
        .iter()
        .enumerate()
        .filter(|(_, word)| **word == 0)
        .max_by_key(|(index, _)| population[*index])
        .map(|(index, _)| index as u8)
        .context("judgment-stamp palette has no transparent index")
}

fn derive_ink_index(source_pixels: &[u8], mask: &[bool], clear_index: u8) -> Result<u8> {
    let mut population = [0usize; 16];
    for (pixel, masked) in source_pixels.iter().zip(mask) {
        if *masked && *pixel != clear_index {
            population[usize::from(*pixel)] += 1;
        }
    }
    population
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != usize::from(clear_index))
        .max_by_key(|(_, count)| **count)
        .filter(|(_, count)| **count > 0)
        .map(|(index, _)| index as u8)
        .context("judgment-stamp source has no ink palette index")
}

fn significant_ink_mask(
    pixels: &[u8],
    clear_index: u8,
    width: usize,
    height: usize,
) -> Result<Vec<bool>> {
    let mut visited = vec![false; pixels.len()];
    let mut components = Vec::<Vec<usize>>::new();
    for start in 0..pixels.len() {
        if visited[start] || pixels[start] == clear_index {
            continue;
        }
        visited[start] = true;
        let mut queue = VecDeque::from([start]);
        let mut component = Vec::new();
        while let Some(offset) = queue.pop_front() {
            component.push(offset);
            let x = offset % width;
            let y = offset / width;
            for neighbor_y in y.saturating_sub(1)..=(y + 1).min(height - 1) {
                for neighbor_x in x.saturating_sub(1)..=(x + 1).min(width - 1) {
                    let neighbor = neighbor_y * width + neighbor_x;
                    if !visited[neighbor] && pixels[neighbor] != clear_index {
                        visited[neighbor] = true;
                        queue.push_back(neighbor);
                    }
                }
            }
        }
        components.push(component);
    }
    let largest = components
        .iter()
        .map(Vec::len)
        .max()
        .context("judgment-stamp source has no ink component")?;
    let minimum_component_size = (largest / 20).max(8);
    let mut mask = vec![false; pixels.len()];
    for component in components
        .into_iter()
        .filter(|component| component.len() >= minimum_component_size)
    {
        for offset in component {
            mask[offset] = true;
        }
    }
    Ok(mask)
}

fn principal_axis_angle(mask: &[bool], width: usize) -> Result<f64> {
    let points = mask
        .iter()
        .enumerate()
        .filter_map(|(offset, ink)| {
            ink.then_some(((offset % width) as f64, (offset / width) as f64))
        })
        .collect::<Vec<_>>();
    ensure!(
        points.len() >= 2,
        "judgment-stamp source axis has too few pixels"
    );
    let count = points.len() as f64;
    let mean_x = points.iter().map(|(x, _)| *x).sum::<f64>() / count;
    let mean_y = points.iter().map(|(_, y)| *y).sum::<f64>() / count;
    let covariance_xx = points
        .iter()
        .map(|(x, _)| (x - mean_x) * (x - mean_x))
        .sum::<f64>();
    let covariance_xy = points
        .iter()
        .map(|(x, y)| (x - mean_x) * (y - mean_y))
        .sum::<f64>();
    let covariance_yy = points
        .iter()
        .map(|(_, y)| (y - mean_y) * (y - mean_y))
        .sum::<f64>();
    let mut angle = 0.5 * (2.0 * covariance_xy).atan2(covariance_xx - covariance_yy);
    if angle < 0.0 {
        angle += PI;
    }
    Ok(angle)
}

fn mask_bounds(mask: &[bool], width: usize, height: usize) -> Option<[usize; 4]> {
    let mut min_x = width;
    let mut min_y = height;
    let mut max_x = 0usize;
    let mut max_y = 0usize;
    let mut found = false;
    for (offset, ink) in mask.iter().copied().enumerate() {
        if !ink {
            continue;
        }
        found = true;
        let x = offset % width;
        let y = offset / width;
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x + 1);
        max_y = max_y.max(y + 1);
    }
    found.then_some([min_x, min_y, max_x, max_y])
}

fn erode_mask(mask: &[bool], width: usize, height: usize) -> Vec<bool> {
    let mut eroded = vec![false; mask.len()];
    for y in 1..height.saturating_sub(1) {
        for x in 1..width.saturating_sub(1) {
            eroded[y * width + x] = (y - 1..=y + 1).all(|neighbor_y| {
                (x - 1..=x + 1).all(|neighbor_x| mask[neighbor_y * width + neighbor_x])
            });
        }
    }
    eroded
}

pub(super) fn plan_judgment_stamp_entries(
    entries: &[PracticalResultEntry],
    catalog: &PracticalResultJudgmentStampTargetCatalog,
    completed_target_ids: &BTreeSet<PracticalResultJudgmentStampTargetId>,
) -> Result<JudgmentStampPlan> {
    let target_ids = catalog
        .targets
        .iter()
        .map(|target| target.target_id.clone())
        .collect::<BTreeSet<_>>();
    ensure!(
        completed_target_ids.is_subset(&target_ids),
        "judgment-stamp Expected Write closure names an unknown target"
    );
    let mut targets_by_entry = BTreeMap::<&str, Vec<&PracticalResultJudgmentStampTargetId>>::new();
    for target in &catalog.targets {
        targets_by_entry
            .entry(target.semantic_entry_id.as_str())
            .or_default()
            .push(&target.target_id);
    }
    let candidate_entries = catalog
        .layout_candidates
        .iter()
        .map(|candidate| candidate.semantic_entry_id.as_str())
        .collect::<BTreeSet<_>>();
    let mut deferred_entry_ids = BTreeSet::new();
    for entry in entries.iter().filter(|entry| {
        entry.strategy == PracticalResultStrategy::JudgmentStamp
            && entry.development_status == PracticalResultDevelopmentStatus::Authored
    }) {
        let targets = targets_by_entry
            .get(entry.id.as_str())
            .map(Vec::as_slice)
            .unwrap_or_default();
        if targets.is_empty()
            || candidate_entries.contains(entry.id.as_str())
            || targets
                .iter()
                .any(|target_id| !completed_target_ids.contains(*target_id))
        {
            deferred_entry_ids.insert(entry.id.clone());
        }
    }
    Ok(JudgmentStampPlan {
        deferred_entry_ids,
        validated_target_count: target_ids.len(),
        layout_candidate_count: catalog.layout_candidates.len(),
        expected_write_count: completed_target_ids.len(),
        write_contract_complete: !target_ids.is_empty() && completed_target_ids == &target_ids,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upright_source_preserves_vertical_character_order_and_orientation() {
        let cell = Cell {
            x: 0,
            y: 0,
            width: 64,
            height: 128,
        };
        let mut source = vec![0; cell.width * cell.height];
        for y in 4..124 {
            for x in 8..56 {
                source[y * cell.width + x] = 1;
            }
        }
        let mut raster = vec![0; cell.width * cell.height];
        // Distinct asymmetric silhouettes expose a quarter-turn or reversed order.
        for y in 12..36 {
            for x in 26..38 {
                raster[y * cell.width + x] = FILL_MARKER;
            }
        }
        for y in 80..112 {
            for x in 12..52 {
                raster[y * cell.width + x] = FILL_MARKER;
            }
        }
        let mut palette = [0; 16];
        palette[1] = 31;
        let output =
            compose_judgment_stamp(&source, cell, &palette, &raster, cell.width, cell.height)
                .unwrap();
        let ink = output.iter().map(|pixel| *pixel == 1).collect::<Vec<_>>();
        let upper = mask_bounds(&ink[..64 * 64], 64, 64).unwrap();
        let lower = mask_bounds(&ink[64 * 64..], 64, 64).unwrap();
        assert!(upper[2] - upper[0] < lower[2] - lower[0]);
        assert!(upper[3] - upper[1] > upper[2] - upper[0]);
        assert!(lower[2] - lower[0] > lower[3] - lower[1]);
    }

    #[test]
    fn significant_components_exclude_isolated_edge_noise() {
        let width = 20;
        let height = 12;
        let mut pixels = vec![0; width * height];
        for y in 3..9 {
            for x in 4..16 {
                pixels[y * width + x] = 2;
            }
        }
        pixels[width - 1] = 2;
        pixels[2 * width - 1] = 2;

        let mask = significant_ink_mask(&pixels, 0, width, height).unwrap();

        assert!(mask[4 * width + 5]);
        assert!(!mask[width - 1]);
        assert!(!mask[2 * width - 1]);
    }

    #[test]
    fn outlined_stamp_keeps_a_transparent_interior() {
        let width = 24;
        let height = 24;
        let mut fill = vec![false; width * height];
        for y in 5..19 {
            for x in 5..19 {
                fill[y * width + x] = true;
            }
        }
        let mut outer = fill.clone();
        for _ in 0..OUTER_DILATION_STEPS {
            outer = dilate_mask(&outer, width, height);
        }
        let mut inner = fill;
        for _ in 0..INNER_EROSION_STEPS {
            inner = erode_mask(&inner, width, height);
        }
        let ring = outer
            .iter()
            .zip(&inner)
            .map(|(outer, inner)| *outer && !*inner)
            .collect::<Vec<_>>();

        assert!(ring[4 * width + 4]);
        assert!(!ring[12 * width + 12]);
    }
}
