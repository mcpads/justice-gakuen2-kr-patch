use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Context, Result, bail, ensure};

use crate::font::IndexedTextRasterizers;
use crate::pipeline::sha256_bytes;
use crate::source_disc::SupportedSourceDisc;

use super::allocation::plan_character_select_atlas_from_assets;
use super::assets::load_character_select_translation_assets;
use super::auxiliary_record_build::build_auxiliary_texture_records;
use super::compressed_record::{CompressedStreamStorage, CompressedTextureStream};
use super::consumer::rewrite_character_select_overlay;
use super::model::{
    CharacterSelectAtlasBuild, CharacterSelectAtlasBuildConfig, CharacterSelectAtlasBuildReport,
    CharacterSelectAtlasRecordBuild, CharacterSelectCompressedStreamBuild,
    CharacterSelectFixedStripBuild, CharacterSelectFontRole, CharacterSelectGlyphBuild,
    CharacterSelectSourceInkCleanupBuild,
};
use super::plan::shared_atlas_bytes;
use super::planned_texture_targets::planned_tim_offsets;
use super::planning_sources::CharacterSelectPlanningSources;
use super::preview::build_target_tim_reports;
use super::record_build::{TextureContributions, TextureRecordSource, build_texture_record};
use super::render::{render_fixed_strips, render_glyphs, render_segmented_fixed_strips};
use super::source::{
    load_character_select_auxiliary_source_from_disc, load_character_select_source_from_disc,
};

pub const CHARACTER_SELECT_BUILD_MANIFEST_FILE: &str = "character-select-atlas-build.json";

pub fn build_character_select_atlas(
    config: &CharacterSelectAtlasBuildConfig,
) -> Result<CharacterSelectAtlasBuild> {
    let source = SupportedSourceDisc::open(&config.cue)?;
    build_character_select_atlas_from_source(config, &source, None, None)
}

pub(crate) fn build_character_select_atlas_from_source(
    config: &CharacterSelectAtlasBuildConfig,
    source: &SupportedSourceDisc,
    plain_loading: Option<&crate::loading_art::PlainLoadingBuild>,
    selector_runtime: Option<&crate::name_input::SelectorRuntimeInstallation>,
) -> Result<CharacterSelectAtlasBuild> {
    prepare_output_directory(&config.output_dir, config.force)?;
    let (source_bin_sha256, sources) = load_character_select_source_from_disc(source)?;
    let auxiliary_sources = load_character_select_auxiliary_source_from_disc(source)?;
    let planning_sources =
        CharacterSelectPlanningSources::from_loaded_records(&sources, &auxiliary_sources)?;
    // Planning and consumer rewriting must use the same validated input snapshot.
    let translation_assets = load_character_select_translation_assets(&config.assets)?;
    let plan = plan_character_select_atlas_from_assets(&planning_sources, &translation_assets)?;
    ensure!(
        sources.iter().all(|source| {
            shared_atlas_bytes(&source.decoded)
                .map(|atlas| sha256_bytes(atlas) == plan.source_shared_atlas_sha256)
                .unwrap_or(false)
        }),
        "SELP1 through SELP5 no longer share the planned source atlas"
    );

    let diagnosis_drafts = std::fs::read(
        config
            .assets
            .join("cooperative/diagnosis/translation-drafts.json"),
    )?;
    let diagnosis_style = &config.fonts.cooperative_diagnosis;
    ensure!(
        diagnosis_style.font_px == 16.0 && diagnosis_style.vertical_shift_px == 0,
        "diagnosis renderer requires the verified 16px unshifted font"
    );
    let mut diagnosis = crate::cooperative_diagnosis::build(
        &sources
            .iter()
            .find(|s| s.overlay_path == "DAT1/PLSEL5.BIN")
            .context("missing PLSEL5")?
            .overlay,
        &auxiliary_sources
            .iter()
            .find(|s| s.path == "DAT2/AISYOU.TIZ")
            .context("missing AISYOU")?
            .decoded,
        &diagnosis_drafts,
        &diagnosis_style.path,
    )?;
    let score_labels = std::fs::read(
        config
            .assets
            .join("cooperative/diagnosis/score-labels.json"),
    )?;
    let score_label_count = crate::cooperative_diagnosis::add_score_labels(
        &mut diagnosis,
        &sources
            .iter()
            .find(|s| s.overlay_path == "DAT1/PLSEL5.BIN")
            .context("missing PLSEL5")?
            .overlay,
        &auxiliary_sources
            .iter()
            .find(|s| s.path == "DAT2/AISYOU.TIZ")
            .context("missing AISYOU")?
            .decoded,
        &score_labels,
        &diagnosis_style.path,
    )?;
    let diagnosis_title = std::fs::read(config.assets.join("cooperative/diagnosis/title.json"))?;
    crate::cooperative_diagnosis::add_title(
        &mut diagnosis,
        &auxiliary_sources
            .iter()
            .find(|s| s.path == "DAT2/AISYOU.TIZ")
            .context("missing AISYOU")?
            .decoded,
        &diagnosis_title,
        &diagnosis_style.path,
    )?;
    let diagnosis_report = crate::cooperative_diagnosis::DiagnosisBuildReport {
        title_sha256: sha256_bytes(&diagnosis_title),
        score_label_count,
        score_labels_sha256: sha256_bytes(&score_labels),
        page_count: diagnosis.page_count,
        glyph_count: diagnosis.glyph_count,
        translation_sha256: sha256_bytes(&diagnosis_drafts),
        font_sha256: sha256_bytes(&std::fs::read(&diagnosis_style.path)?),
        overlay_candidate_sha256: sha256_bytes(&diagnosis.overlay),
        texture_candidate_sha256: sha256_bytes(&diagnosis.texture),
        runtime_verified: false,
    };
    let settings_manifest = std::fs::read(config.assets.join("system-settings/text.json"))?;
    let settings_style = &config.fonts.system_settings;
    ensure!(
        settings_style.font_px == 16.0 && settings_style.vertical_shift_px == 0,
        "system settings require verified 16px unshifted font"
    );
    let settings_source = sources
        .iter()
        .find(|s| s.path == "DAT2/SELP1.BIZ")
        .context("missing SELP1")?;
    let settings = crate::system_settings_text::build(
        &settings_source.overlay,
        &settings_source.decoded,
        &settings_manifest,
        &settings_style.path,
    )?;
    let settings_report = crate::system_settings_text::SystemSettingsBuildReport {
        string_count: settings.string_count,
        glyph_count: settings.glyph_count,
        translation_sha256: sha256_bytes(&settings_manifest),
        font_sha256: sha256_bytes(&std::fs::read(&settings_style.path)?),
        runtime_verified: false,
    };
    let mut rasterizers = IndexedTextRasterizers::default();
    let rendered = render_glyphs(&mut rasterizers, config, &plan.allocations)?;
    let mut rendered_fixed_strips = render_fixed_strips(
        &mut rasterizers,
        config,
        &plan.fixed_strips,
        &translation_assets.localized_sources,
    )?;
    rendered_fixed_strips.extend(render_segmented_fixed_strips(
        &mut rasterizers,
        config,
        &plan.segmented_fixed_strips,
        &translation_assets.localized_sources,
    )?);
    let mut outputs = Vec::with_capacity(sources.len());
    let small_logo = super::small_logo::SmallLogo::load(&config.assets)?;
    let mut records = Vec::with_capacity(sources.len());
    let mut glyphs = BTreeMap::<
        (
            super::model::CharacterSelectTextureSurface,
            CharacterSelectFontRole,
            char,
        ),
        CharacterSelectGlyphBuild,
    >::new();
    let mut fixed_strips = BTreeMap::<String, CharacterSelectFixedStripBuild>::new();
    let mut source_ink_cleanups = BTreeMap::<String, CharacterSelectSourceInkCleanupBuild>::new();
    let mut roster_runtimes = BTreeMap::new();
    for source in &sources {
        let roster_number = match source.path {
            "DAT2/SELP2.BIZ" => Some(2),
            "DAT2/SELP3.BIZ" => Some(3),
            "DAT2/SELP4.BIZ" => Some(4),
            "DAT2/SELP5.BIZ" => Some(5),
            _ => None,
        };
        let roster_runtime =
            if let (Some(runtime), Some(number)) = (selector_runtime, roster_number) {
                Some(runtime.roster(number, &source.overlay)?)
            } else {
                None
            };
        let compression_streams = [CompressedTextureStream {
            stream_index: 0,
            decoded_range: [0, source.decoded.len()],
            storage: CompressedStreamStorage::direct(source.stored.len())?,
        }];
        let product = build_texture_record(
            TextureRecordSource {
                path: source.path,
                stored: &source.stored,
                decoded: &source.decoded,
                compression_streams: &compression_streams,
            },
            &rendered,
            &rendered_fixed_strips,
            &plan.source_ink_cleanups,
            TextureContributions {
                settings: Some(&settings),
                logo: Some(&small_logo),
                selector_runtime,
                roster_runtime: roster_runtime.as_ref(),
                native_text_font: Some(&config.native_text_font),
                prompt_text_font: Some(&config.fonts.label),
                ..Default::default()
            },
        )?;
        let patched_shared_atlas_sha256 =
            sha256_bytes(shared_atlas_bytes(&product.patched_decoded)?);
        let target_tims = build_target_tim_reports(
            &config.output_dir,
            source.output_file,
            &source.decoded,
            &product.patched_decoded,
            {
                let mut offsets = planned_tim_offsets(
                    source.path,
                    &rendered,
                    &rendered_fixed_strips,
                    &plan.source_ink_cleanups,
                );
                if source.path == "DAT2/SELP1.BIZ" {
                    offsets.insert(0x38000);
                }
                offsets
            },
        )?;
        let record = CharacterSelectAtlasRecordBuild {
            source_path: source.path.to_string(),
            output_file: source.output_file.to_string(),
            source_stored_sha256: product.source_stored_sha256.clone(),
            source_decoded_sha256: product.source_decoded_sha256.clone(),
            patched_stored_sha256: product.patched_stored_sha256.clone(),
            patched_decoded_sha256: product.patched_decoded_sha256.clone(),
            patched_shared_atlas_sha256,
            native_text: product
                .native_text
                .context("SELP native text producer missing")?,
            source_record_size: product.source_record_size,
            changed_stored_byte_ranges: product.changed_stored_byte_ranges.clone(),
            compressed_streams: product
                .streams
                .iter()
                .map(|stream| CharacterSelectCompressedStreamBuild {
                    stream_index: stream.stream_index,
                    decoded_range: stream.decoded_range,
                    source_decoded_sha256: stream.source_decoded_sha256.clone(),
                    patched_decoded_sha256: stream.patched_decoded_sha256.clone(),
                    changed: stream.changed,
                    unpadded_stored_size: stream.unpadded_stored_size,
                    encoded_stream_offset: stream.encoded_stream_offset,
                    encoded_stream_capacity: stream.encoded_stream_capacity,
                })
                .collect(),
            target_tims,
        };
        merge_record_graphics(
            &mut glyphs,
            &mut fixed_strips,
            &mut source_ink_cleanups,
            product.glyphs,
            product.fixed_strips,
            product.source_ink_cleanups,
        )?;
        let output_path = config.output_dir.join(&record.output_file);
        std::fs::write(&output_path, &product.stored)
            .with_context(|| format!("failed to write {}", output_path.display()))?;
        outputs.push(product.stored);
        records.push(record);
        if let Some(runtime) = roster_runtime {
            roster_runtimes.insert(source.overlay_path, (roster_number.unwrap(), runtime));
        }
    }
    let auxiliary_products = build_auxiliary_texture_records(
        &config.output_dir,
        &auxiliary_sources,
        &rendered,
        &rendered_fixed_strips,
        &plan.source_ink_cleanups,
        plain_loading,
        &diagnosis,
    )?;
    let mut auxiliary_outputs = Vec::with_capacity(auxiliary_products.len());
    let mut auxiliary_records = Vec::with_capacity(auxiliary_products.len());
    for product in auxiliary_products {
        merge_record_graphics(
            &mut glyphs,
            &mut fixed_strips,
            &mut source_ink_cleanups,
            product.glyphs,
            product.fixed_strips,
            product.source_ink_cleanups,
        )?;
        auxiliary_outputs.push(product.stored);
        auxiliary_records.push(product.report);
    }
    let mut overlay_outputs = Vec::with_capacity(sources.len());
    let mut overlays = Vec::with_capacity(sources.len());
    for source in &sources {
        let (mut patched_overlay, mut overlay_report) = rewrite_character_select_overlay(
            source.overlay_path,
            &source.overlay,
            &plan.allocations,
            &plan.consumer,
            &translation_assets.localized_sources,
        )?;
        if source.overlay_path == "DAT1/PLSEL5.BIN" {
            crate::cooperative_diagnosis::compose(
                &source.overlay,
                &mut patched_overlay,
                &diagnosis.overlay,
                &diagnosis.overlay_ranges,
            )?;
            overlay_report
                .expected_write_ranges
                .extend_from_slice(&diagnosis.overlay_ranges);
            overlay_report.changed_byte_ranges =
                crate::pipeline::difference_ranges(&source.overlay, &patched_overlay);
            overlay_report.patched_sha256 = sha256_bytes(&patched_overlay);
        }
        if source.overlay_path == "DAT1/PLSEL1.BIN" {
            crate::cooperative_diagnosis::compose(
                &source.overlay,
                &mut patched_overlay,
                &settings.overlay,
                &settings.overlay_ranges,
            )?;
            overlay_report
                .expected_write_ranges
                .extend_from_slice(&settings.overlay_ranges);
            overlay_report.changed_byte_ranges =
                crate::pipeline::difference_ranges(&source.overlay, &patched_overlay);
            overlay_report.patched_sha256 = sha256_bytes(&patched_overlay);
        }
        if source.overlay_path == "DAT1/PLSEL1.BIN"
            && let Some(runtime) = selector_runtime
        {
            let hooks = runtime.overlay(&source.overlay)?;
            crate::cooperative_diagnosis::compose(
                &source.overlay,
                &mut patched_overlay,
                &hooks.bytes,
                &hooks.code_ranges,
            )?;
            overlay_report
                .expected_write_ranges
                .extend_from_slice(&hooks.code_ranges);
            overlay_report.changed_byte_ranges =
                crate::pipeline::difference_ranges(&source.overlay, &patched_overlay);
            overlay_report.patched_sha256 = sha256_bytes(&patched_overlay);
        }
        if let Some((roster_number, runtime)) = roster_runtimes.get(source.overlay_path) {
            let hooks = runtime
                .program
                .roster_source_hooks
                .as_ref()
                .context("missing roster source hooks")?;
            crate::cooperative_diagnosis::compose(
                &source.overlay,
                &mut patched_overlay,
                &hooks.overlay,
                &hooks.code_ranges,
            )?;
            overlay_report
                .expected_write_ranges
                .extend_from_slice(&hooks.code_ranges);
            overlay_report.changed_byte_ranges =
                crate::pipeline::difference_ranges(&source.overlay, &patched_overlay);
            overlay_report.patched_sha256 = sha256_bytes(&patched_overlay);
            let report = serde_json::json!({
                "overlay": source.overlay_path, "layout": runtime.layout,
                "resident_sha256": sha256_bytes(&runtime.program.resident_bytes),
                "resident_byte_count": runtime.program.resident_bytes.len(),
                "bootstrap_sha256": sha256_bytes(&runtime.program.bootstrap_bytes),
                "ascii_sha256": sha256_bytes(&runtime.program.ascii_bytes),
                "code_ranges": hooks.code_ranges, "runtime_verified": false,
            });
            std::fs::write(
                config
                    .output_dir
                    .join(format!("roster-names-{}.json", roster_number)),
                serde_json::to_vec_pretty(&report)?,
            )?;
        }
        let output_file = source.overlay_output_file;
        ensure!(
            output_file == overlay_report.output_file,
            "character-select overlay output mapping drifted"
        );
        let output_path = config.output_dir.join(output_file);
        std::fs::write(&output_path, &patched_overlay)
            .with_context(|| format!("failed to write {}", output_path.display()))?;
        overlay_outputs.push(patched_overlay);
        overlays.push(overlay_report);
    }
    let glyphs = glyphs.into_values().collect::<Vec<_>>();
    let fixed_strips = fixed_strips.into_values().collect::<Vec<_>>();
    let source_ink_cleanups = source_ink_cleanups.into_values().collect::<Vec<_>>();
    let planned_fixed_strip_segment_count = plan.fixed_strips.len()
        + plan
            .segmented_fixed_strips
            .iter()
            .map(|strip| strip.segments.len())
            .sum::<usize>();
    ensure!(
        fixed_strips.len() == planned_fixed_strip_segment_count,
        "not every planned character-select fixed strip reached a source record"
    );
    ensure!(
        source_ink_cleanups.len() == plan.source_ink_cleanups.len(),
        "not every planned character-select source-ink cleanup reached a source record"
    );
    let fully_routed_source_ui_ids = plan
        .fully_routed_source_ui_ids
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let unresolved_route_source_ui_ids = plan
        .unresolved_route_source_ui_ids
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    ensure!(
        fully_routed_source_ui_ids
            .union(&unresolved_route_source_ui_ids)
            .count()
            == plan.translated_source_ui_count,
        "character-select route occurrences do not cover the translated logical-source denominator"
    );
    let translations_by_source_ui_id = translation_assets
        .localized_sources
        .iter()
        .map(|source| (source.source_ui_id.as_str(), source.translation_id.as_str()))
        .collect::<BTreeMap<_, _>>();
    let translation_ids_for_sources = |source_ui_ids: &BTreeSet<String>| {
        source_ui_ids
            .iter()
            .filter_map(|source_ui_id| translations_by_source_ui_id.get(source_ui_id.as_str()))
            .map(|translation_id| (*translation_id).to_string())
            .collect::<BTreeSet<_>>()
    };
    let fully_routed_translation_ids = translation_ids_for_sources(&fully_routed_source_ui_ids);
    let unresolved_route_translation_ids =
        translation_ids_for_sources(&unresolved_route_source_ui_ids);
    let translation_ids = translation_assets
        .entries
        .iter()
        .map(|entry| entry.id.clone())
        .collect::<BTreeSet<_>>();
    ensure!(
        fully_routed_translation_ids
            .union(&unresolved_route_translation_ids)
            .cloned()
            .collect::<BTreeSet<_>>()
            == translation_ids,
        "character-select route coverage does not reach every translation entry: fully_routed={fully_routed_translation_ids:?}, unresolved={unresolved_route_translation_ids:?}, translations={translation_ids:?}"
    );
    let fully_routed_translation_ids = fully_routed_translation_ids.into_iter().collect::<Vec<_>>();
    let unresolved_route_translation_ids = unresolved_route_translation_ids
        .into_iter()
        .collect::<Vec<_>>();
    let release_candidate_input_eligible = plan.source_inventory_complete
        && plan.untranslated_source_ui_ids.is_empty()
        && plan.route_census.unresolved_route_occurrence_count == 0;
    let report = CharacterSelectAtlasBuildReport {
        selector_names: selector_runtime.map(|runtime| runtime.report()),
        cooperative_diagnosis: diagnosis_report,
        system_settings: settings_report,
        small_logo: small_logo.report,
        kind: "Justice Gakuen 2 non-release dynamic character-select atlas build".to_string(),
        source_bin_sha256,
        build_spec_sha256: config.build_spec_sha256.clone(),
        translation_assets_sha256: plan.translation_assets_sha256,
        source_shared_atlas_sha256: plan.source_shared_atlas_sha256,
        source_inventory_complete: plan.source_inventory_complete,
        source_inventory_unit_count: plan.source_inventory_unit_count,
        source_inventory_entry_count: plan.source_inventory_entry_count,
        translated_source_ui_count: plan.translated_source_ui_count,
        untranslated_source_ui_count: plan.untranslated_source_ui_count,
        untranslated_source_ui_ids: plan.untranslated_source_ui_ids,
        preserved_source_ui_ids: plan.preserved_source_ui_ids,
        excluded_source_ui_ids: plan.excluded_source_ui_ids,
        runtime_composed_texture_entry_count: plan.runtime_composed_texture_entry_count,
        unresolved_route_occurrence_count: plan.unresolved_route_occurrence_count,
        consumer: plan.consumer,
        fully_routed_translation_ids,
        fully_routed_source_ui_ids: fully_routed_source_ui_ids.into_iter().collect(),
        unresolved_route_source_ui_ids: unresolved_route_source_ui_ids.into_iter().collect(),
        unresolved_route_translation_ids,
        route_census: plan.route_census,
        record_count: records.len(),
        auxiliary_record_count: auxiliary_records.len(),
        glyph_count: glyphs.len(),
        fixed_texture_strip_count: fixed_strips.len(),
        source_ink_cleanup_count: source_ink_cleanups.len(),
        source_glyph_cell_count: plan.source_glyph_cell_count,
        all_records_share_select_heading_allocations: true,
        changed_bytes_confined_to_allocated_cells: true,
        compression_roundtrip_verified: true,
        catalog_prefixes_preserved: true,
        development_input_available: true,
        release_candidate_input_eligible,
        records,
        auxiliary_records,
        overlays,
        source_glyph_cells: plan.source_glyph_cells,
        glyphs,
        fixed_strips,
        source_ink_cleanups,
    };
    let mut bytes = serde_json::to_vec_pretty(&report)?;
    bytes.push(b'\n');
    let manifest_sha256 = sha256_bytes(&bytes);
    std::fs::write(
        config.output_dir.join(CHARACTER_SELECT_BUILD_MANIFEST_FILE),
        &bytes,
    )?;
    Ok(CharacterSelectAtlasBuild {
        records: outputs,
        auxiliary_records: auxiliary_outputs,
        overlays: overlay_outputs,
        manifest_sha256,
        report,
    })
}

fn merge_record_graphics(
    glyphs: &mut BTreeMap<
        (
            super::model::CharacterSelectTextureSurface,
            CharacterSelectFontRole,
            char,
        ),
        CharacterSelectGlyphBuild,
    >,
    fixed_strips: &mut BTreeMap<String, CharacterSelectFixedStripBuild>,
    source_ink_cleanups: &mut BTreeMap<String, CharacterSelectSourceInkCleanupBuild>,
    record_glyphs: Vec<CharacterSelectGlyphBuild>,
    record_fixed_strips: Vec<CharacterSelectFixedStripBuild>,
    record_source_ink_cleanups: Vec<CharacterSelectSourceInkCleanupBuild>,
) -> Result<()> {
    for glyph in record_glyphs {
        let key = (glyph.surface, glyph.font_role, glyph.character);
        if let Some(existing) = glyphs.get(&key) {
            ensure!(
                existing.indexed_sha256 == glyph.indexed_sha256 && existing.cell == glyph.cell,
                "character-select glyph {:?} produced inconsistent record applications",
                glyph.character
            );
        } else {
            glyphs.insert(key, glyph);
        }
    }
    for strip in record_fixed_strips {
        let physical_text_region_id = strip.physical_text_region_id.clone();
        if let Some(existing) = fixed_strips.get(&physical_text_region_id) {
            ensure!(
                existing.indexed_sha256 == strip.indexed_sha256
                    && existing.cell == strip.cell
                    && existing.surface == strip.surface,
                "character-select physical text region {physical_text_region_id} produced inconsistent record applications"
            );
        } else {
            fixed_strips.insert(physical_text_region_id, strip);
        }
    }
    for cleanup in record_source_ink_cleanups {
        let physical_region_id = cleanup.physical_region_id.clone();
        if let Some(existing) = source_ink_cleanups.get(&physical_region_id) {
            ensure!(
                existing.cell == cleanup.cell
                    && existing.surface == cleanup.surface
                    && existing.source_ink_count == cleanup.source_ink_count,
                "character-select source-ink cleanup {physical_region_id} produced inconsistent record applications"
            );
        } else {
            source_ink_cleanups.insert(physical_region_id, cleanup);
        }
    }
    Ok(())
}

fn prepare_output_directory(output_dir: &Path, force: bool) -> Result<()> {
    if output_dir.exists() {
        ensure!(
            output_dir.is_dir(),
            "character-select atlas output is not a directory: {}",
            output_dir.display()
        );
        let mut entries = std::fs::read_dir(output_dir)?;
        if entries.next().transpose()?.is_some() && !force {
            bail!(
                "character-select atlas output is not empty; pass --force to replace {}",
                output_dir.display()
            );
        }
        if force {
            std::fs::remove_dir_all(output_dir)?;
        }
    }
    std::fs::create_dir_all(output_dir)?;
    Ok(())
}
