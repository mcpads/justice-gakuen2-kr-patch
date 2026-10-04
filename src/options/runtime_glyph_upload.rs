#[path = "runtime_glyph_upload/assembly.rs"]
mod assembly;
#[path = "runtime_glyph_upload/model.rs"]
mod model;
#[path = "runtime_glyph_upload/options_context.rs"]
mod options_context;
#[path = "runtime_glyph_upload/records_context.rs"]
mod records_context;
#[cfg(test)]
#[path = "runtime_glyph_upload_tests.rs"]
mod tests;

use std::collections::BTreeSet;

use anyhow::{Result, ensure};

use crate::pipeline::sha256_bytes;
use crate::text::fixed_menu_glyph_code;
use crate::tim::{Cell, read_indexed_cell_in_prefix};
pub(super) use crate::write_scope::changed_ranges_are_within as ranges_are_allowed;

pub(crate) use assembly::RecordsHookInstall;
pub use model::{ContextualGlyphUploadReport, GlyphUploadEntry};

const MENU_FONT_TIM_OFFSET: usize = 0;
pub(super) const RECORDS_PAYLOAD_STORAGE_START: usize = 0x4b020;
pub(super) const MENU_RUNTIME_BASE: u32 = 0x800d_4000;
pub(super) const OPTINFO_RUNTIME_BASE: u32 = 0x800d_4000;
#[cfg(test)]
pub(super) const PACKED_CELL_BYTE_COUNT: usize = 20 * 20 / 2;

pub(super) use super::contextual_glyphs::PRESERVED_SOURCE_GRAPHIC_CODES;

#[derive(Debug, Clone)]
pub(super) struct PreparedGlyph {
    pub(super) role: String,
    pub(super) character: Option<char>,
    pub(super) code: u16,
    pub(super) cell: Cell,
    pub(super) source_preservation: bool,
    pub(super) payload: Vec<u8>,
}

#[derive(Debug, Clone)]
pub(super) struct PreparedTextureUpload {
    pub(super) role: String,
    pub(super) character: Option<char>,
    pub(super) code: Option<u16>,
    pub(super) cell: Cell,
    pub(super) vram_rect: [u16; 4],
    pub(super) source_preservation: bool,
    pub(super) payload: Vec<u8>,
}

impl PreparedTextureUpload {
    fn from_glyph(glyph: &PreparedGlyph) -> Result<Self> {
        Ok(Self {
            role: glyph.role.clone(),
            character: glyph.character,
            code: Some(glyph.code),
            cell: glyph.cell,
            vram_rect: vram_rect(glyph)?,
            source_preservation: glyph.source_preservation,
            payload: glyph.payload.clone(),
        })
    }
}

#[derive(Debug)]
pub(super) struct RuntimeGlyphUploadBuild {
    pub(super) report: ContextualGlyphUploadReport,
    pub(super) menu_expected_write_ranges: Vec<[usize; 2]>,
    pub(super) overlay_expected_write_ranges: Vec<[usize; 2]>,
    pub(super) optinfo_expected_write_ranges: Vec<[usize; 2]>,
    pub(super) main_executable_candidate: Vec<u8>,
    pub(super) main_executable_machine_code_writes: Vec<RecordsHookInstall>,
}

pub(super) struct ContextualGlyphUploadInputs<'a> {
    pub(super) source_menu_decoded: &'a [u8],
    pub(super) output_menu_decoded: &'a mut [u8],
    pub(super) options_contextual_glyphs: &'a [PreparedGlyph],
    pub(super) options_background_uploads: &'a [PreparedTextureUpload],
    pub(super) records_contextual_glyphs: &'a [PreparedGlyph],
    pub(super) records_output_codes: &'a [u16],
    pub(super) source_overlay: &'a [u8],
    pub(super) output_overlay: &'a mut [u8],
    pub(super) source_optinfo_decoded: &'a [u8],
    pub(super) output_optinfo_decoded: &'a mut [u8],
    pub(super) source_main_executable: &'a [u8],
}

pub(super) fn build_contextual_glyph_upload(
    inputs: ContextualGlyphUploadInputs<'_>,
) -> Result<RuntimeGlyphUploadBuild> {
    let ContextualGlyphUploadInputs {
        source_menu_decoded,
        output_menu_decoded,
        options_contextual_glyphs,
        options_background_uploads,
        records_contextual_glyphs,
        records_output_codes,
        source_overlay,
        output_overlay,
        source_optinfo_decoded,
        output_optinfo_decoded,
        source_main_executable,
    } = inputs;
    ensure!(
        source_menu_decoded.len() == output_menu_decoded.len(),
        "contextual glyph upload changed MENU length"
    );
    let records_prepared = select_records_glyphs(
        options_contextual_glyphs,
        records_contextual_glyphs,
        records_output_codes,
    )?;
    let records_source_restorations = capture_records_source_graphics(
        source_menu_decoded,
        output_menu_decoded,
        &records_prepared,
    )?;

    let options = options_context::install(
        options_contextual_glyphs,
        options_background_uploads,
        source_overlay,
        output_overlay,
        source_optinfo_decoded,
        output_optinfo_decoded,
        source_main_executable,
    )?;
    let records = records_context::install(
        &records_prepared,
        &records_source_restorations,
        source_menu_decoded,
        output_menu_decoded,
        source_main_executable,
    )?;
    Ok(RuntimeGlyphUploadBuild {
        report: ContextualGlyphUploadReport {
            source_menu_graphics_preserved: true,
            options_global_menu_write_count: 0,
            options_decompressor_input_windows_verified: true,
            options_contextual_code_population_matches: true,
            records_contextual_code_population_matches: true,
            records_source_graphic_restore_population_matches: true,
            options: options.report,
            options_exit_restore: options.exit_restore_report,
            records: records.entry_report,
            records_card_operation_refresh: records.card_operation_refresh_report,
            records_exit_restore: records.exit_restore_report,
            boot_notice: records.boot_notice_report,
        },
        menu_expected_write_ranges: records.expected_menu_write_ranges,
        overlay_expected_write_ranges: options.expected_overlay_write_ranges,
        optinfo_expected_write_ranges: options.expected_storage_write_ranges,
        main_executable_candidate: records.main_executable_candidate,
        main_executable_machine_code_writes: records.machine_code_writes,
    })
}

fn capture_records_source_graphics(
    source_menu_decoded: &[u8],
    output_menu_decoded: &[u8],
    records_glyphs: &[PreparedGlyph],
) -> Result<Vec<PreparedGlyph>> {
    let mut restorations = Vec::with_capacity(records_glyphs.len());
    for glyph in records_glyphs {
        ensure!(
            glyph.cell.x.is_multiple_of(4) && glyph.cell.width.is_multiple_of(4),
            "Records source restore code 0x{:04x} is not word-aligned",
            glyph.code
        );
        let source_pixels =
            read_indexed_cell_in_prefix(source_menu_decoded, MENU_FONT_TIM_OFFSET, glyph.cell)?;
        ensure!(
            read_indexed_cell_in_prefix(output_menu_decoded, MENU_FONT_TIM_OFFSET, glyph.cell,)?
                == source_pixels,
            "global MENU overwrote Records source cell 0x{:04x}",
            glyph.code
        );
        restorations.push(PreparedGlyph {
            role: "source_menu_graphic_restore".to_string(),
            character: None,
            code: glyph.code,
            cell: glyph.cell,
            source_preservation: true,
            payload: pack_indexed_pixels(&source_pixels)?,
        });
    }
    Ok(restorations)
}

fn select_records_glyphs(
    options_glyphs: &[PreparedGlyph],
    protected_records_glyphs: &[PreparedGlyph],
    records_output_codes: &[u16],
) -> Result<Vec<PreparedGlyph>> {
    let available = options_glyphs
        .iter()
        .chain(protected_records_glyphs)
        .collect::<Vec<_>>();
    let available_codes = available
        .iter()
        .map(|glyph| glyph.code)
        .collect::<BTreeSet<_>>();
    ensure!(
        available_codes.len() == available.len()
            && protected_records_glyphs.len() <= PRESERVED_SOURCE_GRAPHIC_CODES.len()
            && protected_records_glyphs
                .iter()
                .all(|glyph| PRESERVED_SOURCE_GRAPHIC_CODES.contains(&glyph.code)),
        "Records runtime glyph candidates are malformed"
    );
    let output_codes = records_output_codes
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let expected = output_codes
        .iter()
        .copied()
        .filter(|code| !is_source_resident_fixed_code(*code))
        .collect::<BTreeSet<_>>();
    ensure!(
        expected.iter().all(|code| available_codes.contains(code)),
        "Records runtime output uses an unavailable contextual glyph: {}",
        format_codes(&expected.difference(&available_codes).copied().collect())
    );
    let mut selected = available
        .into_iter()
        .filter(|glyph| expected.contains(&glyph.code))
        .cloned()
        .collect::<Vec<_>>();
    selected.sort_by_key(|glyph| glyph.code);
    let observed = selected
        .iter()
        .map(|glyph| glyph.code)
        .collect::<BTreeSet<_>>();
    ensure!(
        observed == expected
            && protected_records_glyphs
                .iter()
                .all(|glyph| observed.contains(&glyph.code)),
        "Records complete contextual glyph population changed: expected {}, got {}",
        format_codes(&expected),
        format_codes(&observed)
    );
    Ok(selected)
}

fn is_source_resident_fixed_code(code: u16) -> bool {
    [
        ' ', '(', ')', '?', '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'A', 'C', 'P', 'R',
        'S', 'T', 'U', 'V',
    ]
    .into_iter()
    .filter_map(fixed_menu_glyph_code)
    .any(|fixed| fixed == code)
}

fn format_codes(codes: &BTreeSet<u16>) -> String {
    codes
        .iter()
        .map(|code| format!("0x{code:04x}"))
        .collect::<Vec<_>>()
        .join(", ")
}

pub(super) fn vram_rect(glyph: &PreparedGlyph) -> Result<[u16; 4]> {
    crate::contextual_texture_upload::menu_atlas_vram_rect(glyph.cell)
}

pub(super) fn encode_descriptor(output: &mut Vec<u8>, rect: [u16; 4], source_address: u32) {
    crate::contextual_texture_upload::encode_texture_upload_descriptor(
        output,
        rect,
        source_address,
    );
}

pub(super) fn entry_report(
    glyph: &PreparedGlyph,
    rect: [u16; 4],
    payload_offset: usize,
    payload_runtime_address: u32,
) -> GlyphUploadEntry {
    upload_entry_report(
        &PreparedTextureUpload {
            role: glyph.role.clone(),
            character: glyph.character,
            code: Some(glyph.code),
            cell: glyph.cell,
            vram_rect: rect,
            source_preservation: glyph.source_preservation,
            payload: glyph.payload.clone(),
        },
        payload_offset,
        payload_runtime_address,
    )
}

pub(super) fn upload_entry_report(
    upload: &PreparedTextureUpload,
    payload_offset: usize,
    payload_runtime_address: u32,
) -> GlyphUploadEntry {
    GlyphUploadEntry {
        role: upload.role.clone(),
        character: upload.character,
        source_preservation: upload.source_preservation,
        code: upload.code.map(|code| format!("0x{code:04x}")),
        cell: upload.cell,
        vram_rect: upload.vram_rect,
        payload_offset: format!("0x{payload_offset:05x}"),
        payload_runtime_address: format!("0x{payload_runtime_address:08x}"),
        payload_sha256: sha256_bytes(&upload.payload),
    }
}

#[cfg(test)]
pub(super) fn pack_indexed_cell(pixels: &[u8]) -> Result<Vec<u8>> {
    ensure!(
        pixels.len() == 20 * 20,
        "contextual glyph pixel count changed"
    );
    pack_indexed_pixels(pixels)
}

pub(super) fn pack_indexed_pixels(pixels: &[u8]) -> Result<Vec<u8>> {
    crate::contextual_texture_upload::pack_4bpp_pixels(pixels)
}
