use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Context, Result, bail, ensure};

use crate::decoded_record_write_plan::DecodedDataClaim;
use crate::development_build_spec::SizedFontSource;
use crate::embedded_tim::{decode_embedded_tim_preview, parse_embedded_tim_at};
use crate::font::{HorizontalTextAlignment, IndexedTextRasterizers};
use crate::pipeline::{difference_ranges, sha256_bytes, write_pretty_json_and_hash};
use crate::source_disc::SupportedSourceDisc;
use crate::tim::{
    Cell, parse_8bpp_prefix, read_8bpp_indexed_cell_in_prefix, read_8bpp_palette_words_in_prefix,
    read_indexed_cell_in_prefix, read_indexed_cell_without_clut_in_prefix,
    write_8bpp_indexed_cell_in_prefix_with_report, write_indexed_cell_in_prefix_with_report,
    write_indexed_cell_without_clut_in_prefix_with_report,
};
use crate::tim_preview::write_tim_preview;

use super::assets::{load_mode_descendant_assets, parse_hex_offset};
use super::catalog::{
    ModeDescendantRecordSpec, PracticalExamRecordSpecs, edit_command_sheet_record_spec,
    fixed_record_specs_for_surfaces, practical_exam_record_specs, practical_result_record_specs,
};
use super::edit_command_sheet_build::{build_edit_command_sheets, load_edit_command_sheet_plan};
use super::edit_fixed_ui_consumers::{
    validate_edit_cpu_tactics_body_consumers, validate_edit_cpu_tactics_instruction_title_consumers,
};
use super::gorin_heading::{compose_gorin_heading, rasterize_gorin_heading};
use super::model::{
    ModeDescendantEntry, ModeDescendantFontRole, ModeDescendantGraphicsBuild,
    ModeDescendantGraphicsBuildConfig, ModeDescendantGraphicsBuildReport, ModeDescendantPlacement,
    ModeDescendantRecord, ModeDescendantStorageKind, ModeDescendantSurface,
    ModeDescendantUnitBuild, ModeDescendantUnitPlacementBuild, TextAlignment,
};
use super::practical_exam::{
    PracticalExamTranslation, build_practical_exam_shared_ui, install_active_title_texture_cache,
};
use super::practical_results::{
    PracticalResultBuildReport, PracticalResultFixedTextBases, PracticalResultPlan,
    build_result_term_titles, mirror_canonical_result_cells_into_indexed_members,
};
use super::record_compositor::{
    ModeDescendantRecordDraft, finalize_mode_descendant_records, source_for_spec,
};
use super::source::{ModeDescendantSourceRecord, load_mode_descendant_build_sources_from_disc};

pub const MODE_DESCENDANT_BUILD_MANIFEST_FILE: &str = "mode-descendant-build.json";

pub fn build_mode_descendant_graphics(
    config: &ModeDescendantGraphicsBuildConfig,
) -> Result<ModeDescendantGraphicsBuild> {
    let source = SupportedSourceDisc::open(&config.cue)?;
    build_mode_descendant_graphics_from_source(config, &source)
}

pub(crate) fn build_mode_descendant_graphics_from_source(
    config: &ModeDescendantGraphicsBuildConfig,
    source: &SupportedSourceDisc,
) -> Result<ModeDescendantGraphicsBuild> {
    prepare_output_directory(&config.output_dir, config.force)?;
    let assets = load_mode_descendant_assets(&config.assets)?;
    let edit_command_sheet_plan = load_edit_command_sheet_plan(&assets.edit_command_sheets_path)?;
    let edit_cpu_tactics_instruction_title_consumer_count =
        validate_edit_cpu_tactics_instruction_title_consumers(source, &assets.entries)?;
    let edit_cpu_tactics_body_consumer_count =
        validate_edit_cpu_tactics_body_consumers(source, &assets.entries)?;
    let authored_surfaces = assets.entries.iter().map(|entry| entry.surface).collect();
    let fixed_record_specs = fixed_record_specs_for_surfaces(&authored_surfaces);
    let practical_record_specs = authored_surfaces
        .contains(&ModeDescendantSurface::PracticalExamSharedUi)
        .then(practical_exam_record_specs);
    let practical_result_plan = authored_surfaces
        .contains(&ModeDescendantSurface::PracticalExamSharedUi)
        .then(|| PracticalResultPlan::load(&config.assets))
        .transpose()?;
    let mut record_specs = fixed_record_specs.clone();
    record_specs.push(edit_command_sheet_record_spec());
    record_specs.push(super::catalog::edit_technique_record_spec());
    if let Some(practical) = &practical_record_specs {
        let result_specs = practical_result_record_specs();
        record_specs.extend([
            practical.basics_texture_producer,
            practical.exam_1999_texture_producer,
            practical.basics_descriptor_consumer,
            practical.exam_1999_descriptor_consumer,
            result_specs.term_titles,
        ]);
    }
    if assets.gorin_retry_path.is_some() {
        record_specs.extend(super::gorin_retry::SPECS.iter());
    }
    if assets.gorin_announcements_path.is_some() {
        record_specs.extend(super::gorin_announcements::SPECS.iter());
    }
    if assets.main_title_path.is_some() {
        record_specs.push(&super::main_title::SPEC);
    }
    if assets.battle_announcements_path.is_some() {
        record_specs.push(&super::battle_announcements::SPEC);
    }
    if assets.training_menu_path.is_some() {
        record_specs.extend(super::training_menu::record_specs());
    }
    if assets.continue_schools_path.is_some() {
        record_specs.push(&super::continue_schools::SPEC);
    }
    if assets.gorin_selector_names_path.is_some() {
        record_specs.extend(super::gorin_selector_names::record_specs());
    }
    let mut source_path_set = BTreeSet::new();
    let mut source_paths = Vec::new();
    for path in record_specs.iter().map(|spec| spec.source_path) {
        if source_path_set.insert(path) {
            source_paths.push(path);
        }
    }
    if let Some(plan) = &practical_result_plan {
        for path in plan.source_paths() {
            if source_path_set.insert(path) {
                source_paths.push(path);
            }
        }
    }
    if assets.continue_schools_path.is_some() && source_path_set.insert("DAT1/MGTIT.BIZ") {
        source_paths.push("DAT1/MGTIT.BIZ");
    }
    let (source_bin_sha256, sources) =
        load_mode_descendant_build_sources_from_disc(source, &source_paths)?;
    ensure!(
        sources
            .iter()
            .map(|source| source.path)
            .collect::<BTreeSet<_>>()
            == source_path_set,
        "mode-descendant loaded-source denominator changed"
    );

    // Count authored placements at each physical consumer, including shared menu copies.
    let record_owned_unit_count = assets
        .entries
        .iter()
        .map(|entry| {
            let fixed_copies = fixed_record_specs
                .iter()
                .filter(|spec| {
                    entry.surface == spec.surface
                        && entry
                            .records
                            .as_ref()
                            .is_none_or(|records| records.contains(&spec.record))
                        && (spec.surface != ModeDescendantSurface::GorinMainMenu
                            || spec.record == ModeDescendantRecord::GorinMainMenu
                            || entry.font_role != ModeDescendantFontRole::GorinHeading)
                })
                .count();
            fixed_copies.max(1)
        })
        .sum();
    let mut units = Vec::with_capacity(record_owned_unit_count);
    let mut record_drafts = Vec::with_capacity(sources.len());
    let mut rasterizers = IndexedTextRasterizers::default();
    for record_spec in fixed_record_specs {
        let source = source_for_spec(&sources, record_spec)?;
        let entries = assets
            .entries
            .iter()
            .filter(|entry| entry.surface == record_spec.surface)
            .filter(|entry| {
                entry
                    .records
                    .as_ref()
                    .is_none_or(|records| records.contains(&record_spec.record))
            })
            .filter(|entry| {
                record_spec.surface != ModeDescendantSurface::GorinMainMenu
                    || record_spec.record == ModeDescendantRecord::GorinMainMenu
                    || entry.font_role != ModeDescendantFontRole::GorinHeading
            })
            .collect::<Vec<_>>();
        ensure!(
            !entries.is_empty(),
            "mode-descendant source {} has no translated units",
            source.path
        );
        let (record, mut record_units) =
            build_fixed_record(config, &mut rasterizers, source, record_spec, &entries)?;
        record_drafts.push(record);
        units.append(&mut record_units);
    }

    let edit_command_sheet_source = source_for_spec(&sources, edit_command_sheet_record_spec())?;
    let (_, kanri) = source.read_record("DAT1/KANRI.BIN")?;
    super::edit_team_up_names::validate_consumer(&edit_command_sheet_plan.team_up_names, &kanri)?;
    let edit_command_sheets = build_edit_command_sheets(
        config,
        &mut rasterizers,
        edit_command_sheet_source,
        edit_command_sheet_record_spec(),
        &edit_command_sheet_plan,
    )?;
    record_drafts.push(edit_command_sheets.record);
    let technique_spec = super::catalog::edit_technique_record_spec();
    let (technique_record, edit_technique_names) = super::edit_technique_names::build(
        config,
        &mut rasterizers,
        source,
        source_for_spec(&sources, technique_spec)?,
        technique_spec,
        &assets.edit_technique_names_path,
        &assets.edit_command_sheets_path,
    )?;
    record_drafts.push(technique_record);

    let mut practical_results = None;
    if let Some(practical) = practical_record_specs {
        let entries = assets
            .entries
            .iter()
            .filter(|entry| entry.surface == ModeDescendantSurface::PracticalExamSharedUi)
            .collect::<Vec<_>>();
        let result_plan = practical_result_plan
            .as_ref()
            .context("practical-result plan disappeared from the practical-exam build")?;
        let (mut practical_records, mut practical_units, result_report) =
            build_practical_exam_records(config, &sources, practical, &entries, result_plan)?;
        record_drafts.append(&mut practical_records);
        units.append(&mut practical_units);
        practical_results = Some(result_report);
    }

    let training_menu = if let Some(path) = &assets.training_menu_path {
        let (records, report) =
            super::training_menu::build(config, &mut rasterizers, &sources, path)?;
        record_drafts.extend(records);
        Some(report)
    } else {
        None
    };
    let continue_schools = if let Some(path) = &assets.continue_schools_path {
        let (record, report) =
            super::continue_schools::build(config, &mut rasterizers, &sources, path)?;
        record_drafts.push(record);
        Some(report)
    } else {
        None
    };
    let gorin_selector_names = assets
        .gorin_selector_names_path
        .as_ref()
        .map(|path| {
            super::gorin_selector_names::apply(
                config,
                &mut record_drafts,
                &sources,
                &assets.entries,
                path,
            )
        })
        .transpose()?;
    let illustrated_panels = super::illustrated_panels::apply(
        config,
        &mut record_drafts,
        &assets.illustrated_panel_paths,
    )?;
    let battle_announcements = assets
        .battle_announcements_path
        .as_ref()
        .map(|path| {
            super::battle_announcements::apply(
                config,
                &mut rasterizers,
                path,
                &sources,
                &mut record_drafts,
            )
        })
        .transpose()?;
    let gorin_retry = assets
        .gorin_retry_path
        .as_ref()
        .map(|path| super::gorin_retry::apply(path, &sources, &mut record_drafts))
        .transpose()?;
    let gorin_announcements = assets
        .gorin_announcements_path
        .as_deref()
        .map(|path| super::gorin_announcements::apply(path, &sources, &mut record_drafts))
        .transpose()?;
    if gorin_announcements.is_some() {
        super::practical_exam::gameplay_hud::apply(
            &sources,
            &mut record_drafts,
            &config.fonts.practical_gameplay,
        )?;
    }
    let main_title = assets
        .main_title_path
        .as_deref()
        .map(|path| super::main_title::apply(path, &sources, &mut record_drafts))
        .transpose()?;
    let gorin_dance_intro = assets
        .gorin_dance_intro_path
        .as_ref()
        .map(|path| super::gorin_dance_intro::apply(path, &mut record_drafts))
        .transpose()?;
    let gorin_hud_labels = assets
        .gorin_hud_labels_path
        .as_ref()
        .map(|path| super::gorin_hud_labels::apply(path, config, &mut record_drafts))
        .transpose()?;
    let gorin_gauge_labels = assets
        .gorin_gauge_labels_path
        .as_ref()
        .map(|path| super::gorin_gauge_labels::apply(path, &mut record_drafts))
        .transpose()?;
    let gorin_home_run = assets
        .gorin_home_run_path
        .as_ref()
        .map(|path| super::gorin_home_run::apply(path, &mut record_drafts))
        .transpose()?;
    let gorin_sprint_announcements = assets
        .gorin_sprint_announcements_path
        .as_ref()
        .map(|path| super::gorin_sprint_announcements::apply(path, &mut record_drafts))
        .transpose()?;
    let gorin_dance_announcements = assets
        .gorin_dance_announcements_path
        .as_ref()
        .map(|path| super::gorin_dance_announcements::apply(path, &mut record_drafts))
        .transpose()?;
    let gorin_dance_logo = assets
        .gorin_dance_logo_path
        .as_ref()
        .map(|path| super::gorin_dance_logo::apply(path, &mut record_drafts))
        .transpose()?;
    let built_records = finalize_mode_descendant_records(record_drafts, &sources)?;
    let mut records = Vec::with_capacity(built_records.len());
    let mut stored_records = BTreeMap::new();
    let mut decoded_write_claims = BTreeMap::new();
    for built in built_records {
        if let Some(decoded) = &built.preview_decoded {
            write_record_previews(&config.output_dir, built.spec, decoded)?;
        }
        std::fs::write(
            config.output_dir.join(built.spec.output_file),
            &built.physical,
        )?;
        ensure!(
            stored_records
                .insert(built.spec.record, built.physical)
                .is_none(),
            "mode-descendant record catalog contains a duplicate"
        );
        ensure!(
            decoded_write_claims
                .insert(built.spec.record, built.decoded_write_claims)
                .is_none(),
            "mode-descendant claim catalog contains a duplicate"
        );
        records.push(built.report);
    }
    ensure!(
        authored_surfaces
            .iter()
            .all(|surface| records.iter().any(|record| record.surface == *surface)),
        "mode-descendant build did not consume every translated surface"
    );

    let report = ModeDescendantGraphicsBuildReport {
        kind: "Justice Gakuen 2 non-release mode-descendant graphics build".to_string(),
        source_bin_sha256,
        build_spec_sha256: config.build_spec_sha256.clone(),
        translation_manifest_sha256: assets.manifest_sha256,
        illustrated_panels,
        gorin_selector_names,
        record_owned_unit_count,
        record_composited_unit_count: units.len(),
        record_owned_source_regions_match: true,
        record_owned_cells_are_unique_and_non_overlapping: true,
        record_owned_changes_confined_to_owned_cells: true,
        edit_cpu_tactics_instruction_title_consumer_count,
        edit_cpu_tactics_body_consumer_count,
        edit_cpu_tactics_instruction_title_consumer_bindings_verified: true,
        edit_command_sheets: edit_command_sheets.report,
        edit_technique_names,
        training_menu,
        battle_announcements,
        gorin_retry,
        gorin_announcements,
        main_title,
        gorin_dance_intro,
        gorin_gauge_labels,
        gorin_home_run,
        gorin_sprint_announcements,
        gorin_dance_announcements,
        gorin_dance_logo,
        gorin_hud_labels,
        continue_schools,
        development_input_available: true,
        release_candidate_input_eligible: false,
        practical_results,
        records,
        units,
    };
    let build_manifest_sha256 = write_pretty_json_and_hash(
        &config.output_dir.join(MODE_DESCENDANT_BUILD_MANIFEST_FILE),
        &report,
        true,
    )?;
    Ok(ModeDescendantGraphicsBuild {
        stored_records,
        decoded_write_claims,
        build_manifest_sha256,
        report,
    })
}

fn build_fixed_record(
    config: &ModeDescendantGraphicsBuildConfig,
    rasterizers: &mut IndexedTextRasterizers,
    source: &ModeDescendantSourceRecord,
    record_spec: &'static ModeDescendantRecordSpec,
    entries: &[&ModeDescendantEntry],
) -> Result<(ModeDescendantRecordDraft, Vec<ModeDescendantUnitBuild>)> {
    ensure!(
        record_spec.storage_kind != ModeDescendantStorageKind::Raw
            && source.storage_kind == record_spec.storage_kind,
        "fixed mode-descendant record {} is not a compressed graphics source",
        source.path
    );
    let mut patched = source.decoded.clone();
    let mut decoded_write_claims = Vec::new();
    let mut units = Vec::with_capacity(entries.len());
    for entry in entries {
        let (
            bits_per_pixel,
            clut_in_tim,
            tim_offset_text,
            cell,
            text_alignment,
            explicit_palette_roles,
            expected_source_region_sha256,
            vertical_shift_px,
        ) = match &entry.placement {
            ModeDescendantPlacement::Fixed {
                bits_per_pixel,
                tim_offset,
                cell,
                alignment,
                source_region_sha256,
                palette_roles,
                vertical_shift_px,
            } => (
                *bits_per_pixel,
                true,
                tim_offset.as_str(),
                *cell,
                *alignment,
                palette_roles
                    .as_ref()
                    .map(|roles| (roles.clear_index, roles.outline_index, roles.fill_index)),
                source_region_sha256.as_str(),
                *vertical_shift_px,
            ),
            ModeDescendantPlacement::Fixed4bppWithoutClut {
                tim_offset,
                cell,
                alignment,
                clear_index,
                outline_index,
                fill_index,
                source_region_sha256,
            } => (
                4,
                false,
                tim_offset.as_str(),
                *cell,
                *alignment,
                Some((*clear_index, *outline_index, *fill_index)),
                source_region_sha256.as_str(),
                0,
            ),
            ModeDescendantPlacement::Dynamic => bail!(
                "mode-descendant surface {:?} requires its dynamic atlas builder",
                entry.surface
            ),
        };
        let tim_offset = parse_hex_offset(tim_offset_text)?;
        let source_pixels = match (bits_per_pixel, clut_in_tim) {
            (4, true) => read_indexed_cell_in_prefix(&source.decoded, tim_offset, cell),
            (4, false) => {
                read_indexed_cell_without_clut_in_prefix(&source.decoded, tim_offset, cell)
            }
            (8, true) => read_8bpp_indexed_cell_in_prefix(&source.decoded, tim_offset, cell),
            (bits, _) => bail!("unsupported fixed mode-descendant TIM depth {bits}"),
        }
        .with_context(|| format!("failed to read source region {}", entry.id))?;
        let source_region_sha256 = sha256_bytes(&source_pixels);
        ensure!(
            source_region_sha256 == expected_source_region_sha256,
            "mode-descendant unit {} source region changed: found {source_region_sha256}",
            entry.id
        );
        let style = font_for_entry(config, entry)?;
        let rasterizer = rasterizers.for_font(&style.path)?;
        let (font_name, font_sha256, measured_advance_px, write, placement) = match bits_per_pixel {
            4 => {
                ensure!(
                    entry.font_role != ModeDescendantFontRole::GorinHeading,
                    "Gorin heading {} moved to a 4-bpp texture",
                    entry.id
                );
                let histogram = index_histogram(&source_pixels, 16)?;
                let school_gradient = entry.font_role == ModeDescendantFontRole::EditSchoolLabel;
                let (clear_index, outline_index, fill_index) = if school_gradient {
                    (0, Some(1), 15)
                } else {
                    explicit_palette_roles
                        .map(Ok)
                        .unwrap_or_else(|| derive_palette_roles(&histogram))?
                };
                let alignment = match text_alignment {
                    TextAlignment::Left => HorizontalTextAlignment::Left,
                    TextAlignment::Center => HorizontalTextAlignment::Center,
                };
                let mut raster = rasterizer.rasterize_shifted(
                    &entry.korean_text,
                    cell.width,
                    cell.height,
                    style.font_px,
                    0.0,
                    vertical_shift_px,
                    clear_index,
                    outline_index,
                    fill_index,
                    alignment,
                )?;
                let fill_gradient = if school_gradient {
                    Some(super::continue_schools::apply_school_gradient(
                        &mut raster.pixels,
                        cell.width,
                    )?)
                } else {
                    None
                };
                if entry.font_role == ModeDescendantFontRole::EditHeading {
                    ensure!(
                        entry.surface == ModeDescendantSurface::EditSharedUi
                            && clut_in_tim
                            && tim_offset == 0
                            && (clear_index, outline_index, fill_index) == (0, Some(1), 5),
                        "EDIT heading requires its native transparent, black, and red indices"
                    );
                    super::edit_heading::compose_heading(cell, &mut raster.pixels)?;
                }
                let write = if clut_in_tim {
                    write_indexed_cell_in_prefix_with_report(
                        &mut patched,
                        tim_offset,
                        cell,
                        &raster.pixels,
                    )?
                } else {
                    write_indexed_cell_without_clut_in_prefix_with_report(
                        &mut patched,
                        tim_offset,
                        cell,
                        &raster.pixels,
                    )?
                };
                (
                    raster.font_name,
                    raster.font_sha256,
                    raster.measured_advance_px,
                    write,
                    ModeDescendantUnitPlacementBuild::FixedCell {
                        record: record_spec.record,
                        bits_per_pixel,
                        clut_in_tim,
                        tim_offset: tim_offset_text.to_string(),
                        source_region_sha256: source_region_sha256.clone(),
                        source_index_histogram: histogram,
                        fill_gradient,
                        clear_index,
                        outline_index,
                        fill_index,
                    },
                )
            }
            8 => {
                ensure!(
                    entry.font_role == ModeDescendantFontRole::GorinHeading
                        && entry.surface == ModeDescendantSurface::GorinMainMenu
                        && text_alignment == TextAlignment::Center,
                    "8-bpp mode-descendant unit {} is not the source-bound Gorin heading",
                    entry.id
                );
                let tim_data = source
                    .decoded
                    .get(tim_offset..)
                    .context("Gorin heading TIM offset left its decoded record")?;
                let tim = parse_8bpp_prefix(tim_data)?;
                let full_cell = Cell {
                    x: 0,
                    y: 0,
                    width: tim.pixel_width(),
                    height: tim.image_height,
                };
                let full_source_pixels =
                    read_8bpp_indexed_cell_in_prefix(&source.decoded, tim_offset, full_cell)?;
                let palette = read_8bpp_palette_words_in_prefix(&source.decoded, tim_offset, 0)?;
                let raster = rasterize_gorin_heading(rasterizer, style, cell, &entry.korean_text)?;
                let composition = compose_gorin_heading(
                    &full_source_pixels,
                    full_cell.width,
                    full_cell.height,
                    cell,
                    &source_pixels,
                    &palette,
                    &raster,
                )?;
                let write = write_8bpp_indexed_cell_in_prefix_with_report(
                    &mut patched,
                    tim_offset,
                    cell,
                    &composition.pixels,
                )?;
                (
                    raster.font_name,
                    raster.font_sha256,
                    raster.measured_advance_px,
                    write,
                    ModeDescendantUnitPlacementBuild::GorinHeading {
                        record: record_spec.record,
                        bits_per_pixel,
                        tim_offset: tim_offset_text.to_string(),
                        source_region_sha256: source_region_sha256.clone(),
                        source_exclusive_palette_index_count: composition
                            .source_exclusive_palette_index_count,
                        reconstructed_background_pixel_count: composition
                            .reconstructed_background_pixel_count,
                        korean_ink_pixel_count: composition.korean_ink_pixel_count,
                        source_pixels_preserved_outside_reconstruction_and_korean_ink: composition
                            .source_pixels_preserved_outside_reconstruction_and_korean_ink,
                    },
                )
            }
            _ => unreachable!("validated fixed TIM depth"),
        };
        let changed_decoded_byte_count = write.changed_byte_count;
        ensure!(
            changed_decoded_byte_count > 0,
            "mode-descendant unit {} changed no bytes",
            entry.id
        );
        decoded_write_claims.extend(DecodedDataClaim::from_effective_ranges(
            &format!("mode-descendant:{}", entry.id),
            &format!("render mode-descendant unit {}", entry.id),
            &source.decoded,
            &patched,
            write.allowed_ranges,
        )?);
        units.push(ModeDescendantUnitBuild {
            id: entry.id.clone(),
            surface: record_spec.surface,
            source_text: entry.source_text.clone(),
            korean_text: entry.korean_text.clone(),
            font_role: entry.font_role,
            font_name,
            font_sha256,
            font_px: style.font_px,
            cells: vec![cell],
            measured_advance_px,
            changed_decoded_byte_count,
            placement,
        });
    }
    Ok((
        ModeDescendantRecordDraft {
            spec: record_spec,
            decoded: patched,
            decoded_write_claims,
        },
        units,
    ))
}

fn build_practical_exam_records(
    config: &ModeDescendantGraphicsBuildConfig,
    sources: &[ModeDescendantSourceRecord],
    specs: PracticalExamRecordSpecs,
    entries: &[&ModeDescendantEntry],
    result_plan: &PracticalResultPlan,
) -> Result<(
    Vec<ModeDescendantRecordDraft>,
    Vec<ModeDescendantUnitBuild>,
    PracticalResultBuildReport,
)> {
    ensure!(
        !entries.is_empty(),
        "practical-exam surface has no translated units"
    );
    let mut entries_by_id = BTreeMap::new();
    let mut translations = Vec::with_capacity(entries.len());
    for entry in entries {
        ensure!(
            matches!(entry.placement, ModeDescendantPlacement::Dynamic),
            "practical-exam unit {} must use dynamic placement",
            entry.id
        );
        ensure!(
            entries_by_id.insert(entry.id.as_str(), *entry).is_none(),
            "duplicate practical-exam unit {}",
            entry.id
        );
        translations.push(PracticalExamTranslation::new(
            entry.id.clone(),
            entry.source_text.clone(),
            entry.korean_text.clone(),
            entry.font_role,
        ));
    }

    let basics_texture_source = source_for_spec(sources, specs.basics_texture_producer)?;
    let exam_1999_texture_source = source_for_spec(sources, specs.exam_1999_texture_producer)?;
    let basics_consumer_source = source_for_spec(sources, specs.basics_descriptor_consumer)?;
    let exam_1999_consumer_source = source_for_spec(sources, specs.exam_1999_descriptor_consumer)?;
    let mut shared = build_practical_exam_shared_ui(
        &basics_texture_source.decoded,
        &exam_1999_texture_source.decoded,
        &basics_consumer_source.decoded,
        &exam_1999_consumer_source.decoded,
        &translations,
        &config.fonts,
    )?;
    ensure!(
        shared.report.semantic_entry_count == entries.len()
            && shared.report.rendered_text_count == shared.report.texts.len()
            && shared.report.rendered_text_count == entries.len()
            && shared.report.rewritten_descriptor_count >= entries.len()
            && shared.report.basics_glyph_count > 0
            && shared.report.exam_1999_glyph_count > 0
            && shared.report.basics_stream_size > 0
            && shared.report.exam_1999_stream_size > 0
            && shared.report.basics_coordinate_table_size > 0
            && shared.report.exam_1999_coordinate_table_size > 0
            && shared.report.basics_renderer_instruction_count == 125
            && shared.report.exam_1999_renderer_instruction_count == 125
            && shared.report.basics_renderer_call_count == 21
            && shared.report.exam_1999_renderer_call_count == 10
            && shared.report.basics_primitive_schedule_write_count == 21
            && shared.report.exam_1999_primitive_schedule_write_count == 10
            && shared.report.basics_primitive_schedule_instruction_count == 21
            && shared.report.exam_1999_primitive_schedule_instruction_count == 10
            && shared.report.active_title_cache_covers_term_titles
            && shared.report.active_title_builder_verified
            && shared.report.active_title_max_sprite_count > 0
            && 4 + shared.report.active_title_max_sprite_count
                <= shared.report.active_title_primitive_pool_capacity,
        "practical-exam glyph-stream plan is incomplete"
    );
    ensure!(
        sha256_bytes(&shared.basics_texture_decoded) == shared.report.basics_texture_sha256
            && sha256_bytes(&shared.exam_1999_texture_decoded)
                == shared.report.exam_1999_texture_sha256
            && sha256_bytes(&shared.basics_consumer_overlay)
                == shared.report.basics_consumer_sha256
            && sha256_bytes(&shared.exam_1999_consumer_overlay)
                == shared.report.exam_1999_consumer_sha256
            && shared.report.basics_texture_sha256.len() == 64
            && shared.report.exam_1999_texture_sha256.len() == 64,
        "practical-exam glyph-stream report changed"
    );
    let mut basics_texture_claims = sealed_practical_claims(
        specs.basics_texture_producer,
        &basics_texture_source.decoded,
        &shared.basics_texture_decoded,
    )?;
    let mut exam_1999_texture_claims = sealed_practical_claims(
        specs.exam_1999_texture_producer,
        &exam_1999_texture_source.decoded,
        &shared.exam_1999_texture_decoded,
    )?;
    let result_specs = practical_result_record_specs();
    let mut result_build = result_plan.build_fixed_text(
        result_specs,
        sources,
        &config.fonts,
        PracticalResultFixedTextBases {
            basics_texture: &shared.basics_texture_decoded,
            exam_1999_texture: &shared.exam_1999_texture_decoded,
            basics_consumer: &shared.basics_consumer_overlay,
            exam_1999_consumer: &shared.exam_1999_consumer_overlay,
        },
    )?;
    let result_term_titles =
        build_result_term_titles(entries, sources, &config.fonts, &config.output_dir)?;
    ensure!(
        result_term_titles.report.archive_member_count == 33
            && result_term_titles.report.localized_member_count == 33
            && result_term_titles.report.producer_selector_count == 2
            && result_term_titles.report.renderer_consumer_count == 4
            && result_term_titles
                .report
                .full_124_by_28_sprite_extent_preserved
            && result_term_titles
                .report
                .changes_confined_to_complete_member_tim_pixels,
        "practical result-title family is incomplete"
    );
    result_build.report.result_term_titles = Some(result_term_titles.report);
    ensure!(
        result_build.report.rendered_fixed_text_unit_count > 0
            && result_build.report.rendered_glyph_sequence_unit_count > 0
            && result_build
                .report
                .completed_action_cell_expected_write_count
                > 0
            && result_build.report.shared_action_suffix_cell_count > 0
            && result_build.report.resident_result_provider_cell_count == 8
            && result_build
                .report
                .resident_result_provider_write_contract_complete
            && result_build
                .report
                .rendered_action_cell_writes_are_unique_and_non_overlapping
            && result_build
                .report
                .rendered_action_changes_confined_to_owned_cells
            && result_build.report.consumer_projection_rewrite_count > 0
            && result_build
                .report
                .completed_fixed_text_expected_write_count
                > 0
            && result_build
                .report
                .rendered_decorative_background_target_count
                == result_build
                    .report
                    .validated_decorative_background_target_count
            && result_build
                .report
                .rendered_decorative_background_target_count
                > 0
            && result_build
                .report
                .completed_decorative_background_expected_write_count
                > 0
            && result_build
                .report
                .decorative_background_changes_confined_to_owned_cells
            && !result_build.report.static_patch_insertion_complete
            && !result_build.report.release_candidate_input_eligible,
        "practical-result partial fixed-text build changed its completion boundary"
    );
    let basics_results_source = source_for_spec(sources, result_specs.basics_results)?;
    let exam_1999_results_source = source_for_spec(sources, result_specs.exam_1999_results)?;
    result_plan.validate_unread_active_title_cache(sources, &shared.active_title_cache)?;
    let active_title_texture = install_active_title_texture_cache(
        &basics_results_source.decoded,
        &result_build.basics_results_decoded,
        &result_build.exam_1999_texture_decoded,
        &shared.active_title_cache,
    )?;
    ensure!(
        active_title_texture.cache_cells.len() == shared.report.active_title_cache_glyph_count,
        "active-title producer bridge omitted a cached glyph"
    );
    let indexed_result_mirror = mirror_canonical_result_cells_into_indexed_members(
        &basics_results_source.decoded,
        &active_title_texture.decoded,
        &exam_1999_results_source.decoded,
        &result_build.exam_1999_results_decoded,
        &active_title_texture.cache_cells,
    )?;
    result_build.report.indexed_result_member_count = indexed_result_mirror.member_count;
    result_build.report.indexed_result_mirrored_cell_count =
        indexed_result_mirror.mirrored_cell_count;
    result_build.report.indexed_result_expected_write_count = indexed_result_mirror.claims.len();
    result_build.report.indexed_result_write_contract_complete = indexed_result_mirror.member_count
        == 2
        && indexed_result_mirror.mirrored_cell_count >= 8
        && !indexed_result_mirror.claims.is_empty();
    ensure!(
        result_build.report.indexed_result_write_contract_complete,
        "indexed practical-result producer mirror is incomplete"
    );
    result_build.exam_1999_results_decoded = indexed_result_mirror.decoded;
    result_build
        .exam_1999_results_decoded_write_claims
        .extend(indexed_result_mirror.claims);
    result_build.basics_results_decoded = active_title_texture.decoded;
    result_build
        .basics_results_decoded_write_claims
        .extend(active_title_texture.claims);
    shared.basics_texture_decoded = result_build.basics_texture_decoded;
    shared.exam_1999_texture_decoded = result_build.exam_1999_texture_decoded;
    shared.basics_consumer_overlay = result_build.basics_consumer_overlay;
    shared.exam_1999_consumer_overlay = result_build.exam_1999_consumer_overlay;
    basics_texture_claims.extend(result_build.basics_texture_decoded_write_claims);
    exam_1999_texture_claims.extend(result_build.exam_1999_texture_decoded_write_claims);

    let mut units = Vec::with_capacity(shared.report.texts.len());
    for text in &shared.report.texts {
        let entry = entries_by_id
            .get(text.id)
            .with_context(|| format!("practical-exam rendered uncataloged unit {}", text.id))?;
        ensure!(
            text.korean_text == entry.korean_text && text.font_role == entry.font_role,
            "practical-exam rendered unit {} changed semantic identity",
            text.id
        );
        units.push(ModeDescendantUnitBuild {
            id: text.id.to_string(),
            surface: ModeDescendantSurface::PracticalExamSharedUi,
            source_text: entry.source_text.clone(),
            korean_text: text.korean_text.clone(),
            font_role: text.font_role,
            font_name: text.font_name.clone(),
            font_sha256: text.font_sha256.clone(),
            font_px: text.font_px,
            cells: text.cells.clone(),
            measured_advance_px: text.measured_advance_px,
            changed_decoded_byte_count: text.changed_decoded_byte_count,
            placement: ModeDescendantUnitPlacementBuild::PracticalExamGlyphStream {
                vertical_shift_px: text.vertical_shift_px,
                glyph_count: text.glyph_count,
                consumer_count: text.consumer_count,
                atlas_advance_px: text.atlas_advance_px,
            },
        });
    }
    ensure!(
        units.len() == entries.len(),
        "practical-exam semantic units were rendered more than once or omitted"
    );

    let records = vec![
        ModeDescendantRecordDraft {
            spec: specs.basics_texture_producer,
            decoded_write_claims: basics_texture_claims,
            decoded: shared.basics_texture_decoded,
        },
        ModeDescendantRecordDraft {
            spec: specs.exam_1999_texture_producer,
            decoded_write_claims: exam_1999_texture_claims,
            decoded: shared.exam_1999_texture_decoded,
        },
        ModeDescendantRecordDraft {
            spec: result_specs.basics_results,
            decoded_write_claims: result_build.basics_results_decoded_write_claims,
            decoded: result_build.basics_results_decoded,
        },
        ModeDescendantRecordDraft {
            spec: result_specs.exam_1999_results,
            decoded_write_claims: result_build.exam_1999_results_decoded_write_claims,
            decoded: result_build.exam_1999_results_decoded,
        },
        ModeDescendantRecordDraft {
            spec: result_specs.term_titles,
            decoded_write_claims: result_term_titles.claims,
            decoded: result_term_titles.decoded,
        },
        ModeDescendantRecordDraft {
            spec: specs.basics_descriptor_consumer,
            decoded_write_claims: sealed_practical_claims(
                specs.basics_descriptor_consumer,
                &basics_consumer_source.decoded,
                &shared.basics_consumer_overlay,
            )?,
            decoded: shared.basics_consumer_overlay,
        },
        ModeDescendantRecordDraft {
            spec: specs.exam_1999_descriptor_consumer,
            decoded_write_claims: sealed_practical_claims(
                specs.exam_1999_descriptor_consumer,
                &exam_1999_consumer_source.decoded,
                &shared.exam_1999_consumer_overlay,
            )?,
            decoded: shared.exam_1999_consumer_overlay,
        },
    ];
    Ok((records, units, result_build.report))
}

fn sealed_practical_claims(
    spec: &ModeDescendantRecordSpec,
    source: &[u8],
    candidate: &[u8],
) -> Result<Vec<DecodedDataClaim>> {
    DecodedDataClaim::from_effective_ranges(
        &format!("mode-descendant:practical:{:?}", spec.record),
        &format!(
            "seal the planned practical-exam record {}",
            spec.source_path
        ),
        source,
        candidate,
        difference_ranges(source, candidate),
    )
}

fn write_record_previews(
    output_dir: &Path,
    record_spec: &ModeDescendantRecordSpec,
    decoded: &[u8],
) -> Result<()> {
    ensure!(
        !record_spec.texture_outputs.is_empty(),
        "mode-descendant record has no previewable texture"
    );
    for texture in record_spec.texture_outputs {
        let tim = parse_embedded_tim_at(decoded, texture.tim_offset)
            .context("mode-descendant record lost its cataloged TIM")?;
        let rgba = decode_embedded_tim_preview(decoded, &tim)?;
        write_tim_preview(&output_dir.join(texture.preview_file), &rgba)?;
    }
    Ok(())
}

fn font_for_entry<'a>(
    config: &'a ModeDescendantGraphicsBuildConfig,
    entry: &ModeDescendantEntry,
) -> Result<&'a SizedFontSource> {
    let style = match entry.font_role {
        ModeDescendantFontRole::GorinHeading => &config.fonts.gorin_heading,
        ModeDescendantFontRole::GorinMenu => &config.fonts.gorin_menu,
        ModeDescendantFontRole::EditHeading => &config.fonts.edit_heading,
        ModeDescendantFontRole::EditLabel => &config.fonts.edit_label,
        ModeDescendantFontRole::EditCompactLabel => &config.fonts.edit_compact_label,
        ModeDescendantFontRole::EditSchoolLabel => &config.fonts.edit_school_label,
        ModeDescendantFontRole::PracticalTitle
        | ModeDescendantFontRole::PracticalMenuLabel
        | ModeDescendantFontRole::PracticalPrompt
        | ModeDescendantFontRole::PracticalHint
        | ModeDescendantFontRole::PracticalResultHeading
        | ModeDescendantFontRole::PracticalResultLabel
        | ModeDescendantFontRole::PracticalResultHint
        | ModeDescendantFontRole::PracticalResultAction
        | ModeDescendantFontRole::PracticalResultSmallJudgment
        | ModeDescendantFontRole::PracticalResultJudgmentStamp
        | ModeDescendantFontRole::PracticalResultBranding => {
            bail!("practical-exam font role reached the fixed-cell builder")
        }
    };
    ensure!(
        matches!(
            (entry.surface, entry.font_role),
            (
                ModeDescendantSurface::GorinMainMenu,
                ModeDescendantFontRole::GorinHeading | ModeDescendantFontRole::GorinMenu
            ) | (
                ModeDescendantSurface::EditSharedUi,
                ModeDescendantFontRole::EditHeading
                    | ModeDescendantFontRole::EditLabel
                    | ModeDescendantFontRole::EditCompactLabel
                    | ModeDescendantFontRole::EditSchoolLabel
            )
        ),
        "mode-descendant unit {} selects a font role for another surface",
        entry.id
    );
    Ok(style)
}

fn index_histogram(pixels: &[u8], palette_size: usize) -> Result<Vec<usize>> {
    let mut histogram = vec![0usize; palette_size];
    for pixel in pixels {
        let count = histogram
            .get_mut(usize::from(*pixel))
            .context("indexed source pixel leaves its declared palette")?;
        *count += 1;
    }
    Ok(histogram)
}

fn derive_palette_roles(histogram: &[usize]) -> Result<(u8, Option<u8>, u8)> {
    let clear_index = histogram
        .iter()
        .enumerate()
        .max_by_key(|(index, count)| (**count, std::cmp::Reverse(*index)))
        .map(|(index, _)| index as u8)
        .context("source cell has no palette histogram")?;
    let mut ink = histogram
        .iter()
        .enumerate()
        .filter(|(index, count)| *index != usize::from(clear_index) && **count > 0)
        .map(|(index, count)| (index as u8, *count))
        .collect::<Vec<_>>();
    ink.sort_by_key(|(index, count)| (std::cmp::Reverse(*count), *index));
    let fill_index = ink
        .first()
        .map(|(index, _)| *index)
        .context("source cell has no ink palette index")?;
    let outline_index = ink.get(1).map(|(index, _)| *index);
    Ok((clear_index, outline_index, fill_index))
}

fn prepare_output_directory(output_dir: &Path, force: bool) -> Result<()> {
    if output_dir.exists() && !force {
        bail!("mode-descendant build output exists; pass --force to replace it");
    }
    if output_dir.exists() {
        std::fs::remove_dir_all(output_dir)
            .with_context(|| format!("failed to remove {}", output_dir.display()))?;
    }
    std::fs::create_dir_all(output_dir)
        .with_context(|| format!("failed to create {}", output_dir.display()))
}

#[cfg(test)]
#[path = "build_tests.rs"]
mod tests;
