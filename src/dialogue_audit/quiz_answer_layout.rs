use anyhow::{Context, Result, ensure};

use super::layout_measurement::measure_dialogue_lines;
use super::translation_model::DialogueTranslationControl;

// MGAME 0x800bc038 advances glyph records by 0x44 bytes. The button
// icon occupies work + 0x50c (0x800bc074), leaving 19 glyph records.
// Reusing the icon as glyph 20 links the same GPU packet twice and cycles DMA.
const ANSWER_GLYPH_CAPACITY: usize = 0x50c / 0x44;

pub(super) fn validate_quiz_answer_layout(
    source_path: &str,
    selector: usize,
    entry_index: usize,
    segments: &[String],
    controls: &[DialogueTranslationControl],
) -> Result<()> {
    // MGAME's 20-question table at 0x800a2760 names entries 0,5,...,95;
    // the four-answer table at 0x800a2774 names the intervening entries.
    // 0x800a9fdc selects bank 6; October's resident image is MGK10.BIZ.
    if source_path != "DAT2/MGK10.BIZ"
        || selector != 6
        || entry_index >= 100
        || entry_index.is_multiple_of(5)
    {
        return Ok(());
    }
    let lines = measure_dialogue_lines(segments, controls)
        .context("quiz answer has invalid text/control structure")?;
    // This consumer ignores line breaks: they do not reset its glyph index.
    let cells: usize = lines.iter().map(|line| line.maximum_cells).sum();
    ensure!(
        cells <= ANSWER_GLYPH_CAPACITY,
        "{source_path} bank {selector} entry {entry_index}: quiz answer needs {cells} glyph slots; only {ANSWER_GLYPH_CAPACITY} precede the button icon (GPU packet overlap)"
    );
    Ok(())
}

// Presentation 5 uses the main 20x20 sprites at (84,136), but the
// retained quiz panel has room for only three 28-pixel rows and sixteen
// cells. A trailing empty source line has no glyphs and is harmless.
pub(super) fn validate_quiz_question_layout(
    source_path: &str,
    selector: usize,
    entry_index: usize,
    segments: &[String],
    controls: &[DialogueTranslationControl],
) -> Result<()> {
    if source_path != "DAT2/MGK10.BIZ"
        || selector != 6
        || entry_index >= 100
        || !entry_index.is_multiple_of(5)
    {
        return Ok(());
    }
    for line in measure_dialogue_lines(segments, controls)? {
        ensure!(
            line.maximum_cells <= 16 && (line.line_index < 3 || line.maximum_cells == 0),
            "{source_path} bank {selector} entry {entry_index}: quiz question row {} needs {} cells; panel admits 16 cells on three visible rows",
            line.line_index + 1,
            line.maximum_cells
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn control(name: &str) -> DialogueTranslationControl {
        DialogueTranslationControl {
            semantic_name: name.into(),
            arguments: Vec::new(),
        }
    }

    #[test]
    fn quiz_questions_reject_right_and_bottom_overflow_but_allow_empty_source_rows() {
        let check = |segments: Vec<String>, controls: Vec<DialogueTranslationControl>| {
            validate_quiz_question_layout("DAT2/MGK10.BIZ", 6, 75, &segments, &controls)
        };
        assert!(
            check(
                vec!["가".repeat(17), String::new()],
                vec![control("message_end")]
            )
            .is_err()
        );
        let controls = vec![
            control("line_break"),
            control("line_break"),
            control("line_break"),
            control("message_end"),
        ];
        assert!(
            check(
                vec![
                    "가".repeat(16),
                    "나".into(),
                    "다".into(),
                    String::new(),
                    String::new()
                ],
                controls.clone()
            )
            .is_ok()
        );
        assert!(
            check(
                vec![
                    "가".into(),
                    "나".into(),
                    "다".into(),
                    "라".into(),
                    String::new()
                ],
                controls
            )
            .is_err()
        );
    }

    #[test]
    fn quiz_answer_rejects_the_reproduced_twentieth_glyph_overlap() {
        let error = validate_quiz_answer_layout(
            "DAT2/MGK10.BIZ",
            6,
            68,
            &["근성 게이지 한 줄로 발동할 수 있다".into(), String::new()],
            &[control("message_end")],
        )
        .unwrap_err();
        assert!(error.to_string().contains("20 glyph slots; only 19"));
    }

    #[test]
    fn quiz_answer_accepts_the_last_non_icon_slot() {
        validate_quiz_answer_layout(
            "DAT2/MGK10.BIZ",
            6,
            99,
            &["가".repeat(19), String::new()],
            &[control("message_end")],
        )
        .unwrap();
    }

    #[test]
    fn quiz_answer_counts_insertions_across_ignored_line_breaks() {
        let error = validate_quiz_answer_layout(
            "DAT2/MGK10.BIZ",
            6,
            1,
            &[
                "가".repeat(10),
                "나".repeat(4),
                String::new(),
                String::new(),
            ],
            &[
                control("line_break"),
                control("protagonist_given_name"),
                control("message_end"),
            ],
        )
        .unwrap_err();
        assert!(error.to_string().contains("20 glyph slots"));
    }

    #[test]
    fn quiz_answer_capacity_does_not_reclassify_questions_or_other_banks() {
        for (path, selector, index) in [
            ("DAT2/MGK10.BIZ", 6, 65),
            ("DAT2/MGK10.BIZ", 6, 100),
            ("DAT2/MGK10.BIZ", 5, 68),
            ("DAT2/MGK12.BIZ", 6, 68),
        ] {
            validate_quiz_answer_layout(
                path,
                selector,
                index,
                &["가".repeat(20), String::new()],
                &[control("message_end")],
            )
            .unwrap();
        }
    }
}
