use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Context, Result, bail, ensure};

use crate::compression::decompress;
use crate::decoded_record_write_plan::{DecodedDataClaim, DecodedRecordWritePlan};
use crate::font::{
    HorizontalTextAlignment, IndexedTextRasterizer, IndexedTextRasterizers, RasterizedIndexedText,
};
use crate::paged_compression::{compress_page_safe_image, source_paged_compression_profile};
use crate::pipeline::{sha256_bytes, write_pretty_json_and_hash};
use crate::source_disc::SupportedSourceDisc;
use crate::tim::{cells_overlap, install_indexed_glyph_in_prefix, read_indexed_cell_in_prefix};

use super::assets::load_diary_header_assets;
use super::club_consumer::validate_club_labels;
use super::menu_consumer::{validate_diary_menu_consumer, validate_diary_menu_entries};
use super::model::{
    DiaryHeaderBuild, DiaryHeaderBuildConfig, DiaryHeaderBuildReport, DiaryHeaderEntryBuild,
    DiaryHeaderFontBuild, DiaryHeaderFontRole, DiaryHeaderFontSources, DiaryHeaderFontStyle,
    DiaryHeaderIndexedRendering, DiaryHeaderProtectedRegion, DiaryHeaderProtectedRegionBuild,
    DiaryHeaderTextLayout,
};
use super::source::{
    DIARY_HEADER_PATH, SOURCE_DECODED_SHA256, SOURCE_STORED_SHA256, parse_diary_header_tim,
    validate_diary_header_loader,
};

pub const DIARY_HEADER_OUTPUT_FILE: &str = "MGCOCK.TIZ";
pub const DIARY_HEADER_BUILD_MANIFEST_FILE: &str = "diary-header-build.json";

pub fn build_diary_header(config: &DiaryHeaderBuildConfig) -> Result<DiaryHeaderBuild> {
    let source = SupportedSourceDisc::open(&config.cue)?;
    build_diary_header_from_source(config, &source)
}

pub(crate) fn build_diary_header_from_source(
    config: &DiaryHeaderBuildConfig,
    source: &SupportedSourceDisc,
) -> Result<DiaryHeaderBuild> {
    prepare_output_directory(&config.output_dir, config.force)?;
    let (manifest, entries) = load_diary_header_assets(&config.assets)?;
    let (_, mgame) = source.read_record("DAT1/MGAME.BIN")?;
    validate_diary_header_loader(&mgame)?;
    validate_club_labels(&mgame, &entries)?;
    super::date_consumer::validate(&mgame, &entries)?;
    super::status_consumer::validate_status_labels(&mgame, &entries)?;
    super::backup_consumer::validate_backup_schools(&mgame, &entries)?;
    validate_diary_menu_consumer(&mgame)?;
    validate_diary_menu_entries(&entries)?;
    super::action_spacing::validate_entries(&entries)?;
    ensure!(
        manifest.source.path == DIARY_HEADER_PATH,
        "diary header source path changed"
    );
    ensure!(
        manifest.source.stored_sha256 == SOURCE_STORED_SHA256
            && manifest.source.decoded_sha256 == SOURCE_DECODED_SHA256,
        "diary header source binding changed"
    );

    let (_, source_stored) = source.read_record(DIARY_HEADER_PATH)?;
    ensure!(
        sha256_bytes(&source_stored) == SOURCE_STORED_SHA256,
        "MGCOCK stored source hash changed"
    );
    let source_decoded = decompress(&source_stored, false)?;
    ensure!(
        sha256_bytes(&source_decoded) == SOURCE_DECODED_SHA256,
        "MGCOCK decoded source hash changed"
    );
    let tim = parse_diary_header_tim(&source_decoded)?;
    super::menu_palette::validate(&config.fonts.action_label, &source_decoded)?;

    validate_regions(
        &entries,
        &manifest.protected_regions,
        tim.pixel_width(),
        tim.image_height,
    )?;
    let mut protected_pixels = Vec::with_capacity(manifest.protected_regions.len());
    let mut protected_region_builds = Vec::with_capacity(manifest.protected_regions.len());
    for region in &manifest.protected_regions {
        let pixels = read_indexed_cell_in_prefix(&source_decoded, 0, region.cell)?;
        let actual_sha256 = sha256_bytes(&pixels);
        ensure!(
            actual_sha256 == region.source_region_sha256,
            "diary header protected source region {} changed: expected {}, got {}",
            region.id,
            region.source_region_sha256,
            actual_sha256
        );
        protected_pixels.push(pixels);
        protected_region_builds.push(DiaryHeaderProtectedRegionBuild {
            id: region.id.clone(),
            cell: region.cell,
            source_region_sha256: region.source_region_sha256.clone(),
        });
    }
    let mut patched_decoded = source_decoded.clone();
    let mut builds = Vec::with_capacity(entries.len());
    let mut decoded_write_claims = Vec::new();
    let mut font_builds = BTreeMap::new();
    let mut rasterizers = IndexedTextRasterizers::default();
    for entry in entries {
        let source_pixels = read_indexed_cell_in_prefix(&source_decoded, 0, entry.cell)?;
        let actual_source_region_sha256 = sha256_bytes(&source_pixels);
        ensure!(
            actual_source_region_sha256 == entry.source_region_sha256,
            "diary header source region {} changed: expected {}, got {}",
            entry.id,
            entry.source_region_sha256,
            actual_source_region_sha256
        );
        let style = font_for_role(&config.fonts, entry.font_role);
        let font_px = entry.font_px.unwrap_or(style.font_px);
        let vertical_shift_px = resolved_vertical_shift(&entry, style);
        let layout = resolved_text_layout(&entry);
        let raster = if let Some(art) = super::indexed_art::load(&config.assets, &entry)? {
            art
        } else {
            rasterize_entry(
                rasterizers.for_font(&style.path)?,
                &entry,
                style,
                &layout,
                font_px,
                vertical_shift_px,
            )?
        };
        super::status_consumer::validate_vertical_outline(&entry, style, &raster)?;
        let install = install_indexed_glyph_in_prefix(
            &mut patched_decoded,
            0,
            entry.cell,
            &raster.pixels,
            &entry.id,
        )?;
        let entry_claims = DecodedDataClaim::from_effective_ranges(
            &format!("diary-header:{}", entry.id),
            &format!("render diary header entry {}", entry.id),
            &source_decoded,
            &patched_decoded,
            install.allowed_decoded_byte_ranges,
        )?;
        decoded_write_claims.extend(entry_claims);
        if entry.indexed_art.is_none() {
            font_builds
                .entry(entry.font_role)
                .or_insert(DiaryHeaderFontBuild {
                    role: entry.font_role,
                    font_name: raster.font_name,
                    font_sha256: raster.font_sha256,
                    font_px: style.font_px,
                    rendering: style.rendering,
                    glyph_layout: super::model::DiaryHeaderGlyphLayout {
                        vertical_shift_px,
                        ..style.glyph_layout
                    },
                });
        }
        builds.push(DiaryHeaderEntryBuild {
            id: entry.id,
            source_text: entry.source_text,
            korean_text: entry.korean_text,
            font_role: entry.font_role,
            cell: entry.cell,
            layout,
            font_px,
            vertical_shift_px,
            source_region_sha256: entry.source_region_sha256,
            measured_advance_px: raster.measured_advance_px,
            ink_bounds: raster.ink_bounds,
            changed_decoded_byte_count: install.changed_decoded_byte_count,
            indexed_art_sha256: entry.indexed_art.map(|art| art.sha256),
        });
    }
    let source_decoded_sha256 = sha256_bytes(&source_decoded);
    let mut write_plan =
        DecodedRecordWritePlan::new(DIARY_HEADER_PATH, &source_decoded, &source_decoded_sha256)?;
    write_plan.register_data_candidate(
        "diary header entries",
        &source_decoded_sha256,
        &patched_decoded,
        &decoded_write_claims,
    )?;
    let planned_decoded = write_plan.apply(None)?;
    ensure!(
        planned_decoded == patched_decoded,
        "diary header decoded plan omitted a rendered entry"
    );
    let patched_decoded = planned_decoded;
    for (region, source_pixels) in manifest
        .protected_regions
        .iter()
        .zip(protected_pixels.iter())
    {
        let patched_pixels = read_indexed_cell_in_prefix(&patched_decoded, 0, region.cell)?;
        ensure!(
            &patched_pixels == source_pixels,
            "diary header build changed protected region {}",
            region.id
        );
    }

    // MGAME loads this atlas through the native paged file loader before
    // submitting the TIM. A standalone codec roundtrip does not protect its
    // decoded tail when the stream ends before the source's final input page.
    let source_compression = source_paged_compression_profile(&source_stored)?;
    let reencoded = compress_page_safe_image(&patched_decoded, source_compression)?;
    let rebuilt_compression = source_paged_compression_profile(&reencoded)?;
    ensure!(
        decompress(&reencoded, false)? == patched_decoded,
        "MGCOCK compression roundtrip changed decoded bytes"
    );
    ensure!(
        reencoded.len() <= source_stored.len(),
        "rebuilt MGCOCK.TIZ exceeds its source record"
    );
    let unpadded_stored_size = reencoded.len();
    let mut stored = reencoded;
    stored.resize(source_stored.len(), 0);
    ensure!(
        decompress(&stored, true)? == patched_decoded,
        "padded MGCOCK.TIZ changed decoded bytes"
    );

    let report = DiaryHeaderBuildReport {
        kind: "Justice Gakuen 2 Korean diary header build".to_string(),
        source_path: DIARY_HEADER_PATH.to_string(),
        source_stored_sha256: SOURCE_STORED_SHA256.to_string(),
        source_decoded_sha256: SOURCE_DECODED_SHA256.to_string(),
        patched_stored_sha256: sha256_bytes(&stored),
        patched_decoded_sha256: sha256_bytes(&patched_decoded),
        build_spec_sha256: config.build_spec_sha256.clone(),
        source_tim_vram: [tim.image_x, tim.image_y],
        source_tim_pixel_size: [tim.pixel_width(), tim.image_height],
        source_clut_vram: [tim.clut_x, tim.clut_y],
        source_palette_count: tim.clut_width * tim.clut_height / 16,
        entry_count: builds.len(),
        protected_region_count: protected_region_builds.len(),
        source_regions_match: true,
        protected_regions_unchanged: true,
        cells_are_unique_and_non_overlapping: true,
        changed_bytes_confined_to_owned_cells: true,
        unpadded_stored_size,
        source_compression_stream_byte_count: source_compression.stream_byte_count,
        source_compression_maximum_match_words: source_compression.maximum_match_words,
        rebuilt_compression_maximum_match_words: rebuilt_compression.maximum_match_words,
        source_compression_maximum_control_block_output_words: source_compression
            .maximum_control_block_output_words,
        rebuilt_compression_maximum_control_block_output_words: rebuilt_compression
            .maximum_control_block_output_words,
        source_compression_control_blocks_crossing_input_pages: source_compression
            .control_blocks_crossing_input_pages,
        rebuilt_compression_control_blocks_crossing_input_pages: rebuilt_compression
            .control_blocks_crossing_input_pages,
        source_record_size: source_stored.len(),
        development_input_available: true,
        release_candidate_input_eligible: false,
        fonts: font_builds.into_values().collect(),
        protected_regions: protected_region_builds,
        entries: builds,
    };
    std::fs::write(config.output_dir.join(DIARY_HEADER_OUTPUT_FILE), &stored)?;
    let build_manifest_sha256 = write_pretty_json_and_hash(
        &config.output_dir.join(DIARY_HEADER_BUILD_MANIFEST_FILE),
        &report,
        false,
    )?;
    Ok(DiaryHeaderBuild {
        stored,
        decoded: patched_decoded,
        build_manifest_sha256,
        report,
    })
}

fn resolved_text_layout(entry: &super::model::DiaryHeaderEntry) -> DiaryHeaderTextLayout {
    match &entry.layout {
        DiaryHeaderTextLayout::GlyphSlots { slots } if slots.is_empty() => {
            DiaryHeaderTextLayout::GlyphSlots {
                slots: (0..entry.korean_text.chars().count()).collect(),
            }
        }
        layout => layout.clone(),
    }
}

// Profile sprites are 20 pixels tall; HUD labels use a different native window.
// They share typography, while the profile cell owns its centered placement.
pub(super) fn resolved_vertical_shift(
    entry: &super::model::DiaryHeaderEntry,
    style: &DiaryHeaderFontStyle,
) -> i32 {
    entry.vertical_shift_px.unwrap_or_else(|| {
        if entry.font_role == DiaryHeaderFontRole::ProfileText {
            0
        } else {
            style.glyph_layout.vertical_shift_px
        }
    })
}

pub(super) fn rasterize_entry(
    rasterizer: &IndexedTextRasterizer,
    entry: &super::model::DiaryHeaderEntry,
    style: &DiaryHeaderFontStyle,
    layout: &DiaryHeaderTextLayout,
    font_px: f32,
    vertical_shift_px: i32,
) -> Result<RasterizedIndexedText> {
    match layout {
        DiaryHeaderTextLayout::GlyphSlots { slots } => {
            rasterize_entry_in_slots(rasterizer, entry, style, slots, font_px, vertical_shift_px)
        }
        DiaryHeaderTextLayout::Continuous => rasterize_text(
            rasterizer,
            style,
            &entry.korean_text,
            entry.cell.width,
            entry.cell.height,
            font_px,
            vertical_shift_px,
        ),
    }
}

fn rasterize_entry_in_slots(
    rasterizer: &IndexedTextRasterizer,
    entry: &super::model::DiaryHeaderEntry,
    style: &DiaryHeaderFontStyle,
    glyph_slots: &[usize],
    font_px: f32,
    vertical_shift_px: i32,
) -> Result<RasterizedIndexedText> {
    let characters = entry.korean_text.chars().collect::<Vec<_>>();
    ensure!(
        characters.len() == glyph_slots.len(),
        "diary header entry {} has {} characters but {} glyph slots",
        entry.id,
        characters.len(),
        glyph_slots.len()
    );
    ensure!(
        entry
            .cell
            .width
            .is_multiple_of(style.glyph_layout.slot_width_px),
        "diary header entry {} width is not a whole number of glyph slots",
        entry.id
    );
    let slot_count = entry.cell.width / style.glyph_layout.slot_width_px;
    let mut unique_slots = BTreeSet::new();
    for slot in glyph_slots {
        ensure!(
            *slot < slot_count,
            "diary header entry {} glyph slot {} is outside its cell",
            entry.id,
            slot
        );
        ensure!(
            unique_slots.insert(*slot),
            "diary header entry {} repeats glyph slot {}",
            entry.id,
            slot
        );
    }

    let mut pixels = vec![0u8; entry.cell.width * entry.cell.height];
    let mut font_identity: Option<(String, String)> = None;
    let mut measured_advance_px = 0.0;
    let mut ink_bounds = [entry.cell.width, entry.cell.height, 0, 0];
    let mut has_ink = false;

    for (character, slot) in characters.into_iter().zip(glyph_slots.iter().copied()) {
        let glyph = rasterize_text(
            rasterizer,
            style,
            &character.to_string(),
            style.glyph_layout.slot_width_px,
            entry.cell.height,
            font_px,
            vertical_shift_px,
        )?;
        match &font_identity {
            Some((font_name, font_sha256)) => ensure!(
                font_name == &glyph.font_name && font_sha256 == &glyph.font_sha256,
                "diary header entry {} changed font while rasterizing glyphs",
                entry.id
            ),
            None => {
                font_identity = Some((glyph.font_name.clone(), glyph.font_sha256.clone()));
            }
        }
        measured_advance_px += glyph.measured_advance_px;
        let target_x = slot * style.glyph_layout.slot_width_px;
        for source_y in 0..entry.cell.height {
            for source_x in 0..style.glyph_layout.slot_width_px {
                let pixel = glyph.pixels[source_y * style.glyph_layout.slot_width_px + source_x];
                if pixel == 0 {
                    continue;
                }
                let target_y = source_y;
                let target_x = target_x + source_x;
                pixels[target_y * entry.cell.width + target_x] = pixel;
                has_ink = true;
                ink_bounds[0] = ink_bounds[0].min(target_x);
                ink_bounds[1] = ink_bounds[1].min(target_y);
                ink_bounds[2] = ink_bounds[2].max(target_x + 1);
                ink_bounds[3] = ink_bounds[3].max(target_y + 1);
            }
        }
    }
    ensure!(
        has_ink,
        "diary header entry {} has no rendered ink",
        entry.id
    );
    let (font_name, font_sha256) = font_identity.context("diary header entry has no font")?;
    Ok(RasterizedIndexedText {
        font_name,
        font_sha256,
        pixels,
        measured_advance_px,
        ink_bounds,
    })
}

fn rasterize_text(
    rasterizer: &IndexedTextRasterizer,
    style: &DiaryHeaderFontStyle,
    text: &str,
    width: usize,
    height: usize,
    font_px: f32,
    vertical_shift_px: i32,
) -> Result<RasterizedIndexedText> {
    match style.rendering {
        DiaryHeaderIndexedRendering::CoverageRamp {
            first_ink_index,
            last_ink_index,
        } => rasterizer.rasterize_shifted_with_coverage_ramp(
            text,
            width,
            height,
            font_px,
            0.0,
            vertical_shift_px,
            0,
            first_ink_index,
            last_ink_index,
            HorizontalTextAlignment::Center,
        ),
        DiaryHeaderIndexedRendering::Outlined {
            outline_index,
            fill_index,
        } => rasterizer.rasterize_shifted(
            text,
            width,
            height,
            font_px,
            0.0,
            vertical_shift_px,
            0,
            Some(outline_index),
            fill_index,
            HorizontalTextAlignment::Center,
        ),
    }
}

fn font_for_role(
    fonts: &DiaryHeaderFontSources,
    role: DiaryHeaderFontRole,
) -> &DiaryHeaderFontStyle {
    match role {
        DiaryHeaderFontRole::CalendarText => &fonts.calendar_text,
        DiaryHeaderFontRole::StatusLabel | DiaryHeaderFontRole::ProfileText => &fonts.status_label,
        DiaryHeaderFontRole::ClubLabel => &fonts.club_label,
        DiaryHeaderFontRole::ActionLabel => &fonts.action_label,
    }
}

pub(super) fn validate_regions(
    entries: &[super::model::DiaryHeaderEntry],
    protected_regions: &[DiaryHeaderProtectedRegion],
    width: usize,
    height: usize,
) -> Result<()> {
    ensure!(!entries.is_empty(), "diary header has no entries");
    ensure!(
        !protected_regions.is_empty(),
        "diary header has no protected regions"
    );
    let mut ids = BTreeSet::new();
    for (index, entry) in entries.iter().enumerate() {
        ensure!(
            !entry.id.trim().is_empty(),
            "diary header entry id is empty"
        );
        ensure!(
            ids.insert(entry.id.clone()),
            "duplicate diary header entry id {}",
            entry.id
        );
        ensure!(
            !entry.source_text.is_empty() && !entry.korean_text.is_empty(),
            "diary header entry {} has empty text",
            entry.id
        );
        if let Some(font_px) = entry.font_px {
            ensure!(
                font_px.is_finite() && font_px > 0.0,
                "diary header entry {} has an invalid font size",
                entry.id
            );
        }
        ensure!(
            entry.cell.width > 0 && entry.cell.height > 0,
            "diary header entry {} has an empty cell",
            entry.id
        );
        ensure!(
            entry.cell.x + entry.cell.width <= width && entry.cell.y + entry.cell.height <= height,
            "diary header entry {} is outside the TIM",
            entry.id
        );
        for other in &entries[..index] {
            ensure!(
                !cells_overlap(entry.cell, other.cell),
                "diary header entries {} and {} overlap",
                entry.id,
                other.id
            );
        }
    }
    let mut protected_ids = BTreeSet::new();
    for (index, region) in protected_regions.iter().enumerate() {
        ensure!(
            !region.id.trim().is_empty(),
            "diary header protected region id is empty"
        );
        ensure!(
            protected_ids.insert(region.id.clone()),
            "duplicate diary header protected region id {}",
            region.id
        );
        ensure!(
            region.cell.width > 0 && region.cell.height > 0,
            "diary header protected region {} has an empty cell",
            region.id
        );
        ensure!(
            region.cell.x + region.cell.width <= width
                && region.cell.y + region.cell.height <= height,
            "diary header protected region {} is outside the TIM",
            region.id
        );
        for other in &protected_regions[..index] {
            ensure!(
                !cells_overlap(region.cell, other.cell),
                "diary header protected regions {} and {} overlap",
                region.id,
                other.id
            );
        }
        for entry in entries {
            ensure!(
                !cells_overlap(region.cell, entry.cell),
                "diary header entry {} overlaps protected region {}",
                entry.id,
                region.id
            );
        }
    }
    Ok(())
}

fn prepare_output_directory(path: &Path, force: bool) -> Result<()> {
    if path.exists() {
        if !force {
            bail!("output directory already exists: {}", path.display());
        }
        std::fs::remove_dir_all(path)
            .with_context(|| format!("failed to replace {}", path.display()))?;
    }
    std::fs::create_dir_all(path).with_context(|| format!("failed to create {}", path.display()))
}
