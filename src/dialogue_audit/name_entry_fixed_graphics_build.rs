use std::collections::BTreeSet;
use std::path::Path;

use anyhow::{Context, Result, ensure};

use crate::font::IndexedTextRasterizer;
use crate::pipeline::sha256_bytes;
pub(super) use crate::tim::cells_overlap;
use crate::tim::{
    Cell, install_indexed_glyph_in_prefix, parse_4bpp_prefix, read_indexed_cell_in_prefix,
};

use super::name_entry_fixed_graphics_model::{
    DialogueNameEntryFixedGraphicBuildReport, DialogueNameEntryFixedGraphicInstall,
    NameEntryFixedGraphicSurfaceSpec, NameEntryFixedGraphicTranslation,
    NameEntryFixedGraphicTranslations, NameEntryFixedGraphicsSource,
};

pub(super) fn validate_fixed_graphic_surfaces_disjoint(
    reports: &[DialogueNameEntryFixedGraphicBuildReport],
) -> Result<()> {
    for (surface_index, surface) in reports.iter().enumerate() {
        for entry in &surface.entries {
            for previous_surface in &reports[..surface_index] {
                ensure!(
                    previous_surface.tim_offset != surface.tim_offset
                        || previous_surface
                            .entries
                            .iter()
                            .all(|previous| !cells_overlap(entry.cell, previous.cell)),
                    "fixed graphic surfaces {} and {} overlap",
                    previous_surface.surface_id,
                    surface.surface_id
                );
            }
        }
    }
    Ok(())
}

pub(super) fn validate_name_input_page_labels(
    reports: &[DialogueNameEntryFixedGraphicBuildReport],
    page_labels: &[(&str, &str)],
) -> Result<()> {
    let mut candidate_control_surfaces = reports
        .iter()
        .filter(|report| report.surface_id == "candidate-controls");
    let candidate_controls = candidate_control_surfaces
        .next()
        .context("candidate-control fixed graphics are missing")?;
    ensure!(
        candidate_control_surfaces.next().is_none(),
        "candidate-control fixed graphics are duplicated"
    );

    for (role, label) in page_labels {
        let entry_id = format!("{role}_page");
        let entry = candidate_controls
            .entries
            .iter()
            .find(|entry| entry.id == entry_id)
            .with_context(|| format!("name-input page label {entry_id} is missing"))?;
        ensure!(
            entry.korean_text == *label,
            "name-input page label {entry_id} differs from the keyboard spec"
        );
    }
    Ok(())
}

pub(super) fn install_name_entry_fixed_graphics(
    source_decoded: &[u8],
    patched_decoded: &mut [u8],
    graphics_root: &Path,
    font_path: &Path,
    source: NameEntryFixedGraphicsSource<'_>,
    spec: NameEntryFixedGraphicSurfaceSpec,
    protected_cells: &[Cell],
) -> Result<DialogueNameEntryFixedGraphicBuildReport> {
    let path = graphics_root.join(spec.translation_file);
    let bytes = std::fs::read(&path).with_context(|| {
        format!(
            "failed to read {} fixed graphics {}",
            spec.id,
            path.display()
        )
    })?;
    let translations: NameEntryFixedGraphicTranslations = serde_json::from_slice(&bytes)
        .with_context(|| {
            format!(
                "failed to parse {} fixed graphics {}",
                spec.id,
                path.display()
            )
        })?;
    ensure!(
        translations.kind == spec.translation_kind,
        "unknown {} graphics kind",
        spec.id
    );
    ensure!(
        translations.source_path == source.path,
        "{} graphics source path changed",
        spec.id
    );
    ensure!(
        translations.source_decoded_sha256 == source.decoded_sha256,
        "{} graphics source identity changed",
        spec.id
    );
    ensure!(
        translations.tim_offset == spec.tim_offset,
        "{} graphics TIM offset changed",
        spec.id
    );
    ensure!(
        !translations.entries.is_empty(),
        "{} graphics contain no entries",
        spec.id
    );
    let tim = parse_4bpp_prefix(
        source_decoded
            .get(spec.tim_offset..)
            .with_context(|| format!("{} TIM offset is truncated", spec.id))?,
    )?;
    ensure!(
        tim.image_x == spec.image_x
            && tim.image_y == spec.image_y
            && tim.pixel_width() == spec.image_width
            && tim.image_height == spec.image_height
            && tim.clut_width == spec.clut_width
            && tim.clut_height == spec.clut_height,
        "{} TIM geometry changed",
        spec.id
    );

    validate_entries(
        &translations.entries,
        tim.pixel_width(),
        tim.image_height,
        protected_cells,
        spec.id,
    )?;
    let rasterizer = IndexedTextRasterizer::load(font_path)?;
    let mut installs = Vec::with_capacity(translations.entries.len());
    for entry in translations.entries {
        let source_pixels =
            read_indexed_cell_in_prefix(source_decoded, spec.tim_offset, entry.cell)?;
        let source_region_sha256 = sha256_bytes(&source_pixels);
        ensure!(
            source_region_sha256 == entry.source_region_sha256,
            "{} source region {} changed: expected {}, got {}",
            spec.id,
            entry.id,
            entry.source_region_sha256,
            source_region_sha256
        );
        let rasterized = rasterizer.rasterize(
            &entry.korean_text,
            entry.cell.width,
            entry.cell.height,
            entry.font_px,
            entry.tracking_px,
            entry.clear_index,
            entry.outline_index,
            entry.fill_index,
            entry.alignment,
        )?;
        let metadata = install_indexed_glyph_in_prefix(
            patched_decoded,
            spec.tim_offset,
            entry.cell,
            &rasterized.pixels,
            spec.target,
        )?;
        installs.push(DialogueNameEntryFixedGraphicInstall {
            id: entry.id,
            source_text: entry.source_text,
            korean_text: entry.korean_text,
            source_region_sha256: entry.source_region_sha256,
            cell: entry.cell,
            font_sha256: rasterized.font_sha256,
            font_px: entry.font_px,
            tracking_px: entry.tracking_px,
            alignment: entry.alignment,
            measured_advance_px: rasterized.measured_advance_px,
            ink_bounds: rasterized.ink_bounds,
            indexed_pixels_sha256: sha256_bytes(&rasterized.pixels),
            changed_decoded_byte_count: metadata.changed_decoded_byte_count,
        });
    }

    Ok(DialogueNameEntryFixedGraphicBuildReport {
        kind: spec.build_kind.to_string(),
        surface_id: spec.id.to_string(),
        translation_file: spec.translation_file.to_string(),
        translation_sha256: sha256_bytes(&bytes),
        source_decoded_sha256: source.decoded_sha256.to_string(),
        tim_offset: format!("0x{:05x}", spec.tim_offset),
        tim_image_width: tim.pixel_width(),
        tim_image_height: tim.image_height,
        entry_count: installs.len(),
        all_source_regions_match: true,
        all_cells_unique_and_non_overlapping: true,
        all_cells_disjoint_from_protected_cells: true,
        entries: installs,
    })
}

fn validate_entries(
    entries: &[NameEntryFixedGraphicTranslation],
    image_width: usize,
    image_height: usize,
    protected_cells: &[Cell],
    surface_id: &str,
) -> Result<()> {
    let mut ids = BTreeSet::new();
    for (index, entry) in entries.iter().enumerate() {
        ensure!(
            !entry.id.trim().is_empty(),
            "{surface_id} entry id is empty"
        );
        ensure!(
            ids.insert(entry.id.as_str()),
            "duplicate {surface_id} entry id"
        );
        ensure!(
            !entry.source_text.trim().is_empty() && !entry.korean_text.trim().is_empty(),
            "{surface_id} entry text is empty"
        );
        ensure!(
            entry.cell.width > 0
                && entry.cell.height > 0
                && entry.cell.x + entry.cell.width <= image_width
                && entry.cell.y + entry.cell.height <= image_height,
            "{surface_id} entry cell is outside the TIM image"
        );
        for previous in &entries[..index] {
            ensure!(
                !cells_overlap(entry.cell, previous.cell),
                "{surface_id} entry cells overlap"
            );
        }
        ensure!(
            protected_cells
                .iter()
                .all(|protected| !cells_overlap(entry.cell, *protected)),
            "{surface_id} entry overlaps a protected glyph cell"
        );
    }
    Ok(())
}
