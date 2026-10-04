use crate::tim::Cell;

use super::name_entry_fixed_graphics_build::{
    cells_overlap, validate_fixed_graphic_surfaces_disjoint, validate_name_input_page_labels,
};
use super::name_entry_fixed_graphics_model::{
    DialogueNameEntryFixedGraphicBuildReport, DialogueNameEntryFixedGraphicInstall,
};
use crate::font::HorizontalTextAlignment;

#[test]
fn fixed_graphic_cells_may_touch_without_overlapping() {
    let left = Cell {
        x: 10,
        y: 20,
        width: 30,
        height: 10,
    };
    let right = Cell {
        x: 40,
        y: 20,
        width: 20,
        height: 10,
    };

    assert!(!cells_overlap(left, right));
    assert!(cells_overlap(left, Cell { x: 39, ..right }));
}

#[test]
fn fixed_graphic_surfaces_reject_cross_shard_overlap() {
    let reports = [
        surface(
            "first",
            Cell {
                x: 10,
                y: 20,
                width: 30,
                height: 10,
            },
        ),
        surface(
            "second",
            Cell {
                x: 39,
                y: 20,
                width: 20,
                height: 10,
            },
        ),
    ];

    let error = validate_fixed_graphic_surfaces_disjoint(&reports).unwrap_err();

    assert!(error.to_string().contains("first and second overlap"));
}

#[test]
fn fixed_graphic_surfaces_may_reuse_coordinates_in_different_tim_images() {
    let cell = Cell {
        x: 10,
        y: 20,
        width: 30,
        height: 10,
    };
    let mut reports = [surface("first", cell), surface("second", cell)];
    reports[1].tim_offset = "0x19000".to_string();

    validate_fixed_graphic_surfaces_disjoint(&reports).unwrap();
}

#[test]
fn candidate_control_labels_match_keyboard_page_roles() {
    let report = page_label_surface([
        ("hangul_page", "자음+모음"),
        ("latin_page", "영문"),
        ("digits_and_symbols_page", "숫자+기호"),
    ]);

    validate_name_input_page_labels(
        &[report],
        &[
            ("hangul", "자음+모음"),
            ("latin", "영문"),
            ("digits_and_symbols", "숫자+기호"),
        ],
    )
    .unwrap();
}

#[test]
fn stale_candidate_control_labels_are_rejected() {
    let report = page_label_surface([
        ("hangul_page", "한글1"),
        ("latin_page", "한글2"),
        ("digits_and_symbols_page", "한글3"),
    ]);

    let error = validate_name_input_page_labels(
        &[report],
        &[
            ("hangul", "자음+모음"),
            ("latin", "영문"),
            ("digits_and_symbols", "숫자+기호"),
        ],
    )
    .unwrap_err();

    assert!(error.to_string().contains("differs from the keyboard spec"));
}

fn surface(id: &str, cell: Cell) -> DialogueNameEntryFixedGraphicBuildReport {
    DialogueNameEntryFixedGraphicBuildReport {
        kind: "test".to_string(),
        surface_id: id.to_string(),
        translation_file: "test.json".to_string(),
        translation_sha256: "test".to_string(),
        source_decoded_sha256: "test".to_string(),
        tim_offset: "0x00000".to_string(),
        tim_image_width: 100,
        tim_image_height: 100,
        entry_count: 1,
        all_source_regions_match: true,
        all_cells_unique_and_non_overlapping: true,
        all_cells_disjoint_from_protected_cells: true,
        entries: vec![DialogueNameEntryFixedGraphicInstall {
            id: "entry".to_string(),
            source_text: "source".to_string(),
            korean_text: "target".to_string(),
            source_region_sha256: "test".to_string(),
            cell,
            font_sha256: "test".to_string(),
            font_px: 14.0,
            tracking_px: 1.0,
            alignment: HorizontalTextAlignment::Center,
            measured_advance_px: 10.0,
            ink_bounds: [0, 0, 10, 10],
            indexed_pixels_sha256: "test".to_string(),
            changed_decoded_byte_count: 1,
        }],
    }
}

fn page_label_surface(labels: [(&str, &str); 3]) -> DialogueNameEntryFixedGraphicBuildReport {
    let mut report = surface(
        "candidate-controls",
        Cell {
            x: 0,
            y: 0,
            width: 10,
            height: 10,
        },
    );
    report.entries = labels
        .into_iter()
        .enumerate()
        .map(|(index, (id, text))| DialogueNameEntryFixedGraphicInstall {
            id: id.to_string(),
            source_text: "source".to_string(),
            korean_text: text.to_string(),
            source_region_sha256: "test".to_string(),
            cell: Cell {
                x: index * 10,
                y: 0,
                width: 10,
                height: 10,
            },
            font_sha256: "test".to_string(),
            font_px: 10.0,
            tracking_px: 0.0,
            alignment: HorizontalTextAlignment::Center,
            measured_advance_px: 10.0,
            ink_bounds: [0, 0, 10, 10],
            indexed_pixels_sha256: "test".to_string(),
            changed_decoded_byte_count: 1,
        })
        .collect();
    report.entry_count = report.entries.len();
    report
}
