//! Renders only fixed result labels backed by source hashes and validated consumers.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, bail, ensure};

use crate::decoded_record_write_plan::DecodedDataClaim;
use crate::development_build_spec::ShiftedSizedFontSource;
use crate::font::{HorizontalTextAlignment, IndexedTextRasterizer, IndexedTextRasterizers};
use crate::mode_descendant_graphics::gorin_heading::{dilate_mask, reconstruct_indexed_background};
use crate::mode_descendant_graphics::model::{ModeDescendantFontRole, ModeDescendantFontSources};
use crate::mode_descendant_graphics::source::ModeDescendantSourceRecord;
use crate::pipeline::sha256_bytes;
use crate::tim::{
    Cell, IndexedCellWrite, cells_overlap, read_4bpp_palette_words_in_prefix,
    read_8bpp_indexed_cell_in_prefix, read_8bpp_palette_words_in_prefix,
    read_indexed_cell_in_prefix, write_8bpp_indexed_cell_in_prefix_with_report,
    write_indexed_cell_in_prefix_with_report,
};

use super::super::catalog::{
    PRACTICAL_1999_RESULTS_PATH, PRACTICAL_1999_TEXTURE_PATH, PRACTICAL_BASICS_RESULTS_PATH,
    PRACTICAL_BASICS_TEXTURE_PATH,
};
use super::assets::parse_hex_offset;
use super::model::{
    PracticalResultDevelopmentStatus, PracticalResultEntry, PracticalResultFixedTextTargetCatalog,
    PracticalResultFixedTextTargetId, PracticalResultIndexedArtwork, PracticalResultStrategy,
};

const TRANSPARENT_MARKER: u8 = 0;
const OUTLINE_MARKER: u8 = 254;
const FILL_MARKER: u8 = 255;
const DEFAULT_SOURCE_TEXT_MASK_DILATION_STEPS: usize = 1;
const SIKEN2_SOURCE_TEXT_MASK_DILATION_STEPS: usize = 3;
const SIKEN10_OUTLINE_PALETTE_WORD: u16 = 0x8421;
const SIKEN10_FILL_PALETTE_WORD: u16 = 0xfcab;
const SIKEN20_RESULT_SUMMARY_TIM_OFFSET: usize = 0x75800;
const SIKEN20_RESULT_SUMMARY_OUTLINE_PALETTE_WORD: u16 = 0x96f0;
const SIKEN20_RESULT_SUMMARY_FILL_PALETTE_WORD: u16 = 0xfd17;
const SIKEN20_RESULT_ANNOUNCEMENT_TARGET_ID: &str =
    "target__exam_result_announcement__siken20_member01_75800";
const SIKEN20_RESULT_ANNOUNCEMENT_OUTLINE_PALETTE_WORD: u16 = 0x0c63;
const SIKEN20_RESULT_ANNOUNCEMENT_FILL_PALETTE_WORD: u16 = 0x3be0;
// Match the decorative backdrop, including its STP bit. RGB-only equality
// produces opaque rectangles when the result sheet is blended at runtime.
const SIKEN20_RESULT_BACKGROUND_PALETTE_WORD: u16 = 0x816e;
const SIKEN20_RESULT_BACK_HINT_TARGET_ID: &str =
    "target__exam_records_back_hint_text__siken20_member01_75800";
const SIKEN20_RESULT_BACK_HINT_OUTLINE_PALETTE_WORD: u16 = 0x0421;
const SIKEN20_RESULT_BACK_HINT_FILL_PALETTE_WORD: u16 = 0x69f4;
const SIKEN20_ATTENDANCE_BOOK_TARGET_ID: &str =
    "target__attendance_book_label__siken20_member01_75800";
const SIKEN20_ATTENDANCE_BOOK_CELL: Cell = Cell {
    x: 224,
    y: 294,
    width: 45,
    height: 34,
};
const SIKEN20_ATTENDANCE_BOOK_BORDER_INDEX: u8 = 60;
const SIKEN20_ATTENDANCE_BOOK_BORDER_WORD: u16 = 0xb18d;
const SIKEN20_ATTENDANCE_BOOK_PAPER_INDEX: u8 = 241;
const SIKEN20_ATTENDANCE_BOOK_PAPER_WORD: u16 = 0xef7b;
const SIKEN20_ATTENDANCE_BOOK_INK_INDEX: u8 = 4;
const SIKEN20_ATTENDANCE_BOOK_INK_WORD: u16 = 0x9084;
const SIKEN1_HEADING_OUTLINE_PALETTE_WORD: u16 = 0x8842;
const SIKEN1_HEADING_FILL_PALETTE_WORD: u16 = 0xd416;
const SIKEN1_LABEL_PALETTE_WORD: u16 = 0xd425;
const SIKEN1_TABLE_LINE_PALETTE_WORD: u16 = 0x827f;
const SIKEN2_HEADING_OUTLINE_PALETTE_WORD: u16 = 0x0421;
const SIKEN2_HEADING_FILL_PALETTE_WORD: u16 = 0x00fc;
const SIKEN2_LABEL_PALETTE_WORD: u16 = 0x0c70;
const SIKEN2_TABLE_LINE_PALETTE_WORD: u16 = 0x63a2;
const SIKEN2_DIRECT_LABEL_PALETTE_INDEX: usize = 7;
const SIKEN2_SMALL_JUDGMENT_PALETTE_INDEX: usize = 6;

#[derive(Clone, Copy)]
struct FixedTextCompositionStyle {
    outline_palette_index: Option<u8>,
    fill_palette_index: u8,
    source_text_mask_dilation_steps: usize,
    table_line_palette_word: Option<u16>,
    sample_source_background: bool,
    paper_blue_tolerance: i16,
    source_ink_classifier: SourceInkClassifier,
}

#[derive(Clone, Copy)]
enum SourceInkClassifier {
    PaperContrast,
    CyanAnnouncement,
    VioletHint,
}

pub(super) struct FixedTextPlan {
    pub(super) deferred_entry_ids: BTreeSet<String>,
    pub(super) validated_target_count: usize,
    pub(super) layout_candidate_count: usize,
    pub(super) expected_write_count: usize,
    pub(super) write_contract_complete: bool,
}

pub(super) struct RenderedFixedText {
    pub(super) decoded: Vec<u8>,
    pub(super) decoded_write_claims: Vec<DecodedDataClaim>,
    pub(super) completed_target_ids: BTreeSet<PracticalResultFixedTextTargetId>,
    pub(super) changed_bytes_by_entry: BTreeMap<String, usize>,
    pub(super) rendered_references_by_entry: BTreeMap<String, usize>,
    pub(super) cells_are_unique_and_non_overlapping: bool,
    pub(super) changes_confined_to_owned_cells: bool,
}

pub(super) struct FixedTextCompletion {
    pub(super) completed_target_ids: BTreeSet<PracticalResultFixedTextTargetId>,
    pub(super) changed_bytes_by_entry: BTreeMap<String, usize>,
    pub(super) rendered_references_by_entry: BTreeMap<String, usize>,
    pub(super) cells_are_unique_and_non_overlapping: bool,
    pub(super) changes_confined_to_owned_cells: bool,
}

pub(super) fn merge_rendered_fixed_text(
    rendered_records: &[&RenderedFixedText],
) -> Result<FixedTextCompletion> {
    ensure!(
        !rendered_records.is_empty(),
        "practical-result fixed-text completion has no rendered records"
    );
    let mut completed_target_ids = BTreeSet::new();
    let mut changed_bytes_by_entry = BTreeMap::<String, usize>::new();
    let mut rendered_references_by_entry = BTreeMap::<String, usize>::new();
    for rendered in rendered_records {
        for target_id in &rendered.completed_target_ids {
            ensure!(
                completed_target_ids.insert(target_id.clone()),
                "practical-result fixed-text target {target_id} completed in two records"
            );
        }
        for (entry_id, changed_byte_count) in &rendered.changed_bytes_by_entry {
            *changed_bytes_by_entry.entry(entry_id.clone()).or_default() += changed_byte_count;
        }
        for (entry_id, rendered_reference_count) in &rendered.rendered_references_by_entry {
            *rendered_references_by_entry
                .entry(entry_id.clone())
                .or_default() += rendered_reference_count;
        }
    }
    Ok(FixedTextCompletion {
        completed_target_ids,
        changed_bytes_by_entry,
        rendered_references_by_entry,
        cells_are_unique_and_non_overlapping: rendered_records
            .iter()
            .all(|rendered| rendered.cells_are_unique_and_non_overlapping),
        changes_confined_to_owned_cells: rendered_records
            .iter()
            .all(|rendered| rendered.changes_confined_to_owned_cells),
    })
}

pub(super) fn render_fixed_text_entries(
    entries: &[PracticalResultEntry],
    catalog: &PracticalResultFixedTextTargetCatalog,
    fonts: &ModeDescendantFontSources,
    source: &ModeDescendantSourceRecord,
    base_decoded: &[u8],
) -> Result<RenderedFixedText> {
    ensure!(
        source.decoded.len() == base_decoded.len(),
        "practical-result fixed-text base changed record extent"
    );
    let entries_by_id = entries
        .iter()
        .map(|entry| (entry.id.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    let mut occupied = Vec::<(usize, Cell)>::new();
    let mut rasterizers = IndexedTextRasterizers::default();
    let mut patched = base_decoded.to_vec();
    let mut decoded_write_claims = Vec::new();
    let mut completed_target_ids = BTreeSet::new();
    let mut changed_bytes_by_entry = BTreeMap::<String, usize>::new();
    let mut rendered_references_by_entry = BTreeMap::<String, usize>::new();

    let targets = catalog
        .targets
        .iter()
        .filter(|target| target.target_record_path == source.path)
        .collect::<Vec<_>>();
    ensure!(
        !targets.is_empty(),
        "practical-result source {} has no authorized fixed-text targets",
        source.path
    );
    for target in targets {
        ensure!(
            matches!(target.target_bpp, 4 | 8),
            "fixed-text target {} has an unsupported pixel format",
            target.target_id
        );
        let tim_offset = parse_hex_offset(&target.target_tim_offset)?;
        ensure!(
            occupied.iter().all(|(offset, cell)| {
                *offset != tim_offset || !cells_overlap(*cell, target.target_cell)
            }),
            "fixed-text target {} overlaps another owned cell",
            target.target_id
        );
        occupied.push((tim_offset, target.target_cell));
        let entry = entries_by_id
            .get(target.semantic_entry_id.as_str())
            .with_context(|| format!("fixed-text target {} lost its entry", target.target_id))?;
        ensure!(
            entry.strategy == PracticalResultStrategy::FixedText
                && entry.development_status == PracticalResultDevelopmentStatus::Authored,
            "fixed-text target {} names an unauthored entry",
            target.target_id
        );
        let korean_text = entry
            .korean_text
            .as_deref()
            .context("authored practical-result fixed text lost Korean text")?;
        let source_pixels = match target.target_bpp {
            4 => read_indexed_cell_in_prefix(&source.decoded, tim_offset, target.target_cell)?,
            8 => read_8bpp_indexed_cell_in_prefix(&source.decoded, tim_offset, target.target_cell)?,
            _ => unreachable!("validated practical-result fixed-text bpp"),
        };
        let actual_preimage_indexed_sha256 = sha256_bytes(&source_pixels);
        ensure!(
            actual_preimage_indexed_sha256 == target.expected_preimage_indexed_sha256,
            "fixed-text target {} preimage changed: expected {}, got {}",
            target.target_id,
            target.expected_preimage_indexed_sha256,
            actual_preimage_indexed_sha256
        );
        let style = font_for_entry(fonts, entry)?;
        let rasterizer = rasterizers.for_font(&style.path)?;
        let alignment = match target.alignment {
            crate::mode_descendant_graphics::model::TextAlignment::Left => {
                HorizontalTextAlignment::Left
            }
            crate::mode_descendant_graphics::model::TextAlignment::Center => {
                HorizontalTextAlignment::Center
            }
        };
        let write = match (target.target_bpp, entry.font_role) {
            (4, ModeDescendantFontRole::PracticalResultSmallJudgment) => {
                render_small_judgment_target(
                    &mut patched,
                    tim_offset,
                    target.target_cell,
                    source.path,
                    entry.font_role,
                    korean_text,
                    style.font_px,
                    style.vertical_shift_px,
                    alignment,
                    &source_pixels,
                    rasterizer,
                )?
            }
            (4, ModeDescendantFontRole::PracticalResultLabel) => render_indexed_label_target(
                &mut patched,
                tim_offset,
                target.target_cell,
                source.path,
                entry.font_role,
                korean_text,
                style.font_px,
                style.vertical_shift_px,
                alignment,
                &source_pixels,
                rasterizer,
            )?,
            (4, role) => bail!(
                "4-bpp fixed-text target {} has unsupported role {role:?}",
                target.target_id
            ),
            (8, _) if target.target_id.as_str() == SIKEN20_ATTENDANCE_BOOK_TARGET_ID => {
                render_attendance_book_label_target(
                    &mut patched,
                    tim_offset,
                    target.target_cell,
                    source.path,
                    korean_text,
                    rasterizer,
                    target.indexed_artwork.as_ref(),
                )
                .with_context(|| format!("render fixed-text target {}", target.target_id))?
            }
            (8, _) => render_result_sheet_target(
                &mut patched,
                tim_offset,
                target.target_cell,
                source.path,
                target.target_id.as_str(),
                entry.font_role,
                korean_text,
                entry.id == "exam_records_remaining_time",
                style.font_px,
                style.vertical_shift_px,
                alignment,
                &source_pixels,
                rasterizer,
            )
            .with_context(|| format!("render fixed-text target {}", target.target_id))?,
            _ => unreachable!("validated practical-result fixed-text bpp"),
        };
        ensure!(
            write.changed_byte_count > 0,
            "fixed-text target {} changed no bytes",
            target.target_id
        );
        decoded_write_claims.extend(DecodedDataClaim::from_effective_ranges(
            &format!("mode-descendant:practical-result:{}", target.target_id),
            &format!("render practical-result fixed text {}", entry.id),
            &source.decoded,
            &patched,
            write.allowed_ranges,
        )?);
        ensure!(
            completed_target_ids.insert(target.target_id.clone()),
            "fixed-text target {} rendered twice",
            target.target_id
        );
        *changed_bytes_by_entry.entry(entry.id.clone()).or_default() += write.changed_byte_count;
        *rendered_references_by_entry
            .entry(entry.id.clone())
            .or_default() += 1;
    }
    ensure!(
        !completed_target_ids.is_empty() && !decoded_write_claims.is_empty(),
        "practical-result fixed-text build completed no targets"
    );
    Ok(RenderedFixedText {
        decoded: patched,
        decoded_write_claims,
        completed_target_ids,
        changed_bytes_by_entry,
        rendered_references_by_entry,
        cells_are_unique_and_non_overlapping: true,
        changes_confined_to_owned_cells: true,
    })
}

fn render_attendance_book_label_target(
    patched: &mut [u8],
    tim_offset: usize,
    cell: Cell,
    source_path: &str,
    korean_text: &str,
    rasterizer: &IndexedTextRasterizer,
    artwork: Option<&PracticalResultIndexedArtwork>,
) -> Result<IndexedCellWrite> {
    ensure!(
        source_path == PRACTICAL_1999_RESULTS_PATH
            && tim_offset == SIKEN20_RESULT_SUMMARY_TIM_OFFSET
            && cell == SIKEN20_ATTENDANCE_BOOK_CELL,
        "attendance-book label moved away from its reviewed object silhouette"
    );
    let palette = read_8bpp_palette_words_in_prefix(patched, tim_offset, 0)?;
    for (index, expected) in [
        (
            SIKEN20_ATTENDANCE_BOOK_BORDER_INDEX,
            SIKEN20_ATTENDANCE_BOOK_BORDER_WORD,
        ),
        (
            SIKEN20_ATTENDANCE_BOOK_PAPER_INDEX,
            SIKEN20_ATTENDANCE_BOOK_PAPER_WORD,
        ),
        (
            SIKEN20_ATTENDANCE_BOOK_INK_INDEX,
            SIKEN20_ATTENDANCE_BOOK_INK_WORD,
        ),
    ] {
        ensure!(
            palette[usize::from(index)] == expected,
            "attendance-book label palette index {index} changed"
        );
    }

    let mut output = read_8bpp_indexed_cell_in_prefix(patched, tim_offset, cell)?;
    if let Some(artwork) = artwork {
        ensure!(korean_text == "출석부", "indexed book artwork text changed");
        let replacement = std::fs::read(&artwork.indices_file)
            .with_context(|| format!("read indexed artwork {}", artwork.indices_file.display()))?;
        ensure!(
            replacement.len() == output.len()
                && sha256_bytes(&replacement) == artwork.indices_sha256,
            "attendance-book indexed artwork size or hash mismatch"
        );
        let palette_bytes = palette
            .iter()
            .flat_map(|word| word.to_le_bytes())
            .collect::<Vec<_>>();
        ensure!(
            sha256_bytes(&palette_bytes) == artwork.palette_sha256,
            "attendance-book artwork palette changed"
        );
        let mut mask = vec![0; output.len()];
        fill_convex_polygon(&mut mask, cell, &[(3, 2), (44, 8), (40, 32), (0, 24)], 1);
        ensure!(
            replacement
                .iter()
                .zip(&output)
                .zip(mask)
                .all(|((&new, &old), owned)| owned != 0 || new == old),
            "attendance-book artwork changes pixels outside the label silhouette"
        );
        return write_8bpp_indexed_cell_in_prefix_with_report(
            patched,
            tim_offset,
            cell,
            &replacement,
        );
    }
    fill_convex_polygon(
        &mut output,
        cell,
        &[(3, 2), (44, 8), (40, 32), (0, 24)],
        SIKEN20_ATTENDANCE_BOOK_BORDER_INDEX,
    );
    fill_convex_polygon(
        &mut output,
        cell,
        &[(5, 4), (42, 9), (38, 29), (3, 22)],
        SIKEN20_ATTENDANCE_BOOK_PAPER_INDEX,
    );

    let raster = rasterizer.rasterize_shifted(
        korean_text,
        38,
        16,
        10.0,
        2.0,
        0,
        TRANSPARENT_MARKER,
        None,
        FILL_MARKER,
        HorizontalTextAlignment::Center,
    )?;
    let painted = paint_affine_raster(
        &mut output,
        cell,
        &raster.pixels,
        38,
        16,
        (4, 6),
        (42, 11),
        (2, 23),
        SIKEN20_ATTENDANCE_BOOK_INK_INDEX,
    );
    ensure!(
        painted > 0,
        "attendance-book Korean label rendered no pixels"
    );
    write_8bpp_indexed_cell_in_prefix_with_report(patched, tim_offset, cell, &output)
}

fn fill_convex_polygon(pixels: &mut [u8], cell: Cell, points: &[(i32, i32)], palette_index: u8) {
    for y in 0..cell.height {
        for x in 0..cell.width {
            let point = (2 * x as i32 + 1, 2 * y as i32 + 1);
            let contains = points
                .iter()
                .zip(points.iter().cycle().skip(1))
                .take(points.len())
                .map(|(&(ax, ay), &(bx, by))| {
                    let ax = 2 * ax;
                    let ay = 2 * ay;
                    let bx = 2 * bx;
                    let by = 2 * by;
                    i64::from(bx - ax) * i64::from(point.1 - ay)
                        - i64::from(by - ay) * i64::from(point.0 - ax)
                })
                .all(|cross| cross >= 0);
            if contains {
                pixels[y * cell.width + x] = palette_index;
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_affine_raster(
    pixels: &mut [u8],
    cell: Cell,
    raster: &[u8],
    raster_width: usize,
    raster_height: usize,
    top_left: (i32, i32),
    top_right: (i32, i32),
    bottom_left: (i32, i32),
    palette_index: u8,
) -> usize {
    let ux = top_right.0 - top_left.0;
    let uy = top_right.1 - top_left.1;
    let vx = bottom_left.0 - top_left.0;
    let vy = bottom_left.1 - top_left.1;
    let determinant = ux * vy - uy * vx;
    debug_assert!(determinant > 0);
    let doubled_determinant = 2 * determinant;
    let mut painted = 0usize;
    for y in 0..cell.height {
        for x in 0..cell.width {
            let dx = 2 * x as i32 + 1 - 2 * top_left.0;
            let dy = 2 * y as i32 + 1 - 2 * top_left.1;
            let u = dx * vy - dy * vx;
            let v = ux * dy - uy * dx;
            if !(0..doubled_determinant).contains(&u) || !(0..doubled_determinant).contains(&v) {
                continue;
            }
            let raster_x = u as usize * raster_width / doubled_determinant as usize;
            let raster_y = v as usize * raster_height / doubled_determinant as usize;
            if raster[raster_y * raster_width + raster_x] == FILL_MARKER {
                pixels[y * cell.width + x] = palette_index;
                painted += 1;
            }
        }
    }
    painted
}

#[allow(clippy::too_many_arguments)]
fn render_indexed_label_target(
    patched: &mut [u8],
    tim_offset: usize,
    cell: Cell,
    source_path: &str,
    font_role: ModeDescendantFontRole,
    korean_text: &str,
    font_px: f32,
    vertical_shift_px: i32,
    alignment: HorizontalTextAlignment,
    source_pixels: &[u8],
    rasterizer: &IndexedTextRasterizer,
) -> Result<IndexedCellWrite> {
    ensure!(
        matches!(
            source_path,
            PRACTICAL_BASICS_RESULTS_PATH | PRACTICAL_1999_RESULTS_PATH
        ) && font_role == ModeDescendantFontRole::PracticalResultLabel,
        "4-bpp indexed fixed text is not a practical-result label"
    );
    let palette =
        read_4bpp_palette_words_in_prefix(patched, tim_offset, SIKEN2_DIRECT_LABEL_PALETTE_INDEX)?;
    let (clear_index, first_ink_index, last_ink_index) =
        super::action_cells::action_palette_roles(&[source_pixels.to_vec()], &palette)?;
    ensure!(
        source_pixels.iter().all(|pixel| {
            *pixel == clear_index || (first_ink_index..=last_ink_index).contains(pixel)
        }),
        "4-bpp practical-result label mixes text with another graphic"
    );
    let lines = korean_text.split_whitespace().collect::<Vec<_>>();
    let line_heights = match lines.len() {
        1 => vec![cell.height],
        2 => {
            let first_line_height = cell.height / 2;
            vec![first_line_height, cell.height - first_line_height]
        }
        _ => bail!("4-bpp practical-result label needs one or two lines"),
    };
    let mut pixels = Vec::with_capacity(cell.width * cell.height);
    for (line, line_height) in lines.into_iter().zip(line_heights) {
        let fitted_font_px = font_px.min(line_height.saturating_sub(1) as f32);
        let raster = rasterizer.rasterize_shifted_with_coverage_ramp(
            line,
            cell.width,
            line_height,
            fitted_font_px,
            0.0,
            vertical_shift_px,
            clear_index,
            first_ink_index,
            last_ink_index,
            alignment,
        )?;
        pixels.extend(raster.pixels);
    }
    write_indexed_cell_in_prefix_with_report(patched, tim_offset, cell, &pixels)
}

#[allow(clippy::too_many_arguments)]
fn render_result_sheet_target(
    patched: &mut [u8],
    tim_offset: usize,
    cell: Cell,
    source_path: &str,
    target_id: &str,
    font_role: ModeDescendantFontRole,
    korean_text: &str,
    stack_words: bool,
    font_px: f32,
    vertical_shift_px: i32,
    alignment: HorizontalTextAlignment,
    source_pixels: &[u8],
    rasterizer: &IndexedTextRasterizer,
) -> Result<IndexedCellWrite> {
    let palette = read_8bpp_palette_words_in_prefix(patched, tim_offset, 0)?;
    let (outline_palette_index, fill_palette_index) = palette_roles(
        source_path,
        tim_offset,
        target_id,
        font_role,
        source_pixels,
        &palette,
    )?;
    // The baked remaining-time label shares a row with separately drawn digits.
    // Preserve its source two-line footprint rather than widening into the record.
    let lines = if stack_words {
        let words = korean_text.split_whitespace().collect::<Vec<_>>();
        ensure!(words.len() == 2, "remaining-time label needs two words");
        words
    } else {
        vec![korean_text]
    };
    let mut pixels = vec![TRANSPARENT_MARKER; cell.width * cell.height];
    for (index, line) in lines.iter().enumerate() {
        let top = index * cell.height / lines.len();
        let bottom = (index + 1) * cell.height / lines.len();
        let fitted_font_px = if stack_words {
            font_px.min((bottom - top).saturating_sub(2) as f32)
        } else {
            font_px
        };
        let raster = rasterizer.rasterize_shifted(
            line,
            cell.width,
            bottom - top,
            fitted_font_px,
            0.0,
            vertical_shift_px,
            TRANSPARENT_MARKER,
            outline_palette_index.map(|_| OUTLINE_MARKER),
            FILL_MARKER,
            alignment,
        )?;
        pixels[top * cell.width..bottom * cell.width].copy_from_slice(&raster.pixels);
    }
    // These two labels sit directly on the decorative backdrop reconstructed
    // before fixed text. Preserve that layer instead of painting a flat rectangle.
    if matches!(
        target_id,
        SIKEN20_RESULT_ANNOUNCEMENT_TARGET_ID | SIKEN20_RESULT_BACK_HINT_TARGET_ID
    ) {
        let mut background = read_8bpp_indexed_cell_in_prefix(patched, tim_offset, cell)?;
        overlay_result_raster(
            &mut background,
            &pixels,
            outline_palette_index,
            fill_palette_index,
        )?;
        return write_8bpp_indexed_cell_in_prefix_with_report(
            patched,
            tim_offset,
            cell,
            &background,
        );
    }
    let composed = compose_result_text(
        source_pixels,
        cell,
        &palette,
        &pixels,
        FixedTextCompositionStyle {
            outline_palette_index,
            fill_palette_index,
            source_text_mask_dilation_steps: source_text_mask_dilation_steps(source_path),
            table_line_palette_word: table_line_palette_word(source_path)?,
            sample_source_background: matches!(
                source_path,
                PRACTICAL_BASICS_TEXTURE_PATH | PRACTICAL_BASICS_RESULTS_PATH
            ),
            paper_blue_tolerance: if source_path == PRACTICAL_BASICS_TEXTURE_PATH {
                8
            } else {
                4
            },
            source_ink_classifier: source_ink_classifier(target_id),
        },
    )?;
    write_8bpp_indexed_cell_in_prefix_with_report(patched, tim_offset, cell, &composed)
}

#[allow(clippy::too_many_arguments)]
fn render_small_judgment_target(
    patched: &mut [u8],
    tim_offset: usize,
    cell: Cell,
    source_path: &str,
    font_role: ModeDescendantFontRole,
    korean_text: &str,
    font_px: f32,
    vertical_shift_px: i32,
    alignment: HorizontalTextAlignment,
    source_pixels: &[u8],
    rasterizer: &IndexedTextRasterizer,
) -> Result<IndexedCellWrite> {
    ensure!(
        matches!(
            source_path,
            PRACTICAL_BASICS_RESULTS_PATH | PRACTICAL_1999_RESULTS_PATH
        ) && font_role == ModeDescendantFontRole::PracticalResultSmallJudgment,
        "4-bpp fixed text is not a practical-result small judgment"
    );
    let palette = read_4bpp_palette_words_in_prefix(
        patched,
        tim_offset,
        SIKEN2_SMALL_JUDGMENT_PALETTE_INDEX,
    )?;
    let roles = small_judgment_palette_roles(source_pixels, &palette)?;
    let character_count = korean_text.chars().count();
    let inner_width = cell.width.saturating_sub(8);
    ensure!(
        character_count > 0 && inner_width > 0,
        "small-judgment text or badge interior is empty"
    );
    let fitted_font_px = font_px.min(inner_width as f32 / character_count as f32);
    let raster = rasterizer.rasterize_shifted(
        korean_text,
        cell.width,
        cell.height,
        fitted_font_px,
        0.0,
        vertical_shift_px,
        TRANSPARENT_MARKER,
        None,
        FILL_MARKER,
        alignment,
    )?;
    let composed = compose_small_judgment_badge(source_pixels, &palette, &raster.pixels, roles)?;
    write_indexed_cell_in_prefix_with_report(patched, tim_offset, cell, &composed)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SmallJudgmentPaletteRoles {
    transparent_index: u8,
    background_index: u8,
    outline_index: u8,
    fill_index: u8,
}

fn small_judgment_palette_roles(
    source_pixels: &[u8],
    palette: &[u16; 16],
) -> Result<SmallJudgmentPaletteRoles> {
    let mut population = [0usize; 16];
    for &index in source_pixels {
        ensure!(index < 16, "small-judgment source pixel is not 4-bpp");
        population[usize::from(index)] += 1;
    }
    let transparent_index = palette
        .iter()
        .enumerate()
        .filter(|(index, word)| population[*index] > 0 && (**word & 0x7fff) == 0)
        .max_by_key(|(index, _)| population[*index])
        .map(|(index, _)| index as u8)
        .context("small-judgment badge has no transparent source index")?;
    let background_index = population
        .iter()
        .enumerate()
        .filter(|(index, count)| {
            **count > 0
                && *index != usize::from(transparent_index)
                && (palette[*index] & 0x7fff) != 0
        })
        .max_by_key(|(index, count)| (**count, std::cmp::Reverse(*index)))
        .map(|(index, _)| index as u8)
        .context("small-judgment badge has no colored background index")?;
    let ink_indexes = population
        .iter()
        .enumerate()
        .filter(|(index, count)| {
            **count > 0
                && *index != usize::from(transparent_index)
                && *index != usize::from(background_index)
                && (palette[*index] & 0x7fff) != 0
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    ensure!(
        ink_indexes.len() >= 2
            && population[usize::from(background_index)]
                > ink_indexes
                    .iter()
                    .map(|index| population[*index])
                    .max()
                    .unwrap_or_default(),
        "small-judgment background and source ink are not separable"
    );
    let brightness = |index: usize| {
        let word = palette[index];
        usize::from((word & 0x1f) as u8)
            + usize::from(((word >> 5) & 0x1f) as u8)
            + usize::from(((word >> 10) & 0x1f) as u8)
    };
    let fill_index = ink_indexes
        .iter()
        .copied()
        .max_by_key(|index| (brightness(*index), std::cmp::Reverse(*index)))
        .context("small-judgment badge has no fill index")?;
    let midpoint = (brightness(usize::from(background_index)) + brightness(fill_index)) / 2;
    let outline_index = ink_indexes
        .iter()
        .copied()
        .filter(|index| *index != fill_index)
        .min_by_key(|index| (brightness(*index).abs_diff(midpoint), *index))
        .context("small-judgment badge has no outline index")?;
    Ok(SmallJudgmentPaletteRoles {
        transparent_index,
        background_index,
        outline_index: outline_index as u8,
        fill_index: fill_index as u8,
    })
}

fn compose_small_judgment_badge(
    source_pixels: &[u8],
    palette: &[u16; 16],
    raster: &[u8],
    roles: SmallJudgmentPaletteRoles,
) -> Result<Vec<u8>> {
    ensure!(
        source_pixels.len() == raster.len() && !source_pixels.is_empty(),
        "small-judgment source and raster geometry differ"
    );
    let mut output = source_pixels.to_vec();
    let mut removed_source_ink_count = 0usize;
    for pixel in &mut output {
        if *pixel != roles.transparent_index && *pixel != roles.background_index {
            ensure!(
                (palette[usize::from(*pixel)] & 0x7fff) != 0,
                "small-judgment source uses an ambiguous transparent index"
            );
            *pixel = roles.background_index;
            removed_source_ink_count += 1;
        }
    }
    ensure!(
        removed_source_ink_count > 0,
        "small-judgment badge has no source ink to replace"
    );
    let mut korean_ink_count = 0usize;
    for (output, marker) in output.iter_mut().zip(raster) {
        if *output == roles.transparent_index {
            continue;
        }
        match *marker {
            TRANSPARENT_MARKER => {}
            OUTLINE_MARKER => {
                *output = roles.outline_index;
                korean_ink_count += 1;
            }
            FILL_MARKER => {
                *output = roles.fill_index;
                korean_ink_count += 1;
            }
            marker => bail!("small-judgment raster contains marker {marker}"),
        }
    }
    ensure!(
        korean_ink_count > 0,
        "small-judgment Korean text fell outside its badge silhouette"
    );
    Ok(output)
}

fn compose_result_text(
    source_pixels: &[u8],
    cell: Cell,
    palette: &[u16; 256],
    raster: &[u8],
    style: FixedTextCompositionStyle,
) -> Result<Vec<u8>> {
    ensure!(
        source_pixels.len() == cell.width * cell.height && raster.len() == source_pixels.len(),
        "practical-result fixed-text cell geometry changed"
    );
    let structural_pixels =
        detect_structural_lines(source_pixels, cell, palette, style.table_line_palette_word);
    let mut reconstruction_mask = source_pixels
        .iter()
        .zip(&structural_pixels)
        .map(|(index, structural)| {
            !*structural
                && is_source_ink_color(
                    palette[usize::from(*index)],
                    style.source_ink_classifier,
                    style.paper_blue_tolerance,
                )
        })
        .collect::<Vec<_>>();
    ensure!(
        reconstruction_mask.iter().any(|masked| *masked),
        "practical-result fixed-text cell has no source ink"
    );
    for _ in 0..style.source_text_mask_dilation_steps {
        reconstruction_mask = dilate_mask(&reconstruction_mask, cell.width, cell.height);
    }
    for (masked, structural) in reconstruction_mask.iter_mut().zip(&structural_pixels) {
        if *structural {
            *masked = false;
        }
    }
    ensure!(
        reconstruction_mask.iter().any(|masked| !*masked),
        "practical-result source-text mask consumed its owned cell"
    );
    let background_palette_indexes = source_pixels
        .iter()
        .zip(&reconstruction_mask)
        .filter_map(|(index, masked)| {
            let word = palette[usize::from(*index)];
            let is_background = match style.source_ink_classifier {
                SourceInkClassifier::PaperContrast => {
                    is_background_color(word, style.paper_blue_tolerance)
                }
                classifier => !is_source_ink_color(word, classifier, style.paper_blue_tolerance),
            };
            (!*masked && is_background).then_some(*index)
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    ensure!(
        !background_palette_indexes.is_empty(),
        "practical-result fixed-text cell has no background palette"
    );
    let mut output = if matches!(
        style.source_ink_classifier,
        SourceInkClassifier::CyanAnnouncement | SourceInkClassifier::VioletHint
    ) {
        reconstruct_solid_background(
            source_pixels,
            palette,
            SIKEN20_RESULT_BACKGROUND_PALETTE_WORD,
        )?
    } else if style.sample_source_background {
        reconstruct_sampled_background(
            source_pixels,
            &reconstruction_mask,
            cell.width,
            palette,
            style.paper_blue_tolerance,
        )?
    } else {
        reconstruct_indexed_background(
            source_pixels,
            &reconstruction_mask,
            cell.width,
            cell.height,
            palette,
            &background_palette_indexes,
        )?
    };
    overlay_result_raster(
        &mut output,
        raster,
        style.outline_palette_index,
        style.fill_palette_index,
    )?;
    Ok(output)
}

fn overlay_result_raster(
    output: &mut [u8],
    raster: &[u8],
    outline_palette_index: Option<u8>,
    fill_palette_index: u8,
) -> Result<()> {
    ensure!(
        output.len() == raster.len(),
        "result raster geometry changed"
    );
    for (output, marker) in output.iter_mut().zip(raster) {
        match *marker {
            TRANSPARENT_MARKER => {}
            OUTLINE_MARKER => {
                *output = outline_palette_index
                    .context("practical-result raster emitted an undeclared outline")?
            }
            FILL_MARKER => *output = fill_palette_index,
            marker => bail!("practical-result raster contains marker {marker}"),
        }
    }
    Ok(())
}

fn reconstruct_solid_background(
    source_pixels: &[u8],
    palette: &[u16; 256],
    background_palette_word: u16,
) -> Result<Vec<u8>> {
    let background_index =
        palette_index_for_source_word(source_pixels, palette, background_palette_word)?;
    Ok(vec![background_index; source_pixels.len()])
}

fn reconstruct_sampled_background(
    source_pixels: &[u8],
    mask: &[bool],
    width: usize,
    palette: &[u16; 256],
    paper_blue_tolerance: i16,
) -> Result<Vec<u8>> {
    let background_samples = source_pixels
        .iter()
        .copied()
        .zip(mask)
        .filter_map(|(index, masked)| {
            (!*masked && is_background_color(palette[usize::from(index)], paper_blue_tolerance))
                .then_some(index)
        })
        .collect::<Vec<_>>();
    ensure!(
        !background_samples.is_empty(),
        "practical-result sampled background has no source paper pixels"
    );
    let mut output = source_pixels.to_vec();
    for (offset, masked) in mask.iter().copied().enumerate() {
        if !masked {
            continue;
        }
        let x = offset % width;
        let y = offset / width;
        let mut mixed = (x as u64).wrapping_mul(0x9e37_79b1_85eb_ca87)
            ^ (y as u64).wrapping_mul(0xc2b2_ae3d_27d4_eb4f);
        mixed ^= mixed >> 30;
        mixed = mixed.wrapping_mul(0xbf58_476d_1ce4_e5b9);
        mixed ^= mixed >> 27;
        mixed = mixed.wrapping_mul(0x94d0_49bb_1331_11eb);
        mixed ^= mixed >> 31;
        output[offset] = background_samples[mixed as usize % background_samples.len()];
    }
    Ok(output)
}

fn source_text_mask_dilation_steps(source_path: &str) -> usize {
    if source_path == PRACTICAL_BASICS_RESULTS_PATH {
        SIKEN2_SOURCE_TEXT_MASK_DILATION_STEPS
    } else {
        DEFAULT_SOURCE_TEXT_MASK_DILATION_STEPS
    }
}

fn table_line_palette_word(source_path: &str) -> Result<Option<u16>> {
    match source_path {
        PRACTICAL_1999_TEXTURE_PATH | PRACTICAL_1999_RESULTS_PATH => Ok(None),
        PRACTICAL_BASICS_TEXTURE_PATH => Ok(Some(SIKEN1_TABLE_LINE_PALETTE_WORD)),
        PRACTICAL_BASICS_RESULTS_PATH => Ok(Some(SIKEN2_TABLE_LINE_PALETTE_WORD)),
        _ => bail!("unsupported practical-result table-line source {source_path}"),
    }
}

fn palette_roles(
    source_path: &str,
    tim_offset: usize,
    target_id: &str,
    font_role: ModeDescendantFontRole,
    source_pixels: &[u8],
    palette: &[u16; 256],
) -> Result<(Option<u8>, u8)> {
    let (outline_word, fill_word) = if target_id == SIKEN20_RESULT_ANNOUNCEMENT_TARGET_ID {
        (
            Some(SIKEN20_RESULT_ANNOUNCEMENT_OUTLINE_PALETTE_WORD),
            SIKEN20_RESULT_ANNOUNCEMENT_FILL_PALETTE_WORD,
        )
    } else if target_id == SIKEN20_RESULT_BACK_HINT_TARGET_ID {
        (
            Some(SIKEN20_RESULT_BACK_HINT_OUTLINE_PALETTE_WORD),
            SIKEN20_RESULT_BACK_HINT_FILL_PALETTE_WORD,
        )
    } else {
        match (source_path, font_role) {
            (PRACTICAL_BASICS_TEXTURE_PATH, ModeDescendantFontRole::PracticalResultHeading) => (
                Some(SIKEN1_HEADING_OUTLINE_PALETTE_WORD),
                SIKEN1_HEADING_FILL_PALETTE_WORD,
            ),
            (
                PRACTICAL_BASICS_TEXTURE_PATH,
                ModeDescendantFontRole::PracticalResultLabel
                | ModeDescendantFontRole::PracticalResultHint,
            ) => (None, SIKEN1_LABEL_PALETTE_WORD),
            (
                PRACTICAL_1999_RESULTS_PATH,
                ModeDescendantFontRole::PracticalResultHeading
                | ModeDescendantFontRole::PracticalResultLabel,
            ) if tim_offset == SIKEN20_RESULT_SUMMARY_TIM_OFFSET => (
                Some(SIKEN20_RESULT_SUMMARY_OUTLINE_PALETTE_WORD),
                SIKEN20_RESULT_SUMMARY_FILL_PALETTE_WORD,
            ),
            (PRACTICAL_1999_TEXTURE_PATH, ModeDescendantFontRole::PracticalResultHeading)
            | (PRACTICAL_1999_TEXTURE_PATH, ModeDescendantFontRole::PracticalResultLabel)
            | (PRACTICAL_1999_TEXTURE_PATH, ModeDescendantFontRole::PracticalResultHint)
            | (PRACTICAL_1999_RESULTS_PATH, ModeDescendantFontRole::PracticalResultHeading)
            | (PRACTICAL_1999_RESULTS_PATH, ModeDescendantFontRole::PracticalResultLabel) => (
                Some(SIKEN10_OUTLINE_PALETTE_WORD),
                SIKEN10_FILL_PALETTE_WORD,
            ),
            (PRACTICAL_BASICS_RESULTS_PATH, ModeDescendantFontRole::PracticalResultHeading) => (
                Some(SIKEN2_HEADING_OUTLINE_PALETTE_WORD),
                SIKEN2_HEADING_FILL_PALETTE_WORD,
            ),
            (PRACTICAL_BASICS_RESULTS_PATH, ModeDescendantFontRole::PracticalResultLabel) => {
                (None, SIKEN2_LABEL_PALETTE_WORD)
            }
            _ => bail!(
                "unsupported practical-result palette role {font_role:?} for source {source_path}"
            ),
        }
    };
    Ok((
        outline_word
            .map(|word| palette_index_for_source_word(source_pixels, palette, word))
            .transpose()?,
        palette_index_for_source_word(source_pixels, palette, fill_word)?,
    ))
}

fn source_ink_classifier(target_id: &str) -> SourceInkClassifier {
    match target_id {
        SIKEN20_RESULT_ANNOUNCEMENT_TARGET_ID => SourceInkClassifier::CyanAnnouncement,
        SIKEN20_RESULT_BACK_HINT_TARGET_ID => SourceInkClassifier::VioletHint,
        _ => SourceInkClassifier::PaperContrast,
    }
}

fn is_source_ink_color(
    word: u16,
    classifier: SourceInkClassifier,
    paper_blue_tolerance: i16,
) -> bool {
    if matches!(classifier, SourceInkClassifier::PaperContrast) {
        return is_source_text_color(word, paper_blue_tolerance);
    }
    let red = i16::from((word & 0x1f) as u8);
    let green = i16::from(((word >> 5) & 0x1f) as u8);
    let blue = i16::from(((word >> 10) & 0x1f) as u8);
    let darkest = red.min(green).min(blue);
    let lightest = red.max(green).max(blue);
    let dark_neutral_outline = lightest <= 5 && lightest - darkest <= 2 && lightest > 0;
    match classifier {
        SourceInkClassifier::PaperContrast => unreachable!("handled above"),
        SourceInkClassifier::CyanAnnouncement => {
            dark_neutral_outline || green >= red + 4 && blue >= red + 2 && green >= blue
        }
        SourceInkClassifier::VioletHint => {
            dark_neutral_outline || blue >= green + 5 && red >= green + 3
        }
    }
}

fn palette_index_for_source_word(
    source_pixels: &[u8],
    palette: &[u16; 256],
    expected_word: u16,
) -> Result<u8> {
    let mut population = [0usize; 256];
    for index in source_pixels {
        population[usize::from(*index)] += 1;
    }
    let best_match = |exact: bool| {
        palette
            .iter()
            .enumerate()
            .filter(|(index, word)| {
                if exact {
                    **word == expected_word
                } else {
                    population[*index] > 0 && (**word & 0x7fff) == (expected_word & 0x7fff)
                }
            })
            .max_by(|(left_index, _), (right_index, _)| {
                population[*left_index]
                    .cmp(&population[*right_index])
                    .then_with(|| right_index.cmp(left_index))
            })
            .map(|(index, _)| index as u8)
    };
    best_match(true)
        .or_else(|| best_match(false))
        .with_context(|| {
            format!("practical-result source palette has no word {expected_word:#06x}")
        })
}

fn is_source_text_color(word: u16, paper_blue_tolerance: i16) -> bool {
    !is_paper_color(word, paper_blue_tolerance)
}

fn is_paper_color(word: u16, blue_tolerance: i16) -> bool {
    let red = i16::from((word & 0x1f) as u8);
    let green = i16::from(((word >> 5) & 0x1f) as u8);
    let blue = i16::from(((word >> 10) & 0x1f) as u8);
    red >= 24 && green >= 20 && blue >= 20 && blue <= red + blue_tolerance
}

fn is_background_color(word: u16, paper_blue_tolerance: i16) -> bool {
    is_paper_color(word, paper_blue_tolerance)
}

fn detect_structural_lines(
    source_pixels: &[u8],
    cell: Cell,
    palette: &[u16; 256],
    table_line_palette_word: Option<u16>,
) -> Vec<bool> {
    let mut structural = vec![false; source_pixels.len()];
    for y in 0..cell.height {
        let row_start = y * cell.width;
        let line_pixel_count = source_pixels[row_start..row_start + cell.width]
            .iter()
            .filter(|index| {
                is_table_line_color(palette[usize::from(**index)], table_line_palette_word)
            })
            .count();
        if line_pixel_count * 4 >= cell.width * 3 {
            for x in 0..cell.width {
                let offset = row_start + x;
                if is_table_line_color(
                    palette[usize::from(source_pixels[offset])],
                    table_line_palette_word,
                ) {
                    structural[offset] = true;
                }
            }
        }
    }
    for x in 0..cell.width {
        let line_pixel_count = (0..cell.height)
            .filter(|y| {
                is_table_line_color(
                    palette[usize::from(source_pixels[y * cell.width + x])],
                    table_line_palette_word,
                )
            })
            .count();
        if line_pixel_count * 4 >= cell.height * 3 {
            for y in 0..cell.height {
                let offset = y * cell.width + x;
                if is_table_line_color(
                    palette[usize::from(source_pixels[offset])],
                    table_line_palette_word,
                ) {
                    structural[offset] = true;
                }
            }
        }
    }
    structural
}

fn is_table_line_color(word: u16, table_line_palette_word: Option<u16>) -> bool {
    if let Some(table_line_palette_word) = table_line_palette_word {
        return (word & 0x7fff) == (table_line_palette_word & 0x7fff);
    }
    let red = i16::from((word & 0x1f) as u8);
    let green = i16::from(((word >> 5) & 0x1f) as u8);
    let blue = i16::from(((word >> 10) & 0x1f) as u8);
    green >= red && green >= blue + 4
}

fn font_for_entry<'a>(
    fonts: &'a ModeDescendantFontSources,
    entry: &PracticalResultEntry,
) -> Result<&'a ShiftedSizedFontSource> {
    let style = match entry.font_role {
        ModeDescendantFontRole::PracticalResultHeading => &fonts.practical_result_heading,
        ModeDescendantFontRole::PracticalResultLabel => &fonts.practical_result_label,
        ModeDescendantFontRole::PracticalResultHint => &fonts.practical_result_hint,
        ModeDescendantFontRole::PracticalResultSmallJudgment => {
            &fonts.practical_result_small_judgment
        }
        role => bail!("unsupported practical-result fixed-text font role {role:?}"),
    };
    Ok(style)
}

pub(super) fn plan_fixed_text_entries(
    entries: &[PracticalResultEntry],
    catalog: &PracticalResultFixedTextTargetCatalog,
    completed_expected_write_target_ids: &BTreeSet<PracticalResultFixedTextTargetId>,
) -> Result<FixedTextPlan> {
    let target_ids = catalog
        .targets
        .iter()
        .map(|target| target.target_id.clone())
        .collect::<BTreeSet<_>>();
    ensure!(
        completed_expected_write_target_ids.is_subset(&target_ids),
        "fixed-text Expected Write closure names a target outside the validated target catalog"
    );
    let mut targets_by_entry = BTreeMap::<&str, Vec<&PracticalResultFixedTextTargetId>>::new();
    for target in &catalog.targets {
        targets_by_entry
            .entry(target.semantic_entry_id.as_str())
            .or_default()
            .push(&target.target_id);
    }

    let mut deferred_entry_ids = BTreeSet::new();
    for entry in entries.iter().filter(|entry| {
        entry.strategy == PracticalResultStrategy::FixedText
            && entry.development_status == PracticalResultDevelopmentStatus::Authored
    }) {
        entry
            .korean_text
            .as_deref()
            .context("authored practical-result fixed text lost Korean text")?;
        let entry_targets = targets_by_entry
            .get(entry.id.as_str())
            .map(Vec::as_slice)
            .unwrap_or_default();
        if entry_targets.is_empty()
            || entry_targets
                .iter()
                .any(|target_id| !completed_expected_write_target_ids.contains(*target_id))
        {
            ensure!(
                deferred_entry_ids.insert(entry.id.clone()),
                "practical-result fixed entry {} was planned twice",
                entry.id
            );
        }
    }
    let write_contract_complete = !target_ids.is_empty()
        && deferred_entry_ids.is_empty()
        && completed_expected_write_target_ids == &target_ids;
    Ok(FixedTextPlan {
        deferred_entry_ids,
        validated_target_count: catalog.targets.len(),
        layout_candidate_count: catalog.layout_candidates.len(),
        expected_write_count: completed_expected_write_target_ids.len(),
        write_contract_complete,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completed_fixed_target_is_not_blocked_by_an_unrelated_deferred_entry() {
        let entries: Vec<PracticalResultEntry> = serde_json::from_value(serde_json::json!([
            {
                "id": "ready",
                "strategy": "fixed_text",
                "source_text": "準備",
                "korean_text": "준비",
                "font_role": "practical_result_label",
                "development_status": "authored",
                "release_status": "needs_human_review"
            },
            {
                "id": "waiting",
                "strategy": "fixed_text",
                "source_text": "保留",
                "korean_text": "보류",
                "font_role": "practical_result_label",
                "development_status": "authored",
                "release_status": "needs_human_review"
            }
        ]))
        .unwrap();
        let catalog: PracticalResultFixedTextTargetCatalog =
            serde_json::from_value(serde_json::json!({
                "kind": "justice_gakuen2_practical_result_fixed_text_target_catalog",
                "targets": [{
                    "target_id": "ready_target",
                    "semantic_entry_id": "ready",
                    "target_record_path": "DAT2/RESULT.BIZ",
                    "target_tim_offset": "0x100",
                    "target_bpp": 8,
                    "target_cell": {"x": 1, "y": 1, "width": 8, "height": 8},
                    "alignment": "left",
                    "replaces_source_region_id": "ready_region",
                    "expected_preimage_indexed_sha256": "00",
                    "expected_consumer_occurrence_ids": ["runtime_occurrence"]
                }],
                "layout_candidates": []
            }))
            .unwrap();
        let completed = [catalog.targets[0].target_id.clone()].into_iter().collect();

        let plan = plan_fixed_text_entries(&entries, &catalog, &completed).unwrap();

        assert!(!plan.deferred_entry_ids.contains("ready"));
        assert!(plan.deferred_entry_ids.contains("waiting"));
        assert!(!plan.write_contract_complete);
    }

    #[test]
    fn result_raster_preserves_pattern_and_stp_outside_ink() {
        let mut background = vec![42, 7, 42, 80, 42, 90];
        overlay_result_raster(
            &mut background,
            &[0, 0, OUTLINE_MARKER, FILL_MARKER, 0, 0],
            Some(3),
            6,
        )
        .unwrap();
        assert_eq!(background, [42, 7, 3, 6, 42, 90]);
    }

    #[test]
    fn source_text_cleanup_preserves_structural_table_lines() {
        let cell = Cell {
            x: 0,
            y: 0,
            width: 12,
            height: 12,
        };
        let mut palette = [0u16; 256];
        palette[2] = SIKEN10_OUTLINE_PALETTE_WORD;
        palette[85] = SIKEN10_FILL_PALETTE_WORD;
        palette[172] = 0xae91;
        palette[249] = 0xf79f;
        let mut source = vec![249; cell.width * cell.height];
        for x in 0..cell.width {
            source[10 * cell.width + x] = 172;
        }
        for y in 3..6 {
            for x in 3..6 {
                source[y * cell.width + x] = 85;
            }
        }
        let mut raster = vec![TRANSPARENT_MARKER; source.len()];
        raster[4 * cell.width + 8] = FILL_MARKER;

        let output = compose_result_text(
            &source,
            cell,
            &palette,
            &raster,
            FixedTextCompositionStyle {
                outline_palette_index: Some(2),
                fill_palette_index: 85,
                source_text_mask_dilation_steps: 1,
                table_line_palette_word: None,
                sample_source_background: false,
                paper_blue_tolerance: 4,
                source_ink_classifier: SourceInkClassifier::PaperContrast,
            },
        )
        .unwrap();

        assert!((0..cell.width).all(|x| output[10 * cell.width + x] == 172));
        assert!((3..6).all(|y| (3..6).all(|x| output[y * cell.width + x] != 85)));
        assert_eq!(output[4 * cell.width + 8], 85);
    }

    #[test]
    fn sampled_cleanup_retains_source_paper_variation() {
        let width = 16;
        let height = 16;
        let mut palette = [0u16; 256];
        palette[10] = 0xf79f;
        palette[11] = 0xef7e;
        palette[85] = SIKEN2_LABEL_PALETTE_WORD;
        let mut source = (0..width * height)
            .map(|offset| if offset % 2 == 0 { 10 } else { 11 })
            .collect::<Vec<_>>();
        let mut mask = vec![false; source.len()];
        for y in 4..12 {
            for x in 4..12 {
                let offset = y * width + x;
                source[offset] = 85;
                mask[offset] = true;
            }
        }

        let output = reconstruct_sampled_background(&source, &mask, width, &palette, 4).unwrap();
        let repaired_indexes = output
            .iter()
            .copied()
            .zip(&mask)
            .filter_map(|(index, masked)| masked.then_some(index))
            .collect::<BTreeSet<_>>();

        assert_eq!(repaired_indexes, BTreeSet::from([10, 11]));
        assert!(
            output
                .iter()
                .copied()
                .zip(&mask)
                .all(|(index, masked)| !*masked || index != 85)
        );
    }

    #[test]
    fn small_judgment_replaces_source_ink_inside_the_existing_badge() {
        let source = vec![0, 1, 2, 3, 4, 1, 1, 0];
        let mut palette = [0u16; 16];
        palette[1] = 0x0001;
        palette[2] = 0x0008;
        palette[3] = 0x000f;
        palette[4] = 0x7fff;
        let roles = small_judgment_palette_roles(&source, &palette).unwrap();
        assert_eq!(roles.transparent_index, 0);
        assert_eq!(roles.background_index, 1);
        assert_eq!(roles.outline_index, 3);
        assert_eq!(roles.fill_index, 4);

        let raster = vec![FILL_MARKER, FILL_MARKER, 0, 0, 0, OUTLINE_MARKER, 0, 0];
        let output = compose_small_judgment_badge(&source, &palette, &raster, roles).unwrap();

        assert_eq!(
            output[0], 0,
            "Korean ink must not escape the badge silhouette"
        );
        assert_eq!(output[1], 4);
        assert_eq!(&output[2..5], &[1, 1, 1]);
        assert_eq!(output[5], 3);
        assert_eq!(output[7], 0);
    }
    #[test]
    fn result_heading_background_keeps_backdrop_semitransparency() {
        let mut palette = [0u16; 256];
        palette[7] = 0x016e;
        palette[42] = 0x816e;
        let source = vec![7; 16];
        let output =
            reconstruct_solid_background(&source, &palette, SIKEN20_RESULT_BACKGROUND_PALETTE_WORD)
                .unwrap();
        assert!(output.iter().all(|&i| palette[i as usize] == 0x816e));
    }
}
