use super::layout_measurement::{
    MAXIMUM_CELLS_PER_LINE, MAXIMUM_LINES_PER_MESSAGE, measure_dialogue_lines,
};
use super::translation_model::DialogueTranslationControl;

fn control(semantic_name: &str) -> DialogueTranslationControl {
    DialogueTranslationControl {
        semantic_name: semantic_name.to_string(),
        arguments: Vec::new(),
    }
}

#[test]
fn zero_width_controls_do_not_consume_dialogue_cells() {
    let segments = vec!["열".to_string(), "혈".to_string(), String::new()];
    let controls = vec![control("palette_style"), control("message_end")];

    let lines = measure_dialogue_lines(&segments, &controls).unwrap();

    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].static_cells, 2);
    assert_eq!(lines[0].maximum_cells, 2);
}

#[test]
fn narration_indent_consumes_a_column_in_the_main_window() {
    let segments = vec![
        "(우와~!".to_string(),
        String::new(),
        "두 사람 다 정말 잘 먹네……. 낮엔".to_string(),
        String::new(),
    ];
    let controls = vec![
        control("line_break"),
        control("name_buffer_padding"),
        control("message_end"),
    ];
    let lines = measure_dialogue_lines(&segments, &controls).unwrap();
    assert_eq!(lines[0].maximum_cells, 5);
    assert_eq!(lines[1].static_cells, 21);
    assert!(lines[1].maximum_cells > MAXIMUM_CELLS_PER_LINE);
}

#[test]
fn trailing_choice_padding_does_not_extend_the_visible_line() {
    let segments = vec!["가".repeat(20), String::new(), String::new()];
    let controls = vec![control("name_buffer_padding"), control("message_end")];
    let lines = measure_dialogue_lines(&segments, &controls).unwrap();
    assert_eq!(lines[0].maximum_cells, 20);
}

#[test]
fn line_breaks_reset_width_and_allow_four_lines() {
    let segments = vec![
        "가".repeat(MAXIMUM_CELLS_PER_LINE),
        "나".repeat(MAXIMUM_CELLS_PER_LINE),
        "다".repeat(MAXIMUM_CELLS_PER_LINE),
        "라".repeat(MAXIMUM_CELLS_PER_LINE),
        String::new(),
    ];
    let controls = vec![
        control("line_break"),
        control("line_break"),
        control("line_break"),
        control("message_end"),
    ];

    let lines = measure_dialogue_lines(&segments, &controls).unwrap();

    assert_eq!(lines.len(), MAXIMUM_LINES_PER_MESSAGE);
    assert!(
        lines
            .iter()
            .all(|line| line.maximum_cells == MAXIMUM_CELLS_PER_LINE)
    );
}

#[test]
fn fifth_line_remains_visible_to_the_capacity_gate() {
    let segments = vec![
        "하나".to_string(),
        "둘".to_string(),
        "셋".to_string(),
        "넷".to_string(),
        "다섯".to_string(),
        String::new(),
    ];
    let controls = vec![
        control("line_break"),
        control("line_break"),
        control("line_break"),
        control("line_break"),
        control("message_end"),
    ];

    let lines = measure_dialogue_lines(&segments, &controls).unwrap();

    assert_eq!(lines.len(), MAXIMUM_LINES_PER_MESSAGE + 1);
}

#[test]
fn static_text_over_capacity_remains_observable() {
    let segments = vec!["가".repeat(MAXIMUM_CELLS_PER_LINE + 1), String::new()];
    let controls = vec![control("message_end")];

    let lines = measure_dialogue_lines(&segments, &controls).unwrap();

    assert_eq!(lines[0].static_cells, MAXIMUM_CELLS_PER_LINE + 1);
}

#[test]
fn relationship_name_width_includes_the_runtime_honorific() {
    let segments = vec!["반가워, ".to_string(), "!".to_string(), String::new()];
    let controls = vec![
        control("relationship_name_kun_kanji"),
        control("message_end"),
    ];

    let lines = measure_dialogue_lines(&segments, &controls).unwrap();

    assert_eq!(lines[0].runtime_insertions, ["relationship_name_kun_kanji"]);
    assert_eq!(lines[0].minimum_cells, 7);
    assert_eq!(lines[0].maximum_cells, 14);
}

#[test]
fn family_only_honorific_branch_can_be_wider_than_the_given_name_branch() {
    let segments = vec![String::new(), String::new(), String::new()];
    let controls = vec![
        control("relationship_family_name_kun_katakana"),
        control("message_end"),
    ];

    let lines = measure_dialogue_lines(&segments, &controls).unwrap();

    assert_eq!(lines[0].minimum_cells, 1);
    assert_eq!(lines[0].maximum_cells, 8);
}

#[test]
fn given_name_uses_the_enforced_producer_limit() {
    let segments = vec![String::new(), String::new(), String::new()];
    let controls = vec![control("protagonist_given_name"), control("message_end")];

    let lines = measure_dialogue_lines(&segments, &controls).unwrap();

    assert_eq!(lines[0].minimum_cells, 1);
    assert_eq!(lines[0].maximum_cells, 6);
}

#[test]
fn nickname_uses_the_enforced_producer_limit() {
    let segments = vec![String::new(), String::new(), String::new()];
    let controls = vec![control("protagonist_nickname"), control("message_end")];

    let lines = measure_dialogue_lines(&segments, &controls).unwrap();

    assert_eq!(lines[0].minimum_cells, 1);
    assert_eq!(lines[0].maximum_cells, 4);
}

#[test]
fn missing_message_end_is_rejected() {
    let segments = vec!["끝나지 않음".to_string()];

    let error = measure_dialogue_lines(&segments, &[]).unwrap_err();

    assert!(error.to_string().contains("lacks message_end"));
}
