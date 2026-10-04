use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Context, Result, ensure};

use crate::compression::decompress;
use crate::cue::CueSheet;
use crate::disc::rebuild;
use crate::font::rasterize_menu_glyphs;
use crate::name_input::build_name_input_runtime_program;
use crate::pipeline::{BASELINE_BIN_SHA256, sha256_bytes, sha256_file};
use crate::tim::{Cell, install_indexed_glyph_in_prefix, parse_4bpp_prefix};

use super::name_entry_application_form::application_form_spec;
use super::name_entry_build_model::DialogueNameEntryCandidateAssignment;
use super::name_entry_candidate_controls::candidate_controls_spec;
use super::name_entry_face_features::{
    face_feature_labels_spec, validate_face_feature_cells, validate_face_feature_consumer,
};
use super::name_entry_favorite_word_choices::favorite_word_choices_spec;
use super::name_entry_fixed_graphics_build::{
    install_name_entry_fixed_graphics, validate_fixed_graphic_surfaces_disjoint,
};
use super::name_entry_fixed_graphics_model::NameEntryFixedGraphicsSource;
use super::name_entry_font_build_model::{
    DialogueNameEntryFontBuildReport, DialogueNameEntryGlyphInstall,
};
use super::name_entry_runtime_code::{
    audit_name_entry_runtime_code_region, install_name_entry_runtime_program,
};
use super::name_entry_school_choices::school_choices_spec;
use super::name_entry_sex_choices::sex_choices_spec;
use super::name_entry_subject_choices::subject_choices_spec;
use crate::paged_compression::{
    compress_page_safe_image, profile_paged_compression, source_paged_compression_profile,
};

pub(crate) const NAME_ENTRY_FONT_PATH: &str = "DAT2/MA_ENT.BIZ";
pub(super) const NAME_ENTRY_FONT_OUTPUT_FILE: &str = "MA_ENT.BIZ";
pub(crate) const SOURCE_STORED_SHA256: &str =
    "31b003368f9c7189448cdaa98baa746dc8dbea1638183c46554f96f480bf21ce";
pub(crate) const SOURCE_DECODED_SHA256: &str =
    "099210a8804eee3270973c24ed6eb294d11499c55b4c1ef8c63e75988227b373";
pub(crate) const SOURCE_DECODED_SIZE: usize = 210_944;
pub(crate) const FONT_TIM_OFFSET: usize = 0x19_000;
pub(super) const BACKGROUND_TIM_OFFSET: usize = 0;
pub(super) const LAYOUT_TABLE_OFFSET: usize = 0x0eea;
const LAYOUT_RECORD_SIZE: usize = 3;
pub(super) const LAYOUT_TABLE_SHA256: &str =
    "255cb5b13142eba6cecd40a66b110f7480f1dc7bd7236f656b69c1ef0563cfb3";
const TEXTURE_PAGE_START: u8 = 0x0c;
const TEXTURE_PAGE_END: u8 = 0x0e;
const TEXTURE_PAGE_PIXEL_WIDTH: usize = 256;
const CELL_WIDTH: usize = 20;
const CELL_HEIGHT: usize = 20;
const COLUMN_COUNT: u8 = 12;
const ROW_COUNT: u8 = 7;
const OUTLINE_PALETTE_INDEX: u8 = 3;
const FILL_PALETTE_INDEX: u8 = 13;

#[derive(Debug, Clone, Copy)]
pub(super) struct NameEntryGlyphCell {
    pub(super) texture_page: u8,
    pub(super) column: u8,
    pub(super) row: u8,
    pub(super) cell: Cell,
}

pub(super) fn build_name_entry_font_asset(
    cue_path: &Path,
    source_overlay: &[u8],
    assignments: &[DialogueNameEntryCandidateAssignment],
    graphics_root: &Path,
    candidate_font_path: &Path,
    candidate_font_px: f32,
    fixed_graphics_font_path: &Path,
) -> Result<(Vec<u8>, DialogueNameEntryFontBuildReport)> {
    let cue = CueSheet::parse(cue_path)?;
    let source_bin_sha256 = sha256_file(&cue.image_path)?;
    ensure!(
        source_bin_sha256 == BASELINE_BIN_SHA256,
        "unsupported source BIN SHA-256: {source_bin_sha256}"
    );
    let (_, source_stored) = rebuild::read_record(&cue.image_path, NAME_ENTRY_FONT_PATH)?;
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
    let mut runtime_code_region = audit_name_entry_runtime_code_region(&source_decoded)?;
    let runtime_program = build_name_input_runtime_program()?;

    let font_tim = parse_4bpp_prefix(
        source_decoded
            .get(FONT_TIM_OFFSET..)
            .context("name-entry font TIM offset is truncated")?,
    )?;
    ensure!(
        font_tim.image_x == 768
            && font_tim.image_y == 0
            && font_tim.pixel_width() == 768
            && font_tim.image_height == 256
            && font_tim.clut_x == 0
            && font_tim.clut_y == 483
            && font_tim.clut_width == 352
            && font_tim.clut_height == 1,
        "name-entry embedded font TIM geometry changed"
    );
    let background_tim = parse_4bpp_prefix(
        source_decoded
            .get(BACKGROUND_TIM_OFFSET..)
            .context("name-entry background TIM is truncated")?,
    )?;
    ensure!(
        background_tim.total_size > FONT_TIM_OFFSET,
        "name-entry source TIM overlap contract changed"
    );
    let source_background_tim_overlap_byte_count = background_tim.total_size - FONT_TIM_OFFSET;

    let cells = load_all_name_entry_glyph_cells(source_overlay)?;
    ensure!(
        cells.len() == assignments.len(),
        "name-entry candidate and layout populations differ"
    );

    let characters = assignments
        .iter()
        .filter(|assignment| !assignment.preserve_source_glyph)
        .map(|assignment| assignment.character.as_str())
        .collect::<String>();
    let rasterized = rasterize_menu_glyphs(
        candidate_font_path,
        &characters,
        candidate_font_px,
        OUTLINE_PALETTE_INDEX,
        FILL_PALETTE_INDEX,
    )?;
    let font_name = rasterized.font_name;
    let font_sha256 = rasterized.font_sha256;
    let glyph_pixels = rasterized
        .glyphs
        .into_iter()
        .map(|glyph| (glyph.character, glyph.pixels))
        .collect::<BTreeMap<_, _>>();
    ensure!(
        glyph_pixels.len()
            == assignments
                .iter()
                .filter(|assignment| !assignment.preserve_source_glyph)
                .count(),
        "name-entry glyph raster population changed"
    );

    let mut patched_decoded = source_decoded.clone();
    install_name_entry_runtime_program(&mut patched_decoded, &runtime_program.bytes)?;
    runtime_code_region.typed_code_installed = true;
    let mut installs = Vec::with_capacity(assignments.len());
    for (assignment, glyph_cell) in assignments.iter().zip(&cells) {
        if assignment.preserve_source_glyph {
            continue;
        }
        let character = assignment
            .character
            .chars()
            .next()
            .context("name-entry candidate is empty")?;
        ensure!(
            assignment.character.chars().count() == 1,
            "name-entry assignment contains more than one character"
        );
        let pixels = glyph_pixels
            .get(&character)
            .context("name-entry rasterized glyph disappeared")?;
        let metadata = install_indexed_glyph_in_prefix(
            &mut patched_decoded,
            FONT_TIM_OFFSET,
            glyph_cell.cell,
            pixels,
            "enrollment name-entry candidate glyph",
        )?;
        installs.push(DialogueNameEntryGlyphInstall {
            source_page: assignment.source_page.clone(),
            page_position: assignment.page_position,
            character: assignment.character.clone(),
            code: assignment.code.clone(),
            texture_page: glyph_cell.texture_page,
            column: glyph_cell.column,
            row: glyph_cell.row,
            cell: glyph_cell.cell,
            indexed_pixels_sha256: sha256_bytes(pixels),
            changed_decoded_byte_count: metadata.changed_decoded_byte_count,
        });
    }
    let protected_glyph_cells = cells.iter().map(|glyph| glyph.cell).collect::<Vec<_>>();
    let fixed_graphics = install_all_name_entry_fixed_graphics(
        source_overlay,
        &source_decoded,
        &mut patched_decoded,
        graphics_root,
        fixed_graphics_font_path,
        &protected_glyph_cells,
    )?;
    let changed_decoded_byte_count = source_decoded
        .iter()
        .zip(&patched_decoded)
        .filter(|(source, patched)| source != patched)
        .count();
    ensure!(
        changed_decoded_byte_count > 0,
        "name-entry font build changed no decoded bytes"
    );

    let source_compression_profile = source_paged_compression_profile(&source_stored)?;
    let compressed = compress_page_safe_image(&patched_decoded, source_compression_profile)?;
    let rebuilt_compression_profile = profile_paged_compression(&compressed)?;
    ensure!(
        compressed.len() <= source_stored.len(),
        "name-entry font build exceeds its original ISO record"
    );
    let compressed_stream_size = compressed.len();
    let mut patched_stored = compressed;
    patched_stored.resize(source_stored.len(), 0);
    ensure!(
        decompress(&patched_stored, true)? == patched_decoded,
        "name-entry font compression roundtrip changed bytes"
    );

    let report = DialogueNameEntryFontBuildReport {
        kind: "Justice Gakuen 2 non-release development name-entry font build".to_string(),
        source_path: NAME_ENTRY_FONT_PATH.to_string(),
        source_stored_sha256,
        source_decoded_sha256,
        source_record_size: source_stored.len(),
        runtime_code_region,
        runtime_program: runtime_program.report,
        embedded_font_tim_offset: format!("0x{FONT_TIM_OFFSET:05x}"),
        embedded_font_tim_size: font_tim.total_size,
        embedded_font_image_vram_x: font_tim.image_x,
        embedded_font_image_vram_y: font_tim.image_y,
        embedded_font_image_width: font_tim.pixel_width(),
        embedded_font_image_height: font_tim.image_height,
        source_background_tim_overlap_byte_count,
        glyph_layout_table_offset: format!("0x{LAYOUT_TABLE_OFFSET:04x}"),
        glyph_layout_table_sha256: LAYOUT_TABLE_SHA256.to_string(),
        font_name,
        font_sha256,
        font_px: candidate_font_px,
        outline_palette_index: OUTLINE_PALETTE_INDEX,
        fill_palette_index: FILL_PALETTE_INDEX,
        installed_glyph_count: installs.len(),
        changed_decoded_byte_count,
        patched_decoded_sha256: sha256_bytes(&patched_decoded),
        compressed_stream_size,
        compression_maximum_match_words: source_compression_profile.maximum_match_words,
        compression_maximum_control_block_output_words: source_compression_profile
            .maximum_control_block_output_words,
        source_compression_stream_byte_count: source_compression_profile.stream_byte_count,
        source_compression_control_blocks_crossing_input_pages: source_compression_profile
            .control_blocks_crossing_input_pages,
        rebuilt_compression_control_blocks_crossing_input_pages: rebuilt_compression_profile
            .control_blocks_crossing_input_pages,
        patched_stored_sha256: sha256_bytes(&patched_stored),
        stored_output_file: NAME_ENTRY_FONT_OUTPUT_FILE.to_string(),
        all_glyph_cells_unique: true,
        compressed_within_original_extent: true,
        compression_roundtrip_verified: true,
        fixed_graphics,
        all_fixed_graphic_surfaces_disjoint: true,
        installs,
    };
    Ok((patched_stored, report))
}

pub(super) fn load_all_name_entry_glyph_cells(
    source_overlay: &[u8],
) -> Result<Vec<NameEntryGlyphCell>> {
    let layout_size = crate::name_input::NAME_GLYPH_CODE_COUNT * LAYOUT_RECORD_SIZE;
    let layout = source_overlay
        .get(LAYOUT_TABLE_OFFSET..LAYOUT_TABLE_OFFSET + layout_size)
        .context("name-entry glyph layout table is truncated")?;
    ensure!(
        sha256_bytes(layout) == LAYOUT_TABLE_SHA256,
        "name-entry glyph layout table changed"
    );
    let mut unique = BTreeSet::new();
    let mut cells = Vec::with_capacity(crate::name_input::NAME_GLYPH_CODE_COUNT);
    for (sequence_position, record) in layout
        .as_chunks::<LAYOUT_RECORD_SIZE>()
        .0
        .iter()
        .enumerate()
    {
        let cell = glyph_cell_from_triplet(record[0], record[1], record[2])?;
        let expected_texture_page = match sequence_position {
            0..84 => 0x0c,
            84..166 => 0x0d,
            166..228 => 0x0e,
            _ => unreachable!("layout iteration is bounded to 228 records"),
        };
        ensure!(
            cell.texture_page == expected_texture_page,
            "name-entry glyph layout crossed its source page"
        );
        ensure!(
            unique.insert((cell.texture_page, cell.column, cell.row)),
            "name-entry glyph layout reuses a physical cell"
        );
        cells.push(cell);
    }
    Ok(cells)
}

pub(super) fn install_all_name_entry_fixed_graphics(
    source_overlay: &[u8],
    source_decoded: &[u8],
    patched_decoded: &mut [u8],
    graphics_root: &Path,
    fixed_graphics_font_path: &Path,
    protected_glyph_cells: &[Cell],
) -> Result<Vec<super::name_entry_fixed_graphics_model::DialogueNameEntryFixedGraphicBuildReport>> {
    validate_face_feature_consumer(source_overlay)?;
    let fixed_graphics_source = NameEntryFixedGraphicsSource {
        path: NAME_ENTRY_FONT_PATH,
        decoded_sha256: SOURCE_DECODED_SHA256,
    };
    let face_features = install_name_entry_fixed_graphics(
        source_decoded,
        patched_decoded,
        graphics_root,
        fixed_graphics_font_path,
        fixed_graphics_source,
        face_feature_labels_spec(),
        protected_glyph_cells,
    )?;
    validate_face_feature_cells(&face_features)?;
    let fixed_graphics = vec![
        face_features,
        install_name_entry_fixed_graphics(
            source_decoded,
            patched_decoded,
            graphics_root,
            fixed_graphics_font_path,
            fixed_graphics_source,
            application_form_spec(),
            &[],
        )?,
        install_name_entry_fixed_graphics(
            source_decoded,
            patched_decoded,
            graphics_root,
            fixed_graphics_font_path,
            fixed_graphics_source,
            candidate_controls_spec(),
            protected_glyph_cells,
        )?,
        install_name_entry_fixed_graphics(
            source_decoded,
            patched_decoded,
            graphics_root,
            fixed_graphics_font_path,
            fixed_graphics_source,
            school_choices_spec(),
            protected_glyph_cells,
        )?,
        install_name_entry_fixed_graphics(
            source_decoded,
            patched_decoded,
            graphics_root,
            fixed_graphics_font_path,
            fixed_graphics_source,
            subject_choices_spec(),
            protected_glyph_cells,
        )?,
        install_name_entry_fixed_graphics(
            source_decoded,
            patched_decoded,
            graphics_root,
            fixed_graphics_font_path,
            fixed_graphics_source,
            favorite_word_choices_spec(),
            protected_glyph_cells,
        )?,
        install_name_entry_fixed_graphics(
            source_decoded,
            patched_decoded,
            graphics_root,
            fixed_graphics_font_path,
            fixed_graphics_source,
            sex_choices_spec(),
            protected_glyph_cells,
        )?,
    ];
    validate_fixed_graphic_surfaces_disjoint(&fixed_graphics)?;
    Ok(fixed_graphics)
}

pub(super) fn glyph_cell_from_triplet(
    texture_page: u8,
    column: u8,
    row: u8,
) -> Result<NameEntryGlyphCell> {
    ensure!(
        (TEXTURE_PAGE_START..=TEXTURE_PAGE_END).contains(&texture_page),
        "name-entry texture page is outside the embedded font atlas"
    );
    ensure!(
        column < COLUMN_COUNT && row < ROW_COUNT,
        "name-entry glyph coordinate is outside the 12x7 page grid"
    );
    let page = usize::from(texture_page - TEXTURE_PAGE_START);
    Ok(NameEntryGlyphCell {
        texture_page,
        column,
        row,
        cell: Cell {
            x: page * TEXTURE_PAGE_PIXEL_WIDTH + usize::from(column) * CELL_WIDTH,
            y: usize::from(row) * CELL_HEIGHT,
            width: CELL_WIDTH,
            height: CELL_HEIGHT,
        },
    })
}
