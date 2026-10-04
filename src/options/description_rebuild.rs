use anyhow::{Context, Result, ensure};

use crate::font::{HorizontalTextAlignment, IndexedTextRasterizer};
use crate::pipeline::difference_ranges;
use crate::tim::{Cell, install_indexed_glyph_in_prefix, read_indexed_cell_in_prefix};
use crate::write_scope::changed_ranges_are_within;

use super::description_assets::LoadedDescriptionAssets;
use super::description_model::{
    DescriptionSpan, DescriptionStream, OptionsDescriptionBuild, OptionsDescriptionGlyphBuild,
    OptionsDescriptionUnit, OptionsDescriptionVariantBuild,
};
use super::description_source::{
    ITEM_SLOT_SIZE, ITEM_TABLE_OFFSETS, STREAM_ARENA_END, STREAM_ARENA_OFFSET,
};
use super::description_stream::{
    CELL_CAPACITY, CELL_HEIGHT, CELL_WIDTH, CELLS_PER_ROW, encode_description_stream,
    parse_description_stream,
};
use super::model::{OptionsFontBuild, OptionsFontStyle};
use super::source::OVERLAY_RUNTIME_BASE;

const CLEAR_INDEX: u8 = 0;
const OUTLINE_INDEX: u8 = 3;
const FILL_INDEX: u8 = 14;

pub(super) struct DescriptionRebuildResult {
    pub(super) font: OptionsFontBuild,
    pub(super) glyphs: Vec<OptionsDescriptionGlyphBuild>,
    pub(super) descriptions: Vec<OptionsDescriptionBuild>,
    pub(super) overlay_changed_byte_ranges: Vec<[usize; 2]>,
}

#[derive(Debug)]
pub(super) struct PlannedDescriptionItem {
    pub(super) id: String,
    pub(super) item_index: usize,
    pub(super) atlas_cells: Vec<char>,
    pub(super) variants: Vec<PlannedDescriptionVariant>,
}

#[derive(Debug)]
pub(super) struct PlannedDescriptionVariant {
    pub(super) role: String,
    pub(super) korean_lines: Vec<String>,
    pub(super) stream: DescriptionStream,
}

pub(super) fn rebuild_options_descriptions(
    overlay: &mut [u8],
    optinfo_decoded: &mut [u8],
    assets: &LoadedDescriptionAssets,
    style: &OptionsFontStyle,
) -> Result<DescriptionRebuildResult> {
    ensure!(
        style.font_px.is_finite() && style.font_px > 0.0,
        "invalid options description font size"
    );
    let plans = plan_description_layout(&assets.units)?;
    let source_overlay = overlay.to_vec();
    let mut stream_cursor = STREAM_ARENA_OFFSET;
    overlay[STREAM_ARENA_OFFSET..STREAM_ARENA_END].fill(0);

    let mut descriptions = Vec::with_capacity(plans.len());
    for plan in &plans {
        let table_offset = ITEM_TABLE_OFFSETS[plan.item_index];
        let mut variants = Vec::with_capacity(plan.variants.len());
        for (variant_index, variant) in plan.variants.iter().enumerate() {
            let encoded = encode_description_stream(&variant.stream)?;
            ensure!(
                stream_cursor + encoded.len() <= STREAM_ARENA_END,
                "Korean options-description streams need more than the source-owned arena"
            );
            overlay[stream_cursor..stream_cursor + encoded.len()].copy_from_slice(&encoded);
            write_u32(
                overlay,
                table_offset + variant_index * 4,
                OVERLAY_RUNTIME_BASE
                    .checked_add(u32::try_from(stream_cursor)?)
                    .context("options-description pointer overflow")?,
            )?;
            ensure!(
                parse_description_stream(&overlay[stream_cursor..stream_cursor + encoded.len()])?
                    == variant.stream,
                "rebuilt options-description stream changed during serialization"
            );
            variants.push(OptionsDescriptionVariantBuild {
                role: variant.role.clone(),
                korean_lines: variant.korean_lines.clone(),
                output_stream_offset: format!("0x{stream_cursor:04x}"),
                output_bytes: encoded.iter().map(|byte| format!("0x{byte:02x}")).collect(),
            });
            stream_cursor += encoded.len();
        }
        descriptions.push(OptionsDescriptionBuild {
            id: plan.id.clone(),
            item_index: plan.item_index,
            used_cell_count: plan.atlas_cells.len(),
            variants,
        });
    }

    let overlay_changed_byte_ranges = difference_ranges(&source_overlay, overlay);
    ensure!(
        !overlay_changed_byte_ranges.is_empty()
            && changed_ranges_are_within(
                &overlay_changed_byte_ranges,
                &description_expected_write_ranges(),
            ),
        "options-description rebuild escaped its stream arena or pointer tables"
    );

    let (font, glyphs) = install_description_atlases(optinfo_decoded, &plans, style)?;
    Ok(DescriptionRebuildResult {
        font,
        glyphs,
        descriptions,
        overlay_changed_byte_ranges,
    })
}

pub(super) fn plan_description_layout(
    units: &[OptionsDescriptionUnit],
) -> Result<Vec<PlannedDescriptionItem>> {
    let mut items = Vec::with_capacity(units.len());
    for unit in units {
        let mut atlas = DescriptionAtlas::default();
        let mut variants = Vec::with_capacity(unit.states.len() + 1);
        for (variant_index, variant) in std::iter::once(&unit.common)
            .chain(&unit.states)
            .enumerate()
        {
            let stream = atlas.encode_lines(&variant.korean_lines)?;
            variants.push(PlannedDescriptionVariant {
                role: if variant_index == 0 {
                    "common".to_string()
                } else {
                    format!("state_{}", variant_index - 1)
                },
                korean_lines: variant.korean_lines.clone(),
                stream,
            });
        }
        ensure!(
            atlas.cells.len() <= CELL_CAPACITY,
            "options-description {} needs {} atlas cells but owns {CELL_CAPACITY}",
            unit.id,
            atlas.cells.len()
        );
        items.push(PlannedDescriptionItem {
            id: unit.id.clone(),
            item_index: unit.item_index,
            atlas_cells: atlas.cells,
            variants,
        });
    }
    Ok(items)
}

pub(super) fn description_expected_write_ranges() -> Vec<[usize; 2]> {
    let mut ranges = vec![[STREAM_ARENA_OFFSET, STREAM_ARENA_END]];
    for (item_index, table_offset) in ITEM_TABLE_OFFSETS.into_iter().enumerate() {
        let pointer_count = if item_index + 1 < ITEM_TABLE_OFFSETS.len() {
            (ITEM_TABLE_OFFSETS[item_index + 1] - table_offset) / 4
        } else {
            (super::description_source::ROOT_TABLE_OFFSET - table_offset) / 4
        };
        ranges.push([table_offset, table_offset + pointer_count * 4]);
    }
    ranges
}

fn install_description_atlases(
    optinfo_decoded: &mut [u8],
    plans: &[PlannedDescriptionItem],
    style: &OptionsFontStyle,
) -> Result<(OptionsFontBuild, Vec<OptionsDescriptionGlyphBuild>)> {
    let rasterizer = IndexedTextRasterizer::load(&style.font)?;
    let mut font_identity = None;
    let mut glyphs = Vec::new();
    for plan in plans {
        let tim_offset = plan.item_index * ITEM_SLOT_SIZE;
        for (atlas_cell, character) in plan.atlas_cells.iter().copied().enumerate() {
            let cell = atlas_cell_rect(atlas_cell);
            let (pixels, ink_bounds) = if character == ' ' {
                (vec![CLEAR_INDEX; CELL_WIDTH * CELL_HEIGHT], None)
            } else {
                let rendered = rasterizer.rasterize(
                    &character.to_string(),
                    CELL_WIDTH,
                    CELL_HEIGHT,
                    style.font_px,
                    0.0,
                    CLEAR_INDEX,
                    Some(OUTLINE_INDEX),
                    FILL_INDEX,
                    HorizontalTextAlignment::Center,
                )?;
                match &font_identity {
                    None => {
                        font_identity =
                            Some((rendered.font_name.clone(), rendered.font_sha256.clone()))
                    }
                    Some(identity) => ensure!(
                        identity == &(rendered.font_name.clone(), rendered.font_sha256.clone()),
                        "options-description font identity changed within one build"
                    ),
                }
                (rendered.pixels, Some(rendered.ink_bounds))
            };
            let source_pixels = read_indexed_cell_in_prefix(optinfo_decoded, tim_offset, cell)?;
            let install = if source_pixels == pixels {
                None
            } else {
                Some(install_indexed_glyph_in_prefix(
                    optinfo_decoded,
                    tim_offset,
                    cell,
                    &pixels,
                    &format!(
                        "options-description item {} atlas cell {} character {:?}",
                        plan.item_index, atlas_cell, character
                    ),
                )?)
            };
            glyphs.push(OptionsDescriptionGlyphBuild {
                item_index: plan.item_index,
                atlas_cell,
                character,
                cell,
                ink_bounds,
                install,
            });
        }
    }
    let (font_name, font_sha256) =
        font_identity.context("options descriptions contain no visible glyphs")?;
    Ok((
        OptionsFontBuild {
            role: "options_description".to_string(),
            font_name,
            font_sha256,
            font_px: style.font_px,
        },
        glyphs,
    ))
}

#[derive(Default)]
struct DescriptionAtlas {
    cells: Vec<char>,
}

impl DescriptionAtlas {
    fn encode_lines(&mut self, lines: &[String]) -> Result<DescriptionStream> {
        ensure!(
            !lines.is_empty(),
            "options-description variant has no lines"
        );
        let lines = lines
            .iter()
            .map(|line| {
                ensure!(
                    !line.is_empty(),
                    "options-description contains an empty line"
                );
                self.encode_line(&line.chars().collect::<Vec<_>>())
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(DescriptionStream { lines })
    }

    fn encode_line(&mut self, line: &[char]) -> Result<Vec<DescriptionSpan>> {
        let mut spans = Vec::new();
        let mut cursor = 0usize;
        while cursor < line.len() {
            let (existing_start, existing_count) = self.longest_match(&line[cursor..]);
            if existing_count >= 3 {
                spans.push(DescriptionSpan {
                    start_cell: u8::try_from(existing_start)?,
                    cell_count: u8::try_from(existing_count)?,
                });
                cursor += existing_count;
                continue;
            }

            let row_capacity = CELLS_PER_ROW - self.cells.len() % CELLS_PER_ROW;
            let remaining = &line[cursor..];
            let future_reuse = (1..remaining.len().min(row_capacity))
                .find(|offset| self.longest_match(&remaining[*offset..]).1 >= 3);
            let count = future_reuse.unwrap_or_else(|| remaining.len().min(row_capacity));
            ensure!(
                count > 0,
                "options-description atlas allocation made no progress"
            );
            ensure!(
                self.cells.len() + count <= CELL_CAPACITY,
                "options-description item exceeds its {CELL_CAPACITY}-cell atlas"
            );
            let start = self.cells.len();
            self.cells.extend_from_slice(&remaining[..count]);
            spans.push(DescriptionSpan {
                start_cell: u8::try_from(start)?,
                cell_count: u8::try_from(count)?,
            });
            cursor += count;
        }
        Ok(spans)
    }

    fn longest_match(&self, input: &[char]) -> (usize, usize) {
        let mut best = (0, 0);
        for start in 0..self.cells.len() {
            let row_end = ((start / CELLS_PER_ROW) + 1) * CELLS_PER_ROW;
            let available = row_end.min(self.cells.len()) - start;
            let count = input
                .iter()
                .zip(&self.cells[start..start + available])
                .take_while(|(left, right)| left == right)
                .count();
            if count > best.1 {
                best = (start, count);
            }
        }
        best
    }
}

fn atlas_cell_rect(atlas_cell: usize) -> Cell {
    Cell {
        x: atlas_cell % CELLS_PER_ROW * CELL_WIDTH,
        y: atlas_cell / CELLS_PER_ROW * CELL_HEIGHT,
        width: CELL_WIDTH,
        height: CELL_HEIGHT,
    }
}

fn write_u32(output: &mut [u8], offset: usize, value: u32) -> Result<()> {
    output
        .get_mut(offset..offset + 4)
        .context("options-description pointer write is out of bounds")?
        .copy_from_slice(&value.to_le_bytes());
    Ok(())
}
