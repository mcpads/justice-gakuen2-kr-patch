//! Builds one physical character-select texture surface from immutable decoded bytes.

use anyhow::{Result, ensure};

use crate::pipeline::sha256_bytes;
use crate::tim::{read_indexed_cell_in_prefix, write_indexed_cell_in_prefix_with_report};

use super::model::{
    CharacterSelectFixedStripBuild, CharacterSelectFixedStripWriteMode, CharacterSelectGlyphBuild,
    CharacterSelectSourceInkCleanupAllocation, CharacterSelectSourceInkCleanupBuild,
    CharacterSelectTextureSurface,
};
use super::render::{RenderedFixedStrip, RenderedGlyph};

pub(super) struct TextureSurfaceBuild {
    pub(super) candidate: Vec<u8>,
    pub(super) allowed_ranges: Vec<[usize; 2]>,
    pub(super) glyphs: Vec<CharacterSelectGlyphBuild>,
    pub(super) fixed_strips: Vec<CharacterSelectFixedStripBuild>,
    pub(super) source_ink_cleanups: Vec<CharacterSelectSourceInkCleanupBuild>,
}

pub(super) fn targeted_surfaces(
    source_path: &str,
    rendered: &[RenderedGlyph],
    rendered_fixed_strips: &[RenderedFixedStrip],
    source_ink_cleanups: &[CharacterSelectSourceInkCleanupAllocation],
) -> Vec<CharacterSelectTextureSurface> {
    rendered
        .iter()
        .filter(|glyph| {
            glyph
                .allocation
                .surface
                .targets_record(source_path, glyph.allocation.tim_offset)
        })
        .map(|glyph| glyph.allocation.surface)
        .chain(
            rendered_fixed_strips
                .iter()
                .filter(|strip| {
                    strip
                        .allocation
                        .surface
                        .targets_record(source_path, strip.allocation.tim_offset)
                })
                .map(|strip| strip.allocation.surface),
        )
        .chain(
            source_ink_cleanups
                .iter()
                .filter(|cleanup| {
                    cleanup
                        .surface
                        .targets_record(source_path, cleanup.tim_offset)
                })
                .map(|cleanup| cleanup.surface),
        )
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub(super) fn build_texture_surface(
    source_path: &str,
    source: &[u8],
    surface: CharacterSelectTextureSurface,
    rendered: &[RenderedGlyph],
    rendered_fixed_strips: &[RenderedFixedStrip],
    source_ink_cleanups: &[CharacterSelectSourceInkCleanupAllocation],
) -> Result<TextureSurfaceBuild> {
    let mut candidate = source.to_vec();
    let mut allowed_ranges = Vec::new();
    let mut glyphs = Vec::new();
    let mut fixed_strips = Vec::new();
    let mut applied_source_ink_cleanups = Vec::new();

    for cleanup in source_ink_cleanups {
        if cleanup.surface != surface
            || !cleanup
                .surface
                .targets_record(source_path, cleanup.tim_offset)
        {
            continue;
        }
        ensure!(
            cleanup.first_ink_index <= cleanup.last_ink_index
                && cleanup.last_ink_index < 16
                && cleanup.replacement_index < 16
                && !(cleanup.first_ink_index..=cleanup.last_ink_index)
                    .contains(&cleanup.replacement_index),
            "{} source-ink cleanup {} has an invalid palette policy",
            source_path,
            cleanup.physical_region_id
        );
        let source_pixels = read_indexed_cell_in_prefix(source, cleanup.tim_offset, cleanup.cell)?;
        let source_ink_count = source_pixels
            .iter()
            .filter(|pixel| (cleanup.first_ink_index..=cleanup.last_ink_index).contains(pixel))
            .count();
        ensure!(
            source_ink_count == cleanup.expected_source_ink_count,
            "{} source-ink cleanup {} changed: expected {} indexed pixels, found {}",
            source_path,
            cleanup.physical_region_id,
            cleanup.expected_source_ink_count,
            source_ink_count
        );
        let mut cleaned_pixels =
            read_indexed_cell_in_prefix(&candidate, cleanup.tim_offset, cleanup.cell)?;
        for pixel in &mut cleaned_pixels {
            if (cleanup.first_ink_index..=cleanup.last_ink_index).contains(pixel) {
                *pixel = cleanup.replacement_index;
            }
        }
        let write = write_indexed_cell_in_prefix_with_report(
            &mut candidate,
            cleanup.tim_offset,
            cleanup.cell,
            &cleaned_pixels,
        )?;
        ensure!(
            write.changed_byte_count > 0,
            "{} source-ink cleanup {} changed no decoded bytes",
            source_path,
            cleanup.physical_region_id
        );
        allowed_ranges.extend(write.allowed_ranges);
        applied_source_ink_cleanups.push(CharacterSelectSourceInkCleanupBuild {
            physical_region_id: cleanup.physical_region_id.clone(),
            surface: cleanup.surface,
            tim_offset: cleanup.tim_offset,
            cell: cleanup.cell,
            first_ink_index: cleanup.first_ink_index,
            last_ink_index: cleanup.last_ink_index,
            replacement_index: cleanup.replacement_index,
            source_ink_count,
            changed_decoded_byte_count: write.changed_byte_count,
        });
    }

    for glyph in rendered {
        if glyph.allocation.surface != surface
            || !glyph
                .allocation
                .surface
                .targets_record(source_path, glyph.allocation.tim_offset)
        {
            continue;
        }
        let source_pixels = read_indexed_cell_in_prefix(
            source,
            glyph.allocation.tim_offset,
            glyph.allocation.cell,
        )?;
        if glyph.allocation.source_cell_occupancy
            == super::model::CharacterSelectSourceCellOccupancy::Blank
        {
            ensure!(
                source_pixels
                    .iter()
                    .all(|pixel| *pixel == glyph.clear_index),
                "{} blank-cell allocation for {:?} is not source-blank",
                source_path,
                glyph.allocation.character
            );
        } else {
            ensure!(
                source_pixels
                    .iter()
                    .any(|pixel| *pixel != glyph.clear_index),
                "{} owned source-glyph allocation for {:?} lost its source ink",
                source_path,
                glyph.allocation.character
            );
        }
        let write = write_indexed_cell_in_prefix_with_report(
            &mut candidate,
            glyph.allocation.tim_offset,
            glyph.allocation.cell,
            &glyph.raster.pixels,
        )?;
        ensure!(
            write.changed_byte_count > 0 || glyph.allocation.character.is_whitespace(),
            "{} glyph {:?} changed no decoded bytes",
            source_path,
            glyph.allocation.character
        );
        allowed_ranges.extend(write.allowed_ranges);
        glyphs.push(CharacterSelectGlyphBuild {
            font_role: glyph.allocation.font_role,
            character: glyph.allocation.character,
            surface: glyph.allocation.surface,
            tim_offset: glyph.allocation.tim_offset,
            texture_page_index: glyph.allocation.texture_page_index,
            texture_uv: glyph.allocation.texture_uv,
            cell: glyph.allocation.cell,
            source_cell_occupancy: glyph.allocation.source_cell_occupancy,
            clear_index: glyph.clear_index,
            outline_index: glyph.outline_index,
            fill_index: glyph.fill_index,
            font_name: glyph.raster.font_name.clone(),
            font_sha256: glyph.raster.font_sha256.clone(),
            font_px: glyph.font_px,
            vertical_shift_px: glyph.vertical_shift_px,
            indexed_sha256: sha256_bytes(&glyph.raster.pixels),
            measured_advance_px: glyph.raster.measured_advance_px,
            changed_decoded_byte_count: write.changed_byte_count,
        });
    }

    for strip in rendered_fixed_strips {
        if strip.allocation.surface != surface
            || !strip
                .allocation
                .surface
                .targets_record(source_path, strip.allocation.tim_offset)
        {
            continue;
        }
        let source_pixels = read_indexed_cell_in_prefix(
            &candidate,
            strip.allocation.tim_offset,
            strip.allocation.cell,
        )?;
        let write_pixels = match strip.allocation.write_mode {
            CharacterSelectFixedStripWriteMode::ReplaceRegion => {
                ensure!(
                    source_pixels
                        .iter()
                        .any(|pixel| *pixel != strip.allocation.clear_index),
                    "{} fixed strip {} lost its source ink",
                    source_path,
                    strip.allocation.source_ui_ids.join(", ")
                );
                strip.raster.pixels.clone()
            }
            CharacterSelectFixedStripWriteMode::OverlayNonClearPixels => {
                ensure!(
                    strip
                        .raster
                        .pixels
                        .iter()
                        .any(|pixel| *pixel != strip.allocation.clear_index),
                    "{} text overlay {} has no non-clear pixels",
                    source_path,
                    strip.allocation.source_ui_ids.join(", ")
                );
                source_pixels
                    .into_iter()
                    .zip(&strip.raster.pixels)
                    .map(|(source, overlay)| {
                        if *overlay == strip.allocation.clear_index {
                            source
                        } else {
                            *overlay
                        }
                    })
                    .collect()
            }
        };
        let write = write_indexed_cell_in_prefix_with_report(
            &mut candidate,
            strip.allocation.tim_offset,
            strip.allocation.cell,
            &write_pixels,
        )?;
        ensure!(
            write.changed_byte_count > 0,
            "{} fixed strip {} changed no decoded bytes",
            source_path,
            strip.allocation.source_ui_ids.join(", ")
        );
        allowed_ranges.extend(write.allowed_ranges);
        fixed_strips.push(CharacterSelectFixedStripBuild {
            physical_text_region_id: strip.allocation.physical_text_region_id.clone(),
            source_ui_ids: strip.allocation.source_ui_ids.clone(),
            translation_id: strip.allocation.translation_id.clone(),
            text_selection: strip.allocation.text_selection,
            text_flow: strip.allocation.text_flow,
            write_mode: strip.allocation.write_mode,
            korean_text: strip.korean_text.clone(),
            font_role: strip.allocation.font_role,
            surface: strip.allocation.surface,
            tim_offset: strip.allocation.tim_offset,
            cell: strip.allocation.cell,
            text_cell: strip.allocation.text_cell,
            clear_index: strip.allocation.clear_index,
            outline_index: strip.outline_index,
            fill_index: strip.fill_index,
            font_name: strip.raster.font_name.clone(),
            font_sha256: strip.raster.font_sha256.clone(),
            font_px: strip.font_px,
            vertical_shift_px: strip.vertical_shift_px,
            indexed_sha256: sha256_bytes(&strip.raster.pixels),
            measured_advance_px: strip.raster.measured_advance_px,
            changed_decoded_byte_count: write.changed_byte_count,
        });
    }

    ensure!(
        !glyphs.is_empty() || !fixed_strips.is_empty() || !applied_source_ink_cleanups.is_empty(),
        "{source_path} surface {surface:?} has no physical allocations"
    );
    Ok(TextureSurfaceBuild {
        candidate,
        allowed_ranges,
        glyphs,
        fixed_strips,
        source_ink_cleanups: applied_source_ink_cleanups,
    })
}
