use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result, bail, ensure};
use serde::Serialize;

use crate::compression::decompress;
use crate::cue::CueSheet;
use crate::decoded_record_write_plan::{DecodedDataClaim, DecodedRecordWritePlan};
use crate::font::{HorizontalTextAlignment, IndexedTextRasterizers};
use crate::menu_compression::compress_menu_with_source_limits;
use crate::pipeline::{
    EMBEDDED_MOJI2_TIM_SIZE, difference_ranges, sha256_bytes, write_pretty_json_and_hash,
};
use crate::source_disc::SupportedSourceDisc;
use crate::tim::install_indexed_glyph;
use crate::write_scope::changed_ranges_are_within;

use super::allocation::{
    GlyphAllocation, OptionsGlyphRequests, UntranslatedGlyphCells, collect_options_glyph_requests,
};
use super::assets::{
    load_assets, select_authored_options_build_units, validate_complete_source_population,
};
use super::background_build::{OptionsBackgroundBuild, rebuild_options_background};
use super::description_assets::load_description_assets;
use super::description_rebuild::{description_expected_write_ranges, rebuild_options_descriptions};
use super::main_executable_contributions::{
    OptionsMainExecutableContributions, compose_options_main_executable,
};
use super::model::{
    OptionsBuildConfig, OptionsFontBuild, OptionsFontRole, OptionsFontStyle, OptionsGlyphBuild,
    OptionsRecordBuildReport, OptionsReleaseStatus,
};
use super::records_main;
use super::records_runtime::rebuild_records_runtime;
use super::runtime_glyph_upload::{
    ContextualGlyphUploadInputs, PreparedGlyph, RuntimeGlyphUploadBuild,
    build_contextual_glyph_upload, pack_indexed_pixels,
};
use super::source::load_options_source_from_disc;
use super::source_catalog::{catalog_large_placement_text_offsets, catalog_newopt_pointer_records};
use super::text_rebuild::{encode_unit, options_text_expected_write_ranges, rebuild_options_text};

pub(crate) const RECORD_BUILD_MANIFEST_FILE: &str = "options-record-build.json";
const MENU_OUTPUT_FILE: &str = "options-menu.biz";
const OVERLAY_OUTPUT_FILE: &str = "options-newopt.bin";
const OPTINFO_OUTPUT_FILE: &str = "options-optinfo.tiz";
const MAIN_EXECUTABLE_OUTPUT_FILE: &str = "options-runtime-slps.bin";
const CLEAR_INDEX: u8 = 0;
const OUTLINE_INDEX: u8 = 3;
const FILL_INDEX: u8 = 14;

pub(crate) struct OptionsRecordBuild {
    pub(crate) cue: CueSheet,
    pub(crate) source_menu_stored: Vec<u8>,
    pub(crate) source_menu_decoded: Vec<u8>,
    pub(crate) menu_stored: Vec<u8>,
    pub(crate) menu_decoded: Vec<u8>,
    pub(crate) menu_write_claims: Vec<DecodedDataClaim>,
    pub(crate) overlay: Vec<u8>,
    pub(crate) optinfo_stored: Vec<u8>,
    pub(crate) optinfo_decoded: Vec<u8>,
    pub(crate) source_main_executable: Vec<u8>,
    pub(crate) main_executable: Vec<u8>,
    pub(super) main_executable_contributions: OptionsMainExecutableContributions,
    pub(crate) build_manifest_sha256: String,
    pub(crate) report: OptionsRecordBuildReport,
}

struct InstalledOptionsGlyphs {
    fonts: Vec<OptionsFontBuild>,
    reports: Vec<OptionsGlyphBuild>,
    options_contextual: Vec<PreparedGlyph>,
    records_contextual: Vec<PreparedGlyph>,
}

pub(crate) fn build_options_records(config: &OptionsBuildConfig) -> Result<OptionsRecordBuild> {
    let source = SupportedSourceDisc::open(&config.cue)?;
    build_options_records_from_source(config, &source)
}

pub(crate) fn build_options_records_from_source(
    config: &OptionsBuildConfig,
    source_disc: &SupportedSourceDisc,
) -> Result<OptionsRecordBuild> {
    let requests = collect_menu_requests(&config.assets, &config.fonts, source_disc)?;
    let plan = crate::menu_atlas_plan::MenuAtlasPlan::build(&requests.requests(), &[])?;
    build_options_records_with_plan(config, source_disc, &plan)
}

pub(crate) fn collect_menu_requests(
    root: &std::path::Path,
    fonts: &super::model::OptionsFontStyles,
    source_disc: &SupportedSourceDisc,
) -> Result<OptionsGlyphRequests> {
    let source = load_options_source_from_disc(source_disc)?;
    let assets = load_assets(
        root,
        &source.source_bin_sha256,
        &source.overlay,
        &source.menu_stored,
        &source.menu_decoded,
    )?;
    validate_complete_source_population(
        &assets.units,
        &catalog_newopt_pointer_records(&source.overlay)?,
    )?;
    let units = select_authored_options_build_units(&assets.units)?;
    let large = catalog_large_placement_text_offsets(&source.overlay)?;
    let untranslated = UntranslatedGlyphCells::from_units(&assets.units, &large)?;
    collect_options_glyph_requests(&units, fonts, &untranslated.preserved_codes())
}

pub(crate) fn build_options_records_with_plan(
    config: &OptionsBuildConfig,
    source_disc: &SupportedSourceDisc,
    plan: &crate::menu_atlas_plan::MenuAtlasPlan,
) -> Result<OptionsRecordBuild> {
    prepare_record_outputs(&config.output_dir, config.force)?;
    let source = load_options_source_from_disc(source_disc)?;
    let assets = load_assets(
        &config.assets,
        &source.source_bin_sha256,
        &source.overlay,
        &source.menu_stored,
        &source.menu_decoded,
    )?;
    let source_catalog = catalog_newopt_pointer_records(&source.overlay)?;
    validate_complete_source_population(&assets.units, &source_catalog)?;
    let build_units = select_authored_options_build_units(&assets.units)?;
    let description_assets = load_description_assets(&config.assets, &source)?;
    let large_text_source_offsets = catalog_large_placement_text_offsets(&source.overlay)?;
    let untranslated_cells =
        UntranslatedGlyphCells::from_units(&assets.units, &large_text_source_offsets)?;
    let preserved_codes = untranslated_cells.preserved_codes();
    let requests = collect_options_glyph_requests(&build_units, &config.fonts, &preserved_codes)?;
    let allocation = requests.resolve(plan)?;
    let untranslated_glyph_conflicts =
        untranslated_cells.conflicts_for(&allocation.provisional_physical_codes);
    ensure!(
        untranslated_glyph_conflicts.is_empty(),
        "options development font overwrites untranslated NEWOPT glyph cells: {}",
        untranslated_glyph_conflicts.join("; ")
    );
    let mut contextual_menu_decoded = source.menu_decoded.clone();
    let InstalledOptionsGlyphs {
        fonts,
        reports: glyphs,
        options_contextual: options_contextual_glyphs,
        records_contextual: records_contextual_glyphs,
    } = install_glyphs(
        &mut contextual_menu_decoded[..EMBEDDED_MOJI2_TIM_SIZE],
        &allocation.global_glyphs,
        &allocation.records_contextual_glyphs,
        &config.fonts,
    )?;
    let OptionsBackgroundBuild {
        report: background,
        contextual_uploads: options_background_uploads,
    } = rebuild_options_background(
        &config.assets,
        &source.source_bin_sha256,
        &source.menu_stored,
        &source.menu_decoded,
        &mut contextual_menu_decoded,
        &config.fonts.background,
    )?;
    let mut changed_menu_decoded = source.menu_decoded.clone();
    let (mut changed_overlay, text_units) =
        rebuild_options_text(&source.overlay, &build_units, &allocation)?;
    let records_main_codes = build_units
        .iter()
        .filter(|unit| records_main::contains_main_executable_unit(&unit.id))
        .map(|unit| Ok((unit.id.as_str(), encode_unit(unit, &allocation)?)))
        .collect::<Result<Vec<_>>>()?;
    let records_main_slices = records_main_codes
        .iter()
        .map(|(id, codes)| (*id, codes.as_slice()))
        .collect::<Vec<_>>();
    let records_main_executable =
        records_main::rebuild_main_executable(&source.main_executable, &records_main_slices)?;
    let records_runtime_codes = build_units
        .iter()
        .filter(|unit| super::records_runtime::contains_unit(&unit.id))
        .map(|unit| Ok((unit.id.as_str(), encode_unit(unit, &allocation)?)))
        .collect::<Result<Vec<_>>>()?;
    let records_runtime_slices = records_runtime_codes
        .iter()
        .map(|(id, codes)| (*id, codes.as_slice()))
        .collect::<Vec<_>>();
    let mut changed_optinfo_decoded = source.optinfo_decoded.clone();
    let description_rebuild = rebuild_options_descriptions(
        &mut changed_overlay,
        &mut changed_optinfo_decoded,
        &description_assets,
        &config.fonts.description,
    )?;
    let (records_runtime_executable, records_runtime) = rebuild_records_runtime(
        &source.overlay,
        &source.main_executable,
        &build_units,
        &records_runtime_slices,
    )?;
    let records_output_codes = records_main_codes
        .iter()
        .chain(records_runtime_codes.iter())
        .flat_map(|(_, codes)| codes.iter().copied())
        .collect::<Vec<_>>();
    let runtime_glyph_upload = build_contextual_glyph_upload(ContextualGlyphUploadInputs {
        source_menu_decoded: &source.menu_decoded,
        output_menu_decoded: &mut changed_menu_decoded,
        options_contextual_glyphs: &options_contextual_glyphs,
        options_background_uploads: &options_background_uploads,
        records_contextual_glyphs: &records_contextual_glyphs,
        records_output_codes: &records_output_codes,
        source_overlay: &source.overlay,
        output_overlay: &mut changed_overlay,
        source_optinfo_decoded: &source.optinfo_decoded,
        output_optinfo_decoded: &mut changed_optinfo_decoded,
        source_main_executable: &source.main_executable,
    })?;
    let RuntimeGlyphUploadBuild {
        report: runtime_glyph_upload_report,
        menu_expected_write_ranges,
        overlay_expected_write_ranges,
        optinfo_expected_write_ranges,
        main_executable_candidate: records_context_hooks,
        main_executable_machine_code_writes: records_context_hook_writes,
    } = runtime_glyph_upload;
    let main_executable_contributions = OptionsMainExecutableContributions {
        source_sha256: sha256_bytes(&source.main_executable),
        records_main: records_main_executable,
        records_runtime: records_runtime_executable,
        records_context_hooks,
        records_context_hook_writes,
    };
    let changed_main_executable =
        compose_options_main_executable(&source.main_executable, &main_executable_contributions)?;
    let menu_decoded_changed_byte_ranges =
        difference_ranges(&source.menu_decoded, &changed_menu_decoded);
    ensure!(
        !menu_decoded_changed_byte_ranges.is_empty(),
        "options MENU.BIZ build changed no bytes"
    );
    let mut allowed_menu_ranges = Vec::new();
    allowed_menu_ranges.extend(menu_expected_write_ranges.iter());
    ensure!(
        changed_ranges_are_within(&menu_decoded_changed_byte_ranges, &allowed_menu_ranges),
        "options MENU.BIZ build escaped its glyph, background, or contextual upload Expected Writes"
    );
    let options_glyph_pixel_ranges = glyphs
        .iter()
        .filter(|glyph| glyph.runtime_context == "options")
        .flat_map(|glyph| glyph.install.allowed_decoded_byte_ranges.iter().copied())
        .collect::<Vec<_>>();
    let options_background_pixel_ranges = background
        .regions
        .iter()
        .flat_map(|region| region.allowed_decoded_byte_ranges.iter().copied())
        .collect::<Vec<_>>();
    ensure!(
        !options_glyph_pixel_ranges.is_empty() && !options_background_pixel_ranges.is_empty(),
        "options contextual source-pixel preservation ranges are empty"
    );
    let options_source_menu_glyph_pixels_preserved = source_bytes_match_in_ranges(
        &source.menu_decoded,
        &changed_menu_decoded,
        &options_glyph_pixel_ranges,
    );
    let options_source_menu_background_pixels_preserved = source_bytes_match_in_ranges(
        &source.menu_decoded,
        &changed_menu_decoded,
        &options_background_pixel_ranges,
    );
    ensure!(
        options_source_menu_glyph_pixels_preserved,
        "final MENU retains an Options glyph pixel write"
    );
    ensure!(
        options_source_menu_background_pixels_preserved,
        "final MENU retains an Options background pixel write"
    );
    let mut menu_write_claims = Vec::new();
    menu_write_claims.extend(DecodedDataClaim::from_ranges(
        "menu:options:records-context",
        "store Records contextual glyph payload and typed programs",
        menu_expected_write_ranges.iter().copied(),
    ));
    menu_write_claims.retain(|claim| {
        source.menu_decoded[claim.range.clone()] != changed_menu_decoded[claim.range.clone()]
    });
    let main_executable_changed_byte_ranges =
        difference_ranges(&source.main_executable, &changed_main_executable);
    ensure!(
        !main_executable_changed_byte_ranges.is_empty(),
        "options main-executable contributors changed no bytes"
    );
    ensure!(
        changed_overlay.len() == source.overlay.len(),
        "options text rebuild changed NEWOPT.BIN length"
    );
    let text_overlay_ranges = options_text_expected_write_ranges(&build_units)?;
    let description_overlay_ranges = description_expected_write_ranges();
    let mut overlay_write_claims = DecodedDataClaim::from_effective_ranges(
        "options:overlay:text",
        "replace authored options text records",
        &source.overlay,
        &changed_overlay,
        text_overlay_ranges,
    )?;
    overlay_write_claims.extend(DecodedDataClaim::from_effective_ranges(
        "options:overlay:descriptions",
        "store options description streams and pointer tables",
        &source.overlay,
        &changed_overlay,
        description_overlay_ranges,
    )?);
    overlay_write_claims.extend(DecodedDataClaim::from_effective_ranges(
        "options:overlay:contextual-glyph-upload",
        "install the typed options contextual glyph uploader",
        &source.overlay,
        &changed_overlay,
        overlay_expected_write_ranges,
    )?);
    let source_overlay_sha256 = sha256_bytes(&source.overlay);
    let mut overlay_write_plan = DecodedRecordWritePlan::new(
        super::source::OVERLAY_PATH,
        &source.overlay,
        &source_overlay_sha256,
    )?;
    overlay_write_plan.register_data_candidate(
        "options overlay contributors",
        &source_overlay_sha256,
        &changed_overlay,
        &overlay_write_claims,
    )?;
    let planned_overlay = overlay_write_plan.apply(None)?;
    ensure!(
        planned_overlay == changed_overlay,
        "options overlay decoded plan omitted a contribution"
    );
    let changed_overlay = planned_overlay;
    let overlay_changed_byte_ranges = difference_ranges(&source.overlay, &changed_overlay);
    let mut optinfo_write_claims = Vec::new();
    for glyph in &description_rebuild.glyphs {
        let Some(install) = &glyph.install else {
            continue;
        };
        optinfo_write_claims.extend(DecodedDataClaim::from_effective_ranges(
            &format!(
                "options:optinfo:item-{}:cell-{}",
                glyph.item_index, glyph.atlas_cell
            ),
            &format!(
                "install options description glyph {:?} for item {}",
                glyph.character, glyph.item_index
            ),
            &source.optinfo_decoded,
            &changed_optinfo_decoded,
            install.allowed_decoded_byte_ranges.iter().copied(),
        )?);
    }
    optinfo_write_claims.extend(DecodedDataClaim::from_effective_ranges(
        "options:optinfo:contextual-glyph-upload",
        "store options contextual glyph upload payloads",
        &source.optinfo_decoded,
        &changed_optinfo_decoded,
        optinfo_expected_write_ranges,
    )?);
    let source_optinfo_decoded_sha256 = sha256_bytes(&source.optinfo_decoded);
    let mut optinfo_write_plan = DecodedRecordWritePlan::new(
        super::description_source::OPTINFO_PATH,
        &source.optinfo_decoded,
        &source_optinfo_decoded_sha256,
    )?;
    optinfo_write_plan.register_data_candidate(
        "options information contributors",
        &source_optinfo_decoded_sha256,
        &changed_optinfo_decoded,
        &optinfo_write_claims,
    )?;
    let planned_optinfo_decoded = optinfo_write_plan.apply(None)?;
    ensure!(
        planned_optinfo_decoded == changed_optinfo_decoded,
        "options information decoded plan omitted a contribution"
    );
    let changed_optinfo_decoded = planned_optinfo_decoded;
    let optinfo_decoded_changed_byte_ranges =
        difference_ranges(&source.optinfo_decoded, &changed_optinfo_decoded);

    let (reencoded_menu, source_compression, rebuilt_compression) =
        compress_menu_with_source_limits(&changed_menu_decoded, &source.menu_stored)?;
    ensure!(
        decompress(&reencoded_menu, false)? == changed_menu_decoded,
        "options MENU.BIZ compression roundtrip changed decoded bytes"
    );
    ensure!(
        reencoded_menu.len() <= source.menu_stored.len(),
        "options MENU.BIZ exceeds its original extent"
    );
    ensure!(
        reencoded_menu[..4] == source.menu_stored[..4],
        "options MENU.BIZ changed its catalog prefix"
    );
    let rebuilt_menu_stored_size = reencoded_menu.len();
    let mut padded_menu = reencoded_menu;
    padded_menu.resize(source.menu_stored.len(), 0);
    ensure!(
        decompress(&padded_menu, true)? == changed_menu_decoded,
        "padded options MENU.BIZ changed decoded bytes"
    );

    let (reencoded_optinfo, source_optinfo_compression, rebuilt_optinfo_compression) =
        compress_menu_with_source_limits(&changed_optinfo_decoded, &source.optinfo_stored)?;
    ensure!(
        decompress(&reencoded_optinfo, false)? == changed_optinfo_decoded,
        "options OPTINFO.TIZ compression roundtrip changed decoded bytes"
    );
    ensure!(
        reencoded_optinfo.len() <= source.optinfo_stored.len(),
        "options OPTINFO.TIZ exceeds its original extent"
    );
    ensure!(
        reencoded_optinfo[..4] == source.optinfo_stored[..4],
        "options OPTINFO.TIZ changed its catalog prefix"
    );
    let rebuilt_optinfo_stored_size = reencoded_optinfo.len();
    let mut padded_optinfo = reencoded_optinfo;
    padded_optinfo.resize(source.optinfo_stored.len(), 0);
    ensure!(
        decompress(&padded_optinfo, true)? == changed_optinfo_decoded,
        "padded options OPTINFO.TIZ changed decoded bytes"
    );

    let authored_unit_count = build_units.len();
    let release_approved_unit_count = build_units
        .iter()
        .filter(|unit| unit.release_status == OptionsReleaseStatus::Approved)
        .count();
    let description_item_count = description_assets.units.len();
    let description_release_approved_item_count = description_assets
        .units
        .iter()
        .filter(|unit| unit.release_status == OptionsReleaseStatus::Approved)
        .count();
    let development_input_available = authored_unit_count == build_units.len()
        && description_item_count == super::description_source::ITEM_COUNT
        && background.development_input_available;
    let translations_release_approved = release_approved_unit_count == build_units.len()
        && description_release_approved_item_count == description_item_count
        && background.release_candidate_input_eligible;
    let mut fonts = fonts;
    fonts.push(description_rebuild.font);
    let report = OptionsRecordBuildReport {
        build_spec_sha256: config.build_spec_sha256.clone(),
        source_bin_sha256: source.source_bin_sha256,
        source_menu_stored_sha256: sha256_bytes(&source.menu_stored),
        output_menu_stored_sha256: sha256_bytes(&padded_menu),
        source_menu_decoded_sha256: sha256_bytes(&source.menu_decoded),
        output_menu_decoded_sha256: sha256_bytes(&changed_menu_decoded),
        source_overlay_size: source.overlay.len(),
        source_overlay_sha256: sha256_bytes(&source.overlay),
        output_overlay_sha256: sha256_bytes(&changed_overlay),
        source_optinfo_stored_sha256: sha256_bytes(&source.optinfo_stored),
        output_optinfo_stored_sha256: sha256_bytes(&padded_optinfo),
        source_optinfo_decoded_sha256: sha256_bytes(&source.optinfo_decoded),
        output_optinfo_decoded_sha256: sha256_bytes(&changed_optinfo_decoded),
        source_main_executable_sha256: sha256_bytes(&source.main_executable),
        source_main_executable_size: source.main_executable.len(),
        output_main_executable_sha256: sha256_bytes(&changed_main_executable),
        translation_manifest_sha256: assets.manifest_sha256,
        description_manifest_sha256: description_assets.manifest_sha256,
        background,
        records_runtime,
        runtime_glyph_upload: runtime_glyph_upload_report,
        unit_count: build_units.len(),
        authored_unit_count,
        release_approved_unit_count,
        description_item_count,
        description_release_approved_item_count,
        development_input_available,
        translations_release_approved,
        allocation_status: "source MENU preserves all Options glyph and background pixels; Options uploads its compressed entry-scoped textures and reloads source MENU on exit; Records-only glyphs retain their independent entry/exit context".to_string(),
        allocation_proven_reclaimable: false,
        release_candidate_input_eligible: false,
        options_source_menu_glyph_pixels_preserved,
        options_source_menu_background_pixels_preserved,
        provisional_code_count: allocation.provisional_physical_codes.len(),
        menu_decoded_changed_byte_ranges,
        overlay_changed_byte_ranges,
        description_overlay_changed_byte_ranges: description_rebuild
            .overlay_changed_byte_ranges,
        optinfo_decoded_changed_byte_ranges,
        main_executable_changed_byte_ranges,
        original_menu_stored_size: source.menu_stored.len(),
        rebuilt_menu_stored_size,
        menu_padding_size: source.menu_stored.len() - rebuilt_menu_stored_size,
        source_compression_maximum_match_words: source_compression.maximum_match_words,
        source_compression_maximum_control_block_output_words: source_compression
            .maximum_control_block_output_words,
        rebuilt_compression_maximum_match_words: rebuilt_compression.maximum_match_words,
        rebuilt_compression_maximum_control_block_output_words: rebuilt_compression
            .maximum_control_block_output_words,
        original_optinfo_stored_size: source.optinfo_stored.len(),
        rebuilt_optinfo_stored_size,
        optinfo_padding_size: source.optinfo_stored.len() - rebuilt_optinfo_stored_size,
        source_optinfo_compression_maximum_match_words: source_optinfo_compression
            .maximum_match_words,
        source_optinfo_compression_maximum_control_block_output_words: source_optinfo_compression
            .maximum_control_block_output_words,
        rebuilt_optinfo_compression_maximum_match_words: rebuilt_optinfo_compression
            .maximum_match_words,
        rebuilt_optinfo_compression_maximum_control_block_output_words:
            rebuilt_optinfo_compression.maximum_control_block_output_words,
        fonts,
        glyphs,
        text_units,
        description_glyphs: description_rebuild.glyphs,
        descriptions: description_rebuild.descriptions,
        menu_output_file: MENU_OUTPUT_FILE.to_string(),
        overlay_output_file: OVERLAY_OUTPUT_FILE.to_string(),
        optinfo_output_file: OPTINFO_OUTPUT_FILE.to_string(),
        main_executable_output_file: MAIN_EXECUTABLE_OUTPUT_FILE.to_string(),
    };
    let build_manifest_sha256 = write_record_outputs(
        &config.output_dir,
        &padded_menu,
        &changed_overlay,
        &padded_optinfo,
        &changed_main_executable,
        &report,
    )?;
    Ok(OptionsRecordBuild {
        cue: source.cue,
        source_menu_stored: source.menu_stored,
        source_menu_decoded: source.menu_decoded,
        menu_stored: padded_menu,
        menu_decoded: changed_menu_decoded,
        menu_write_claims,
        overlay: changed_overlay,
        optinfo_stored: padded_optinfo,
        optinfo_decoded: changed_optinfo_decoded,
        source_main_executable: source.main_executable,
        main_executable: changed_main_executable,
        main_executable_contributions,
        build_manifest_sha256,
        report,
    })
}

fn source_bytes_match_in_ranges(source: &[u8], output: &[u8], ranges: &[[usize; 2]]) -> bool {
    ranges.iter().all(|[start, end]| {
        source
            .get(*start..*end)
            .zip(output.get(*start..*end))
            .is_some_and(|(source_bytes, output_bytes)| source_bytes == output_bytes)
    })
}

fn install_glyphs(
    menu_font_tim: &mut [u8],
    options_allocation: &[GlyphAllocation],
    records_contextual_allocation: &[GlyphAllocation],
    styles: &super::model::OptionsFontStyles,
) -> Result<InstalledOptionsGlyphs> {
    let mut font_identities = BTreeMap::new();
    let mut rasterizers = IndexedTextRasterizers::default();
    let mut glyphs =
        Vec::with_capacity(options_allocation.len() + records_contextual_allocation.len());
    let mut options_contextual = Vec::with_capacity(options_allocation.len());
    let mut records_contextual = Vec::with_capacity(records_contextual_allocation.len());
    for (glyph, runtime_context) in options_allocation
        .iter()
        .map(|glyph| (glyph, "options"))
        .chain(
            records_contextual_allocation
                .iter()
                .map(|glyph| (glyph, "records")),
        )
    {
        let style = font_style(styles, glyph.role);
        ensure!(
            style.font_px.is_finite() && style.font_px > 0.0,
            "invalid {} font size",
            glyph.role.key()
        );
        let rendered = rasterizers.for_font(&style.font)?.rasterize(
            &glyph.character.to_string(),
            glyph.cell.width,
            glyph.cell.height,
            style.font_px,
            0.0,
            CLEAR_INDEX,
            Some(OUTLINE_INDEX),
            FILL_INDEX,
            HorizontalTextAlignment::Center,
        )?;
        match font_identities.entry(glyph.role) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert((rendered.font_name.clone(), rendered.font_sha256.clone()));
            }
            std::collections::btree_map::Entry::Occupied(entry) => ensure!(
                entry.get() == &(rendered.font_name.clone(), rendered.font_sha256.clone()),
                "{} font identity changed within one build",
                glyph.role.key()
            ),
        }
        let target = format!(
            "development-only {} glyph {:?}",
            glyph.role.key(),
            glyph.character
        );
        let install = install_indexed_glyph(menu_font_tim, glyph.cell, &rendered.pixels, &target)?;
        let prepared = PreparedGlyph {
            role: glyph.role.key().to_string(),
            character: Some(glyph.character),
            code: glyph.code,
            cell: glyph.cell,
            source_preservation: false,
            payload: pack_indexed_pixels(&rendered.pixels)?,
        };
        if runtime_context == "options" {
            options_contextual.push(prepared);
        } else {
            records_contextual.push(prepared);
        }
        glyphs.push(OptionsGlyphBuild {
            role: glyph.role.key().to_string(),
            character: glyph.character,
            code: format!("0x{:04x}", glyph.code),
            cell: glyph.cell,
            ink_bounds: rendered.ink_bounds,
            install,
            global_menu_resident: false,
            runtime_context: runtime_context.to_string(),
        });
    }
    let fonts = font_identities
        .into_iter()
        .map(|(role, (font_name, font_sha256))| OptionsFontBuild {
            role: role.key().to_string(),
            font_name,
            font_sha256,
            font_px: font_style(styles, role).font_px,
        })
        .collect();
    options_contextual.sort_by_key(|glyph| glyph.code);
    records_contextual.sort_by_key(|glyph| glyph.code);
    Ok(InstalledOptionsGlyphs {
        fonts,
        reports: glyphs,
        options_contextual,
        records_contextual,
    })
}

fn font_style(
    styles: &super::model::OptionsFontStyles,
    role: OptionsFontRole,
) -> &OptionsFontStyle {
    match role {
        OptionsFontRole::Heading => &styles.heading,
        OptionsFontRole::Help => &styles.help,
        OptionsFontRole::Label => &styles.label,
        OptionsFontRole::Value => &styles.value,
        OptionsFontRole::Action => &styles.action,
        OptionsFontRole::RecordsMainHeading => &styles.records_main.heading,
        OptionsFontRole::RecordsMainItem => &styles.records_main.item,
        OptionsFontRole::RecordsPrompt => &styles.records_prompt,
        OptionsFontRole::RecordsStatusHeading => &styles.records_status.heading,
        OptionsFontRole::RecordsStatusMessage => &styles.records_status.message,
    }
}

fn prepare_record_outputs(output_dir: &Path, force: bool) -> Result<()> {
    std::fs::create_dir_all(output_dir)?;
    for name in [
        MENU_OUTPUT_FILE,
        OVERLAY_OUTPUT_FILE,
        "records-kanri.bin",
        "records-main-slps.bin",
        OPTINFO_OUTPUT_FILE,
        MAIN_EXECUTABLE_OUTPUT_FILE,
        RECORD_BUILD_MANIFEST_FILE,
    ] {
        let path = output_dir.join(name);
        if path.exists() && !force {
            bail!("options record output exists; pass --force to replace it");
        }
        if path.exists() {
            std::fs::remove_file(&path)
                .with_context(|| format!("failed to remove {}", path.display()))?;
        }
    }
    Ok(())
}

fn write_record_outputs(
    output_dir: &Path,
    menu_stored: &[u8],
    overlay: &[u8],
    optinfo_stored: &[u8],
    main_executable: &[u8],
    report: &OptionsRecordBuildReport,
) -> Result<String> {
    std::fs::write(output_dir.join(MENU_OUTPUT_FILE), menu_stored)?;
    std::fs::write(output_dir.join(OVERLAY_OUTPUT_FILE), overlay)?;
    std::fs::write(output_dir.join(OPTINFO_OUTPUT_FILE), optinfo_stored)?;
    std::fs::write(
        output_dir.join(MAIN_EXECUTABLE_OUTPUT_FILE),
        main_executable,
    )?;
    let manifest = OptionsRecordManifest {
        kind: "Justice Gakuen 2 source-bound options record build",
        records: report,
    };
    write_pretty_json_and_hash(
        &output_dir.join(RECORD_BUILD_MANIFEST_FILE),
        &manifest,
        true,
    )
}

#[derive(Serialize)]
struct OptionsRecordManifest<'a> {
    kind: &'static str,
    #[serde(flatten)]
    records: &'a OptionsRecordBuildReport,
}
