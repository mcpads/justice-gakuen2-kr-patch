use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Context, Result, bail, ensure};

use crate::compression::decompress;
use crate::contextual_texture_upload::pack_4bpp_pixels;
use crate::decoded_record_write_plan::{DecodedDataClaim, DecodedRecordWritePlan};
use crate::font::{HorizontalTextAlignment, IndexedTextRasterizer};
use crate::menu_atlas_plan::MenuGlyphAllocation;
use crate::name_input::{KANRI_PRIVATE_ASCII_GLYPHS, KanriNameStorageLayout};
use crate::paged_compression::{compress_page_safe_image, source_paged_compression_profile};
use crate::pipeline::{difference_ranges, sha256_bytes, sha256_file, write_pretty_json_and_hash};
use crate::source_disc::SupportedSourceDisc;
use crate::text::SKIP_GLYPH_CODE;

use super::assets::{load_assets, parse_hex_u16, parse_hex_usize};
use super::atlas_requests::collect_edit_requests;
use super::kanri_name_runtime::{
    KANRI_SPIRIT_GAUGE_UNIT_RENDERER_CODE, KanriFixedUiGlyphAlias, KanriStaticGlyph,
    KanriTaggedNameRuntimeInstall, install_kanri_tagged_name_runtime,
};
use super::model::{
    DevelopmentStatus, EditRuntimeDirectGlyphBuild, EditRuntimeDirectGlyphTranslation,
    EditRuntimeTextBuild, EditRuntimeTextBuildConfig, EditRuntimeTextBuildReport,
    EditRuntimeTextEntryBuild, EditRuntimeTextGlyphBuild, EditRuntimeTextTranslation,
    HorizontalAlignment, KanriDisplayAllocationEvidence, ReleaseStatus,
};
use super::pass_fixed_presentation::{
    build_pass_fixed_presentation, direct_glyph_characters, preserved_pass_consumer_codes,
};
use super::source::load_source;

pub(crate) const BUILD_MANIFEST_FILE: &str = "edit-runtime-text-build.json";
const OVERLAY_OUTPUT_FILE: &str = "edit-kanri.bin";
const PASS_OUTPUT_FILE: &str = "edit-pass.bin";
const EDIT_SHARED_UI_OUTPUT_FILE: &str = "editmoji-runtime-text.tiz";
const CLEAR_INDEX: u8 = 0;
const OUTLINE_INDEX: u8 = 3;
const FILL_INDEX: u8 = 14;

pub(crate) fn validate_edit_runtime_text_source_bindings(
    source_disc: &SupportedSourceDisc,
    assets: &Path,
) -> Result<()> {
    let source = load_source(source_disc)?;
    load_assets(assets, &source)?;
    super::password_alphabet::PasswordAlphabet::load(assets)?;
    preserved_pass_consumer_codes(&source.pass)?;
    Ok(())
}

pub(crate) fn build_edit_runtime_text(
    config: &EditRuntimeTextBuildConfig,
    source_disc: &SupportedSourceDisc,
    plan: &crate::menu_atlas_plan::MenuAtlasPlan,
) -> Result<EditRuntimeTextBuild> {
    prepare_outputs(&config.output_dir, config.force)?;
    ensure!(
        config.font.font_px.is_finite() && config.font.font_px > 0.0,
        "EDIT runtime text font size must be finite and positive"
    );
    let source = load_source(source_disc)?;
    let assets = load_assets(&config.assets, &source)?;
    let password_alphabet = super::password_alphabet::PasswordAlphabet::load(&config.assets)?;
    let (korean_text_by_asset_id, _requested) = text_and_characters(&assets)?;
    let requests = collect_edit_requests(&assets, &source, &password_alphabet)?;
    let allocation = requests.resolve(plan)?;
    let input_edit_shared_ui_decoded = decompress(&config.edit_shared_ui_stored, true)
        .context("failed to decode the fixed-UI EDITMOJI contributor")?;
    ensure!(
        config.edit_shared_ui_stored.len() == source.edit_shared_ui_stored.len()
            && input_edit_shared_ui_decoded.len() == source.edit_shared_ui_decoded.len(),
        "EDITMOJI contributor changed its source record extent"
    );
    let prepared_glyphs = prepare_edit_glyphs(&allocation.static_glyphs, &config.font)?;
    let password_glyphs = if let Some(style) = &config.password_font {
        let allocation = password_alphabet
            .characters()
            .map(|character| {
                let planned = plan.glyph(&super::atlas_requests::password_key(character))?;
                Ok((
                    character,
                    MenuGlyphAllocation {
                        character,
                        code: planned.code,
                        cell: planned.cell,
                        reused: false,
                    },
                ))
            })
            .collect::<Result<BTreeMap<_, _>>>()?;
        prepare_edit_glyphs(&allocation, style)?.static_glyphs
    } else {
        Vec::new()
    };
    let direct_glyph_entries =
        prepare_direct_glyph_entries(&assets.direct_glyph_entries, &allocation.static_glyphs)?;
    let spirit_gauge_unit = direct_glyph_entries
        .iter()
        .find(|entry| entry.id == "status_overview_spirit_gauge_unit")
        .context("EDIT runtime text assets lost the spirit-gauge unit")?;
    let spirit_gauge_unit_alias = KanriFixedUiGlyphAlias {
        source_instruction_offset: parse_hex_usize(
            &spirit_gauge_unit.source_instruction_offset,
            "spirit-gauge unit instruction offset",
        )?,
        source_code: parse_hex_u16(
            &spirit_gauge_unit.source_code,
            "spirit-gauge unit source code",
        )?,
        output_code: parse_hex_u16(
            &spirit_gauge_unit.output_code,
            "spirit-gauge unit output code",
        )?,
        renderer_input_code: KANRI_SPIRIT_GAUGE_UNIT_RENDERER_CODE,
    };

    let mut overlay_text_candidate = source.overlay.clone();
    let entries = rebuild_entries(
        &mut overlay_text_candidate,
        &assets.entries,
        &allocation.static_glyphs,
        assets.renderer_glyph_advance_px,
        &prepared_glyphs.reports,
    )?;
    let pass_fixed_presentation = build_pass_fixed_presentation(
        &source.pass,
        &source.edit_shared_ui_decoded,
        &overlay_text_candidate,
        &config.font,
        &allocation.static_glyphs,
        &korean_text_by_asset_id,
        &password_alphabet,
    )?;
    let mut overlay_text_write_claims = Vec::new();
    for entry in assets
        .entries
        .iter()
        .filter(|entry| entry.development_status == DevelopmentStatus::Authored)
    {
        let record_offset = parse_hex_usize(&entry.source_offset, "source offset")?;
        let placement_offset = parse_hex_usize(&entry.placement_record_offset, "placement offset")?;
        let mut ranges = vec![
            [record_offset, record_offset + entry.source_record_size],
            [placement_offset, placement_offset + 2],
        ];
        if super::button_layout::button_index(&entry.id).is_some() {
            ranges.push([placement_offset + 8, placement_offset + 10]);
        }
        overlay_text_write_claims.extend(DecodedDataClaim::from_effective_ranges(
            &format!("edit-runtime-text:{}", entry.id),
            &format!("replace EDIT runtime text entry {}", entry.id),
            &source.overlay,
            &overlay_text_candidate,
            ranges,
        )?);
    }

    let storage_layout = KanriNameStorageLayout::plan(
        &config.name_glyph_materialization,
        config.name_entry_runtime_coordinate_list_storage_byte_range,
    )?;
    let mut overlay_runtime_candidate = source.overlay.clone();
    let mut edit_shared_ui_runtime_candidate = source.edit_shared_ui_decoded.clone();
    let KanriTaggedNameRuntimeInstall {
        report: tagged_name_runtime,
        overlay_write_claims: tagged_name_overlay_write_claims,
        edit_shared_ui_write_claims: tagged_name_edit_shared_ui_write_claims,
    } = install_kanri_tagged_name_runtime(
        &source.overlay,
        &mut overlay_runtime_candidate,
        &source.edit_shared_ui_decoded,
        &mut edit_shared_ui_runtime_candidate,
        &source.main_executable,
        &source.name_entry_font_record,
        &source.name_entry_font_source_stored,
        &config.name_entry_font_stored,
        &config.name_entry_font_patched_decoded_sha256,
        &config.name_entry_runtime_atlas,
        &config.name_glyph_materialization,
        &config.name_glyph_consumer_layout,
        config.name_entry_runtime_coordinate_list_storage_byte_range,
        &storage_layout,
        &prepared_glyphs.static_glyphs,
        &password_glyphs,
        &allocation.dynamic_display_codes,
        spirit_gauge_unit_alias,
        config.outline_pixel_address,
        config.outline_cleanup_address,
    )?;

    let source_edit_shared_ui_decoded_sha256 = sha256_bytes(&source.edit_shared_ui_decoded);
    let mut edit_shared_ui_write_plan = DecodedRecordWritePlan::new(
        super::source::EDIT_SHARED_UI_PATH,
        &source.edit_shared_ui_decoded,
        &source_edit_shared_ui_decoded_sha256,
    )?;
    edit_shared_ui_write_plan.register_data_candidate(
        "EDIT fixed UI",
        &source_edit_shared_ui_decoded_sha256,
        &input_edit_shared_ui_decoded,
        &config.edit_shared_ui_write_claims,
    )?;
    edit_shared_ui_write_plan.register_data_candidate(
        "PASS fixed-presentation glyphs",
        &source_edit_shared_ui_decoded_sha256,
        &pass_fixed_presentation.edit_shared_ui_candidate,
        &pass_fixed_presentation.edit_shared_ui_claims,
    )?;
    edit_shared_ui_write_plan.register_data_candidate(
        "KANRI tagged-name runtime",
        &source_edit_shared_ui_decoded_sha256,
        &edit_shared_ui_runtime_candidate,
        &tagged_name_edit_shared_ui_write_claims,
    )?;
    let edit_shared_ui_decoded = edit_shared_ui_write_plan.apply(None)?;
    let edit_shared_ui_decoded_changed_byte_ranges =
        difference_ranges(&input_edit_shared_ui_decoded, &edit_shared_ui_decoded);
    ensure!(
        !edit_shared_ui_decoded_changed_byte_ranges.is_empty(),
        "EDIT runtime text build changed no bytes after fixed-UI composition"
    );
    let edit_shared_ui_stored =
        compress_edit_shared_ui(&edit_shared_ui_decoded, &source.edit_shared_ui_stored)?;

    let source_overlay_sha256 = sha256_bytes(&source.overlay);
    let mut overlay_write_plan = DecodedRecordWritePlan::new(
        super::source::OVERLAY_PATH,
        &source.overlay,
        &source_overlay_sha256,
    )?;
    overlay_write_plan.register_data_candidate(
        "EDIT runtime text entries",
        &source_overlay_sha256,
        &overlay_text_candidate,
        &overlay_text_write_claims,
    )?;
    overlay_write_plan.register_data_candidate(
        "KANRI tagged-name hooks",
        &source_overlay_sha256,
        &overlay_runtime_candidate,
        &tagged_name_overlay_write_claims,
    )?;
    let overlay = overlay_write_plan.apply(None)?;
    let overlay_changed_byte_ranges = difference_ranges(&source.overlay, &overlay);
    let source_pass_sha256 = sha256_bytes(&source.pass);
    let mut pass_write_plan =
        DecodedRecordWritePlan::new(super::source::PASS_PATH, &source.pass, &source_pass_sha256)?;
    pass_write_plan.register_data_candidate(
        "PASS fixed presentation",
        &source_pass_sha256,
        &pass_fixed_presentation.pass,
        &pass_fixed_presentation.pass_claims,
    )?;
    let mut password_machines = crate::psx_machine_code_sources::PsxMachineCodeSources::default();
    super::password_codec::register(
        &source.pass,
        &source.overlay,
        &mut pass_write_plan,
        &mut password_machines,
        crate::name_input::KANRI_NAME_STORAGE_ORIGIN
            + config
                .name_glyph_materialization
                .repertoire_membership_offset()? as u32,
        crate::name_input::KANRI_NAME_STORAGE_ORIGIN
            + storage_layout.ascii_code_table_byte_range[0] as u32,
    )?;
    let pass = pass_write_plan.apply(Some(&password_machines))?;

    let authored_entry_count = assets
        .entries
        .iter()
        .filter(|entry| entry.development_status == DevelopmentStatus::Authored)
        .count()
        + assets
            .direct_glyph_entries
            .iter()
            .filter(|entry| entry.development_status == DevelopmentStatus::Authored)
            .count()
        + assets
            .pass_fixed_presentation_text_entries
            .iter()
            .filter(|entry| entry.development_status == DevelopmentStatus::Authored)
            .count();
    let release_approved_entry_count = assets
        .entries
        .iter()
        .filter(|entry| entry.release_status == ReleaseStatus::Approved)
        .count()
        + assets
            .direct_glyph_entries
            .iter()
            .filter(|entry| entry.release_status == ReleaseStatus::Approved)
            .count()
        + assets
            .pass_fixed_presentation_text_entries
            .iter()
            .filter(|entry| entry.release_status == ReleaseStatus::Approved)
            .count();
    let report = EditRuntimeTextBuildReport {
        kind: "Justice Gakuen 2 source-bound EDIT runtime text development build".to_string(),
        build_spec_sha256: config.build_spec_sha256.clone(),
        source_bin_sha256: source.source_bin_sha256,
        source_overlay_size: source.overlay.len(),
        source_overlay_sha256: sha256_bytes(&source.overlay),
        output_overlay_sha256: sha256_bytes(&overlay),
        source_pass_size: source.pass.len(),
        source_pass_sha256,
        output_pass_sha256: sha256_bytes(&pass),
        pass_fixed_presentation: pass_fixed_presentation.report,
        source_edit_shared_ui_stored_sha256: sha256_bytes(&source.edit_shared_ui_stored),
        source_edit_shared_ui_decoded_sha256: sha256_bytes(&source.edit_shared_ui_decoded),
        input_edit_shared_ui_stored_sha256: sha256_bytes(&config.edit_shared_ui_stored),
        input_edit_shared_ui_decoded_sha256: sha256_bytes(&input_edit_shared_ui_decoded),
        output_edit_shared_ui_stored_sha256: sha256_bytes(&edit_shared_ui_stored),
        output_edit_shared_ui_decoded_sha256: sha256_bytes(&edit_shared_ui_decoded),
        runtime_font_tim_offset: assets.runtime_font.tim_offset,
        runtime_font_tim_sha256: assets.runtime_font.tim_sha256,
        translation_manifest_sha256: assets.manifest_sha256,
        pass_fixed_presentation_text_sha256: assets.pass_fixed_presentation_text_sha256,
        font_sha256: sha256_file(&config.font.path)?,
        font_px: config.font.font_px,
        vertical_shift_px: config.font.vertical_shift_px,
        renderer_glyph_advance_px: assets.renderer_glyph_advance_px,
        entry_count: assets.entries.len()
            + assets.direct_glyph_entries.len()
            + assets.pass_fixed_presentation_text_entries.len(),
        authored_entry_count,
        release_approved_entry_count,
        development_input_available: authored_entry_count > 0,
        release_candidate_input_eligible: release_approved_entry_count
            == assets.entries.len()
                + assets.direct_glyph_entries.len()
                + assets.pass_fixed_presentation_text_entries.len(),
        source_records_match: true,
        source_placements_match: true,
        changed_bytes_confined_to_owned_ranges: true,
        reused_glyph_count: 0,
        installed_glyph_count: prepared_glyphs.reports.len(),
        runtime_font_page_glyph_count: 0,
        kanri_initial_glyph_count: prepared_glyphs.static_glyphs.len(),
        kanri_display_allocation: allocation.evidence,
        tagged_name_runtime,
        edit_shared_ui_decoded_changed_byte_ranges,
        overlay_changed_byte_ranges,
        glyphs: prepared_glyphs.reports,
        entries,
        direct_glyph_entries,
        overlay_output_file: OVERLAY_OUTPUT_FILE.to_string(),
        pass_output_file: PASS_OUTPUT_FILE.to_string(),
        edit_shared_ui_output_file: EDIT_SHARED_UI_OUTPUT_FILE.to_string(),
        runtime_consumer_verified: false,
    };
    let build_manifest_sha256 = write_outputs(
        &config.output_dir,
        &overlay,
        &pass,
        &edit_shared_ui_stored,
        &report,
    )?;
    Ok(EditRuntimeTextBuild {
        overlay,
        pass,
        edit_shared_ui_stored,
        build_manifest_sha256,
        report,
    })
}

fn requested_characters(entries: &[EditRuntimeTextTranslation]) -> BTreeSet<char> {
    entries
        .iter()
        .filter(|entry| entry.development_status == DevelopmentStatus::Authored)
        .flat_map(|entry| entry.korean_text.iter().flat_map(|text| text.chars()))
        .filter(|character| reused_kanri_menu_glyph_code(*character).is_none())
        .collect()
}

pub(super) fn reused_kanri_menu_glyph_code(character: char) -> Option<u16> {
    // Only spacing is font-independent. Visible Latin and punctuation must
    // use the record's role, rather than the password/native ASCII supplier.
    (character == ' ').then_some(SKIP_GLYPH_CODE)
}

fn requested_direct_glyph_characters(
    entries: &[EditRuntimeDirectGlyphTranslation],
) -> BTreeSet<char> {
    entries
        .iter()
        .filter(|entry| entry.development_status == DevelopmentStatus::Authored)
        .flat_map(|entry| entry.korean_text.chars())
        .collect()
}

fn prepare_direct_glyph_entries(
    entries: &[EditRuntimeDirectGlyphTranslation],
    allocation: &BTreeMap<char, MenuGlyphAllocation>,
) -> Result<Vec<EditRuntimeDirectGlyphBuild>> {
    entries
        .iter()
        .map(|entry| {
            let character = entry
                .korean_text
                .chars()
                .next()
                .context("EDIT runtime direct-glyph output is empty")?;
            let output_code = allocation
                .get(&character)
                .with_context(|| {
                    format!(
                        "EDIT runtime direct-glyph {} has no fixed glyph allocation",
                        entry.id
                    )
                })?
                .code;
            Ok(EditRuntimeDirectGlyphBuild {
                id: entry.id.clone(),
                source_instruction_offset: entry.source_instruction_offset.clone(),
                source_code: entry.source_code.clone(),
                source_text: entry.source_text.clone(),
                korean_text: entry.korean_text.clone(),
                output_code: format!("0x{output_code:04x}"),
                renderer_input_code: format!("0x{KANRI_SPIRIT_GAUGE_UNIT_RENDERER_CODE:04x}"),
                development_status: entry.development_status,
                release_status: entry.release_status,
            })
        })
        .collect()
}

#[derive(Debug)]
pub(super) struct KanriDisplayAllocation {
    pub(super) static_glyphs: BTreeMap<char, MenuGlyphAllocation>,
    pub(super) dynamic_display_codes: Vec<u16>,
    pub(super) evidence: KanriDisplayAllocationEvidence,
}

struct PreparedEditGlyphs {
    reports: Vec<EditRuntimeTextGlyphBuild>,
    static_glyphs: Vec<KanriStaticGlyph>,
}

fn prepare_edit_glyphs(
    allocation: &BTreeMap<char, MenuGlyphAllocation>,
    style: &crate::development_build_spec::ShiftedSizedFontSource,
) -> Result<PreparedEditGlyphs> {
    let rasterizer = IndexedTextRasterizer::load(&style.path)?;
    let mut reports = Vec::with_capacity(allocation.len());
    let mut static_glyphs = Vec::with_capacity(allocation.len());
    for allocation in allocation.values() {
        let rendered = rasterizer.rasterize_shifted(
            &allocation.character.to_string(),
            allocation.cell.width,
            allocation.cell.height,
            style.font_px,
            0.0,
            style.vertical_shift_px,
            CLEAR_INDEX,
            Some(OUTLINE_INDEX),
            FILL_INDEX,
            HorizontalTextAlignment::Center,
        )?;
        let payload = pack_4bpp_pixels(&rendered.pixels)?;
        reports.push(EditRuntimeTextGlyphBuild {
            character: allocation.character,
            code: format!("0x{:04x}", allocation.code),
            cell: allocation.cell,
            reused: false,
            storage: "kanri_private_vram".to_string(),
            ink_bounds: Some(rendered.ink_bounds),
            source_region_sha256: None,
            allowed_decoded_byte_ranges: None,
            changed_decoded_byte_count: None,
        });
        static_glyphs.push(KanriStaticGlyph {
            character: allocation.character,
            code: allocation.code,
            payload,
        });
    }
    Ok(PreparedEditGlyphs {
        reports,
        static_glyphs,
    })
}

fn compress_edit_shared_ui(decoded: &[u8], source_stored: &[u8]) -> Result<Vec<u8>> {
    let source_profile = source_paged_compression_profile(source_stored)?;
    let reencoded = compress_page_safe_image(decoded, source_profile)?;
    ensure!(
        reencoded.len() <= source_stored.len() && reencoded[..4] == source_stored[..4],
        "EDIT runtime text EDITMOJI stream exceeded or changed its source catalog"
    );
    ensure!(
        decompress(&reencoded, false)? == decoded,
        "EDIT runtime text EDITMOJI compression roundtrip changed decoded bytes"
    );
    let mut stored = reencoded;
    stored.resize(source_stored.len(), 0);
    ensure!(
        decompress(&stored, true)? == decoded,
        "padded EDIT runtime text EDITMOJI changed decoded bytes"
    );
    Ok(stored)
}

fn rebuild_entries(
    overlay: &mut [u8],
    entries: &[EditRuntimeTextTranslation],
    allocation: &BTreeMap<char, MenuGlyphAllocation>,
    glyph_advance_px: i16,
    glyphs: &[EditRuntimeTextGlyphBuild],
) -> Result<Vec<EditRuntimeTextEntryBuild>> {
    let mut builds = Vec::with_capacity(entries.len());
    for entry in entries {
        if entry.development_status == DevelopmentStatus::Untranslated {
            builds.push(EditRuntimeTextEntryBuild {
                id: entry.id.clone(),
                source_offset: entry.source_offset.clone(),
                source_text: entry.source_text.clone(),
                korean_text: None,
                output_codes: Vec::new(),
                placement_record_offset: entry.placement_record_offset.clone(),
                x: entry.source_x,
                y: entry.source_y,
                glyph_advance_px,
                button_layout: None,
                development_status: entry.development_status,
                release_status: entry.release_status,
            });
            continue;
        }
        let korean_text = entry
            .korean_text
            .as_ref()
            .context("authored EDIT runtime text lacks Korean text")?;
        let output_codes = korean_text
            .chars()
            .map(|character| {
                reused_kanri_menu_glyph_code(character)
                    .map(Ok)
                    .unwrap_or_else(|| {
                        allocation
                            .get(&character)
                            .map(|glyph| glyph.code)
                            .with_context(|| {
                                format!("no EDIT runtime text glyph for {character:?}")
                            })
                    })
            })
            .collect::<Result<Vec<_>>>()?;
        ensure!(
            2 + output_codes.len() * 2 <= entry.source_record_size,
            "EDIT runtime text {} Korean text exceeds its {}-byte source record",
            entry.id,
            entry.source_record_size
        );
        let source_offset = parse_hex_usize(&entry.source_offset, "source offset")?;
        let record = overlay
            .get_mut(source_offset..source_offset + entry.source_record_size)
            .context("EDIT runtime text output record is out of bounds")?;
        write_record(record, &output_codes)?;

        let button_layout = super::button_layout::button_index(&entry.id)
            .map(|index| {
                super::button_layout::layout_button(
                    overlay,
                    index,
                    output_codes.len(),
                    entry.source_y,
                )
            })
            .transpose()?;
        if let Some(layout) = &button_layout {
            for (column, character) in korean_text.chars().enumerate() {
                let ink = if reused_kanri_menu_glyph_code(character).is_some() {
                    // The skip code advances without drawing pixels.
                    [0, 0, 0, 0]
                } else {
                    glyphs
                        .iter()
                        .find(|glyph| glyph.character == character)
                        .and_then(|glyph| glyph.ink_bounds)
                        .context("EDIT button label lacks measured glyph ink")?
                };
                layout.validate_ink(column, entry.source_y, ink)?;
            }
        }
        let x = if let Some(layout) = &button_layout {
            layout.x
        } else {
            match entry.horizontal_alignment {
                HorizontalAlignment::PreserveCenter => centered_x(
                    entry.source_x,
                    entry.source_codes.len(),
                    output_codes.len(),
                    glyph_advance_px,
                )?,
            }
        };
        let placement_offset = parse_hex_usize(&entry.placement_record_offset, "placement offset")?;
        let output_advance = button_layout
            .as_ref()
            .map_or(glyph_advance_px, |layout| layout.glyph_advance_px);
        if button_layout.is_some() {
            overlay
                .get_mut(placement_offset + 8..placement_offset + 10)
                .context("EDIT button spacing is out of bounds")?
                .copy_from_slice(
                    &(super::button_layout::SOURCE_GLYPH_ADVANCE - output_advance).to_le_bytes(),
                );
        }
        overlay
            .get_mut(placement_offset..placement_offset + 2)
            .context("EDIT runtime text placement x is out of bounds")?
            .copy_from_slice(&x.to_le_bytes());
        builds.push(EditRuntimeTextEntryBuild {
            id: entry.id.clone(),
            source_offset: entry.source_offset.clone(),
            source_text: entry.source_text.clone(),
            korean_text: Some(korean_text.clone()),
            output_codes: output_codes
                .iter()
                .map(|code| format!("0x{code:04x}"))
                .collect(),
            placement_record_offset: entry.placement_record_offset.clone(),
            x,
            y: entry.source_y,
            glyph_advance_px: output_advance,
            button_layout,
            development_status: entry.development_status,
            release_status: entry.release_status,
        });
    }
    Ok(builds)
}

fn centered_x(
    source_x: i16,
    source_glyph_count: usize,
    output_glyph_count: usize,
    glyph_advance_px: i16,
) -> Result<i16> {
    ensure!(
        glyph_advance_px > 0 && glyph_advance_px % 2 == 0,
        "EDIT renderer glyph advance must be a positive even number"
    );
    let count_delta = i32::try_from(source_glyph_count)? - i32::try_from(output_glyph_count)?;
    let x = i32::from(source_x) + count_delta * i32::from(glyph_advance_px / 2);
    i16::try_from(x).context("centered EDIT runtime text x exceeds i16")
}

fn write_record(record: &mut [u8], codes: &[u16]) -> Result<()> {
    ensure!(
        2 + codes.len() * 2 <= record.len(),
        "encoded EDIT runtime text exceeds its source record"
    );
    record.fill(0);
    record[..2].copy_from_slice(&u16::try_from(codes.len())?.to_le_bytes());
    for (index, code) in codes.iter().copied().enumerate() {
        let start = 2 + index * 2;
        record[start..start + 2].copy_from_slice(&code.to_le_bytes());
    }
    Ok(())
}

fn prepare_outputs(output_dir: &Path, force: bool) -> Result<()> {
    std::fs::create_dir_all(output_dir)?;
    for name in [
        OVERLAY_OUTPUT_FILE,
        PASS_OUTPUT_FILE,
        EDIT_SHARED_UI_OUTPUT_FILE,
        BUILD_MANIFEST_FILE,
    ] {
        let path = output_dir.join(name);
        if path.exists() && !force {
            bail!("EDIT runtime text output exists; pass --force to replace it");
        }
        if path.exists() {
            std::fs::remove_file(&path)
                .with_context(|| format!("failed to remove {}", path.display()))?;
        }
    }
    Ok(())
}

fn write_outputs(
    output_dir: &Path,
    overlay: &[u8],
    pass: &[u8],
    edit_shared_ui_stored: &[u8],
    report: &EditRuntimeTextBuildReport,
) -> Result<String> {
    std::fs::write(output_dir.join(OVERLAY_OUTPUT_FILE), overlay)?;
    std::fs::write(output_dir.join(PASS_OUTPUT_FILE), pass)?;
    std::fs::write(
        output_dir.join(EDIT_SHARED_UI_OUTPUT_FILE),
        edit_shared_ui_stored,
    )?;
    write_pretty_json_and_hash(&output_dir.join(BUILD_MANIFEST_FILE), report, true)
}

#[cfg(test)]
pub(super) fn centered_x_for_test(
    source_x: i16,
    source_glyph_count: usize,
    output_glyph_count: usize,
    glyph_advance_px: i16,
) -> Result<i16> {
    centered_x(
        source_x,
        source_glyph_count,
        output_glyph_count,
        glyph_advance_px,
    )
}

pub(super) fn text_and_characters(
    assets: &super::assets::LoadedEditRuntimeTextAssets,
) -> Result<(BTreeMap<String, String>, BTreeSet<char>)> {
    let mut korean_text_by_asset_id = assets
        .entries
        .iter()
        .filter(|entry| entry.development_status == DevelopmentStatus::Authored)
        .filter_map(|entry| {
            entry
                .korean_text
                .as_ref()
                .map(|text| (entry.id.clone(), text.clone()))
        })
        .collect::<BTreeMap<_, _>>();
    for entry in &assets.pass_fixed_presentation_text_entries {
        ensure!(
            korean_text_by_asset_id
                .insert(entry.id.clone(), entry.korean_text.clone())
                .is_none(),
            "PASS fixed-presentation semantic asset {} duplicates another EDIT text ID",
            entry.id
        );
    }
    let pass_direct_glyph_characters = direct_glyph_characters(&korean_text_by_asset_id)?;
    let mut requested = requested_characters(&assets.entries);
    requested.extend(
        korean_text_by_asset_id
            .values()
            .flat_map(|text| text.chars())
            .filter(|character| !pass_direct_glyph_characters.contains(character))
            .filter(|character| reused_kanri_menu_glyph_code(*character).is_none()),
    );
    requested.extend(requested_direct_glyph_characters(
        &assets.direct_glyph_entries,
    ));
    requested.extend(KANRI_PRIVATE_ASCII_GLYPHS.chars());
    Ok((korean_text_by_asset_id, requested))
}
