use std::path::Path;

use anyhow::{Context, Result, ensure};

use super::dialogue_translation_input::load_primary_dialogue_translation_input;
use super::layout_measurement::{
    CELL_WIDTH_PIXELS, LINE_ADVANCE_PIXELS, MAXIMUM_CELLS_PER_LINE, MAXIMUM_LINES_PER_MESSAGE,
    TEXT_ORIGIN_X_PIXELS, TEXT_ORIGIN_Y_PIXELS, measure_dialogue_lines, runtime_insertion_widths,
};
use super::layout_model::{
    DialogueLayoutAuditConfig, DialogueLayoutAuditReport, DialogueLayoutIssue,
    DialogueLayoutIssueKind, DialogueRendererGeometry,
};
use super::translation_model::DialogueTranslationAuditConfig;
use super::translation_workspace_io::json_bytes;
#[cfg(test)]
use super::translation_workspace_model::DialogueTranslationDecisionStatus;
use super::translation_workspace_model::DialogueTranslationWorkspaceAuditReport;

pub fn audit_dialogue_layout(
    config: &DialogueLayoutAuditConfig,
) -> Result<DialogueLayoutAuditReport> {
    let primary_input = load_primary_dialogue_translation_input(&DialogueTranslationAuditConfig {
        cue: config.cue.clone(),
        codebook: config.codebook.clone(),
        translation: config.translation.clone(),
        output: config.translation_audit_output.clone(),
    })?;
    let translation_audit = primary_input.report;
    let independent_review_complete =
        admit_layout_input(&translation_audit, config.require_approved)?;
    ensure!(
        primary_input.authored_segments.len() == translation_audit.semantic_group_count,
        "layout source/translation coverage differs"
    );

    let mut issues = Vec::new();
    let mut semantic_group_count = 0usize;
    let mut too_many_lines_group_count = 0usize;
    let mut static_text_too_wide_group_count = 0usize;
    let mut runtime_insertion_may_overflow_group_count = 0usize;
    let mut largest_static_line_cells = 0usize;
    let mut largest_maximum_line_cells = 0usize;
    let mut source_maximum_static_line_cells = 0usize;
    let mut source_maximum_lines_per_message = 0usize;

    for (owner, groups) in &primary_input.source.groups_by_owner {
        for source_group in groups {
            let korean_segments = primary_input
                .authored_segments
                .get(&source_group.semantic_source_sha256)
                .context("layout input semantic identity changed")?;
            let translation_path = primary_input
                .decision_paths
                .get(&source_group.semantic_source_sha256)
                .context("layout input decision path disappeared")?;
            let (prepared_segments, prepared_controls) = super::choice_columns::prepare(
                &source_group.semantic_source_sha256,
                korean_segments,
                &source_group.controls,
            )?;
            let lines = measure_dialogue_lines(&prepared_segments, &prepared_controls)?;
            let source_lines =
                measure_dialogue_lines(&source_group.source_segments, &source_group.controls)?;
            semantic_group_count += 1;
            source_maximum_lines_per_message =
                source_maximum_lines_per_message.max(source_lines.len());
            for line in &source_lines {
                source_maximum_static_line_cells =
                    source_maximum_static_line_cells.max(line.static_cells);
            }

            let too_many_lines = lines.len() > MAXIMUM_LINES_PER_MESSAGE;
            let static_text_too_wide = lines
                .iter()
                .any(|line| line.static_cells > MAXIMUM_CELLS_PER_LINE);
            let runtime_insertion_may_overflow = lines.iter().any(|line| {
                line.static_cells <= MAXIMUM_CELLS_PER_LINE
                    && line.maximum_cells > MAXIMUM_CELLS_PER_LINE
            });
            for line in &lines {
                largest_static_line_cells = largest_static_line_cells.max(line.static_cells);
                largest_maximum_line_cells = largest_maximum_line_cells.max(line.maximum_cells);
            }
            if too_many_lines || static_text_too_wide || runtime_insertion_may_overflow {
                let mut issue_kinds = Vec::new();
                if too_many_lines {
                    too_many_lines_group_count += 1;
                    issue_kinds.push(DialogueLayoutIssueKind::TooManyLines);
                }
                if static_text_too_wide {
                    static_text_too_wide_group_count += 1;
                    issue_kinds.push(DialogueLayoutIssueKind::StaticTextTooWide);
                }
                if runtime_insertion_may_overflow {
                    runtime_insertion_may_overflow_group_count += 1;
                    issue_kinds.push(DialogueLayoutIssueKind::RuntimeInsertionMayOverflow);
                }
                issues.push(DialogueLayoutIssue {
                    source_path: owner.source_path.clone(),
                    translation_path: translation_path.clone(),
                    bank_selector: owner.bank_selector,
                    variant_selector: owner.variant_selector,
                    route_table_offset: owner.route_table_offset.clone(),
                    semantic_source_sha256: source_group.semantic_source_sha256.clone(),
                    referenced_coordinate_ids: source_group.referenced_coordinate_ids.clone(),
                    issue_kinds,
                    lines,
                });
            }
        }
    }
    ensure!(
        semantic_group_count == translation_audit.semantic_group_count,
        "layout semantic group denominator changed"
    );

    let overflow_group_count = issues.len();
    let report = DialogueLayoutAuditReport {
        kind: "Justice Gakuen 2 Korean dialogue layout audit".to_string(),
        source_bin_sha256: translation_audit.source_bin_sha256,
        codebook_sha256: translation_audit.codebook_sha256,
        source_corpus_sha256: translation_audit.source_corpus_sha256,
        script_inventory_sha256: translation_audit.script_inventory_sha256,
        independent_review_required: config.require_approved,
        independent_review_complete,
        approved_translation_group_count: translation_audit.approved_group_count,
        pending_translation_review_group_count: translation_audit.pending_review_group_count,
        changes_requested_translation_group_count: translation_audit.changes_requested_group_count,
        renderer_geometry: DialogueRendererGeometry {
            maximum_cells_per_line: MAXIMUM_CELLS_PER_LINE,
            maximum_lines_per_message: MAXIMUM_LINES_PER_MESSAGE,
            cell_width_pixels: CELL_WIDTH_PIXELS,
            line_advance_pixels: LINE_ADVANCE_PIXELS,
            text_origin_x_pixels: TEXT_ORIGIN_X_PIXELS,
            text_origin_y_pixels: TEXT_ORIGIN_Y_PIXELS,
            evidence: "MGAME 20x20 glyphs at (84,332), advances (20,28), and paired native window packets ending at (496,448); alternate windows and selectors require separate verification"
                .to_string(),
        },
        runtime_insertion_widths: runtime_insertion_widths(),
        semantic_group_count,
        fitting_group_count: semantic_group_count - overflow_group_count,
        overflow_group_count,
        too_many_lines_group_count,
        static_text_too_wide_group_count,
        runtime_insertion_may_overflow_group_count,
        largest_static_line_cells,
        largest_maximum_line_cells,
        source_maximum_static_line_cells,
        source_maximum_lines_per_message,
        layout_within_renderer_capacity: overflow_group_count == 0,
        issues,
    };
    write_report(&config.output, &report)?;
    Ok(report)
}

pub(super) fn admit_layout_input(
    translation_audit: &DialogueTranslationWorkspaceAuditReport,
    require_approved: bool,
) -> Result<bool> {
    ensure!(
        translation_audit.untranslated_group_count == 0
            && translation_audit.draft_group_count + translation_audit.ready_for_review_group_count
                == translation_audit.semantic_group_count,
        "layout development audit requires authored text for every translation decision"
    );
    let approval_complete = translation_audit.approved_group_count
        == translation_audit.semantic_group_count
        && translation_audit.ready_for_review_group_count == translation_audit.semantic_group_count
        && translation_audit.pending_review_group_count == 0
        && translation_audit.changes_requested_group_count == 0;
    if require_approved {
        ensure!(
            approval_complete,
            "--require-approved requires every current translation byte to be independently approved"
        );
    }
    Ok(approval_complete)
}

#[cfg(test)]
pub(super) fn authored_layout_segments(
    decision: &super::translation_workspace_model::DialogueTranslationDecision,
) -> Result<Vec<String>> {
    ensure!(
        matches!(
            decision.status,
            DialogueTranslationDecisionStatus::Draft
                | DialogueTranslationDecisionStatus::ReadyForReview
        ),
        "layout development input is not authored"
    );
    decision
        .korean_segments
        .iter()
        .map(|segment| {
            segment
                .clone()
                .context("authored layout input contains an empty decision")
        })
        .collect()
}

fn write_report(path: &Path, report: &DialogueLayoutAuditReport) -> Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    std::fs::write(path, json_bytes(report)?)
        .with_context(|| format!("failed to write {}", path.display()))
}
