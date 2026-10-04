use super::*;
use crate::menu_atlas::{MENU_ATLAS_HEIGHT, MENU_ATLAS_WIDTH};
use crate::tim::Cell;

const SOURCE_BIN: &str = "source-bin";
const MENU_STORED: &str = "menu-stored";
const MENU_DECODED: &str = "menu-decoded";
const ATLAS_INDEXED: &str = "atlas-indexed";

fn document(labels: Vec<MenuAtlasLabel>) -> MenuAtlasLabelDocument {
    MenuAtlasLabelDocument {
        kind: DOCUMENT_KIND.to_string(),
        source_bin_sha256: SOURCE_BIN.to_string(),
        menu_stored_sha256: MENU_STORED.to_string(),
        menu_decoded_sha256: MENU_DECODED.to_string(),
        source_atlas_indexed_sha256: ATLAS_INDEXED.to_string(),
        atlas_width: MENU_ATLAS_WIDTH,
        atlas_height: MENU_ATLAS_HEIGHT,
        labels,
    }
}

fn label(id: &str, bounds: Cell) -> MenuAtlasLabel {
    MenuAtlasLabel {
        id: id.to_string(),
        kind: MenuAtlasLabelKind::Unknown,
        review_status: MenuAtlasLabelReviewStatus::NeedsReview,
        bounds,
        semantic_role: None,
        source_text: None,
        logical_codes: Vec::new(),
        notes: None,
    }
}

fn validate(document: &MenuAtlasLabelDocument) -> Result<()> {
    validate_menu_atlas_labels(
        document,
        MenuAtlasLabelSourceIdentity {
            source_bin_sha256: SOURCE_BIN,
            menu_stored_sha256: MENU_STORED,
            menu_decoded_sha256: MENU_DECODED,
            source_atlas_indexed_sha256: ATLAS_INDEXED,
            atlas_width: MENU_ATLAS_WIDTH,
            atlas_height: MENU_ATLAS_HEIGHT,
        },
    )
}

#[test]
fn admits_arbitrary_non_grid_graphic_bounds() {
    let labels = document(vec![label(
        "records-crown-and-text",
        Cell {
            x: 613,
            y: 17,
            width: 47,
            height: 29,
        },
    )]);
    validate(&labels).unwrap();
}

#[test]
fn rejects_labels_outside_the_source_bound_atlas() {
    let labels = document(vec![label(
        "outside",
        Cell {
            x: 1000,
            y: 250,
            width: 40,
            height: 20,
        },
    )]);
    assert!(
        validate(&labels)
            .unwrap_err()
            .to_string()
            .contains("outside the source atlas")
    );
}

#[test]
fn confirmed_labels_require_a_semantic_classification_and_role() {
    let mut candidate = label(
        "unknown-confirmed",
        Cell {
            x: 8,
            y: 9,
            width: 11,
            height: 12,
        },
    );
    candidate.review_status = MenuAtlasLabelReviewStatus::Confirmed;
    let labels = document(vec![candidate]);
    assert!(
        validate(&labels)
            .unwrap_err()
            .to_string()
            .contains("remains unknown")
    );
}

#[test]
fn rejects_duplicate_logical_codes_inside_one_label() {
    let mut candidate = label(
        "duplicate-code",
        Cell {
            x: 8,
            y: 9,
            width: 11,
            height: 12,
        },
    );
    candidate.logical_codes = vec!["0x02a0".to_string(), "0x02a0".to_string()];
    let labels = document(vec![candidate]);
    assert!(
        validate(&labels)
            .unwrap_err()
            .to_string()
            .contains("repeats logical code")
    );
}

#[test]
fn reports_overlapping_regions_without_rejecting_layered_semantics() {
    let labels = vec![
        label(
            "baked-label",
            Cell {
                x: 10,
                y: 10,
                width: 40,
                height: 20,
            },
        ),
        label(
            "embedded-icon",
            Cell {
                x: 15,
                y: 12,
                width: 8,
                height: 8,
            },
        ),
    ];
    assert_eq!(
        overlapping_label_pairs(&labels),
        [["baked-label".to_string(), "embedded-icon".to_string()]]
    );
    validate(&document(labels)).unwrap();
}

fn logical_code_map() -> MenuAtlasLogicalCodeMap {
    MenuAtlasLogicalCodeMap {
        kind: "test logical code map".to_string(),
        source_bin_sha256: SOURCE_BIN.to_string(),
        menu_decoded_sha256: MENU_DECODED.to_string(),
        address_flow_state_budget: 32_768,
        code_count: 1,
        used_code_count: 1,
        exact_dialogue_pixel_match_count: 1,
        references: vec![MenuAtlasLogicalCodeReference {
            code: "0x0000".to_string(),
            page: 0,
            column: 0,
            row: 0,
            physical_x: 0,
            physical_y: 0,
            footprint_fragments: vec![Cell {
                x: 0,
                y: 0,
                width: 20,
                height: 20,
            }],
            pixel_sha256: "pixel".to_string(),
            exact_dialogue_pixel_text: Some(" ".to_string()),
            occurrence_count: 3,
            overlays: vec!["DAT1/TEST.BIN".to_string()],
        }],
        limitations: Vec::new(),
    }
}

fn migration_review() -> MenuAtlasMigrationReviewContext {
    MenuAtlasMigrationReviewContext {
        kind: "Justice Gakuen 2 source-bound shared MENU atlas migration obligation audit"
            .to_string(),
        source_bin_sha256: SOURCE_BIN.to_string(),
        menu_stored_sha256: MENU_STORED.to_string(),
        menu_decoded_sha256: MENU_DECODED.to_string(),
        source_atlas_indexed_sha256: ATLAS_INDEXED.to_string(),
        address_flow_state_budget: 32_768,
        statically_used_source_code_count: 1,
        candidate_write_count: 1,
        source_code_migration_obligation_count: 1,
        migration_proof_complete: false,
        candidate_writes: vec![super::model::MenuAtlasMigrationReviewWrite {
            id: "options:0x0000".to_string(),
            component: "options".to_string(),
            character: '한',
            code: "0x0000".to_string(),
            cell: Cell {
                x: 0,
                y: 0,
                width: 40,
                height: 40,
            },
            source_code_occurrence_count: 3,
        }],
        source_code_migration_obligations: vec![super::model::MenuAtlasMigrationReviewObligation {
            code: "0x0000".to_string(),
            footprint_fragments: vec![Cell {
                x: 0,
                y: 0,
                width: 20,
                height: 20,
            }],
            overwritten_pixel_count: 400,
            fully_overwritten: true,
            occurrence_count: 3,
            overlays: vec!["DAT1/TEST.BIN".to_string()],
            exact_dialogue_pixel_text: Some(" ".to_string()),
            candidate_write_ids: vec!["options:0x0000".to_string()],
        }],
    }
}

#[test]
fn migration_review_must_match_the_source_and_logical_code_evidence() {
    validate_migration_review_context(
        &migration_review(),
        &document(Vec::new()),
        &logical_code_map(),
    )
    .unwrap();

    let mut wrong_source = migration_review();
    wrong_source.source_bin_sha256 = "different-source".to_string();
    assert!(
        validate_migration_review_context(
            &wrong_source,
            &document(Vec::new()),
            &logical_code_map()
        )
        .unwrap_err()
        .to_string()
        .contains("different source artifact")
    );
}

#[test]
fn migration_review_rejects_an_affected_code_that_drifted_from_the_code_map() {
    let mut review = migration_review();
    review.source_code_migration_obligations[0].occurrence_count += 1;

    assert!(
        validate_migration_review_context(&review, &document(Vec::new()), &logical_code_map())
            .unwrap_err()
            .to_string()
            .contains("disagrees with the logical-code map")
    );
}
