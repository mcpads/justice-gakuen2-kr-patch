use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::{Context, Result, ensure};
use serde::Serialize;

use crate::compression::decompress;
use crate::font::rasterize_menu_glyphs;
use crate::name_input::{
    DIGIT_KEYS, LATIN_KEYS, NAME_GLYPH_PACK_STORAGE_BYTES, NAME_INPUT_REDISPLAY_RUNTIME_ORIGIN,
    NAME_INPUT_RUNTIME_ORIGIN, NameGlyphBandBuildReport, NameGlyphPackRuntimeLookupInstallReport,
    NameGlyphPackTimInstallReport, NameInputRedisplayRuntimeProgramReport,
    NameInputRuntimeAtlasLayout, NameInputRuntimeProgramReport, SYMBOL_KEYS, SelectorAsciiGlyphs,
    build_name_glyph_band_pack, build_name_input_band_redisplay_runtime_program,
    build_name_input_band_runtime_program, build_shared_name_outline_runtime_program,
    encode_name_glyph_pack_cells, install_name_glyph_pack_auxiliary,
    install_name_glyph_pack_cells_in_tim, install_name_glyph_pack_runtime_lookup,
    load_name_input_keyboard, plan_name_glyph_consumer_layout, plan_name_input_runtime_atlas,
};
use crate::pipeline::{difference_ranges, sha256_bytes};
use crate::source_disc::SupportedSourceDisc;
use crate::tim::{install_indexed_glyph_in_prefix, parse_4bpp_prefix};

use super::name_entry::{OVERLAY_PATH, load_supported_name_entry_overlay_from_source};
use super::name_entry_build::prepare_output_directory;
use super::name_entry_confirmation_hook::NameEntryConfirmationHookReport;
use super::name_entry_delete_handler::NameEntryDeleteHandlerReport;
use super::name_entry_fixed_graphics_build::validate_name_input_page_labels;
use super::name_entry_fixed_graphics_model::DialogueNameEntryFixedGraphicBuildReport;
use super::name_entry_font_build::{
    BACKGROUND_TIM_OFFSET, FONT_TIM_OFFSET, LAYOUT_TABLE_OFFSET, LAYOUT_TABLE_SHA256,
    NAME_ENTRY_FONT_OUTPUT_FILE, NAME_ENTRY_FONT_PATH, SOURCE_DECODED_SHA256, SOURCE_DECODED_SIZE,
    SOURCE_STORED_SHA256, install_all_name_entry_fixed_graphics, load_all_name_entry_glyph_cells,
};
use super::name_entry_keyboard_build::KoreanNameKeyboardOverlayReport;
use super::name_entry_nickname_companion::NameEntryNicknameCompanionReport;
use super::name_entry_nickname_hangul_page::NameEntryNicknameHangulPageReport;
use super::name_entry_overlay_compositor::{
    ComposedNameEntryOverlayBuild, compose_name_entry_overlay,
};
use super::name_entry_record_compositor::{
    NameEntryRecordClaim, NameEntryRecordContribution, compose_name_entry_record,
    effective_data_claims,
};
use super::name_entry_redisplay_hook::NameEntryRedisplayHookReport;
use super::name_entry_redisplay_runtime::{
    NameEntryRedisplayRuntimeRegionReport, REDISPLAY_RUNTIME_REGION_OFFSET,
    audit_name_entry_redisplay_runtime_region, install_name_entry_redisplay_runtime_program,
};
use super::name_entry_runtime_code::{
    NameEntryRuntimeCodeRegionAudit, RUNTIME_CODE_REGION_OFFSET,
    audit_name_entry_runtime_code_region, install_name_entry_runtime_program,
};
use super::name_entry_selection_hook::NameEntrySelectionHookReport;
use super::name_entry_slot_navigation_guard::NameEntrySlotNavigationGuardReport;
use crate::paged_compression::{profile_paged_compression, source_paged_compression_profile};

const PATCHED_OVERLAY_FILE: &str = "MGENT.BIN";
pub(super) const BUILD_MANIFEST_FILE: &str = "composed-name-input-build.json";
const OUTLINE_PALETTE_INDEX: u8 = 3;
const FILL_PALETTE_INDEX: u8 = 13;

#[derive(Clone, Debug)]
pub struct ComposedNameInputBuildConfig {
    pub cue: PathBuf,
    pub keyboard: PathBuf,
    pub graphics: PathBuf,
    pub keyboard_font: PathBuf,
    pub keyboard_font_px: f32,
    pub composed_glyph_font: PathBuf,
    pub composed_glyph_font_px: f32,
    pub fixed_graphics_font: PathBuf,
    pub roster_font: PathBuf,
    pub roster_font_px: f32,
    pub output_dir: PathBuf,
    pub force: bool,
}

#[derive(Debug, Serialize)]
pub struct ComposedNameInputBuildReport {
    pub kind: String,
    pub source_bin_sha256: String,
    pub source_overlay_path: String,
    pub source_overlay_size: usize,
    pub source_overlay_sha256: String,
    pub keyboard_spec_path: String,
    pub keyboard_spec_sha256: String,
    pub keyboard_overlay: KoreanNameKeyboardOverlayReport,
    pub nickname_hangul_page: NameEntryNicknameHangulPageReport,
    pub nickname_companion: NameEntryNicknameCompanionReport,
    pub shared_outline_runtime: crate::name_input::SharedNameOutlineRuntimeProgramReport,
    pub selection_hook: NameEntrySelectionHookReport,
    pub delete_handler: NameEntryDeleteHandlerReport,
    pub slot_navigation_guard: NameEntrySlotNavigationGuardReport,
    pub confirmation_hook: NameEntryConfirmationHookReport,
    pub redisplay_hook: NameEntryRedisplayHookReport,
    pub patched_overlay_file: String,
    pub patched_overlay_sha256: String,
    pub font: ComposedNameInputFontBuildReport,
    pub static_asset_build_complete: bool,
    pub runtime_integration_complete: bool,
    pub runtime_consumer_verified: bool,
    pub release_candidate_eligible: bool,
}

#[derive(Debug)]
pub struct ComposedNameInputBuild {
    pub(crate) selector_runtime: crate::name_input::SelectorRuntimeInstallation,
    pub(crate) direct_glyphs: Vec<crate::name_input::NameInputDirectGlyphSource>,
    pub font_stored: Vec<u8>,
    pub overlay: Vec<u8>,
    pub manifest_sha256: String,
    pub report: ComposedNameInputBuildReport,
}

#[derive(Debug, Serialize)]
pub struct SelectorAsciiStorageReport {
    pub storage_byte_range: Option<[usize; 2]>,
    pub sha256: String,
    pub glyph_count: usize,
    pub crop: [usize; 4],
    pub tim_readback_verified: bool,
}

#[derive(Debug, Serialize)]
pub struct ComposedNameInputFontBuildReport {
    pub source_path: String,
    pub source_stored_sha256: String,
    pub source_decoded_sha256: String,
    pub source_record_size: usize,
    pub keyboard_font_name: String,
    pub keyboard_font_sha256: String,
    pub keyboard_font_px: f32,
    pub installed_key_count: usize,
    pub cleared_cache_cell_count: usize,
    pub glyph_pack: NameGlyphBandBuildReport,
    pub storage_tim_install: NameGlyphPackTimInstallReport,
    pub selector_ascii: SelectorAsciiStorageReport,
    pub roster_font: crate::name_input::RosterNameFontReport,
    pub runtime_lookup_install: NameGlyphPackRuntimeLookupInstallReport,
    pub runtime_coordinate_list_storage_byte_range: [usize; 2],
    pub runtime_atlas: NameInputRuntimeAtlasLayout,
    pub runtime_code_region: NameEntryRuntimeCodeRegionAudit,
    pub runtime_program: NameInputRuntimeProgramReport,
    pub redisplay_runtime_region: NameEntryRedisplayRuntimeRegionReport,
    pub redisplay_runtime_program: NameInputRedisplayRuntimeProgramReport,
    pub embedded_font_tim_offset: String,
    pub embedded_font_tim_size: usize,
    pub glyph_layout_table_offset: String,
    pub glyph_layout_table_sha256: String,
    pub fixed_graphics: Vec<DialogueNameEntryFixedGraphicBuildReport>,
    pub changed_decoded_byte_count: usize,
    pub patched_decoded_sha256: String,
    pub compressed_stream_size: usize,
    pub patched_stored_sha256: String,
    pub compressed_within_original_extent: bool,
    pub compression_roundtrip_verified: bool,
}

pub fn build_composed_name_input(
    config: &ComposedNameInputBuildConfig,
) -> Result<ComposedNameInputBuild> {
    let source = SupportedSourceDisc::open(&config.cue)?;
    build_composed_name_input_from_source(config, &source)
}

pub(crate) fn build_composed_name_input_from_source(
    config: &ComposedNameInputBuildConfig,
    source: &SupportedSourceDisc,
) -> Result<ComposedNameInputBuild> {
    prepare_output_directory(&config.output_dir, config.force)?;
    let (source_overlay, source_bin_sha256, source_overlay_sha256) =
        load_supported_name_entry_overlay_from_source(source)?;
    let keyboard = load_name_input_keyboard(&config.keyboard)?;
    let consumer_layout = plan_name_glyph_consumer_layout(&keyboard)?;
    let glyph_pack = build_name_glyph_band_pack(
        &config.composed_glyph_font,
        config.composed_glyph_font_px,
        NAME_GLYPH_PACK_STORAGE_BYTES,
    )?;
    let shared_outline_runtime = build_shared_name_outline_runtime_program()?;
    let glyph_cells = load_all_name_entry_glyph_cells(&source_overlay)?;
    let runtime_atlas = plan_name_input_runtime_atlas(
        &glyph_cells
            .iter()
            .map(|glyph| glyph.cell)
            .collect::<Vec<_>>(),
        &keyboard,
    )?;
    let runtime_program = build_name_input_band_runtime_program(
        &runtime_atlas,
        &glyph_pack.pack,
        &shared_outline_runtime,
        crate::name_input::NICKNAME_HUD_GLYPH_STORE_ORIGIN,
    )?;
    let materializer_address = runtime_program
        .glyph_fill_materializer_address
        .context("stored-atlas runtime omitted its glyph materializer")?;
    let mut redisplay_runtime_program = build_name_input_band_redisplay_runtime_program(
        &runtime_atlas,
        materializer_address,
        &glyph_pack.pack,
    )?;
    let ComposedNameEntryOverlayBuild {
        bytes: overlay,
        keyboard: keyboard_overlay,
        nickname_hangul_page,
        nickname_companion,
        selection_hook,
        delete_handler,
        slot_navigation_guard,
        confirmation_hook,
        redisplay_hook,
    } = compose_name_entry_overlay(
        &source_overlay,
        &source_overlay_sha256,
        &keyboard,
        &consumer_layout,
        &mut redisplay_runtime_program,
    )?;
    let patched_overlay_sha256 = sha256_bytes(&overlay);
    let (patched_font, font, selector_runtime) =
        build_composed_name_input_font(ComposedNameInputFontInputs {
            config,
            source,
            source_overlay: &source_overlay,
            keyboard: &keyboard,
            runtime_program: &runtime_program,
            redisplay_runtime_program: &redisplay_runtime_program,
            runtime_atlas,
            glyph_pack,
        })?;

    std::fs::write(config.output_dir.join(PATCHED_OVERLAY_FILE), &overlay)?;
    std::fs::write(
        config.output_dir.join(NAME_ENTRY_FONT_OUTPUT_FILE),
        &patched_font,
    )?;
    let report = ComposedNameInputBuildReport {
        kind: "Justice Gakuen 2 composed Korean name-input development build".to_string(),
        source_bin_sha256,
        source_overlay_path: OVERLAY_PATH.to_string(),
        source_overlay_size: source_overlay.len(),
        source_overlay_sha256,
        keyboard_spec_path: keyboard.spec_path.clone(),
        keyboard_spec_sha256: keyboard.spec_sha256.clone(),
        keyboard_overlay,
        nickname_hangul_page,
        nickname_companion,
        shared_outline_runtime: shared_outline_runtime.report,
        selection_hook,
        delete_handler,
        slot_navigation_guard,
        confirmation_hook,
        redisplay_hook,
        patched_overlay_file: PATCHED_OVERLAY_FILE.to_string(),
        patched_overlay_sha256,
        font,
        static_asset_build_complete: true,
        runtime_integration_complete: false,
        runtime_consumer_verified: false,
        release_candidate_eligible: false,
    };
    let mut report_bytes = serde_json::to_vec_pretty(&report)?;
    report_bytes.push(b'\n');
    let manifest_sha256 = sha256_bytes(&report_bytes);
    std::fs::write(config.output_dir.join(BUILD_MANIFEST_FILE), report_bytes)?;
    let direct_glyphs = consumer_layout
        .active_glyphs
        .iter()
        .filter_map(|key| {
            let c = key.character.chars().next()?;
            c.is_ascii().then_some((key, c as u8))
        })
        .map(|(key, character)| {
            let cell = glyph_cells[key.selectable_sequence_position].cell;
            Ok(crate::name_input::NameInputDirectGlyphSource {
                legacy_code: key.code,
                character,
                pixel_byte_offset: u16::try_from(cell.y * 384 + cell.x / 2)?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(ComposedNameInputBuild {
        selector_runtime,
        direct_glyphs,
        font_stored: patched_font,
        overlay,
        manifest_sha256,
        report,
    })
}

struct ComposedNameInputFontInputs<'a> {
    config: &'a ComposedNameInputBuildConfig,
    source: &'a SupportedSourceDisc,
    source_overlay: &'a [u8],
    keyboard: &'a crate::name_input::NameInputKeyboardPlan,
    runtime_program: &'a crate::name_input::NameInputRuntimeProgram,
    redisplay_runtime_program: &'a crate::name_input::NameInputRedisplayRuntimeProgram,
    runtime_atlas: NameInputRuntimeAtlasLayout,
    glyph_pack: crate::name_input::NameGlyphBandBuild,
}

fn build_composed_name_input_font(
    inputs: ComposedNameInputFontInputs<'_>,
) -> Result<(
    Vec<u8>,
    ComposedNameInputFontBuildReport,
    crate::name_input::SelectorRuntimeInstallation,
)> {
    let ComposedNameInputFontInputs {
        config,
        source,
        source_overlay,
        keyboard,
        runtime_program,
        redisplay_runtime_program,
        runtime_atlas,
        glyph_pack,
    } = inputs;
    let (_, source_stored) = source.read_record(NAME_ENTRY_FONT_PATH)?;
    let source_stored_sha256 = sha256_bytes(&source_stored);
    ensure!(
        source_stored_sha256 == SOURCE_STORED_SHA256,
        "unsupported {NAME_ENTRY_FONT_PATH} SHA-256: {source_stored_sha256}"
    );
    let source_decoded = decompress(&source_stored, true)?;
    let source_decoded_sha256 = sha256_bytes(&source_decoded);
    ensure!(
        source_decoded.len() == SOURCE_DECODED_SIZE
            && source_decoded_sha256 == SOURCE_DECODED_SHA256,
        "unsupported decoded {NAME_ENTRY_FONT_PATH} identity"
    );
    let font_tim = parse_4bpp_prefix(
        source_decoded
            .get(FONT_TIM_OFFSET..)
            .context("name-entry font TIM offset is truncated")?,
    )?;
    let background_tim = parse_4bpp_prefix(
        source_decoded
            .get(BACKGROUND_TIM_OFFSET..)
            .context("name-entry background TIM is truncated")?,
    )?;
    ensure!(
        font_tim.image_x == 768
            && font_tim.image_y == 0
            && font_tim.pixel_width() == 768
            && font_tim.image_height == 256
            && background_tim.total_size > FONT_TIM_OFFSET,
        "name-entry embedded TIM geometry changed"
    );
    let cells = load_all_name_entry_glyph_cells(source_overlay)?;
    let layout_cells = cells.iter().map(|glyph| glyph.cell).collect::<Vec<_>>();

    let key_characters = keyboard
        .pages
        .iter()
        .flat_map(|page| &page.assignments)
        .filter_map(|assignment| {
            let character = assignment.character.chars().next()?;
            (character != ' ').then_some(character)
        })
        .collect::<String>();
    let rasterized = rasterize_menu_glyphs(
        &config.keyboard_font,
        &key_characters,
        config.keyboard_font_px,
        OUTLINE_PALETTE_INDEX,
        FILL_PALETTE_INDEX,
    )?;
    let keyboard_font_name = rasterized.font_name;
    let keyboard_font_sha256 = rasterized.font_sha256;
    let key_pixels = rasterized
        .glyphs
        .into_iter()
        .map(|glyph| (glyph.character, glyph.pixels))
        .collect::<BTreeMap<_, _>>();

    let mut keyboard_candidate = source_decoded.clone();
    let mut installed_key_count = 0_usize;
    let mut page_start = 0_usize;
    for page in &keyboard.pages {
        for assignment in &page.assignments {
            let sequence_position = page_start + assignment.page_position;
            let cell = cells
                .get(sequence_position)
                .context("name-input key layout position is out of range")?
                .cell;
            let character = assignment
                .character
                .chars()
                .next()
                .context("name-input key is empty")?;
            ensure!(
                assignment.character.chars().count() == 1,
                "name-input key contains multiple characters"
            );
            let blank = vec![0_u8; cell.width * cell.height];
            let pixels = if character == ' ' {
                &blank
            } else {
                key_pixels
                    .get(&character)
                    .context("rasterized name-input key disappeared")?
            };
            install_indexed_glyph_in_prefix(
                &mut keyboard_candidate,
                FONT_TIM_OFFSET,
                cell,
                pixels,
                "composed Korean name-input key",
            )?;
            installed_key_count += 1;
        }
        page_start += page.source_selectable_capacity;
    }

    let blank_cell = vec![0_u8; 20 * 20];
    for slot in &keyboard.cache.slots {
        let cell = cells[slot.atlas_layout_record_index].cell;
        install_indexed_glyph_in_prefix(
            &mut keyboard_candidate,
            FONT_TIM_OFFSET,
            cell,
            &blank_cell,
            "persistent composed-name glyph cache",
        )?;
    }

    let mut storage_image =
        encode_name_glyph_pack_cells(glyph_pack.pack.bytes(), &keyboard.glyph_pack_storage)?;
    let atlas_lookup_byte_count = runtime_atlas.lookup_table_bytes.len();
    let runtime_metadata = runtime_atlas.lookup_table_bytes.clone();
    let runtime_lookup_install =
        install_name_glyph_pack_runtime_lookup(&mut storage_image, &runtime_metadata)?;
    let runtime_coordinate_list_storage_byte_range = [
        runtime_lookup_install.storage_byte_range[0] + atlas_lookup_byte_count,
        runtime_lookup_install.storage_byte_range[1],
    ];
    ensure!(
        runtime_coordinate_list_storage_byte_range[1]
            - runtime_coordinate_list_storage_byte_range[0]
            == 0,
        "name glyph runtime coordinate-list storage range changed"
    );
    let ascii_reference = rasterize_menu_glyphs(
        &config.composed_glyph_font,
        &format!("{LATIN_KEYS}{DIGIT_KEYS}{SYMBOL_KEYS}").replace(' ', ""),
        config.composed_glyph_font_px,
        OUTLINE_PALETTE_INDEX,
        FILL_PALETTE_INDEX,
    )?;
    let ascii_start = glyph_pack.pack.bytes().len().next_multiple_of(4);
    let ascii_capacity = runtime_lookup_install.storage_byte_range[0]
        .checked_sub(ascii_start)
        .context("selector ASCII has no storage before coordinate metadata")?;
    // Selector-only ASCII is supplied by its resident SELP1 texture resource.
    // It must not compete with the lossless Hangul pack in the input font cells.
    let ascii = SelectorAsciiGlyphs::build(&ascii_reference, 0x1600)?;
    let stored_in_font = ascii.bytes.len() <= ascii_capacity;
    if stored_in_font {
        install_name_glyph_pack_auxiliary(&mut storage_image, ascii_start, &ascii.bytes)?;
    }
    let selector_ascii = SelectorAsciiStorageReport {
        storage_byte_range: if stored_in_font {
            Some([ascii_start, ascii_start + ascii.bytes.len()])
        } else {
            None
        },
        sha256: sha256_bytes(&ascii.bytes),
        glyph_count: ascii.glyph_count,
        crop: ascii.crop,
        tim_readback_verified: stored_in_font,
    };
    let mut selector_runtime = crate::name_input::SelectorRuntimeInstallation::build(
        &runtime_atlas,
        &glyph_pack.pack,
        &ascii,
    )?;
    let mut glyph_pack_candidate = source_decoded.clone();
    let storage_tim_install = install_name_glyph_pack_cells_in_tim(
        &mut glyph_pack_candidate,
        FONT_TIM_OFFSET,
        &layout_cells,
        &storage_image,
    )?;

    let mut runtime_code_region = audit_name_entry_runtime_code_region(&source_decoded)?;
    let mut runtime_candidate = source_decoded.clone();
    install_name_entry_runtime_program(&mut runtime_candidate, &runtime_program.bytes)?;
    runtime_code_region.typed_code_installed = true;
    let mut redisplay_runtime_region =
        audit_name_entry_redisplay_runtime_region(&source_decoded, font_tim.total_size)?;
    let mut redisplay_runtime_candidate = source_decoded.clone();
    install_name_entry_redisplay_runtime_program(
        &source_decoded,
        &mut redisplay_runtime_candidate,
        &redisplay_runtime_program.bytes,
    )?;
    redisplay_runtime_region.typed_code_installed = true;
    let mut redisplay_runtime_program_report = redisplay_runtime_program.report.clone();
    redisplay_runtime_program_report.installed = true;

    let mut fixed_graphics_candidate = source_decoded.clone();
    let fixed_graphics = install_all_name_entry_fixed_graphics(
        source_overlay,
        &source_decoded,
        &mut fixed_graphics_candidate,
        &config.graphics,
        &config.fixed_graphics_font,
        &layout_cells,
    )?;
    let page_labels = keyboard
        .pages
        .iter()
        .map(|page| (page.role.as_str(), page.label.as_str()))
        .collect::<Vec<_>>();
    validate_name_input_page_labels(&fixed_graphics, &page_labels)?;
    super::name_entry_control_labels::validate_hint_graphic(&fixed_graphics)?;
    let mut runtime_claims = if runtime_program.instruction_offset == 0 {
        Vec::new()
    } else {
        effective_data_claims(
            "name-entry-font:runtime-prefix",
            "install name-input runtime lookup data",
            &source_decoded,
            &runtime_candidate,
            [[
                RUNTIME_CODE_REGION_OFFSET,
                RUNTIME_CODE_REGION_OFFSET + runtime_program.instruction_offset,
            ]],
        )?
    };
    if let Some(range) = runtime_program.report.redisplay_cache_tag_byte_range {
        runtime_claims.extend(effective_data_claims(
            "name-entry-font:redisplay-cache-tags",
            "initialize per-slot glyph identity tags with the font and cache pixels",
            &source_decoded,
            &runtime_candidate,
            [range.map(|offset| RUNTIME_CODE_REGION_OFFSET + offset)],
        )?);
    }
    if let Some(range) = runtime_program.report.medial_transition_table_byte_range {
        runtime_claims.extend(effective_data_claims(
            "name-entry-font:medial-transitions",
            "compose selected Hangul medials",
            &source_decoded,
            &runtime_candidate,
            [range.map(|offset| RUNTIME_CODE_REGION_OFFSET + offset)],
        )?);
    }
    let runtime_instruction_start = RUNTIME_CODE_REGION_OFFSET + runtime_program.instruction_offset;
    let runtime_instruction_end =
        runtime_instruction_start + runtime_program.report.instruction_byte_count;
    runtime_claims.push(NameEntryRecordClaim::MachineCode {
        id: "name-entry-font:input-runtime".to_string(),
        purpose: "execute composed-name input and glyph materialization".to_string(),
        range: runtime_instruction_start..runtime_instruction_end,
        runtime_address: NAME_INPUT_RUNTIME_ORIGIN
            + u32::try_from(runtime_program.instruction_offset)?,
        instructions: runtime_program.instructions.clone(),
    });
    let mut redisplay_runtime_claims = effective_data_claims(
        "name-entry-font:redisplay-tables",
        "install composed-name redisplay lookup tables",
        &source_decoded,
        &redisplay_runtime_candidate,
        [
            redisplay_runtime_program
                .report
                .compound_final_table_byte_range
                .map(|offset| REDISPLAY_RUNTIME_REGION_OFFSET + offset),
            redisplay_runtime_program
                .report
                .simple_final_table_byte_range
                .map(|offset| REDISPLAY_RUNTIME_REGION_OFFSET + offset),
            redisplay_runtime_program
                .report
                .cache_cell_table_byte_range
                .map(|offset| REDISPLAY_RUNTIME_REGION_OFFSET + offset),
        ],
    )?;
    redisplay_runtime_claims.push(NameEntryRecordClaim::MachineCode {
        id: "name-entry-font:redisplay-runtime".to_string(),
        purpose: "materialize tagged names before redisplay".to_string(),
        range: REDISPLAY_RUNTIME_REGION_OFFSET
            ..REDISPLAY_RUNTIME_REGION_OFFSET
                + redisplay_runtime_program.report.instruction_byte_count,
        runtime_address: NAME_INPUT_REDISPLAY_RUNTIME_ORIGIN,
        instructions: redisplay_runtime_program.instructions.clone(),
    });
    let mut patched_decoded = compose_name_entry_record(
        NAME_ENTRY_FONT_PATH,
        &source_decoded,
        &source_decoded_sha256,
        vec![
            NameEntryRecordContribution {
                owner: "name-entry keyboard glyph renderer",
                candidate: &keyboard_candidate,
                claims: effective_data_claims(
                    "name-entry-font:keyboard-glyphs",
                    "render keyboard keys and clear persistent cache cells",
                    &source_decoded,
                    &keyboard_candidate,
                    difference_ranges(&source_decoded, &keyboard_candidate),
                )?,
            },
            NameEntryRecordContribution {
                owner: "name-entry glyph pack storage",
                candidate: &glyph_pack_candidate,
                claims: effective_data_claims(
                    "name-entry-font:glyph-pack",
                    "store composed Hangul glyph data and runtime lookup metadata",
                    &source_decoded,
                    &glyph_pack_candidate,
                    difference_ranges(&source_decoded, &glyph_pack_candidate),
                )?,
            },
            NameEntryRecordContribution {
                owner: "name-entry fixed graphics renderer",
                candidate: &fixed_graphics_candidate,
                claims: effective_data_claims(
                    "name-entry-font:fixed-graphics",
                    "render fixed Korean name-entry graphics",
                    &source_decoded,
                    &fixed_graphics_candidate,
                    difference_ranges(&source_decoded, &fixed_graphics_candidate),
                )?,
            },
            NameEntryRecordContribution {
                owner: "name-entry input runtime",
                candidate: &runtime_candidate,
                claims: runtime_claims,
            },
            NameEntryRecordContribution {
                owner: "name-entry redisplay runtime",
                candidate: &redisplay_runtime_candidate,
                claims: redisplay_runtime_claims,
            },
        ],
    )?;
    let changed_decoded_byte_count = source_decoded
        .iter()
        .zip(&patched_decoded)
        .filter(|(source, patched)| source != patched)
        .count();
    ensure!(
        changed_decoded_byte_count > 0,
        "composed name-input font build changed no decoded bytes"
    );

    let roster_font =
        crate::name_input::RosterNameFont::build(&config.roster_font, config.roster_font_px)?;
    roster_font.append_to(&mut patched_decoded)?;
    let roster_report = roster_font.report.clone();
    selector_runtime.install_roster_font(roster_font)?;
    let source_compression_profile = source_paged_compression_profile(&source_stored)?;
    let compressed = crate::paged_compression::compress_page_safe_image_in_slot(
        &patched_decoded,
        source_compression_profile,
        source_stored.len(),
    )?;
    let rebuilt_compression_profile = profile_paged_compression(&compressed)?;
    ensure!(
        rebuilt_compression_profile.control_blocks_crossing_input_pages == 0,
        "composed name-input compression crosses an input page"
    );
    let compressed_stream_size = compressed.len();
    ensure!(
        compressed.len() <= source_stored.len(),
        "compact name font exceeds its original disc extent"
    );
    let mut patched_stored = compressed;
    patched_stored.resize(source_stored.len(), 0);
    ensure!(
        patched_stored[..4] == source_stored[..4],
        "name font changed the native CD validation prefix"
    );
    ensure!(
        decompress(&patched_stored, true)? == patched_decoded,
        "composed name-input compression roundtrip changed bytes"
    );

    Ok((
        patched_stored.clone(),
        ComposedNameInputFontBuildReport {
            source_path: NAME_ENTRY_FONT_PATH.to_string(),
            source_stored_sha256,
            source_decoded_sha256,
            source_record_size: source_stored.len(),
            keyboard_font_name,
            keyboard_font_sha256,
            keyboard_font_px: config.keyboard_font_px,
            installed_key_count,
            cleared_cache_cell_count: keyboard.cache.slots.len(),
            glyph_pack: glyph_pack.report,
            storage_tim_install,
            selector_ascii,
            roster_font: roster_report,
            runtime_lookup_install,
            runtime_coordinate_list_storage_byte_range,
            runtime_atlas,
            runtime_code_region,
            runtime_program: runtime_program.report.clone(),
            redisplay_runtime_region,
            redisplay_runtime_program: redisplay_runtime_program_report,
            embedded_font_tim_offset: format!("0x{FONT_TIM_OFFSET:05x}"),
            embedded_font_tim_size: font_tim.total_size,
            glyph_layout_table_offset: format!("0x{LAYOUT_TABLE_OFFSET:04x}"),
            glyph_layout_table_sha256: LAYOUT_TABLE_SHA256.to_string(),
            fixed_graphics,
            changed_decoded_byte_count,
            patched_decoded_sha256: sha256_bytes(&patched_decoded),
            compressed_stream_size,
            patched_stored_sha256: sha256_bytes(&patched_stored),
            compressed_within_original_extent: patched_stored.len() <= source_stored.len(),
            compression_roundtrip_verified: true,
        },
        selector_runtime,
    ))
}
