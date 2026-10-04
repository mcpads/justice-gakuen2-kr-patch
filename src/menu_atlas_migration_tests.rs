use super::*;
use crate::menu_atlas_labeling::MenuAtlasLabelKind;

fn write(
    id: &str,
    component: MenuAtlasWriterComponent,
    code: &str,
    cell: Cell,
) -> CandidateWriteInput {
    CandidateWriteInput {
        id: id.to_string(),
        component,
        character: '한',
        code: code.to_string(),
        cell,
    }
}

fn reference(code: &str, occurrence_count: usize, overlays: &[&str]) -> SourceCodeReference {
    SourceCodeReference {
        code: code.to_string(),
        footprint_fragments: logical_code_fragments(
            parse_menu_code(code).unwrap(),
            MENU_GLYPH_CELL_WIDTH,
            MENU_GLYPH_CELL_HEIGHT,
        )
        .unwrap(),
        occurrence_count,
        overlays: overlays
            .iter()
            .map(|overlay| (*overlay).to_string())
            .collect(),
        exact_dialogue_pixel_text: Some("日".to_string()),
    }
}

#[test]
fn reports_source_consumers_as_obligations_instead_of_free_slots() {
    let writes = [write(
        "options:0x0000",
        MenuAtlasWriterComponent::Options,
        "0x0000",
        Cell {
            x: 0,
            y: 0,
            width: 20,
            height: 20,
        },
    )];
    let references = [reference("0x0000", 3, &["DAT1/NEWOPT.BIN"])];

    let analysis = analyze_migration(&writes, &references, &[]).unwrap();

    assert_eq!(analysis.source_code_migration_obligations.len(), 1);
    assert_eq!(
        analysis.source_code_occurrence_migration_obligation_count,
        3
    );
    assert_eq!(
        analysis.source_consumer_overlays,
        ["DAT1/NEWOPT.BIN".to_string()]
    );
    assert!(analysis.source_code_migration_obligations[0].fully_overwritten);
    assert_eq!(analysis.unclassified_unique_written_pixel_count, 400);
}

#[test]
fn includes_a_byte_wrapped_source_footprint_that_crosses_the_page_edge() {
    let writes = [write(
        "options:0x0000",
        MenuAtlasWriterComponent::Options,
        "0x0000",
        Cell {
            x: 0,
            y: 0,
            width: 20,
            height: 20,
        },
    )];
    let references = [reference("0x000c", 1, &["DAT1/KANRI.BIN"])];

    let analysis = analyze_migration(&writes, &references, &[]).unwrap();

    assert_eq!(analysis.source_code_migration_obligations.len(), 1);
    assert_eq!(
        analysis.source_code_migration_obligations[0].overwritten_pixel_count,
        80
    );
    assert!(!analysis.source_code_migration_obligations[0].fully_overwritten);
}

#[test]
fn confirmed_free_rectangle_labels_cover_only_the_pixels_they_classify() {
    let writes = [write(
        "title_menu:0x0000",
        MenuAtlasWriterComponent::TitleMenu,
        "0x0000",
        Cell {
            x: 0,
            y: 0,
            width: 20,
            height: 20,
        },
    )];
    let labels = [MenuAtlasLabel {
        id: "left-half-interface".to_string(),
        kind: MenuAtlasLabelKind::InterfaceGraphic,
        review_status: MenuAtlasLabelReviewStatus::Confirmed,
        bounds: Cell {
            x: 0,
            y: 0,
            width: 10,
            height: 20,
        },
        semantic_role: Some("left half test region".to_string()),
        source_text: None,
        logical_codes: Vec::new(),
        notes: None,
    }];

    let analysis = analyze_migration(&writes, &[], &labels).unwrap();

    assert_eq!(
        analysis.confirmed_label_covered_unique_written_pixel_count,
        200
    );
    assert_eq!(analysis.unclassified_unique_written_pixel_count, 200);
    assert_eq!(
        analysis.candidate_writes[0].confirmed_label_ids,
        ["left-half-interface".to_string()]
    );
}

#[test]
fn distinguishes_declared_area_from_unique_pixels_when_contributors_overlap() {
    let cell = Cell {
        x: 0,
        y: 0,
        width: 20,
        height: 20,
    };
    let writes = [
        write(
            "options:0x0000",
            MenuAtlasWriterComponent::Options,
            "0x0000",
            cell,
        ),
        write(
            "title_menu:0x0000",
            MenuAtlasWriterComponent::TitleMenu,
            "0x0000",
            cell,
        ),
    ];

    let analysis = analyze_migration(&writes, &[], &[]).unwrap();

    assert_eq!(analysis.candidate_declared_written_pixel_count, 800);
    assert_eq!(analysis.candidate_unique_written_pixel_count, 400);
    assert_eq!(analysis.candidate_write_overlaps.len(), 1);
    assert_eq!(
        analysis.candidate_write_overlaps[0].overlapping_pixel_count,
        400
    );
}

#[test]
fn readback_comparison_detects_pixels_changed_outside_declared_writes() {
    let writes = [write(
        "options:0x0000",
        MenuAtlasWriterComponent::Options,
        "0x0000",
        Cell {
            x: 0,
            y: 0,
            width: 20,
            height: 20,
        },
    )];
    let source = vec![0; MENU_ATLAS_WIDTH * MENU_ATLAS_HEIGHT];
    let mut candidate = source.clone();
    candidate[0] = 1;
    candidate[20] = 1;

    let readback = analyze_candidate_atlas_readback(&source, &candidate, &writes).unwrap();

    assert_eq!(readback.changed_pixel_count, 2);
    assert_eq!(readback.changed_pixel_outside_declared_writes_count, 1);
    assert_eq!(readback.declared_write_unchanged_pixel_count, 399);
}
