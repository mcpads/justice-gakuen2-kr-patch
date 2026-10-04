use anyhow::{Context, Result, ensure};
use serde::Serialize;

use super::dialogue_translation_input::PrimaryDialogueTranslationInput;
use super::layout_measurement::{
    MAXIMUM_CELLS_PER_LINE, MAXIMUM_LINES_PER_MESSAGE, measure_dialogue_lines,
};
use super::translation_model::DialogueTranslationControl;

#[derive(Debug, Serialize, serde::Deserialize)]
pub struct PrimaryDialogueLayoutBuildReport {
    pub checked_authored_group_count: usize,
    pub preserved_untranslated_group_count: usize,
    pub maximum_cells_per_line: usize,
    pub maximum_lines_per_message: usize,
    pub largest_maximum_line_cells: usize,
    pub largest_lines_per_message: usize,
    pub all_authored_primary_messages_fit: bool,
}

pub(super) fn validate_primary_message_layout(
    input: &PrimaryDialogueTranslationInput,
) -> Result<PrimaryDialogueLayoutBuildReport> {
    let mut report = PrimaryDialogueLayoutBuildReport {
        checked_authored_group_count: 0,
        preserved_untranslated_group_count: input.report.untranslated_group_count,
        maximum_cells_per_line: MAXIMUM_CELLS_PER_LINE,
        maximum_lines_per_message: MAXIMUM_LINES_PER_MESSAGE,
        largest_maximum_line_cells: 0,
        largest_lines_per_message: 0,
        all_authored_primary_messages_fit: true,
    };
    for group in input.source.groups_by_owner.values().flatten() {
        let Some(segments) = input.authored_segments.get(&group.semantic_source_sha256) else {
            continue;
        };
        let decision_path = input
            .decision_paths
            .get(&group.semantic_source_sha256)
            .context("primary layout decision path disappeared")?;
        let (segments, controls) = super::choice_columns::prepare(
            &group.semantic_source_sha256,
            segments,
            &group.controls,
        )?;
        let (line_count, maximum_width) = validate_message_capacity(&segments, &controls)
            .with_context(|| {
                format!(
                    "primary dialogue layout failed for {} ({decision_path})",
                    group.semantic_source_sha256
                )
            })?;
        report.checked_authored_group_count += 1;
        report.largest_lines_per_message = report.largest_lines_per_message.max(line_count);
        report.largest_maximum_line_cells = report.largest_maximum_line_cells.max(maximum_width);
    }
    ensure!(
        report.checked_authored_group_count == input.authored_segments.len()
            && report.checked_authored_group_count + report.preserved_untranslated_group_count
                == input.report.semantic_group_count,
        "primary build layout coverage differs from admitted translation input"
    );
    Ok(report)
}

fn validate_message_capacity(
    segments: &[String],
    controls: &[DialogueTranslationControl],
) -> Result<(usize, usize)> {
    let lines = measure_dialogue_lines(segments, controls)?;
    ensure!(
        lines.len() <= MAXIMUM_LINES_PER_MESSAGE,
        "message has {} lines; the primary renderer allows {MAXIMUM_LINES_PER_MESSAGE}",
        lines.len()
    );
    for line in &lines {
        ensure!(
            line.maximum_cells <= MAXIMUM_CELLS_PER_LINE,
            "line {} requires up to {} cells including runtime insertions; the primary renderer allows {MAXIMUM_CELLS_PER_LINE}",
            line.line_index + 1,
            line.maximum_cells
        );
    }
    Ok((
        lines.len(),
        lines
            .iter()
            .map(|line| line.maximum_cells)
            .max()
            .unwrap_or(0),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn control(name: &str) -> DialogueTranslationControl {
        DialogueTranslationControl {
            semantic_name: name.to_string(),
            arguments: Vec::new(),
        }
    }

    #[test]
    fn primary_build_accepts_exact_width_with_a_maximum_length_name() {
        let segments = ["가".repeat(14), String::new(), String::new()];
        let controls = [control("protagonist_given_name"), control("message_end")];
        assert_eq!(
            validate_message_capacity(&segments, &controls).unwrap(),
            (1, 20)
        );
    }

    #[test]
    fn primary_build_rejects_static_text_past_the_rendered_line() {
        let error =
            validate_message_capacity(&["가".repeat(21), String::new()], &[control("message_end")])
                .unwrap_err();
        assert!(error.to_string().contains("21 cells"));
    }

    #[test]
    fn primary_build_rejects_a_name_that_can_overflow_the_line() {
        let segments = ["가".repeat(15), String::new(), String::new()];
        let controls = [control("protagonist_given_name"), control("message_end")];
        let error = validate_message_capacity(&segments, &controls).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("21 cells including runtime insertions")
        );
    }

    #[test]
    fn primary_build_rejects_the_reproduced_bird_narration_overflow() {
        let error = validate_message_capacity(
            &[
                "(나는 새끼 새를 살며시 부드럽게 쥐고,".to_string(),
                String::new(),
            ],
            &[control("message_end")],
        )
        .unwrap_err();
        assert!(error.to_string().contains("22 cells"));
    }

    #[test]
    fn primary_build_rejects_a_fifth_line() {
        let segments = [
            "가".to_string(),
            "나".to_string(),
            "다".to_string(),
            "라".to_string(),
            "마".to_string(),
            String::new(),
        ];
        let mut controls = vec![control("line_break"); 4];
        controls.push(control("message_end"));
        let error = validate_message_capacity(&segments, &controls).unwrap_err();
        assert!(error.to_string().contains("5 lines"));
    }
}
