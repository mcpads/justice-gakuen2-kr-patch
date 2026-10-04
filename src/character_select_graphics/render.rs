//! Rasterizes the in-memory character-select plan without choosing record ownership.

use anyhow::{Context, Result, ensure};

use crate::font::{
    HorizontalTextAlignment, IndexedTextRasterizer, IndexedTextRasterizers, RasterizedIndexedText,
};

use super::model::{
    CharacterSelectAtlasBuildConfig, CharacterSelectFixedStripAllocation, CharacterSelectFontRole,
    CharacterSelectGlyphAllocation, CharacterSelectLocalizedSource,
    CharacterSelectSegmentedFixedStripAllocation, CharacterSelectTextFlow,
    CharacterSelectTextSelection,
};

const CLEAR_INDEX: u8 = 0;

#[derive(Clone, Copy)]
struct IndexedInk {
    outline_index: u8,
    fill_index: u8,
}

fn indexed_ink(font_role: CharacterSelectFontRole) -> IndexedInk {
    match font_role {
        CharacterSelectFontRole::ModeMenuHeading => IndexedInk {
            outline_index: 1,
            fill_index: 10,
        },
        CharacterSelectFontRole::ModeMenuLabel => IndexedInk {
            outline_index: 2,
            fill_index: 15,
        },
        CharacterSelectFontRole::CooperativeEmblemCharacter => IndexedInk {
            outline_index: 12,
            fill_index: 15,
        },
        CharacterSelectFontRole::TournamentCertificateTitle
        | CharacterSelectFontRole::TournamentCertificateLabel
        | CharacterSelectFontRole::TournamentCertificateBody => IndexedInk {
            outline_index: 14,
            // TOROFY index 0 is transparent in the native textured sprite.
            // Use the original opaque dark ink instead of exposing the bracket.
            fill_index: 1,
        },
        // Both selection-help readers use CLUT (0,482): index 14 is yellow,
        // index 2 is the dark source outline; 15 belongs to magenta labels.
        CharacterSelectFontRole::SelectionHelp => IndexedInk {
            outline_index: 2,
            fill_index: 14,
        },
        CharacterSelectFontRole::SoloStatePrompt => IndexedInk {
            outline_index: 7,
            fill_index: 3,
        },
        CharacterSelectFontRole::CommonPauseMenu => IndexedInk {
            outline_index: 3,
            fill_index: 15,
        },
        CharacterSelectFontRole::SoloStoryIntro => IndexedInk {
            outline_index: 2,
            fill_index: 15,
        },
        CharacterSelectFontRole::SoloEpisodeCard => IndexedInk {
            outline_index: 14,
            fill_index: 1,
        },
        _ => IndexedInk {
            outline_index: 3,
            fill_index: 15,
        },
    }
}

pub(super) struct RenderedGlyph {
    pub(super) allocation: CharacterSelectGlyphAllocation,
    pub(super) raster: RasterizedIndexedText,
    pub(super) font_px: f32,
    pub(super) vertical_shift_px: i32,
    pub(super) clear_index: u8,
    pub(super) outline_index: u8,
    pub(super) fill_index: u8,
}

pub(super) struct RenderedFixedStrip {
    pub(super) allocation: CharacterSelectFixedStripAllocation,
    pub(super) korean_text: String,
    pub(super) raster: RasterizedIndexedText,
    pub(super) font_px: f32,
    pub(super) vertical_shift_px: i32,
    pub(super) outline_index: u8,
    pub(super) fill_index: u8,
}

pub(super) fn render_glyphs(
    rasterizers: &mut IndexedTextRasterizers,
    config: &CharacterSelectAtlasBuildConfig,
    allocations: &[CharacterSelectGlyphAllocation],
) -> Result<Vec<RenderedGlyph>> {
    let mut rendered = Vec::with_capacity(allocations.len());
    for allocation in allocations {
        let style = config.fonts.style(allocation.font_role);
        let ink = indexed_ink(allocation.font_role);
        let rendered_character = allocation.character.to_string();
        let raster_text = if allocation.character.is_whitespace() {
            "="
        } else {
            rendered_character.as_str()
        };
        let mut raster = rasterizers
            .for_font(&style.path)?
            .rasterize_shifted(
                raster_text,
                allocation.cell.width,
                allocation.cell.height,
                style.font_px,
                0.0,
                style.vertical_shift_px,
                CLEAR_INDEX,
                Some(ink.outline_index),
                ink.fill_index,
                HorizontalTextAlignment::Center,
            )
            .with_context(|| {
                format!(
                    "failed to rasterize character-select {:?} glyph {:?}",
                    allocation.font_role, allocation.character
                )
            })?;
        if allocation.character.is_whitespace() {
            raster.pixels.fill(CLEAR_INDEX);
            raster.measured_advance_px = 0.0;
            raster.ink_bounds = [0; 4];
        }
        rendered.push(RenderedGlyph {
            allocation: allocation.clone(),
            raster,
            font_px: style.font_px,
            vertical_shift_px: style.vertical_shift_px,
            clear_index: CLEAR_INDEX,
            outline_index: ink.outline_index,
            fill_index: ink.fill_index,
        });
    }
    Ok(rendered)
}

pub(super) fn render_fixed_strips(
    rasterizers: &mut IndexedTextRasterizers,
    config: &CharacterSelectAtlasBuildConfig,
    allocations: &[CharacterSelectFixedStripAllocation],
    localized_sources: &[CharacterSelectLocalizedSource],
) -> Result<Vec<RenderedFixedStrip>> {
    let mut rendered = Vec::with_capacity(allocations.len());
    for allocation in allocations {
        let korean_text = select_fixed_strip_text(
            &allocation.physical_text_region_id,
            &allocation.source_ui_ids,
            &allocation.translation_id,
            allocation.text_selection,
            localized_sources,
        )?;
        ensure_fixed_strip_font_role(allocation.font_role, &allocation.source_ui_ids)?;
        let style = config.fonts.style(allocation.font_role);
        let ink = indexed_ink(allocation.font_role);
        let rasterizer = rasterizers.for_font(&style.path)?;
        let raster = rasterize_fixed_text(
            rasterizer,
            &korean_text,
            allocation.text_cell.width,
            allocation.text_cell.height,
            style.font_px,
            0.0,
            style.vertical_shift_px,
            allocation.clear_index,
            ink,
            allocation.font_role,
            allocation.text_flow,
        )
        .with_context(|| {
            format!(
                "failed to rasterize character-select fixed strip {}",
                allocation.source_ui_ids.join(", ")
            )
        })?;
        let raster = embed_text_raster_in_physical_region(raster, allocation)?;
        rendered.push(RenderedFixedStrip {
            allocation: allocation.clone(),
            korean_text,
            raster,
            font_px: style.font_px,
            vertical_shift_px: style.vertical_shift_px,
            outline_index: ink.outline_index,
            fill_index: ink.fill_index,
        });
    }
    Ok(rendered)
}

pub(super) fn render_segmented_fixed_strips(
    rasterizers: &mut IndexedTextRasterizers,
    config: &CharacterSelectAtlasBuildConfig,
    allocations: &[CharacterSelectSegmentedFixedStripAllocation],
    localized_sources: &[CharacterSelectLocalizedSource],
) -> Result<Vec<RenderedFixedStrip>> {
    let segment_count = allocations
        .iter()
        .map(|allocation| allocation.segments.len())
        .sum();
    let mut rendered = Vec::with_capacity(segment_count);
    for allocation in allocations {
        let korean_text = select_fixed_strip_text(
            &allocation.logical_text_region_id,
            &allocation.source_ui_ids,
            &allocation.translation_id,
            allocation.text_selection,
            localized_sources,
        )?;
        ensure_fixed_strip_font_role(allocation.font_role, &allocation.source_ui_ids)?;
        let style = config.fonts.style(allocation.font_role);
        let ink = indexed_ink(allocation.font_role);
        let rasterizer = rasterizers.for_font(&style.path)?;
        let render_width = allocation.logical_width / allocation.horizontal_scale;
        let logical_raster = rasterize_fixed_text(
            rasterizer,
            &korean_text,
            render_width,
            allocation.logical_height,
            style.font_px,
            allocation.tracking_px,
            style.vertical_shift_px,
            allocation.clear_index,
            ink,
            allocation.font_role,
            allocation.text_flow,
        )
        .with_context(|| {
            format!(
                "failed to rasterize segmented character-select strip {}",
                allocation.logical_text_region_id
            )
        })?;
        let logical_raster = scale_indexed_raster_horizontally(
            logical_raster,
            render_width,
            allocation.logical_height,
            allocation.horizontal_scale,
            allocation.clear_index,
        )?;
        for segment in &allocation.segments {
            let raster = crop_indexed_raster(
                &logical_raster,
                allocation.logical_width,
                allocation.logical_height,
                segment.logical_origin,
                segment.cell.width,
                segment.cell.height,
                allocation.clear_index,
            )?;
            rendered.push(RenderedFixedStrip {
                allocation: CharacterSelectFixedStripAllocation {
                    physical_text_region_id: segment.physical_text_region_id.to_string(),
                    source_ui_ids: allocation.source_ui_ids.clone(),
                    translation_id: allocation.translation_id.clone(),
                    text_selection: allocation.text_selection,
                    text_flow: allocation.text_flow,
                    write_mode: allocation.write_mode,
                    font_role: allocation.font_role,
                    surface: allocation.surface,
                    tim_offset: allocation.tim_offset,
                    cell: segment.cell,
                    text_cell: segment.cell,
                    clear_index: allocation.clear_index,
                },
                korean_text: korean_text.clone(),
                raster,
                font_px: style.font_px,
                vertical_shift_px: style.vertical_shift_px,
                outline_index: ink.outline_index,
                fill_index: ink.fill_index,
            });
        }
    }
    Ok(rendered)
}

fn scale_indexed_raster_horizontally(
    mut raster: RasterizedIndexedText,
    source_width: usize,
    height: usize,
    scale: usize,
    clear_index: u8,
) -> Result<RasterizedIndexedText> {
    ensure!(
        source_width > 0 && height > 0 && scale > 0 && raster.pixels.len() == source_width * height,
        "invalid indexed horizontal-scale input"
    );
    if scale == 1 {
        return Ok(raster);
    }
    let target_width = source_width * scale;
    let mut pixels = Vec::with_capacity(target_width * height);
    for row in raster.pixels.chunks_exact(source_width) {
        for pixel in row {
            pixels.extend(std::iter::repeat_n(*pixel, scale));
        }
    }
    raster.pixels = pixels;
    raster.measured_advance_px *= scale as f32;
    raster.ink_bounds = indexed_ink_bounds(&raster.pixels, target_width, height, clear_index)
        .context("horizontally scaled indexed text has no ink")?;
    Ok(raster)
}

fn select_fixed_strip_text(
    region_id: &str,
    source_ui_ids: &[String],
    translation_id: &str,
    selection: CharacterSelectTextSelection,
    localized_sources: &[CharacterSelectLocalizedSource],
) -> Result<String> {
    let entry = source_ui_ids
        .iter()
        .find_map(|source_ui_id| {
            localized_sources
                .iter()
                .find(|entry| entry.source_ui_id == *source_ui_id)
        })
        .with_context(|| format!("missing character-select fixed strip {region_id}"))?;
    ensure!(
        source_ui_ids.iter().all(|source_ui_id| {
            localized_sources.iter().any(|source| {
                source.source_ui_id == *source_ui_id && source.translation_id == translation_id
            })
        }),
        "character-select fixed strip {region_id} changed semantic translation"
    );
    match selection {
        CharacterSelectTextSelection::Character { index } => entry
            .korean_text
            .chars()
            .nth(index)
            .with_context(|| {
                format!(
                    "character-select fixed region {region_id} selects missing character {index} from {translation_id}"
                )
            })
            .map(|character| character.to_string()),
        CharacterSelectTextSelection::Line { index } => entry
            .korean_text
            .lines()
            .nth(index)
            .with_context(|| {
                format!("character-select translation {translation_id} lost line {index}")
            })
            .map(str::to_string),
        CharacterSelectTextSelection::Entire => Ok(entry.korean_text.clone()),
        CharacterSelectTextSelection::Suffix { skip_characters } => {
            ensure!(
                skip_characters < entry.korean_text.chars().count(),
                "character-select translation {translation_id} has no selected suffix"
            );
            Ok(entry.korean_text.chars().skip(skip_characters).collect())
        }
    }
}

fn ensure_fixed_strip_font_role(
    role: CharacterSelectFontRole,
    source_ui_ids: &[String],
) -> Result<()> {
    ensure!(
        matches!(
            role,
            CharacterSelectFontRole::Label
                | CharacterSelectFontRole::TournamentBracketLabel
                | CharacterSelectFontRole::ModeMenuLabel
                | CharacterSelectFontRole::CooperativeEmblemCharacter
                | CharacterSelectFontRole::TournamentCertificateTitle
                | CharacterSelectFontRole::TournamentCertificateLabel
                | CharacterSelectFontRole::TournamentCertificateBody
                | CharacterSelectFontRole::RosterName
                | CharacterSelectFontRole::FixedPrompt
                | CharacterSelectFontRole::CompactPrompt
                | CharacterSelectFontRole::SoloStatePrompt
                | CharacterSelectFontRole::CommonPauseMenu
                | CharacterSelectFontRole::SoloStoryIntro
                | CharacterSelectFontRole::SoloEpisodeCard
                | CharacterSelectFontRole::PracticalSelectionLabel
                | CharacterSelectFontRole::LeagueStandingLabel
        ),
        "character-select fixed strip {} uses a non-strip font role",
        source_ui_ids.join(", ")
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn rasterize_fixed_text(
    rasterizer: &IndexedTextRasterizer,
    text: &str,
    width: usize,
    height: usize,
    font_px: f32,
    tracking_px: f32,
    vertical_shift_px: i32,
    clear_index: u8,
    ink: IndexedInk,
    font_role: CharacterSelectFontRole,
    text_flow: CharacterSelectTextFlow,
) -> Result<RasterizedIndexedText> {
    match text_flow {
        CharacterSelectTextFlow::Horizontal | CharacterSelectTextFlow::HorizontalLeft => {
            let alignment = match text_flow {
                CharacterSelectTextFlow::Horizontal => HorizontalTextAlignment::Center,
                CharacterSelectTextFlow::HorizontalLeft => HorizontalTextAlignment::Left,
                CharacterSelectTextFlow::Vertical => unreachable!(),
            };
            if font_role == CharacterSelectFontRole::SoloStoryIntro {
                rasterizer.rasterize_shifted_with_coverage_ramp(
                    text,
                    width,
                    height,
                    font_px,
                    tracking_px,
                    vertical_shift_px,
                    clear_index,
                    ink.outline_index,
                    ink.fill_index,
                    alignment,
                )
            } else {
                rasterizer.rasterize_shifted(
                    text,
                    width,
                    height,
                    font_px,
                    tracking_px,
                    vertical_shift_px,
                    clear_index,
                    Some(ink.outline_index),
                    ink.fill_index,
                    alignment,
                )
            }
        }
        CharacterSelectTextFlow::Vertical => rasterize_vertical_text(
            rasterizer,
            text,
            width,
            height,
            font_px,
            vertical_shift_px,
            clear_index,
            ink,
        ),
    }
}

pub(super) fn crop_indexed_raster(
    source: &RasterizedIndexedText,
    source_width: usize,
    source_height: usize,
    origin: [usize; 2],
    width: usize,
    height: usize,
    clear_index: u8,
) -> Result<RasterizedIndexedText> {
    ensure!(
        source.pixels.len() == source_width * source_height
            && origin[0] + width <= source_width
            && origin[1] + height <= source_height,
        "segmented character-select raster crop escaped its logical surface"
    );
    let mut pixels = Vec::with_capacity(width * height);
    for y in origin[1]..origin[1] + height {
        let row_start = y * source_width + origin[0];
        pixels.extend_from_slice(&source.pixels[row_start..row_start + width]);
    }
    let ink_bounds = indexed_ink_bounds(&pixels, width, height, clear_index).unwrap_or([0; 4]);
    Ok(RasterizedIndexedText {
        font_name: source.font_name.clone(),
        font_sha256: source.font_sha256.clone(),
        pixels,
        measured_advance_px: source.measured_advance_px,
        ink_bounds,
    })
}

#[allow(clippy::too_many_arguments)]
fn rasterize_vertical_text(
    rasterizer: &IndexedTextRasterizer,
    text: &str,
    width: usize,
    height: usize,
    font_px: f32,
    vertical_shift_px: i32,
    clear_index: u8,
    ink: IndexedInk,
) -> Result<RasterizedIndexedText> {
    let characters = text
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<Vec<_>>();
    ensure!(
        !characters.is_empty(),
        "vertical character-select text is empty"
    );
    let slot_height = height / characters.len();
    ensure!(
        slot_height > 0,
        "vertical character-select text has more characters than rows"
    );
    let used_height = slot_height * characters.len();
    let origin_y = (height - used_height) / 2;
    let mut pixels = vec![clear_index; width * height];
    let mut font_name = None;
    let mut font_sha256 = None;
    for (index, character) in characters.iter().enumerate() {
        let glyph = rasterizer.rasterize_shifted(
            &character.to_string(),
            width,
            slot_height,
            font_px,
            0.0,
            vertical_shift_px,
            clear_index,
            Some(ink.outline_index),
            ink.fill_index,
            HorizontalTextAlignment::Center,
        )?;
        if let Some(existing) = &font_name {
            ensure!(
                existing == &glyph.font_name,
                "vertical text font identity changed"
            );
        } else {
            font_name = Some(glyph.font_name.clone());
        }
        if let Some(existing) = &font_sha256 {
            ensure!(
                existing == &glyph.font_sha256,
                "vertical text font hash changed"
            );
        } else {
            font_sha256 = Some(glyph.font_sha256.clone());
        }
        let target_y = origin_y + index * slot_height;
        for row in 0..slot_height {
            let source_start = row * width;
            let target_start = (target_y + row) * width;
            pixels[target_start..target_start + width]
                .copy_from_slice(&glyph.pixels[source_start..source_start + width]);
        }
    }
    let ink_bounds = indexed_ink_bounds(&pixels, width, height, clear_index)
        .context("vertical character-select text has no ink")?;
    Ok(RasterizedIndexedText {
        font_name: font_name.context("vertical character-select font disappeared")?,
        font_sha256: font_sha256.context("vertical character-select font hash disappeared")?,
        pixels,
        measured_advance_px: used_height as f32,
        ink_bounds,
    })
}

fn indexed_ink_bounds(
    pixels: &[u8],
    width: usize,
    height: usize,
    clear_index: u8,
) -> Option<[usize; 4]> {
    let mut bounds = [width, height, 0, 0];
    let mut found = false;
    for y in 0..height {
        for x in 0..width {
            if pixels[y * width + x] == clear_index {
                continue;
            }
            found = true;
            bounds[0] = bounds[0].min(x);
            bounds[1] = bounds[1].min(y);
            bounds[2] = bounds[2].max(x + 1);
            bounds[3] = bounds[3].max(y + 1);
        }
    }
    found.then_some(bounds)
}

fn embed_text_raster_in_physical_region(
    mut raster: RasterizedIndexedText,
    allocation: &CharacterSelectFixedStripAllocation,
) -> Result<RasterizedIndexedText> {
    ensure!(
        allocation.text_cell.x >= allocation.cell.x
            && allocation.text_cell.y >= allocation.cell.y
            && allocation.text_cell.x + allocation.text_cell.width
                <= allocation.cell.x + allocation.cell.width
            && allocation.text_cell.y + allocation.text_cell.height
                <= allocation.cell.y + allocation.cell.height,
        "character-select text cell for {} escaped its physical write region",
        allocation.physical_text_region_id
    );
    if allocation.text_cell == allocation.cell {
        return Ok(raster);
    }
    ensure!(
        raster.pixels.len() == allocation.text_cell.width * allocation.text_cell.height,
        "character-select text raster for {} changed dimensions",
        allocation.physical_text_region_id
    );
    let offset_x = allocation.text_cell.x - allocation.cell.x;
    let offset_y = allocation.text_cell.y - allocation.cell.y;
    let mut physical_pixels =
        vec![allocation.clear_index; allocation.cell.width * allocation.cell.height];
    for text_y in 0..allocation.text_cell.height {
        let source_start = text_y * allocation.text_cell.width;
        let target_start = (offset_y + text_y) * allocation.cell.width + offset_x;
        physical_pixels[target_start..target_start + allocation.text_cell.width].copy_from_slice(
            &raster.pixels[source_start..source_start + allocation.text_cell.width],
        );
    }
    raster.pixels = physical_pixels;
    raster.ink_bounds[0] += offset_x;
    raster.ink_bounds[1] += offset_y;
    raster.ink_bounds[2] += offset_x;
    raster.ink_bounds[3] += offset_y;
    Ok(raster)
}
