//! Replaces finite Japanese result backdrops without touching the result sheet.

use crate::font::rotate_indexed_raster;

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};

use crate::decoded_record_write_plan::DecodedDataClaim;
use crate::font::{HorizontalTextAlignment, IndexedTextRasterizers};
use crate::mode_descendant_graphics::model::{ModeDescendantFontRole, ModeDescendantFontSources};
use crate::mode_descendant_graphics::source::ModeDescendantSourceRecord;
use crate::pipeline::sha256_bytes;
use crate::tim::{
    read_8bpp_indexed_cell_in_prefix, read_8bpp_palette_words_in_prefix,
    write_8bpp_indexed_cell_in_prefix_with_report,
};

use super::assets::parse_hex_offset;
use super::model::{
    PracticalResultDecorativeBackgroundTargetCatalog, PracticalResultDecorativeBackgroundTargetId,
    PracticalResultDecorativeCompositionStage, PracticalResultDevelopmentStatus,
    PracticalResultEntry, PracticalResultStrategy,
};

pub(super) struct RenderedDecorativeBackgrounds {
    pub(super) decoded: Vec<u8>,
    pub(super) decoded_write_claims: Vec<DecodedDataClaim>,
    pub(super) completed_target_ids: BTreeSet<PracticalResultDecorativeBackgroundTargetId>,
    pub(super) changed_bytes_by_entry: BTreeMap<String, usize>,
    pub(super) rendered_references_by_entry: BTreeMap<String, usize>,
    pub(super) rendered_text_placement_count: usize,
    pub(super) palettes_preserved: bool,
    pub(super) changes_confined_to_owned_cells: bool,
}

pub(super) struct DecorativeBackgroundCompletion {
    pub(super) completed_target_count: usize,
    pub(super) expected_write_count: usize,
    pub(super) changed_bytes_by_entry: BTreeMap<String, usize>,
    pub(super) rendered_references_by_entry: BTreeMap<String, usize>,
    pub(super) rendered_text_placement_count: usize,
    pub(super) palettes_preserved: bool,
    pub(super) changes_confined_to_owned_cells: bool,
}

struct MaskedTimWriter<'a> {
    patched: &'a mut [u8],
    tim_offset: usize,
    source_cell: crate::tim::Cell,
    replacement_mask: &'a [bool],
    background_palette_index: u8,
    allowed_ranges: &'a mut Vec<[usize; 2]>,
}

pub(super) fn merge_rendered_decorative_backgrounds(
    rendered_records: &[&RenderedDecorativeBackgrounds],
) -> Result<DecorativeBackgroundCompletion> {
    ensure!(
        !rendered_records.is_empty(),
        "practical-result decorative completion has no rendered records"
    );
    let mut target_ids = BTreeSet::new();
    let mut changed_bytes_by_entry = BTreeMap::<String, usize>::new();
    let mut rendered_references_by_entry = BTreeMap::<String, usize>::new();
    let mut expected_write_count = 0usize;
    let mut rendered_text_placement_count = 0usize;
    for rendered in rendered_records {
        for target_id in &rendered.completed_target_ids {
            ensure!(
                target_ids.insert(target_id.clone()),
                "decorative-background target {target_id} completed in two records"
            );
        }
        expected_write_count += rendered.decoded_write_claims.len();
        rendered_text_placement_count += rendered.rendered_text_placement_count;
        for (entry_id, changed_byte_count) in &rendered.changed_bytes_by_entry {
            *changed_bytes_by_entry.entry(entry_id.clone()).or_default() += changed_byte_count;
        }
        for (entry_id, rendered_reference_count) in &rendered.rendered_references_by_entry {
            *rendered_references_by_entry
                .entry(entry_id.clone())
                .or_default() += rendered_reference_count;
        }
    }
    Ok(DecorativeBackgroundCompletion {
        completed_target_count: target_ids.len(),
        expected_write_count,
        changed_bytes_by_entry,
        rendered_references_by_entry,
        rendered_text_placement_count,
        palettes_preserved: rendered_records
            .iter()
            .all(|rendered| rendered.palettes_preserved),
        changes_confined_to_owned_cells: rendered_records
            .iter()
            .all(|rendered| rendered.changes_confined_to_owned_cells),
    })
}

pub(super) fn render_decorative_backgrounds(
    entries: &[PracticalResultEntry],
    catalog: &PracticalResultDecorativeBackgroundTargetCatalog,
    fonts: &ModeDescendantFontSources,
    source: &ModeDescendantSourceRecord,
    base_decoded: &[u8],
    composition_stage: PracticalResultDecorativeCompositionStage,
) -> Result<RenderedDecorativeBackgrounds> {
    ensure!(
        source.decoded.len() == base_decoded.len(),
        "decorative-background base changed record extent"
    );
    let entries_by_id = entries
        .iter()
        .map(|entry| (entry.id.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    let targets = catalog
        .targets
        .iter()
        .filter(|target| {
            target.target_record_path == source.path
                && target.composition_stage == composition_stage
        })
        .collect::<Vec<_>>();
    ensure!(
        !targets.is_empty(),
        "practical-result source {} has no decorative-background target",
        source.path
    );

    let mut patched = base_decoded.to_vec();
    let mut decoded_write_claims = Vec::new();
    let mut completed_target_ids = BTreeSet::new();
    let mut changed_bytes_by_entry = BTreeMap::<String, usize>::new();
    let mut rendered_references_by_entry = BTreeMap::<String, usize>::new();
    let mut rendered_text_placement_count = 0usize;
    let mut rasterizers = IndexedTextRasterizers::default();
    for target in targets {
        let tim_offset = parse_hex_offset(&target.target_tim_offset)?;
        let source_pixels =
            read_8bpp_indexed_cell_in_prefix(&source.decoded, tim_offset, target.source_cell)?;
        ensure!(
            sha256_bytes(&source_pixels) == target.expected_preimage_indexed_sha256,
            "decorative-background target {} source preimage changed",
            target.target_id
        );
        let palette = read_8bpp_palette_words_in_prefix(&source.decoded, tim_offset, 0)?;
        ensure_palette_entry(
            &palette,
            target.background_palette_index,
            &target.background_palette_word,
            target.target_id.as_str(),
        )?;

        let mut allowed_ranges = Vec::new();
        let mut replacement_mask =
            vec![false; target.source_cell.width * target.source_cell.height];
        if !target.preserve_regions.is_empty() {
            ensure!(
                composition_stage == PracticalResultDecorativeCompositionStage::BeforeFixedText,
                "mixed decorative-background target {} ran after fixed text",
                target.target_id
            );
            replacement_mask.fill(true);
            for region in &target.preserve_regions {
                for span in &region.spans {
                    let local_y = span.y - target.source_cell.y;
                    let local_x = span.x - target.source_cell.x;
                    let start = local_y * target.source_cell.width + local_x;
                    let end = start + span.width;
                    replacement_mask[start..end].fill(false);
                }
            }
            for cell in &target.preserve_cells {
                for y in cell.y..cell.y + cell.height {
                    let local_y = y - target.source_cell.y;
                    let local_x = cell.x - target.source_cell.x;
                    let start = local_y * target.source_cell.width + local_x;
                    replacement_mask[start..start + cell.width].fill(false);
                }
            }
        } else {
            for cell in &target.clear_cells {
                ensure!(
                    read_8bpp_indexed_cell_in_prefix(base_decoded, tim_offset, *cell)?
                        == read_8bpp_indexed_cell_in_prefix(&source.decoded, tim_offset, *cell)?,
                    "decorative-background target {} overlaps an earlier compositor in {cell:?}",
                    target.target_id
                );
                for y in cell.y..cell.y + cell.height {
                    let local_y = y - target.source_cell.y;
                    let local_x = cell.x - target.source_cell.x;
                    let start = local_y * target.source_cell.width + local_x;
                    replacement_mask[start..start + cell.width].fill(true);
                }
            }
        }

        let mut writer = MaskedTimWriter {
            patched: &mut patched,
            tim_offset,
            source_cell: target.source_cell,
            replacement_mask: &replacement_mask,
            background_palette_index: target.background_palette_index,
            allowed_ranges: &mut allowed_ranges,
        };
        let changed_byte_count = writer.fill_background()?;
        ensure!(
            changed_byte_count > 0,
            "decorative-background target {} changed no background pixels",
            target.target_id
        );

        let mut rendered_entries = BTreeSet::new();
        let mut invisible_placements = Vec::new();
        for placement in &target.text_placements {
            let entry = entries_by_id
                .get(placement.semantic_entry_id.as_str())
                .with_context(|| {
                    format!(
                        "decorative-background target {} lost semantic entry {}",
                        target.target_id, placement.semantic_entry_id
                    )
                })?;
            ensure!(
                entry.strategy == PracticalResultStrategy::DecorativeMask
                    && entry.font_role == ModeDescendantFontRole::PracticalResultBranding
                    && entry.development_status == PracticalResultDevelopmentStatus::Authored,
                "decorative-background target {} names an unauthored branding entry",
                target.target_id
            );
            ensure_palette_entry(
                &palette,
                placement.outline_palette_index,
                &placement.outline_palette_word,
                target.target_id.as_str(),
            )?;
            ensure_palette_entry(
                &palette,
                placement.fill_palette_index,
                &placement.fill_palette_word,
                target.target_id.as_str(),
            )?;
            let korean_text = entry
                .korean_text
                .as_deref()
                .context("authored decorative-background entry lost Korean text")?;
            let style = &fonts.practical_result_branding;
            let rasterizer = rasterizers.for_font(&style.path)?;
            let alignment = match placement.alignment {
                crate::mode_descendant_graphics::model::TextAlignment::Left => {
                    HorizontalTextAlignment::Left
                }
                crate::mode_descendant_graphics::model::TextAlignment::Center => {
                    HorizontalTextAlignment::Center
                }
            };
            let raster = rasterizer.rasterize_shifted(
                korean_text,
                placement.cell.width,
                placement.cell.height,
                placement.font_px,
                0.0,
                style.vertical_shift_px,
                target.background_palette_index,
                Some(placement.outline_palette_index),
                placement.fill_palette_index,
                alignment,
            )?;
            let rotated = rotate_indexed_raster(
                &raster.pixels,
                placement.cell.width,
                placement.cell.height,
                target.background_palette_index,
                placement.clockwise_rotation_degrees,
            )?;
            let changed_byte_count = writer.write_text(placement.cell, &rotated)?;
            if changed_byte_count == 0 {
                invisible_placements.push(placement.placement_id.as_str());
                continue;
            }
            *changed_bytes_by_entry.entry(entry.id.clone()).or_default() += changed_byte_count;
            rendered_entries.insert(entry.id.clone());
            rendered_text_placement_count += 1;
        }
        ensure!(
            invisible_placements.is_empty(),
            "decorative-background target {} has invisible placements: {}",
            target.target_id,
            invisible_placements.join(", ")
        );
        for entry_id in rendered_entries {
            *rendered_references_by_entry.entry(entry_id).or_default() += 1;
        }
        ensure!(
            read_8bpp_palette_words_in_prefix(&patched, tim_offset, 0)? == palette,
            "decorative-background target {} changed its source palette",
            target.target_id
        );

        let claims = DecodedDataClaim::from_effective_ranges(
            &format!("mode-descendant:practical-result:{}", target.target_id),
            "replace a source-bound practical-result decorative backdrop",
            &source.decoded,
            &patched,
            allowed_ranges,
        )?;
        ensure!(
            !claims.is_empty(),
            "decorative-background target {} produced no Expected Writes",
            target.target_id
        );
        decoded_write_claims.extend(claims);
        ensure!(
            completed_target_ids.insert(target.target_id.clone()),
            "decorative-background target {} rendered twice",
            target.target_id
        );
    }
    Ok(RenderedDecorativeBackgrounds {
        decoded: patched,
        decoded_write_claims,
        completed_target_ids,
        changed_bytes_by_entry,
        rendered_references_by_entry,
        rendered_text_placement_count,
        palettes_preserved: true,
        changes_confined_to_owned_cells: true,
    })
}

impl MaskedTimWriter<'_> {
    fn fill_background(&mut self) -> Result<usize> {
        ensure!(
            self.replacement_mask.len() == self.source_cell.width * self.source_cell.height,
            "decorative-background replacement mask geometry changed"
        );
        let mut changed_byte_count = 0usize;
        for local_y in 0..self.source_cell.height {
            let row_start = local_y * self.source_cell.width;
            let mut local_x = 0usize;
            while local_x < self.source_cell.width {
                if !self.replacement_mask[row_start + local_x] {
                    local_x += 1;
                    continue;
                }
                let run_start = local_x;
                while local_x < self.source_cell.width && self.replacement_mask[row_start + local_x]
                {
                    local_x += 1;
                }
                let run_width = local_x - run_start;
                let write = write_8bpp_indexed_cell_in_prefix_with_report(
                    self.patched,
                    self.tim_offset,
                    crate::tim::Cell {
                        x: self.source_cell.x + run_start,
                        y: self.source_cell.y + local_y,
                        width: run_width,
                        height: 1,
                    },
                    &vec![self.background_palette_index; run_width],
                )?;
                changed_byte_count += write.changed_byte_count;
                self.allowed_ranges.extend(write.allowed_ranges);
            }
        }
        Ok(changed_byte_count)
    }

    fn write_text(&mut self, placement_cell: crate::tim::Cell, pixels: &[u8]) -> Result<usize> {
        ensure!(
            self.replacement_mask.len() == self.source_cell.width * self.source_cell.height
                && pixels.len() == placement_cell.width * placement_cell.height,
            "decorative text mask geometry changed"
        );
        let mut changed_byte_count = 0usize;
        for placement_y in 0..placement_cell.height {
            let absolute_y = placement_cell.y + placement_y;
            let source_local_y = absolute_y - self.source_cell.y;
            let mut placement_x = 0usize;
            while placement_x < placement_cell.width {
                let absolute_x = placement_cell.x + placement_x;
                let source_local_x = absolute_x - self.source_cell.x;
                let placement_offset = placement_y * placement_cell.width + placement_x;
                let replacement_offset = source_local_y * self.source_cell.width + source_local_x;
                if !self.replacement_mask[replacement_offset]
                    || pixels[placement_offset] == self.background_palette_index
                {
                    placement_x += 1;
                    continue;
                }
                let run_start = placement_x;
                while placement_x < placement_cell.width {
                    let absolute_x = placement_cell.x + placement_x;
                    let source_local_x = absolute_x - self.source_cell.x;
                    let placement_offset = placement_y * placement_cell.width + placement_x;
                    let replacement_offset =
                        source_local_y * self.source_cell.width + source_local_x;
                    if !self.replacement_mask[replacement_offset]
                        || pixels[placement_offset] == self.background_palette_index
                    {
                        break;
                    }
                    placement_x += 1;
                }
                let run_width = placement_x - run_start;
                let pixel_start = placement_y * placement_cell.width + run_start;
                let write = write_8bpp_indexed_cell_in_prefix_with_report(
                    self.patched,
                    self.tim_offset,
                    crate::tim::Cell {
                        x: placement_cell.x + run_start,
                        y: absolute_y,
                        width: run_width,
                        height: 1,
                    },
                    &pixels[pixel_start..pixel_start + run_width],
                )?;
                changed_byte_count += write.changed_byte_count;
                self.allowed_ranges.extend(write.allowed_ranges);
            }
        }
        Ok(changed_byte_count)
    }
}

fn ensure_palette_entry(
    palette: &[u16; 256],
    palette_index: u8,
    expected_word: &str,
    target_id: &str,
) -> Result<()> {
    let expected_word = u16::from_str_radix(
        expected_word
            .strip_prefix("0x")
            .context("decorative palette word is not hexadecimal")?,
        16,
    )
    .context("invalid decorative palette word")?;
    ensure!(
        palette[usize::from(palette_index)] == expected_word,
        "decorative-background target {target_id} palette index {palette_index} changed"
    );
    Ok(())
}
