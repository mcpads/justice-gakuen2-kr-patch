//! Product-owned EDIT command-sheet translation plan and archive builder.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Context, Result, ensure};
use serde::Deserialize;

use crate::decoded_record_write_plan::DecodedDataClaim;
use crate::embedded_tim::{decode_embedded_tim_preview, parse_embedded_tim_at};
use crate::font::{HorizontalTextAlignment, IndexedTextRasterizers};
use crate::pipeline::{difference_ranges, sha256_bytes};
use crate::tim::{
    Cell, IndexedImage, cells_overlap, read_4bpp_indexed_image_in_prefix,
    read_4bpp_palette_words_in_prefix, read_indexed_cell_in_prefix,
    write_indexed_cell_in_prefix_with_report,
};
use crate::tim_preview::write_tim_preview;
use crate::write_scope::changed_ranges_are_within;

use super::catalog::ModeDescendantRecordSpec;
use super::edit_command_sheets::{EDIT_COMMAND_SHEET_ARCHIVE, EDIT_COMMAND_SHEETS_PATH};
use super::edit_move_names::{
    MoveNameTranslations, load_move_names, move_name_background, validate_move_cell,
};
use super::model::{
    EditCommandSheetBuildReport, EditCommandSheetMemberBuildReport,
    EditCommandSheetMoveNameBuildReport, ModeDescendantGraphicsBuildConfig,
};
use super::record_compositor::ModeDescendantRecordDraft;
use super::source::ModeDescendantSourceRecord;

const MANIFEST_KIND: &str = "justice_gakuen2_edit_command_sheet_plan";
const MEMBER_COUNT: usize = 32;
const MEMBER_DECODED_SIZE: usize = 0x2b800;
const SECONDARY_TIM_OFFSET: usize = 0x13000;
const PRIMARY_WIDTH: usize = 412;
const PRIMARY_HEIGHT: usize = 369;
const COMMON_LABEL_COUNT: usize = 8;
const COMMON_LABEL_OCCURRENCE_COUNT: usize = MEMBER_COUNT * COMMON_LABEL_COUNT;
const MOVE_NAME_OCCURRENCE_COUNT: usize = 210;

#[derive(Debug)]
pub(super) struct EditCommandSheetPlan {
    manifest_sha256: String,
    manifest: EditCommandSheetManifest,
    move_names: MoveNameTranslations,
    pub(super) team_up_names: super::edit_team_up_names::Plan,
    plain_conditions: super::edit_conditions::Plan,
    air_allowed_badges: super::edit_badges::Plan,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EditCommandSheetManifest {
    kind: String,
    source_path: String,
    semantic_labels: Vec<EditCommandSheetSemanticLabel>,
    plain_conditions_path: std::path::PathBuf,
    air_allowed_badges_path: std::path::PathBuf,
    move_names_path: std::path::PathBuf,
    team_up_names_path: std::path::PathBuf,
    occurrences: Vec<EditCommandSheetOccurrence>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EditCommandSheetSemanticLabel {
    id: String,
    source_text: String,
    fill_color_role: EditCommandSheetFillColorRole,
    background: EditCommandSheetBackground,
    korean_lines: Vec<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum EditCommandSheetFillColorRole {
    Dark,
    White,
    Red,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum EditCommandSheetBackground {
    SameRowOutsideCell {
        sample_x: usize,
        sample_width: usize,
    },
    SolidSourceModalIndex,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum EditCommandSheetOccurrenceRole {
    CommonLabel,
    MoveNamePendingTextReview,
    MoveName,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EditCommandSheetOccurrence {
    id: String,
    member_index: usize,
    role: EditCommandSheetOccurrenceRole,
    semantic_id: Option<String>,
    move_name_source_text: Option<String>,
    render_cell: Option<Cell>,
    cell: Cell,
    source_region_sha256: String,
    source_ink_indices: Vec<u8>,
    source_ink_mask_sha256: String,
    source_modal_palette_index: u8,
    fill_index: Option<u8>,
    outline_index: Option<u8>,
}

pub(super) struct EditCommandSheetBuild {
    pub(super) record: ModeDescendantRecordDraft,
    pub(super) report: EditCommandSheetBuildReport,
}

pub(super) fn load_edit_command_sheet_plan(path: &Path) -> Result<EditCommandSheetPlan> {
    let bytes =
        std::fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    let manifest: EditCommandSheetManifest = serde_json::from_slice(&bytes)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    let move_names = load_move_names(
        &path
            .parent()
            .context("EDIT manifest has no parent")?
            .join(&manifest.move_names_path),
    )?;
    validate_manifest(&manifest, &move_names)?;
    let plain_conditions = super::edit_conditions::load(
        &path
            .parent()
            .context("EDIT manifest has no parent")?
            .join(&manifest.plain_conditions_path),
    )?;
    let air_allowed_badges = super::edit_badges::load(
        &path
            .parent()
            .context("EDIT manifest has no parent")?
            .join(&manifest.air_allowed_badges_path),
    )?;
    let team_up_names = super::edit_team_up_names::load(
        &path
            .parent()
            .context("EDIT manifest has no parent")?
            .join(&manifest.team_up_names_path),
    )?;
    Ok(EditCommandSheetPlan {
        team_up_names,
        manifest_sha256: sha256_bytes(&bytes),
        manifest,
        move_names,
        plain_conditions,
        air_allowed_badges,
    })
}

fn validate_manifest(
    manifest: &EditCommandSheetManifest,
    move_names: &MoveNameTranslations,
) -> Result<()> {
    ensure!(
        manifest.kind == MANIFEST_KIND,
        "unsupported EDIT command-sheet manifest kind {:?}",
        manifest.kind
    );
    ensure!(
        manifest.source_path == EDIT_COMMAND_SHEETS_PATH,
        "EDIT command-sheet source path changed"
    );
    ensure!(
        manifest.semantic_labels.len() == COMMON_LABEL_COUNT,
        "EDIT command-sheet semantic-label denominator changed"
    );

    let mut semantics = BTreeMap::new();
    for semantic in &manifest.semantic_labels {
        ensure!(
            !semantic.id.trim().is_empty()
                && !semantic.source_text.trim().is_empty()
                && !semantic.korean_lines.is_empty()
                && semantic
                    .korean_lines
                    .iter()
                    .all(|line| !line.trim().is_empty()),
            "EDIT command-sheet semantic label is incomplete"
        );
        ensure!(
            semantics.insert(semantic.id.as_str(), semantic).is_none(),
            "duplicate EDIT command-sheet semantic label {}",
            semantic.id
        );
    }

    let mut ids = BTreeSet::new();
    let mut member_semantics = BTreeSet::new();
    let mut member_cells = vec![Vec::<(&str, Cell)>::new(); MEMBER_COUNT];
    let mut referenced_moves = BTreeSet::new();
    let mut common_count = 0;
    let mut move_name_count = 0;
    for occurrence in &manifest.occurrences {
        ensure!(
            ids.insert(occurrence.id.as_str()),
            "duplicate EDIT command-sheet occurrence {}",
            occurrence.id
        );
        ensure!(
            occurrence.member_index < MEMBER_COUNT
                && occurrence.cell.width > 0
                && occurrence.cell.height > 0
                && occurrence.cell.x + occurrence.cell.width <= PRIMARY_WIDTH
                && occurrence.cell.y + occurrence.cell.height <= PRIMARY_HEIGHT,
            "EDIT command-sheet occurrence {} leaves its primary TIM",
            occurrence.id
        );
        validate_sha256(&occurrence.source_region_sha256, &occurrence.id)?;
        validate_sha256(&occurrence.source_ink_mask_sha256, &occurrence.id)?;
        ensure!(
            occurrence.source_modal_palette_index < 16
                && !occurrence.source_ink_indices.is_empty()
                && occurrence
                    .source_ink_indices
                    .iter()
                    .all(|index| *index < 16)
                && occurrence
                    .source_ink_indices
                    .iter()
                    .copied()
                    .collect::<BTreeSet<_>>()
                    .len()
                    == occurrence.source_ink_indices.len(),
            "EDIT command-sheet occurrence {} has invalid source palette roles",
            occurrence.id
        );
        let writable_cell = occurrence.render_cell.unwrap_or(occurrence.cell);
        ensure!(
            writable_cell.width > 0
                && writable_cell.height > 0
                && writable_cell
                    .x
                    .checked_add(writable_cell.width)
                    .is_some_and(|end| end <= PRIMARY_WIDTH)
                && writable_cell
                    .y
                    .checked_add(writable_cell.height)
                    .is_some_and(|end| end <= PRIMARY_HEIGHT),
            "EDIT command-sheet writable cell leaves its primary TIM"
        );
        for (other_id, other_cell) in &member_cells[occurrence.member_index] {
            ensure!(
                !cells_overlap(writable_cell, *other_cell),
                "EDIT command-sheet occurrences {} and {} overlap",
                occurrence.id,
                other_id
            );
        }
        member_cells[occurrence.member_index].push((&occurrence.id, writable_cell));

        match occurrence.role {
            EditCommandSheetOccurrenceRole::CommonLabel => {
                ensure!(
                    occurrence.move_name_source_text.is_none() && occurrence.render_cell.is_none(),
                    "common label claims move-name layout"
                );
                let semantic_id = occurrence.semantic_id.as_deref().with_context(|| {
                    format!(
                        "EDIT command-sheet common occurrence {} has no semantic label",
                        occurrence.id
                    )
                })?;
                let semantic = semantics.get(semantic_id).with_context(|| {
                    format!(
                        "EDIT command-sheet common occurrence {} has an unknown semantic label",
                        occurrence.id
                    )
                })?;
                ensure!(
                    member_semantics.insert((occurrence.member_index, semantic_id)),
                    "EDIT command-sheet common occurrence {} has an invalid or duplicate semantic label",
                    occurrence.id
                );
                let fill_index = occurrence.fill_index.with_context(|| {
                    format!(
                        "EDIT command-sheet common occurrence {} has no fill index",
                        occurrence.id
                    )
                })?;
                ensure!(
                    occurrence.source_ink_indices.contains(&fill_index)
                        && occurrence.outline_index.is_none_or(|index| {
                            occurrence.source_ink_indices.contains(&index) && index != fill_index
                        }),
                    "EDIT command-sheet common occurrence {} has invalid rendered palette roles",
                    occurrence.id
                );
                match semantic.background {
                    EditCommandSheetBackground::SameRowOutsideCell {
                        sample_x,
                        sample_width,
                    } => ensure!(
                        sample_width > occurrence.cell.width
                            && sample_x <= occurrence.cell.x
                            && occurrence.cell.x + occurrence.cell.width <= sample_x + sample_width
                            && sample_x + sample_width <= PRIMARY_WIDTH,
                        "EDIT command-sheet common occurrence {} has an invalid background sample band",
                        occurrence.id
                    ),
                    EditCommandSheetBackground::SolidSourceModalIndex => {}
                }
                common_count += 1;
            }
            EditCommandSheetOccurrenceRole::MoveNamePendingTextReview => {
                ensure!(
                    occurrence.semantic_id.is_none()
                        && occurrence.move_name_source_text.is_none()
                        && occurrence.render_cell.is_none()
                        && occurrence.fill_index.is_none()
                        && occurrence.outline_index.is_none(),
                    "pending EDIT command-sheet move {} claims authored semantics",
                    occurrence.id
                );
                move_name_count += 1;
            }
            EditCommandSheetOccurrenceRole::MoveName => {
                let source_text = occurrence
                    .move_name_source_text
                    .as_deref()
                    .context("authored EDIT move has no source text")?;
                ensure!(
                    move_names.entries.contains_key(source_text),
                    "EDIT move has no translation: {source_text}"
                );
                referenced_moves.insert(source_text);
                ensure!(
                    occurrence.semantic_id.is_none()
                        && occurrence.outline_index.is_none()
                        && occurrence
                            .fill_index
                            .is_some_and(|index| occurrence.source_ink_indices.contains(&index)),
                    "authored EDIT move has invalid semantics or palette roles"
                );
                validate_move_cell(
                    occurrence.cell,
                    occurrence
                        .render_cell
                        .context("authored EDIT move has no render cell")?,
                )?;
                move_name_count += 1;
            }
        }
    }
    ensure!(
        referenced_moves == move_names.entries.keys().map(String::as_str).collect(),
        "EDIT move translation inventory contains unused entries"
    );
    ensure!(
        common_count == COMMON_LABEL_OCCURRENCE_COUNT
            && move_name_count == MOVE_NAME_OCCURRENCE_COUNT
            && manifest.occurrences.len()
                == COMMON_LABEL_OCCURRENCE_COUNT + MOVE_NAME_OCCURRENCE_COUNT,
        "EDIT command-sheet occurrence denominator changed"
    );
    ensure!(
        (0..MEMBER_COUNT).all(|member_index| {
            semantics
                .keys()
                .all(|semantic_id| member_semantics.contains(&(member_index, *semantic_id)))
        }),
        "EDIT command-sheet common-label member coverage is incomplete"
    );
    Ok(())
}

fn validate_sha256(value: &str, id: &str) -> Result<()> {
    ensure!(
        value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "EDIT command-sheet occurrence {id} has an invalid SHA-256"
    );
    Ok(())
}

pub(super) fn build_edit_command_sheets(
    config: &ModeDescendantGraphicsBuildConfig,
    rasterizers: &mut IndexedTextRasterizers,
    source: &ModeDescendantSourceRecord,
    record_spec: &'static ModeDescendantRecordSpec,
    plan: &EditCommandSheetPlan,
) -> Result<EditCommandSheetBuild> {
    ensure!(
        source.path == EDIT_COMMAND_SHEETS_PATH
            && record_spec.source_path == EDIT_COMMAND_SHEETS_PATH
            && source.decoded.len() == MEMBER_COUNT * MEMBER_DECODED_SIZE
            && EDIT_COMMAND_SHEET_ARCHIVE.members.len() == MEMBER_COUNT,
        "EDIT command-sheet build source identity changed"
    );
    let semantics = plan
        .manifest
        .semantic_labels
        .iter()
        .map(|semantic| (semantic.id.as_str(), semantic))
        .collect::<BTreeMap<_, _>>();
    let style = &config.fonts.edit_compact_label;
    let rasterizer = rasterizers.for_font(&style.path)?;
    let mut patched = source.decoded.clone();
    let mut allowed_ranges = Vec::new();
    let mut member_authored_counts = [0usize; MEMBER_COUNT];
    let mut member_pending_counts = [0usize; MEMBER_COUNT];
    let mut member_move_counts = [0usize; MEMBER_COUNT];
    let mut move_names = Vec::new();
    let source_primary_images = (0..MEMBER_COUNT)
        .map(|member_index| {
            read_4bpp_indexed_image_in_prefix(&source.decoded, member_index * MEMBER_DECODED_SIZE)
        })
        .collect::<Result<Vec<_>>>()?;
    let source_primary_palettes = (0..MEMBER_COUNT)
        .map(|member_index| {
            read_4bpp_palette_words_in_prefix(
                &source.decoded,
                member_index * MEMBER_DECODED_SIZE,
                0,
            )
        })
        .collect::<Result<Vec<_>>>()?;
    let distinct_source_primary_clut_count = source_primary_palettes
        .iter()
        .copied()
        .collect::<BTreeSet<_>>()
        .len();
    std::fs::create_dir_all(config.output_dir.join("edit-command-sheets"))
        .context("failed to create EDIT command-sheet preview directory")?;

    for occurrence in &plan.manifest.occurrences {
        let tim_offset = occurrence.member_index * MEMBER_DECODED_SIZE;
        let source_pixels =
            read_indexed_cell_in_prefix(&source.decoded, tim_offset, occurrence.cell)?;
        ensure!(
            sha256_bytes(&source_pixels) == occurrence.source_region_sha256,
            "EDIT command-sheet occurrence {} source region changed",
            occurrence.id
        );
        let source_ink_indices = occurrence
            .source_ink_indices
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let source_ink_mask = source_pixels
            .iter()
            .map(|pixel| u8::from(source_ink_indices.contains(pixel)))
            .collect::<Vec<_>>();
        ensure!(
            sha256_bytes(&source_ink_mask) == occurrence.source_ink_mask_sha256,
            "EDIT command-sheet occurrence {} source ink mask changed",
            occurrence.id
        );

        match occurrence.role {
            EditCommandSheetOccurrenceRole::MoveNamePendingTextReview => {
                member_pending_counts[occurrence.member_index] += 1;
            }
            EditCommandSheetOccurrenceRole::MoveName => {
                let source_text = occurrence
                    .move_name_source_text
                    .as_deref()
                    .context("validated EDIT move lost source text")?;
                let lines = &plan.move_names.entries[source_text];
                let cell = occurrence
                    .render_cell
                    .context("validated EDIT move lost render cell")?;
                let fill = occurrence
                    .fill_index
                    .context("validated EDIT move lost fill index")?;
                ensure!(
                    fill_color_matches_role(
                        source_primary_palettes[occurrence.member_index][usize::from(fill)],
                        EditCommandSheetFillColorRole::Dark
                    ),
                    "EDIT move {} fill palette role changed",
                    occurrence.id
                );
                let mut pixels = move_name_background(
                    &source_primary_images[occurrence.member_index],
                    occurrence.cell,
                    cell,
                    occurrence.source_modal_palette_index,
                    &source_primary_palettes[occurrence.member_index],
                )
                .with_context(|| format!("EDIT move {} source background", occurrence.id))?;
                let line_advance_px = render_semantic_label(
                    rasterizer,
                    style.font_px,
                    lines,
                    cell.width,
                    cell.height,
                    fill,
                    None,
                    &mut pixels,
                )
                .with_context(|| format!("EDIT move {} ({source_text}) layout", occurrence.id))?;
                let write = write_indexed_cell_in_prefix_with_report(
                    &mut patched,
                    tim_offset,
                    cell,
                    &pixels,
                )?;
                ensure!(
                    write.changed_byte_count > 0,
                    "EDIT move {} changed no bytes",
                    occurrence.id
                );
                allowed_ranges.extend(write.allowed_ranges);
                member_move_counts[occurrence.member_index] += 1;
                move_names.push(EditCommandSheetMoveNameBuildReport {
                    id: occurrence.id.clone(),
                    source_text: source_text.to_string(),
                    korean_lines: lines.clone(),
                    source_cell: occurrence.cell,
                    render_cell: cell,
                    font_px: style.font_px,
                    line_advance_px,
                    source_background_index: occurrence.source_modal_palette_index,
                    fill_index: fill,
                });
            }
            EditCommandSheetOccurrenceRole::CommonLabel => {
                let semantic_id = occurrence
                    .semantic_id
                    .as_deref()
                    .context("validated common occurrence lost its semantic label")?;
                let semantic = semantics
                    .get(semantic_id)
                    .context("validated common semantic label disappeared")?;
                let fill_index = occurrence
                    .fill_index
                    .context("validated common occurrence lost its fill index")?;
                ensure!(
                    fill_color_matches_role(
                        source_primary_palettes[occurrence.member_index][usize::from(fill_index)],
                        semantic.fill_color_role,
                    ),
                    "EDIT command-sheet occurrence {} fill palette role changed",
                    occurrence.id
                );
                let mut output_pixels = reconstruct_label_background(
                    &source_primary_images[occurrence.member_index],
                    occurrence.cell,
                    &semantic.background,
                    occurrence.source_modal_palette_index,
                )?;
                render_semantic_label(
                    rasterizer,
                    style.font_px,
                    &semantic.korean_lines,
                    occurrence.cell.width,
                    occurrence.cell.height,
                    fill_index,
                    occurrence.outline_index,
                    &mut output_pixels,
                )?;
                let write = write_indexed_cell_in_prefix_with_report(
                    &mut patched,
                    tim_offset,
                    occurrence.cell,
                    &output_pixels,
                )?;
                ensure!(
                    write.changed_byte_count > 0,
                    "EDIT command-sheet occurrence {} changed no bytes",
                    occurrence.id
                );
                allowed_ranges.extend(write.allowed_ranges);
                member_authored_counts[occurrence.member_index] += 1;
            }
        }
    }

    let plain_conditions = super::edit_conditions::apply(
        &plan.plain_conditions,
        config,
        &source.decoded,
        &mut patched,
        &mut allowed_ranges,
    )?;

    let air_allowed_badges = super::edit_badges::apply(
        &plan.air_allowed_badges,
        config,
        &source.decoded,
        &mut patched,
        &mut allowed_ranges,
    )?;
    let team_up_names = super::edit_team_up_names::apply(
        &plan.team_up_names,
        &config.fonts.edit_team_up_name,
        rasterizers,
        &source.decoded,
        &mut patched,
        &mut allowed_ranges,
    )?;
    ensure!(
        changed_ranges_are_within(
            &difference_ranges(&source.decoded, &patched),
            &allowed_ranges
        ),
        "EDIT command-sheet build changed bytes outside authored labels"
    );

    let mut members = Vec::with_capacity(MEMBER_COUNT);
    for member_index in 0..MEMBER_COUNT {
        let member_start = member_index * MEMBER_DECODED_SIZE;
        let member_end = member_start + MEMBER_DECODED_SIZE;
        ensure!(
            read_4bpp_palette_words_in_prefix(&source.decoded, member_start, 0)?
                == read_4bpp_palette_words_in_prefix(&patched, member_start, 0)?,
            "EDIT command-sheet member {member_index} primary CLUT changed"
        );
        ensure!(
            read_4bpp_palette_words_in_prefix(
                &source.decoded,
                member_start + SECONDARY_TIM_OFFSET,
                0
            )? == read_4bpp_palette_words_in_prefix(
                &patched,
                member_start + SECONDARY_TIM_OFFSET,
                0
            )?,
            "EDIT command-sheet member {member_index} secondary CLUT changed"
        );
        let source_preview = format!("edit-command-sheets/member-{member_index:02}-source.png");
        let patched_preview = format!("edit-command-sheets/member-{member_index:02}-patched.png");
        write_texture_preview(
            &config.output_dir.join(&source_preview),
            &source.decoded,
            member_start,
        )?;
        write_texture_preview(
            &config.output_dir.join(&patched_preview),
            &patched,
            member_start,
        )?;
        let changed_decoded_byte_count = source.decoded[member_start..member_end]
            .iter()
            .zip(&patched[member_start..member_end])
            .filter(|(left, right)| left != right)
            .count();
        ensure!(
            member_authored_counts[member_index] == COMMON_LABEL_COUNT
                && changed_decoded_byte_count > 0,
            "EDIT command-sheet member {member_index} was not fully authored"
        );
        members.push(EditCommandSheetMemberBuildReport {
            member_index,
            authored_common_label_occurrence_count: member_authored_counts[member_index],
            pending_move_name_occurrence_count: member_pending_counts[member_index],
            authored_move_name_occurrence_count: member_move_counts[member_index],
            changed_decoded_byte_count,
            source_preview_file: source_preview,
            patched_preview_file: patched_preview,
        });
    }

    write_texture_preview(
        &config
            .output_dir
            .join("edit-command-sheets/cooperative-names-patched.png"),
        &patched,
        SECONDARY_TIM_OFFSET,
    )?;

    let decoded_write_claims = DecodedDataClaim::from_effective_ranges(
        "mode-descendant:edit-command-sheets",
        "render source-bound common labels and development move names in EDIT command sheets",
        &source.decoded,
        &patched,
        allowed_ranges,
    )?;
    ensure!(
        !decoded_write_claims.is_empty(),
        "EDIT command-sheet build produced no effective write claims"
    );

    Ok(EditCommandSheetBuild {
        record: ModeDescendantRecordDraft {
            spec: record_spec,
            decoded: patched,
            decoded_write_claims,
        },
        report: EditCommandSheetBuildReport {
            team_up_names,
            air_allowed_badges,
            plain_conditions,
            source_path: EDIT_COMMAND_SHEETS_PATH.to_string(),
            manifest_sha256: plan.manifest_sha256.clone(),
            move_name_manifest_sha256: plan.move_names.sha256.clone(),
            move_name_translation_count: plan.move_names.entries.len(),
            move_name_translation_status: "draft".to_string(),
            move_names,
            member_count: MEMBER_COUNT,
            semantic_label_count: semantics.len(),
            physical_occurrence_count: plan.manifest.occurrences.len(),
            authored_common_label_occurrence_count: COMMON_LABEL_OCCURRENCE_COUNT,
            pending_move_name_occurrence_count: member_pending_counts.iter().sum(),
            authored_move_name_occurrence_count: member_move_counts.iter().sum(),
            distinct_source_primary_clut_count,
            fill_palette_roles_verified: true,
            full_label_backgrounds_reconstructed: true,
            source_regions_match: true,
            changes_confined_to_authored_occurrences: true,
            primary_cluts_preserved: true,
            secondary_cluts_preserved: true,
            members,
        },
    })
}

fn reconstruct_label_background(
    source: &IndexedImage,
    cell: Cell,
    background: &EditCommandSheetBackground,
    source_modal_palette_index: u8,
) -> Result<Vec<u8>> {
    ensure!(
        source.width == PRIMARY_WIDTH
            && source.height == PRIMARY_HEIGHT
            && cell.x + cell.width <= source.width
            && cell.y + cell.height <= source.height,
        "EDIT command-sheet background reconstruction geometry changed"
    );
    match background {
        EditCommandSheetBackground::SolidSourceModalIndex => {
            Ok(vec![source_modal_palette_index; cell.width * cell.height])
        }
        EditCommandSheetBackground::SameRowOutsideCell {
            sample_x,
            sample_width,
        } => {
            let sample_end = sample_x + sample_width;
            ensure!(
                *sample_width > cell.width
                    && *sample_x <= cell.x
                    && cell.x + cell.width <= sample_end
                    && sample_end <= source.width,
                "EDIT command-sheet row-sample band changed"
            );
            let background_columns = (*sample_x..sample_end)
                .filter(|x| *x < cell.x || *x >= cell.x + cell.width)
                .collect::<Vec<_>>();
            let mut output = Vec::with_capacity(cell.width * cell.height);
            for y in cell.y..cell.y + cell.height {
                for x in cell.x..cell.x + cell.width {
                    let source_x = background_columns
                        .iter()
                        .copied()
                        .filter(|candidate| candidate % 4 == x % 4)
                        .min_by_key(|candidate| (candidate.abs_diff(x), *candidate))
                        .context(
                            "EDIT command-sheet background row lost its palette-phase sample",
                        )?;
                    output.push(source.pixels[y * source.width + source_x]);
                }
            }
            Ok(output)
        }
    }
}

fn fill_color_matches_role(word: u16, role: EditCommandSheetFillColorRole) -> bool {
    match role {
        EditCommandSheetFillColorRole::Dark => matches!(word & 0x7fff, 0x0421 | 0x0842),
        EditCommandSheetFillColorRole::White => word & 0x7fff == 0x7fff,
        EditCommandSheetFillColorRole::Red => word & 0x7fff == 0x001f,
    }
}

#[allow(clippy::too_many_arguments)]
fn render_semantic_label(
    rasterizer: &crate::font::IndexedTextRasterizer,
    font_px: f32,
    lines: &[String],
    width: usize,
    height: usize,
    fill_index: u8,
    outline_index: Option<u8>,
    output: &mut [u8],
) -> Result<Vec<f32>> {
    ensure!(
        !lines.is_empty() && output.len() == width * height,
        "EDIT command-sheet semantic label has invalid geometry"
    );
    let sentinel = (0u8..16)
        .find(|index| *index != fill_index && Some(*index) != outline_index)
        .context("EDIT command-sheet label has no raster sentinel")?;
    let mut advances = Vec::new();
    for (line_index, line) in lines.iter().enumerate() {
        let y = line_index * height / lines.len();
        let bottom = (line_index + 1) * height / lines.len();
        let line_height = bottom - y;
        let raster = rasterizer.rasterize(
            line,
            width,
            line_height,
            font_px,
            0.0,
            sentinel,
            outline_index,
            fill_index,
            HorizontalTextAlignment::Center,
        )?;
        advances.push(raster.measured_advance_px);
        for row in 0..line_height {
            for x in 0..width {
                let pixel = raster.pixels[row * width + x];
                if pixel != sentinel {
                    output[(y + row) * width + x] = pixel;
                }
            }
        }
    }
    Ok(advances)
}

fn write_texture_preview(path: &Path, decoded: &[u8], tim_offset: usize) -> Result<()> {
    let tim = parse_embedded_tim_at(decoded, tim_offset)?;
    let rgba = decode_embedded_tim_preview(decoded, &tim)?;
    write_tim_preview(path, &rgba)
}

#[cfg(test)]
mod tests {
    use crate::tim::{Cell, IndexedImage};

    use super::{EditCommandSheetBackground, reconstruct_label_background};

    #[test]
    fn reconstruction_replaces_the_full_cell_from_same_row_palette_phase() {
        let mut pixels = vec![0; 412 * 369];
        for y in 10..12 {
            for x in 0..12 {
                pixels[y * 412 + x] = u8::try_from(x % 4).unwrap();
            }
            pixels[y * 412 + 4..y * 412 + 8].copy_from_slice(&[9, 9, 9, 9]);
        }
        let source = IndexedImage {
            width: 412,
            height: 369,
            pixels,
        };
        let output = reconstruct_label_background(
            &source,
            Cell {
                x: 4,
                y: 10,
                width: 4,
                height: 2,
            },
            &EditCommandSheetBackground::SameRowOutsideCell {
                sample_x: 0,
                sample_width: 12,
            },
            15,
        )
        .unwrap();
        assert_eq!(output, [0, 1, 2, 3, 0, 1, 2, 3]);
    }

    #[test]
    fn solid_reconstruction_uses_the_occurrence_palette_index() {
        let source = IndexedImage {
            width: 412,
            height: 369,
            pixels: vec![0; 412 * 369],
        };
        let output = reconstruct_label_background(
            &source,
            Cell {
                x: 10,
                y: 10,
                width: 3,
                height: 2,
            },
            &EditCommandSheetBackground::SolidSourceModalIndex,
            14,
        )
        .unwrap();
        assert_eq!(output, [14; 6]);
    }
}
