use anyhow::{Context, Result, ensure};

use crate::decoded_record_write_plan::DecodedDataClaim;
use crate::development_build_spec::SizedFontSource;
use crate::font::{HorizontalTextAlignment, IndexedTextRasterizer, IndexedTextRasterizers};
use crate::pipeline::sha256_bytes;
use crate::tim::{
    Cell, parse_4bpp_prefix, read_4bpp_indexed_image_in_prefix, read_indexed_cell_in_prefix,
    write_indexed_cell_in_prefix,
};

use super::model::{
    DiarySceneFixedPresentationFamily, DiarySceneFixedPresentationFontRole,
    DiarySceneFixedPresentationRendering, DiarySceneFontSources,
};

const PRESERVED_REMAINDER_EXCLUDED_INDEX: u8 = 0x10;

#[cfg(test)]
#[path = "cooperative_selection_tests.rs"]
mod cooperative_selection_tests;

pub(super) struct FixedPresentationContribution {
    pub(super) family_id: String,
    pub(super) candidate: Vec<u8>,
    pub(super) claims: Vec<DecodedDataClaim>,
    pub(super) translated_region_ids: Vec<String>,
    pub(super) preserved_region_ids: Vec<String>,
}

pub(super) fn family_owns_consumer(
    family: &DiarySceneFixedPresentationFamily,
    path: &str,
    member_index: usize,
) -> bool {
    family
        .consumers
        .iter()
        .any(|consumer| consumer.path == path && consumer.member_index == member_index)
}

pub(super) fn build_fixed_presentation_contribution(
    source_decoded: &[u8],
    family: &DiarySceneFixedPresentationFamily,
    fonts: &DiarySceneFontSources,
) -> Result<FixedPresentationContribution> {
    ensure!(
        sha256_bytes(source_decoded) == family.source_decoded_sha256,
        "fixed-presentation family {} decoded source changed",
        family.id
    );
    let tim_prefix = source_decoded
        .get(family.tim_offset..)
        .with_context(|| format!("fixed-presentation family {} TIM disappeared", family.id))?;
    let tim = parse_4bpp_prefix(tim_prefix)?;
    let source_tim = tim_prefix
        .get(..tim.total_size)
        .context("fixed-presentation TIM is truncated")?;
    ensure!(
        sha256_bytes(source_tim) == family.source_tim_sha256,
        "fixed-presentation family {} TIM source changed",
        family.id
    );
    let source_image = read_4bpp_indexed_image_in_prefix(source_decoded, family.tim_offset)?;
    ensure!(
        family.transparent_index < 16,
        "fixed-presentation family {} has an invalid transparent index",
        family.id
    );
    let all_cells = family
        .translated_regions
        .iter()
        .map(|region| region.cell)
        .chain(family.preserved_regions.iter().map(|region| region.cell))
        .collect::<Vec<_>>();
    if let Some(remainder) = &family.preserved_remainder {
        let signature = preserved_remainder_signature(
            &source_image.pixels,
            source_image.width,
            source_image.height,
            &all_cells,
        )?;
        ensure!(
            sha256_bytes(&signature) == remainder.source_indexed_sha256,
            "fixed-presentation preserved remainder {} source changed",
            remainder.id
        );
    } else {
        ensure!(
            uncovered_nontransparent_pixel_count(
                &source_image.pixels,
                source_image.width,
                source_image.height,
                family.transparent_index,
                &all_cells,
            )? == 0,
            "fixed-presentation family {} leaves source presentation pixels unlabeled",
            family.id
        );
    }

    for region in &family.preserved_regions {
        let pixels = read_indexed_cell_in_prefix(source_decoded, family.tim_offset, region.cell)?;
        let found = sha256_bytes(&pixels);
        ensure!(
            found == region.source_indexed_sha256,
            "fixed-presentation preserved region {} source changed: found {found}",
            region.id,
        );
    }

    let mut candidate = source_decoded.to_vec();
    let mut rasterizers = IndexedTextRasterizers::default();
    let mut region_ranges = Vec::with_capacity(family.translated_regions.len());
    let mut translated_region_ids = Vec::with_capacity(family.translated_regions.len());
    for region in &family.translated_regions {
        let source_pixels =
            read_indexed_cell_in_prefix(source_decoded, family.tim_offset, region.cell)?;
        let found = sha256_bytes(&source_pixels);
        ensure!(
            found == region.source_indexed_sha256,
            "fixed-presentation translated region {} source changed: found {found}",
            region.id,
        );
        let style = font_for_role(fonts, region.font_role);
        let rasterizer = rasterizers.for_font(&style.path)?;
        let pixels = render_region(
            rasterizer,
            style,
            region.cell,
            &region.korean_text,
            region.rendering,
        )?;
        let ranges =
            write_indexed_cell_in_prefix(&mut candidate, family.tim_offset, region.cell, &pixels)?;
        let output_pixels =
            read_indexed_cell_in_prefix(&candidate, family.tim_offset, region.cell)?;
        ensure!(
            output_pixels == pixels && output_pixels != source_pixels,
            "fixed-presentation translated region {} did not replace its source",
            region.id
        );
        region_ranges.push((region, ranges));
        translated_region_ids.push(region.id.clone());
    }

    let mut preserved_region_ids = family
        .preserved_regions
        .iter()
        .map(|region| {
            let pixels = read_indexed_cell_in_prefix(&candidate, family.tim_offset, region.cell)?;
            ensure!(
                sha256_bytes(&pixels) == region.source_indexed_sha256,
                "fixed-presentation contribution changed preserved region {}",
                region.id
            );
            Ok(region.id.clone())
        })
        .collect::<Result<Vec<_>>>()?;
    if let Some(remainder) = &family.preserved_remainder {
        let candidate_image = read_4bpp_indexed_image_in_prefix(&candidate, family.tim_offset)?;
        let signature = preserved_remainder_signature(
            &candidate_image.pixels,
            candidate_image.width,
            candidate_image.height,
            &all_cells,
        )?;
        ensure!(
            sha256_bytes(&signature) == remainder.source_indexed_sha256,
            "fixed-presentation contribution changed preserved remainder {}",
            remainder.id
        );
        preserved_region_ids.push(remainder.id.clone());
    }

    let mut claims = Vec::new();
    for (region, ranges) in region_ranges {
        claims.extend(DecodedDataClaim::from_effective_ranges(
            &format!("diary-scene-fixed-presentation:{}:{}", family.id, region.id),
            &format!(
                "replace fixed presentation {:?} with {:?}",
                region.source_text, region.korean_text
            ),
            source_decoded,
            &candidate,
            ranges,
        )?);
    }
    ensure!(
        !claims.is_empty(),
        "fixed-presentation family {} produced no effective writes",
        family.id
    );
    Ok(FixedPresentationContribution {
        family_id: family.id.clone(),
        candidate,
        claims,
        translated_region_ids,
        preserved_region_ids,
    })
}

fn font_for_role(
    fonts: &DiarySceneFontSources,
    role: DiarySceneFixedPresentationFontRole,
) -> &SizedFontSource {
    match role {
        DiarySceneFixedPresentationFontRole::MovementMapLabel => &fonts.movement_map_label,
        DiarySceneFixedPresentationFontRole::Heading => &fonts.exam_heading,
        DiarySceneFixedPresentationFontRole::Stamp => &fonts.exam_stamp,
        DiarySceneFixedPresentationFontRole::Evaluation => &fonts.exam_evaluation,
        DiarySceneFixedPresentationFontRole::Completion => &fonts.exam_finished,
        DiarySceneFixedPresentationFontRole::TrainingListLabel => &fonts.training_list_label,
        DiarySceneFixedPresentationFontRole::TrainingStatusAxis => &fonts.training_status_axis,
        DiarySceneFixedPresentationFontRole::CooperativeSelection => &fonts.cooperative_selection,
    }
}

fn render_region(
    rasterizer: &IndexedTextRasterizer,
    style: &SizedFontSource,
    cell: Cell,
    text: &str,
    rendering: DiarySceneFixedPresentationRendering,
) -> Result<Vec<u8>> {
    match rendering {
        DiarySceneFixedPresentationRendering::SplitLine {
            clear_index,
            outline_index,
            fill_index,
            second_width,
        } => {
            ensure!(
                cell.height.is_multiple_of(2) && second_width > 0 && second_width <= cell.width,
                "invalid split-line geometry"
            );
            let line_width = cell.width + second_width;
            let line_height = cell.height / 2;
            let line = render_region(
                rasterizer,
                style,
                Cell {
                    x: 0,
                    y: 0,
                    width: line_width,
                    height: line_height,
                },
                text,
                DiarySceneFixedPresentationRendering::Outlined {
                    clear_index,
                    outline_index,
                    fill_index,
                    horizontal_alignment: HorizontalTextAlignment::Left,
                },
            )?;
            let mut pixels = vec![clear_index; cell.width * cell.height];
            for y in 0..line_height {
                pixels[y * cell.width..(y + 1) * cell.width]
                    .copy_from_slice(&line[y * line_width..y * line_width + cell.width]);
                let start = (y + line_height) * cell.width;
                pixels[start..start + second_width]
                    .copy_from_slice(&line[y * line_width + cell.width..(y + 1) * line_width]);
            }
            Ok(pixels)
        }
        DiarySceneFixedPresentationRendering::Outlined {
            clear_index,
            outline_index,
            fill_index,
            horizontal_alignment,
        } => {
            let raster = rasterizer.rasterize(
                text,
                cell.width,
                cell.height,
                style.font_px,
                0.0,
                clear_index,
                Some(outline_index),
                fill_index,
                HorizontalTextAlignment::Center,
            )?;
            let [left, top, right, bottom] = raster.ink_bounds;
            ensure!(
                top > 0 && bottom < cell.height && left > 0 && right < cell.width,
                "fixed-presentation text {text:?} loses its outline margin: {:?}",
                raster.ink_bounds
            );
            // Align the complete outlined ink, including negative font bearings.
            // Rendering directly at x=0 can silently clip the left outline.
            let target_left = match horizontal_alignment {
                HorizontalTextAlignment::Left => 1,
                HorizontalTextAlignment::Center => left,
                HorizontalTextAlignment::Right => cell.width - (right - left) - 1,
            };
            let mut pixels = vec![clear_index; cell.width * cell.height];
            for y in top..bottom {
                let source = y * cell.width + left;
                let destination = y * cell.width + target_left;
                pixels[destination..destination + right - left]
                    .copy_from_slice(&raster.pixels[source..source + right - left]);
            }
            Ok(pixels)
        }
        DiarySceneFixedPresentationRendering::CoverageRamp {
            clear_index,
            first_ink_index,
            last_ink_index,
            horizontal_alignment,
        } => Ok(rasterizer
            .rasterize_shifted_with_coverage_ramp(
                text,
                cell.width,
                cell.height,
                style.font_px,
                0.0,
                0,
                clear_index,
                first_ink_index,
                last_ink_index,
                horizontal_alignment.unwrap_or(HorizontalTextAlignment::Center),
            )?
            .pixels),
        DiarySceneFixedPresentationRendering::VerticalStamp {
            clear_index,
            fill_index,
            border_inset,
            border_thickness,
        } => render_vertical_stamp(
            rasterizer,
            style,
            cell,
            text,
            clear_index,
            fill_index,
            border_inset,
            border_thickness,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn render_vertical_stamp(
    rasterizer: &IndexedTextRasterizer,
    style: &SizedFontSource,
    cell: Cell,
    text: &str,
    clear_index: u8,
    fill_index: u8,
    border_inset: usize,
    border_thickness: usize,
) -> Result<Vec<u8>> {
    let characters = text.chars().collect::<Vec<_>>();
    ensure!(
        characters.len() == 2,
        "vertical exam stamp must contain exactly two characters"
    );
    ensure!(
        clear_index < 16 && fill_index < 16 && clear_index != fill_index,
        "vertical exam stamp palette roles are invalid"
    );
    ensure!(
        border_thickness > 0
            && border_inset + border_thickness < cell.width / 2
            && border_inset + border_thickness < cell.height / 2,
        "vertical exam stamp border does not fit its cell"
    );
    let mut pixels = vec![clear_index; cell.width * cell.height];
    draw_rectangular_border(
        &mut pixels,
        cell.width,
        cell.height,
        border_inset,
        border_thickness,
        fill_index,
    );
    let protected_inset = border_inset + border_thickness;
    let row_height = cell.height / 2;
    ensure!(row_height > 0, "vertical exam stamp has no glyph rows");
    for (index, character) in characters.into_iter().enumerate() {
        let raster = rasterizer.rasterize(
            &character.to_string(),
            cell.width,
            row_height,
            style.font_px,
            0.0,
            clear_index,
            None,
            fill_index,
            HorizontalTextAlignment::Center,
        )?;
        ensure!(
            raster.ink_bounds[0] >= protected_inset
                && raster.ink_bounds[2] <= cell.width - protected_inset,
            "vertical exam stamp glyph touches its border"
        );
        let band_top = if index == 0 {
            protected_inset
        } else {
            cell.height / 2
        };
        let band_bottom = if index == 0 {
            cell.height / 2
        } else {
            cell.height - protected_inset
        };
        let ink_height = raster.ink_bounds[3] - raster.ink_bounds[1];
        ensure!(
            ink_height <= band_bottom - band_top,
            "vertical exam stamp glyph is taller than its protected row"
        );
        let absolute_top = band_top + (band_bottom - band_top - ink_height) / 2;
        let absolute_bottom = absolute_top + ink_height;
        let destination_shift = absolute_top as i32 - raster.ink_bounds[1] as i32;
        ensure!(
            absolute_top >= protected_inset && absolute_bottom <= cell.height - protected_inset,
            "vertical exam stamp glyph {character:?} touches its top or bottom border: {absolute_top}..{absolute_bottom}, protected {protected_inset}..{}",
            cell.height - protected_inset,
        );
        for y in 0..row_height {
            for x in 0..cell.width {
                let source = raster.pixels[y * cell.width + x];
                if source != clear_index {
                    let destination_y = (y as i32 + destination_shift) as usize;
                    pixels[destination_y * cell.width + x] = source;
                }
            }
        }
    }
    Ok(pixels)
}

fn draw_rectangular_border(
    pixels: &mut [u8],
    width: usize,
    height: usize,
    inset: usize,
    thickness: usize,
    color: u8,
) {
    for layer in 0..thickness {
        let left = inset + layer;
        let right = width - inset - layer - 1;
        let top = inset + layer;
        let bottom = height - inset - layer - 1;
        for x in left..=right {
            pixels[top * width + x] = color;
            pixels[bottom * width + x] = color;
        }
        for y in top..=bottom {
            pixels[y * width + left] = color;
            pixels[y * width + right] = color;
        }
    }
}

fn uncovered_nontransparent_pixel_count(
    pixels: &[u8],
    width: usize,
    height: usize,
    transparent_index: u8,
    cells: &[Cell],
) -> Result<usize> {
    ensure!(
        pixels.len() == width * height,
        "fixed-presentation indexed image geometry changed"
    );
    for cell in cells {
        ensure!(
            cell.width > 0
                && cell.height > 0
                && cell.x + cell.width <= width
                && cell.y + cell.height <= height,
            "fixed-presentation region leaves its TIM"
        );
    }
    Ok(pixels
        .iter()
        .enumerate()
        .filter(|(offset, pixel)| {
            if **pixel == transparent_index {
                return false;
            }
            let x = *offset % width;
            let y = *offset / width;
            !cells.iter().any(|cell| {
                cell.x <= x && x < cell.x + cell.width && cell.y <= y && y < cell.y + cell.height
            })
        })
        .count())
}

fn preserved_remainder_signature(
    pixels: &[u8],
    width: usize,
    height: usize,
    excluded_cells: &[Cell],
) -> Result<Vec<u8>> {
    ensure!(
        pixels.len() == width * height,
        "fixed-presentation indexed image geometry changed"
    );
    for cell in excluded_cells {
        ensure!(
            cell.width > 0
                && cell.height > 0
                && cell.x + cell.width <= width
                && cell.y + cell.height <= height,
            "fixed-presentation region leaves its TIM"
        );
    }
    ensure!(
        excluded_cells
            .iter()
            .map(|cell| cell.width * cell.height)
            .sum::<usize>()
            < width * height,
        "fixed-presentation preserved remainder is empty"
    );
    Ok(pixels
        .iter()
        .enumerate()
        .map(|(offset, pixel)| {
            let x = offset % width;
            let y = offset / width;
            if excluded_cells.iter().any(|cell| {
                cell.x <= x && x < cell.x + cell.width && cell.y <= y && y < cell.y + cell.height
            }) {
                PRESERVED_REMAINDER_EXCLUDED_INDEX
            } else {
                *pixel
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_regions_must_cover_every_nontransparent_pixel() {
        let pixels = [0, 1, 0, 0, 0, 2, 0, 3];
        let cells = [Cell {
            x: 1,
            y: 0,
            width: 2,
            height: 2,
        }];

        assert_eq!(
            uncovered_nontransparent_pixel_count(&pixels, 4, 2, 0, &cells).unwrap(),
            1
        );
    }

    #[test]
    fn preserved_remainder_signature_masks_only_declared_regions() {
        let pixels = [1, 2, 3, 4, 5, 6, 7, 8];
        let signature = preserved_remainder_signature(
            &pixels,
            4,
            2,
            &[Cell {
                x: 1,
                y: 0,
                width: 2,
                height: 2,
            }],
        )
        .unwrap();

        assert_eq!(
            signature,
            [
                1,
                PRESERVED_REMAINDER_EXCLUDED_INDEX,
                PRESERVED_REMAINDER_EXCLUDED_INDEX,
                4,
                5,
                PRESERVED_REMAINDER_EXCLUDED_INDEX,
                PRESERVED_REMAINDER_EXCLUDED_INDEX,
                8,
            ]
        );
    }

    #[test]
    fn stamp_border_stays_inside_its_declared_cell() {
        let mut pixels = vec![0; 12 * 16];
        draw_rectangular_border(&mut pixels, 12, 16, 2, 2, 7);

        assert!(pixels[..12 * 2].iter().all(|pixel| *pixel == 0));
        assert!(
            pixels[2 * 12 + 2..=2 * 12 + 9]
                .iter()
                .all(|pixel| *pixel == 7)
        );
        assert_eq!(pixels.iter().filter(|pixel| **pixel == 7).count(), 64);
    }
}
